use eframe::egui::{self, Color32, RichText, Vec2};
use std::collections::BTreeMap;
use terminal_ui_kit::{Action, Direction, Element, View};
pub fn responsive_columns(width: f32, gap: f32, font_size: f32) -> usize {
    (((width + gap) / ((240.0 * font_size / 15.0).max(160.0) + gap)).floor() as usize).clamp(1, 3)
}
pub struct Surface {
    pub values: BTreeMap<String, String>,
    pub events: Vec<Action>,
    pub design_mode: bool,
    pub syntax: bool,
    pub light: bool,
    pub size: f32,
    pub ui_size: f32,
    pub accent_color: Color32,
    pub installed_icons: std::collections::HashMap<String, Option<egui::TextureHandle>>,
    #[cfg(test)]
    pub action_bounds: BTreeMap<String, egui::Rect>,
}
impl Default for Surface {
    fn default() -> Self {
        Self {
            values: BTreeMap::new(),
            events: vec![],
            design_mode: false,
            syntax: true,
            light: false,
            size: 15.0,
            ui_size: 15.0,
            accent_color: Color32::from_rgb(182, 182, 178),
            installed_icons: Default::default(),
            #[cfg(test)]
            action_bounds: Default::default(),
        }
    }
}
impl Surface {
    pub fn icon(
        &mut self,
        ui: &egui::Ui,
        rect: egui::Rect,
        name: &str,
        label: &str,
        color: Color32,
    ) {
        if let Some(path) = name.strip_prefix("file:") {
            let texture = self.installed_icons.entry(path.into()).or_insert_with(|| {
                crate::installed_icon::load(std::path::Path::new(path)).map(|image| {
                    ui.ctx()
                        .load_texture(path, image, egui::TextureOptions::LINEAR)
                })
            });
            if let Some(texture) = texture {
                ui.painter().image(
                    texture.id(),
                    rect,
                    egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
                    Color32::WHITE,
                );
                return;
            }
            crate::icons::shell(ui.painter(), rect, label, color);
            return;
        }
        use crate::icons::Icon;
        let icon = match name {
            "home" => Icon::Home,
            "code" => Icon::Code,
            "grid" => Icon::Grid,
            "palette" => Icon::Palette,
            "settings" => Icon::Settings,
            "folder" => Icon::Folder,
            "history" => Icon::History,
            "keys" => Icon::Keys,
            "plus" => Icon::Plus,
            _ => Icon::Terminal,
        };
        crate::icons::paint(ui.painter(), rect, icon, color);
    }
    fn tile(&mut self, ui: &mut egui::Ui, view: &View, children: &[Element], role: &str) {
        let Some((id, label)) = children.iter().find_map(|e| {
            if let Element::Action { id, label } = e {
                Some((id, label))
            } else {
                None
            }
        }) else {
            return;
        };
        let detail = children
            .iter()
            .find_map(|e| {
                if let Element::Text { text } = e {
                    Some(text.as_str())
                } else {
                    None
                }
            })
            .unwrap_or("");
        let icon = children.iter().find_map(|e| {
            if let Element::Icon { name } = e {
                Some(name.as_str())
            } else {
                None
            }
        });
        let item = role.starts_with("item");
        let enabled = !self.design_mode && role != "tile_disabled";
        let scale = self.ui_size / 15.0;
        let height = if item {
            40.0 * scale.max(1.0)
        } else {
            60.0 * scale.max(1.0)
        };
        let fill = if item && role != "item_selected" {
            Color32::TRANSPARENT
        } else {
            self.surface()
        };
        let response = ui.add_enabled(
            enabled,
            egui::Button::new("")
                .min_size(egui::vec2(ui.available_width(), height))
                .fill(fill)
                .stroke(egui::Stroke::NONE)
                .corner_radius(0),
        );
        response
            .widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, label));
        let color = if role == "item_selected" || response.has_focus() {
            self.accent()
        } else {
            ui.visuals().text_color()
        };
        let rect = response.rect;
        if item {
            let hover = crate::widgets::hover(ui, response.id, enabled && response.hovered());
            if hover > 0.0 {
                let wash = ui.visuals().text_color().gamma_multiply(hover * 0.065);
                ui.painter().rect_filled(rect, 0.0, wash);
            }
        }
        #[cfg(test)]
        self.action_bounds.insert(id.clone(), rect);
        if let Some(icon) = icon {
            self.icon(
                ui,
                egui::Rect::from_center_size(
                    egui::pos2(rect.left() + 24.0, rect.center().y),
                    egui::vec2(24.0, 24.0),
                ),
                icon,
                label,
                color,
            );
        }
        let left = if icon.is_some() { 48.0 } else { 12.0 };
        if item && rect.width() < 80.0 {
            if response.on_hover_text(label).clicked() {
                self.events.push(Action {
                    version: 1,
                    view: view.id.clone(),
                    action: id.clone(),
                    values: self.values.clone(),
                });
            }
            return;
        }
        let mut job = egui::text::LayoutJob::default();
        job.wrap.max_width = (rect.width() - left - 12.0).max(8.0);
        job.append(
            label,
            0.0,
            egui::TextFormat {
                font_id: egui::FontId::proportional(14.0 * scale),
                color,
                ..Default::default()
            },
        );
        if !detail.is_empty() {
            job.append(
                &format!("\n{detail}"),
                0.0,
                egui::TextFormat {
                    font_id: egui::FontId::proportional(11.0 * scale),
                    color: self.muted(),
                    ..Default::default()
                },
            );
        }
        let galley = ui.fonts(|f| f.layout_job(job));
        let pos = egui::pos2(rect.left() + left, rect.center().y - galley.size().y * 0.5);
        ui.painter()
            .with_clip_rect(rect.shrink(6.0))
            .galley(pos, galley, color);
        if response
            .on_hover_text(if detail.is_empty() { label } else { detail })
            .clicked()
        {
            self.events.push(Action {
                version: 1,
                view: view.id.clone(),
                action: id.clone(),
                values: self.values.clone(),
            });
        }
    }
    fn accent(&self) -> Color32 {
        self.accent_color
    }
    fn muted(&self) -> Color32 {
        if self.light {
            Color32::from_rgb(98, 98, 98)
        } else {
            Color32::from_rgb(163, 163, 163)
        }
    }
    fn surface(&self) -> Color32 {
        if self.light {
            Color32::from_rgb(241, 241, 241)
        } else {
            Color32::from_rgb(26, 26, 26)
        }
    }
    pub fn render_elements(&mut self, ui: &mut egui::Ui, view: &View, elements: &[Element]) {
        if !elements.is_empty() && elements.iter().all(|e| matches!(e, Element::Place { .. })) {
            let mut size = Vec2::new(ui.available_width(), 24.0);
            for e in elements {
                if let Element::Place {
                    x,
                    y,
                    width,
                    height,
                    ..
                } = e
                {
                    size.x = size.x.max(x + width);
                    size.y = size.y.max(y + height);
                }
            }
            let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
            for (index, e) in elements.iter().enumerate() {
                if let Element::Place {
                    x,
                    y,
                    width,
                    height,
                    child,
                } = e
                {
                    let bounds = egui::Rect::from_min_size(
                        rect.min + Vec2::new(*x, *y),
                        Vec2::new(*width, *height),
                    );
                    ui.push_id(index, |ui| {
                        ui.allocate_new_ui(egui::UiBuilder::new().max_rect(bounds), |ui| {
                            ui.set_clip_rect(bounds);
                            self.render_elements(ui, view, std::slice::from_ref(child.as_ref()));
                        });
                    });
                }
            }
            return;
        }
        for (index, element) in elements.iter().enumerate() {
            ui.push_id(index, |ui| match element {
                Element::Graphics { height, source } => {
                    crate::pixel_surface::show(ui, source, *height);
                }
                Element::Text { text } => {
                    if !text.is_empty() {
                        ui.label(RichText::new(text).color(self.muted()));
                    }
                }
                Element::Heading { text } => {
                    ui.add_space(8.0);
                    ui.label(RichText::new(text).size(self.ui_size * 1.6).strong());
                }
                Element::Icon { name } => {
                    ui.label(
                        egui::RichText::new(if name == "folder" {
                            "/"
                        } else if name == "term" || name == "terminal" {
                            ">_"
                        } else {
                            name
                        })
                        .monospace()
                        .strong(),
                    );
                }
                Element::Input { id, label, value } => {
                    ui.label(label);
                    let surface = self.surface();
                    ui.add(
                        egui::TextEdit::singleline(
                            self.values
                                .entry(id.clone())
                                .or_insert_with(|| value.clone()),
                        )
                        .interactive(!self.design_mode)
                        .id_source(id)
                        .frame(true)
                        .background_color(surface)
                        .desired_width(ui.available_width())
                        .margin(egui::vec2(12.0, 10.0)),
                    );
                }
                Element::Action { id, label } => {
                    if crate::widgets::add(
                        ui,
                        egui::Button::new(RichText::new(label).color(self.accent()).strong())
                            .frame(false),
                    )
                    .clicked()
                        && !self.design_mode
                    {
                        let event = Action {
                            version: 1,
                            view: view.id.clone(),
                            action: id.clone(),
                            values: self.values.clone(),
                        };
                        self.events.push(event);
                    }
                }
                Element::Table { columns, rows } => {
                    egui::ScrollArea::horizontal()
                        .id_salt(index)
                        .show(ui, |ui| {
                            egui::Grid::new(index).spacing([32.0, 12.0]).show(ui, |ui| {
                                for c in columns {
                                    ui.label(RichText::new(c).strong());
                                }
                                ui.end_row();
                                for row in rows {
                                    for cell in row {
                                        ui.label(cell);
                                    }
                                    ui.end_row();
                                }
                            });
                        });
                }
                Element::Progress { label, value } => {
                    ui.label(label);
                    ui.add(
                        egui::ProgressBar::new(*value)
                            .fill(self.accent())
                            .corner_radius(0)
                            .show_percentage(),
                    );
                }
                Element::Code { language, text } => {
                    ui.label(RichText::new(language).color(self.muted()));
                    ui.label(crate::highlight::layout_sized(
                        text,
                        if self.syntax { language } else { "plain" },
                        self.light,
                        self.size,
                    ));
                }
                Element::Space { size } => ui.add_space(*size),
                Element::Flex => ui.add_space(12.0),
                Element::List { items } => {
                    for item in items {
                        ui.label(item);
                    }
                }
                Element::Place {
                    width,
                    height,
                    child,
                    ..
                } => {
                    ui.allocate_ui(Vec2::new(*width, *height), |ui| {
                        self.render_elements(ui, view, std::slice::from_ref(child.as_ref()))
                    });
                }
                Element::Stack {
                    role: Some(role),
                    children,
                    gap,
                    ..
                } if role == "grid" => {
                    let available = ui.available_width();
                    let columns = responsive_columns(available, *gap, self.ui_size);
                    let width =
                        ((available - gap * (columns - 1) as f32) / columns as f32).max(1.0);
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = *gap;
                        for (row_index, row) in children.chunks(columns).enumerate() {
                            ui.push_id(row_index, |ui| {
                                ui.horizontal_top(|ui| {
                                    ui.spacing_mut().item_spacing.x = *gap;
                                    for (i, child) in row.iter().enumerate() {
                                        ui.push_id(i, |ui| {
                                            ui.allocate_ui_with_layout(
                                                Vec2::new(width, 0.0),
                                                egui::Layout::top_down(egui::Align::Min),
                                                |ui| {
                                                    ui.set_width(width);
                                                    self.render_elements(
                                                        ui,
                                                        view,
                                                        std::slice::from_ref(child),
                                                    );
                                                },
                                            );
                                        });
                                    }
                                })
                            });
                        }
                    });
                }
                Element::Stack {
                    role: Some(role),
                    children,
                    ..
                } if matches!(
                    role.as_str(),
                    "tile" | "tile_disabled" | "item" | "item_selected"
                ) =>
                {
                    self.tile(ui, view, children, role)
                }
                Element::Stack {
                    role: Some(role),
                    children,
                    ..
                } if role == "fold" => {
                    let title = children
                        .first()
                        .and_then(|e| {
                            if let Element::Heading { text } = e {
                                Some(text.as_str())
                            } else {
                                None
                            }
                        })
                        .unwrap_or("Details");
                    egui::CollapsingHeader::new(title).show(ui, |ui| {
                        self.render_elements(
                            ui,
                            view,
                            &children[usize::from(matches!(
                                children.first(),
                                Some(Element::Heading { .. })
                            ))..],
                        )
                    });
                }
                Element::Stack {
                    direction,
                    gap,
                    children,
                    ..
                } => match direction {
                    Direction::Row => {
                        let available = ui.available_width();
                        let widths: Vec<f32> = children.iter().map(|e| measure(ui, e)).collect();
                        let flex = children
                            .iter()
                            .filter(|e| matches!(e, Element::Flex))
                            .count();
                        let total = widths.iter().sum::<f32>()
                            + *gap * (children.len().saturating_sub(1) as f32);
                        let extra = if flex > 0 {
                            ((available - total) / flex as f32).max(0.0)
                        } else {
                            0.0
                        };
                        ui.horizontal_wrapped(|ui| {
                            ui.spacing_mut().item_spacing.x = *gap;
                            for (e, width) in children.iter().zip(widths) {
                                if matches!(e, Element::Flex) {
                                    ui.add_space(extra);
                                } else {
                                    ui.allocate_ui(Vec2::new(width.min(available), 0.0), |ui| {
                                        self.render_elements(ui, view, std::slice::from_ref(e));
                                    });
                                }
                            }
                        });
                    }
                    Direction::Column => {
                        ui.vertical(|ui| {
                            ui.spacing_mut().item_spacing.y = *gap;
                            self.render_elements(ui, view, children);
                        });
                    }
                },
            });
        }
    }
}
fn measure(ui: &egui::Ui, e: &Element) -> f32 {
    let font = egui::TextStyle::Body.resolve(ui.style());
    let text = |s: &str| {
        ui.fonts(|f| {
            f.layout_no_wrap(s.into(), font.clone(), ui.visuals().text_color())
                .size()
                .x
        }) + 24.0
    };
    match e {
        Element::Action { label, .. }
        | Element::Input { label, .. }
        | Element::Progress { label, .. } => {
            text(label).max(if matches!(e, Element::Input { .. }) {
                220.0
            } else {
                80.0
            })
        }
        Element::Text { text: s } => text(s),
        Element::Heading { text: s } => {
            ui.fonts(|f| {
                f.layout_no_wrap(
                    s.clone(),
                    egui::FontId::proportional(font.size * 1.6),
                    ui.visuals().text_color(),
                )
                .size()
                .x
            }) + 24.0
        }
        Element::Stack {
            role: Some(role), ..
        } if matches!(
            role.as_str(),
            "tile" | "tile_disabled" | "item" | "item_selected"
        ) =>
        {
            240.0
        }
        Element::Stack { children, gap, .. } => {
            children.iter().map(|e| measure(ui, e)).sum::<f32>()
                + *gap * children.len().saturating_sub(1) as f32
        }
        Element::Flex => 0.0,
        Element::Icon { .. } => 24.0,
        Element::Space { size } => *size,
        Element::Place { width, .. } => *width,
        _ => 300.0,
    }
}
