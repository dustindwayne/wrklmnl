// Included in dock.rs so this adapter shares the existing PTY/docking owner.
#[derive(Clone)]
struct GroupDrag(u64);
impl Dock {
    fn split_preset(&mut self, count: usize) -> anyhow::Result<()> {
        anyhow::ensure!(
            matches!(count, 2 | 4 | 6 | 8),
            "Choose a 2, 4, 6 or 8 way layout"
        );
        self.sync_groups();
        let profile = self
            .profile()
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("Open a terminal first"))?;
        let mut ids: Vec<u64> = self.visible_groups().iter().map(|g| g.active).collect();
        if let Some(index) = ids.iter().position(|id| *id == self.active) {
            ids.swap(0, index);
        }
        self.tab_target = None;
        while ids.len() < count {
            self.launch(profile.clone())?;
            ids.push(self.active);
        }
        ids.truncate(count);
        self.split_root = Some(Node::grid(&ids, if count == 2 { 2 } else { count / 2 }));
        self.active = ids[0];
        self.tabs = false;
        self.split_view = true;
        Ok(())
    }
    fn draw_split_groups(
        &mut self,
        ui: &mut egui::Ui,
        rect: Rect,
        node: &mut Node,
        primary: &mut Session,
        light: bool,
        size: f32,
        actions: &mut Vec<Action>,
    ) {
        match node {
            Node::Split {
                vertical,
                ratio,
                a,
                b,
            } => {
                let span = if *vertical {
                    rect.height()
                } else {
                    rect.width()
                };
                let offset = span * *ratio;
                let (ra, gap, rb) = if *vertical {
                    (
                        Rect::from_min_max(
                            rect.min,
                            egui::pos2(rect.right(), rect.top() + offset - 5.0),
                        ),
                        Rect::from_min_max(
                            egui::pos2(rect.left(), rect.top() + offset - 5.0),
                            egui::pos2(rect.right(), rect.top() + offset + 5.0),
                        ),
                        Rect::from_min_max(
                            egui::pos2(rect.left(), rect.top() + offset + 5.0),
                            rect.max,
                        ),
                    )
                } else {
                    (
                        Rect::from_min_max(
                            rect.min,
                            egui::pos2(rect.left() + offset - 5.0, rect.bottom()),
                        ),
                        Rect::from_min_max(
                            egui::pos2(rect.left() + offset - 5.0, rect.top()),
                            egui::pos2(rect.left() + offset + 5.0, rect.bottom()),
                        ),
                        Rect::from_min_max(
                            egui::pos2(rect.left() + offset + 5.0, rect.top()),
                            rect.max,
                        ),
                    )
                };
                let response = ui
                    .interact(
                        gap,
                        ui.id().with(("group_divider", a.first(), b.first())),
                        Sense::drag(),
                    )
                    .on_hover_cursor(if *vertical {
                        egui::CursorIcon::ResizeVertical
                    } else {
                        egui::CursorIcon::ResizeHorizontal
                    });
                if response.dragged() {
                    let delta = ui.input(|i| {
                        if *vertical {
                            i.pointer.delta().y
                        } else {
                            i.pointer.delta().x
                        }
                    });
                    *ratio = (*ratio + delta / span.max(1.0)).clamp(0.15, 0.85);
                }
                let stroke = egui::Stroke::new(
                    1.0_f32,
                    if light {
                        egui::Color32::from_black_alpha(
                            if response.hovered() || response.dragged() {
                                0x44
                            } else {
                                0x22
                            },
                        )
                    } else {
                        egui::Color32::from_white_alpha(
                            if response.hovered() || response.dragged() {
                                0x44
                            } else {
                                0x22
                            },
                        )
                    },
                );
                if *vertical {
                    ui.painter()
                        .line_segment([gap.left_center(), gap.right_center()], stroke);
                } else {
                    ui.painter()
                        .line_segment([gap.center_top(), gap.center_bottom()], stroke);
                }
                self.draw_split_groups(ui, ra, a, primary, light, size, actions);
                self.draw_split_groups(ui, rb, b, primary, light, size, actions);
            }
            Node::Leaf(id) => {
                let rect = rect.shrink(8.0_f32.min(rect.width().min(rect.height()).max(0.0) / 4.0));
                let Some(group) = self
                    .visible_groups()
                    .into_iter()
                    .find(|g| g.tabs.contains(id))
                else {
                    return;
                };
                ui.push_id(group.id, |ui| {
                    ui.allocate_new_ui(
                        egui::UiBuilder::new()
                            .max_rect(rect)
                            .layout(egui::Layout::top_down(egui::Align::Min)),
                        |ui| {
                            ui.set_width(rect.width());
                            ui.set_height(rect.height());
                            ui.set_clip_rect(rect);
                            self.group_tabs(ui, &group, primary, actions);
                            let active = self
                                .groups
                                .items
                                .iter()
                                .find(|g| g.id == group.id)
                                .map(|g| g.active)
                                .unwrap_or(group.active);
                            let focused = self.active == active;
                            if let Some(session) = self.session_mut(primary, active) {
                                session.interaction.active = focused;
                                if let Err(error) = crate::terminal::draw(ui, session, light, size)
                                {
                                    actions.push(Action::Error(error.to_string()));
                                }
                            }
                            if rect
                                .contains(ui.input(|i| i.pointer.hover_pos().unwrap_or_default()))
                                && ui.input(|i| i.pointer.any_pressed())
                            {
                                self.active = active;
                            }
                        },
                    );
                });
            }
        }
    }
    fn open_group(&mut self, group: u64, primary: &mut Session) {
        if let Some(group) = self.visible_groups().iter().find(|g| g.id == group) {
            self.active = group.active;
            self.tabs = true;
            self.split_view = false;
            self.focus(primary);
        }
    }
    #[cfg(feature = "capture")]
    pub fn prepare_group_capture(&mut self, primary: &Session, expanded: bool) {
        self.sync_groups();
        if let Some(group) = self.groups.items.first().cloned() {
            self.add_group_tab(group.id, primary);
            if let Some(profile) = self.primary.clone() {
                let _ = self.launch(profile);
            }
            self.active = self
                .groups
                .items
                .iter()
                .find(|g| g.id == group.id)
                .unwrap()
                .active;
        }
        self.tabs = expanded;
        self.split_view = false;
        if let Some(count) = std::env::var("TERMINAL_CAPTURE_SPLITS")
            .ok()
            .and_then(|s| s.parse().ok())
        {
            let _ = self.split_preset(count);
        }
    }
    fn sync_groups(&mut self) {
        let mut live = vec![];
        if let Some(root) = &self.root {
            root.leaves(&mut live);
        }
        live.extend(self.floating.iter().copied());
        self.groups.sync(&live);
    }
    pub fn cancel_tab_target(&mut self) {
        self.tab_target = None;
    }
    fn tab_label(&self, id: u64, primary: &Session) -> String {
        let session = if id == 0 {
            Some(primary)
        } else {
            self.panes.get(&id).map(|p| &p.session)
        };
        session
            .map(|s| {
                format!(
                    "{} / {}",
                    std::path::Path::new(&s.shell)
                        .file_stem()
                        .unwrap_or_default()
                        .to_string_lossy(),
                    s.cwd.file_name().unwrap_or_default().to_string_lossy()
                )
            })
            .unwrap_or_default()
    }
    fn add_group_tab(&mut self, group: u64, primary: &Session) {
        let Some(id) = self
            .groups
            .items
            .iter()
            .find(|g| g.id == group)
            .map(|g| g.active)
        else {
            return;
        };
        let profile = if id == 0 {
            self.primary.clone().map(|mut p| {
                p.cwd = primary.cwd.clone();
                p
            })
        } else {
            self.panes.get(&id).map(|p| p.profile.clone())
        };
        if let Some(profile) = profile {
            self.tab_target = Some(group);
            if let Err(error) = self.launch(profile) {
                self.message = format!("New tab: {error}");
                self.tab_target = None;
            }
        }
    }
    fn group_tabs(
        &mut self,
        ui: &mut egui::Ui,
        group: &crate::terminal_groups::Group,
        primary: &mut Session,
        actions: &mut Vec<Action>,
    ) {
        ui.horizontal(|ui| {
            egui::ScrollArea::horizontal()
                .id_salt(("group_tabs", group.id))
                .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
                .auto_shrink([false, true])
                .max_width((ui.available_width() - 108.0).max(24.0))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 4.0;
                        for id in &group.tabs {
                            ui.push_id(id, |ui| {
                                let label = self.tab_label(*id, primary);
                                let short = std::path::Path::new(
                                    label.split(" / ").next().unwrap_or("Terminal"),
                                )
                                .file_name()
                                .unwrap_or_default()
                                .to_string_lossy()
                                .into_owned();
                                let tab = crate::widgets::text_tab(ui, &short, group.active == *id);
                                if tab.on_hover_text(&label).clicked() {
                                    self.active = *id;
                                    self.groups.select(*id);
                                    if self.tabs || self.split_view {
                                        self.focus(primary);
                                    }
                                }
                                if crate::icons::button(ui, crate::icons::Icon::Close, "Close tab")
                                    .clicked()
                                {
                                    actions.push(Action::Close(*id));
                                }
                            });
                        }
                    });
                });
            if crate::icons::button(
                ui,
                crate::icons::Icon::Plus,
                "New tab in this terminal group",
            )
            .clicked()
            {
                self.add_group_tab(group.id, primary);
                if self.tabs || self.split_view {
                    self.focus(primary);
                }
            }
            ui.menu_button("...", |ui| {
                for id in &group.tabs {
                    if ui
                        .selectable_label(group.active == *id, self.tab_label(*id, primary))
                        .clicked()
                    {
                        self.active = *id;
                        self.groups.select(*id);
                        if self.tabs || self.split_view {
                            self.focus(primary);
                        }
                        ui.close_menu();
                    }
                }
                if crate::widgets::button(ui, "Choose shell for new tab").clicked() {
                    self.tab_target = Some(group.id);
                    self.global_action = Some("group_new_tab".into());
                    ui.close_menu();
                }
                for (label, action) in [
                    ("Split right", Action::Split(group.active, Side::Right)),
                    ("Split below", Action::Split(group.active, Side::Bottom)),
                    ("Detach active tab", Action::Float(group.active)),
                    ("Restart active tab", Action::Restart(group.active)),
                ] {
                    if crate::widgets::button(ui, label).clicked() {
                        actions.push(action);
                        ui.close_menu();
                    }
                }
            });
        });
    }
    fn visible_groups(&self) -> Vec<crate::terminal_groups::Group> {
        let mut docked = vec![];
        if let Some(root) = &self.root {
            root.leaves(&mut docked);
        }
        self.groups
            .items
            .iter()
            .filter_map(|group| {
                let mut group = group.clone();
                group.tabs.retain(|id| docked.contains(id));
                if group.tabs.is_empty() {
                    return None;
                }
                if !group.tabs.contains(&group.active) {
                    group.active = group.tabs[0];
                }
                Some(group)
            })
            .collect()
    }
    fn group_grid(
        &mut self,
        ui: &mut egui::Ui,
        primary: &mut Session,
        light: bool,
        actions: &mut Vec<Action>,
    ) {
        let groups = self.visible_groups();
        if groups.is_empty() {
            ui.label(if self.floating.is_empty() {
                "No terminal groups open."
            } else {
                "Sessions are in detached windows. Use Dock to return them here."
            });
            return;
        }
        egui::ScrollArea::vertical().auto_shrink([false,false]).scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden).show(ui,|ui|{
            let columns=crate::ui_runtime::responsive_columns(ui.available_width(),12.0,15.0);
            let side=((ui.available_width()-12.0*(columns-1) as f32)/columns as f32).max(1.0);
            for (row_index,row) in groups.chunks(columns).enumerate(){ui.push_id(row_index,|ui|{
                ui.horizontal_top(|ui|{
                    ui.spacing_mut().item_spacing.x=12.0;
                    for group in row {ui.push_id(group.id,|ui|{
                        let (cell,_)=ui.allocate_exact_size(egui::vec2(side,side),Sense::hover());
                        let fill=if light{egui::Color32::from_gray(241)}else{egui::Color32::from_gray(26)};
                        ui.painter().rect_filled(cell,0.0,fill);
                        #[cfg(test)]
                        ui.ctx().data_mut(|d|d.insert_temp(egui::Id::new(("terminal_grid_cell",group.id)),cell));
                        let inner=cell.shrink(12.0);
                        let mut content=ui.new_child(egui::UiBuilder::new().max_rect(inner).layout(egui::Layout::top_down(egui::Align::Min)));
                        content.set_clip_rect(inner);
                        self.group_tabs(&mut content,group,primary,actions);
                        let label=self.tab_label(group.active,primary);
                        if content.add(egui::Label::new(egui::RichText::new(&label).small().color(content.visuals().weak_text_color())).truncate().sense(Sense::click())).on_hover_text(&label).clicked(){self.open_group(group.id,primary);}
                        let preview_size=egui::vec2(inner.width(),content.available_height().max(1.0));
                        let (rect,response)=content.allocate_exact_size(preview_size,Sense::click_and_drag());
                        let session=if group.active==0{Some(&*primary)}else{self.panes.get(&group.active).map(|p|&p.session)};
                        if let Some(session)=session {
                            let text=session.parser.screen().contents();
                            let lines:Vec<_>=text.lines().collect();
                            let line_count=(rect.height()/17.0).floor().max(1.0) as usize;
                            let preview=lines[lines.len().saturating_sub(line_count)..].join("\n");
                            content.painter().with_clip_rect(rect.intersect(inner)).text(rect.min,egui::Align2::LEFT_TOP,preview,egui::FontId::monospace(12.0),content.visuals().text_color());
                        }
                        response.widget_info(||egui::WidgetInfo::labeled(egui::WidgetType::Button,true,format!("Open {label}")));
                        if response.clicked(){self.open_group(group.id,primary);}
                        if response.drag_started(){egui::DragAndDrop::set_payload(content.ctx(),GroupDrag(group.id));}
                        if let Some(source)=response.dnd_release_payload::<GroupDrag>(){self.groups.reorder(source.0,group.id);}
                        response.on_hover_text("Click to open. Drag to reorder this group and all its tabs.");
                    });}
                });ui.add_space(12.0);
            });}
        });
    }

    fn group_expanded(
        &mut self,
        ui: &mut egui::Ui,
        primary: &mut Session,
        light: bool,
        size: f32,
        actions: &mut Vec<Action>,
    ) {
        let groups = self.visible_groups();
        let Some(group) = groups
            .iter()
            .find(|g| g.tabs.contains(&self.active))
            .or_else(|| groups.first())
            .cloned()
        else {
            return;
        };
        self.group_tabs(ui, &group, primary, actions);
        let active = self
            .groups
            .items
            .iter()
            .find(|g| g.id == group.id)
            .map(|g| g.active)
            .filter(|id| group.tabs.contains(id))
            .unwrap_or(group.active);
        self.active = active;
        // An independent top-down layout prevents a tab strip's narrow layout
        // from becoming the terminal's PTY width.
        let rect = ui.available_rect_before_wrap();
        ui.allocate_new_ui(
            egui::UiBuilder::new()
                .max_rect(rect)
                .layout(egui::Layout::top_down(egui::Align::Min)),
            |ui| {
                ui.set_width(rect.width());
                ui.set_height(rect.height());
                ui.set_clip_rect(rect);
                if let Some(session) = self.session_mut(primary, active) {
                    session.interaction.active = true;
                    if session.exited {
                        ui.label("Session ended. Close or restart this tab.");
                    }
                    if let Err(error) = crate::terminal::draw(ui, session, light, size) {
                        actions.push(Action::Error(format!("Terminal: {error}")));
                    }
                }
            },
        );
    }
}
