//! WRKLMNL screens authored in the shared UI shorthand.
use eframe::egui;
use std::collections::BTreeMap;
use terminal_ui_kit::{Action, Element};
use terminal_ui_language::Document;

#[cfg(test)]
pub const DASHBOARD: &str = include_str!("../../../ui/dashboard.txt");
pub const PICKER: &str = include_str!("../../../ui/terminal-picker.txt");
pub const LAUNCHER: &str = include_str!("../../../ui/shell-launcher.txt");
#[cfg(test)]
pub const NAVIGATION: &str = include_str!("../../../ui/navigation.txt");

#[derive(Default)]
pub struct Workspace {
    pub surface: crate::ui_runtime::Surface,
    documents: BTreeMap<String, (String, Document)>,
}
impl Workspace {
    pub fn render(
        &mut self,
        ui: &mut egui::Ui,
        id: &str,
        source: &str,
        values: BTreeMap<String, String>,
        selected: Option<&str>,
    ) -> Result<Vec<Action>, String> {
        let replace = self.documents.get(id).is_none_or(|(old, _)| old != source);
        if replace {
            self.documents
                .insert(id.into(), (source.into(), Document::parse(source)?));
        }
        let mut view = self.documents[id].1.render(&values)?;
        view.id = id.into();
        if let Some(selected) = selected {
            select(&mut view.elements, selected);
        }
        self.surface.values = values;
        self.surface.events.clear();
        ui.push_id(id, |ui| {
            self.surface.render_elements(ui, &view, &view.elements)
        });
        Ok(std::mem::take(&mut self.surface.events))
    }
}
fn select(elements: &mut [Element], selected: &str) {
    for element in elements {
        if let Element::Stack { role, children, .. } = element {
            if role.as_deref() == Some("item")
                && children
                    .iter()
                    .any(|e| matches!(e,Element::Action{id,..} if id==selected))
            {
                *role = Some("item_selected".into());
            }
            select(children, selected);
        }
    }
}

pub fn shells(
    items: &[crate::shell_catalog::Choice],
    values: &mut BTreeMap<String, String>,
) -> String {
    items.iter().enumerate().map(|(i,item)|{
        values.insert(format!("shell_name_{i}"),item.label.clone());
        values.insert(format!("shell_detail_{i}"),item.detail.clone());
        values.insert(format!("shell_icon_{i}"),format!("file:{}",item.icon_path().display()));
        format!("tile[ico:$shell_icon_{i} > \"{{shell_name_{i}}}\"@shell_{i} > t:$shell_detail_{i}]")
    }).collect::<Vec<_>>().join(" > ")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn actual_workspace_sources_bind_and_roundtrip() {
        for source in [DASHBOARD, PICKER, LAUNCHER, NAVIGATION] {
            Document::parse(source).unwrap();
        }
        let choices = vec![crate::shell_catalog::Choice {
            label: "Shell [quoted] \"test\" {data}".into(),
            detail: "Launch safely".into(),
            executable: "/bin/bash".into(),
            args: vec![],
        }];
        let mut values = BTreeMap::from([
            ("folder".into(), "/tmp".into()),
            ("custom_shell".into(), "bash".into()),
            ("recent_query".into(), String::new()),
            ("shell_status".into(), String::new()),
        ]);
        let tiles = shells(&choices, &mut values);
        let launcher = LAUNCHER.replace("#SHELLS", &tiles);
        for source in [
            DASHBOARD
                .replace("content[] #LAUNCHER", &launcher)
                .replace("content[] #RECENTS", "t:\"No recent locations yet\""),
            PICKER
                .replace("content[] #LAUNCHER", &launcher)
                .replace("content[] #BACK", "\"Back to sessions\"@back_sessions"),
            NAVIGATION.replace("content[] #EDITOR", "item[ico:code > Editor@page_5]"),
        ] {
            let document = Document::parse(&source).unwrap();
            let rendered = document.render(&values).unwrap();
            rendered.validate().unwrap();
            let roundtrip = Document::parse(&document.source()).unwrap();
            assert_eq!(roundtrip.source(), document.source());
        }
    }
    #[test]
    fn responsive_grid_aligns_fits_and_routes_second_row_clicks() {
        let source = format!(
            "grid:12[{}]",
            (0..8)
                .map(|i| format!("tile[ico:terminal > Shell@launch_{i} > t:Installed]"))
                .collect::<Vec<_>>()
                .join(" > ")
        );
        for width in [240.0, 520.0, 1000.0] {
            let ctx = egui::Context::default();
            let mut workspace = Workspace::default();
            let mut frame = |events: Vec<egui::Event>| {
                let mut actions = vec![];
                let _ = ctx.run(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            egui::vec2(width, 1000.0),
                        )),
                        events,
                        ..Default::default()
                    },
                    |ctx| {
                        egui::CentralPanel::default().show(ctx, |ui| {
                            actions = workspace
                                .render(ui, "grid", &source, BTreeMap::new(), None)
                                .unwrap();
                        });
                    },
                );
                (actions, workspace.surface.action_bounds.clone())
            };
            let (_, bounds) = frame(vec![]);
            assert_eq!(bounds.len(), 8);
            for rect in bounds.values() {
                assert!(
                    rect.right() <= width + 0.5,
                    "Grid overflows {width}: {rect:?}"
                );
            }
            let first = bounds["launch_0"];
            for rect in bounds
                .values()
                .filter(|r| (r.left() - first.left()).abs() > 1.0 && r.top() < first.bottom())
            {
                assert!(
                    (rect.top() - first.top()).abs() < 0.5,
                    "Grid cells must share a top edge"
                );
            }
            let target = bounds["launch_5"].center();
            frame(vec![
                egui::Event::PointerMoved(target),
                egui::Event::PointerButton {
                    pos: target,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
            ]);
            let (actions, _) = frame(vec![egui::Event::PointerButton {
                pos: target,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            }]);
            assert_eq!(actions.len(), 1);
            assert_eq!(actions[0].action, "launch_5");
        }
    }
}
