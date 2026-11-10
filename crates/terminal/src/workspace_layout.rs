use crate::icons::{button, Icon};
use eframe::egui;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Module {
    Scratchpad,
    Connections,
    Terminals,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Page {
    Dashboard,
    Terminals,
    Connections,
    Files,
    Recent,
    Editor,
    Settings,
    Shortcuts,
}

impl Module {
    fn label(self) -> &'static str {
        match self {
            Self::Scratchpad => "Scratchpad",
            Self::Connections => "Connections",
            Self::Terminals => "Terminals",
        }
    }
}

impl Page {
    fn item(self) -> (&'static str, &'static str, u8) {
        match self {
            Self::Dashboard => ("Dashboard", "home", 0),
            Self::Terminals => ("Terminals", "terminal", 1),
            Self::Connections => ("Connections", "grid", 3),
            Self::Files => ("Files", "folder", 7),
            Self::Recent => ("Recent", "history", 8),
            Self::Editor => ("Editor", "code", 5),
            Self::Settings => ("Settings", "settings", 4),
            Self::Shortcuts => ("Shortcuts", "keys", 6),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Layout {
    pub dashboard: [Module; 3],
    pub primary_on_right: bool,
    pages: [Page; 8],
}

impl Default for Layout {
    fn default() -> Self {
        Self {
            dashboard: [Module::Scratchpad, Module::Connections, Module::Terminals],
            primary_on_right: false,
            pages: [
                Page::Dashboard,
                Page::Terminals,
                Page::Connections,
                Page::Files,
                Page::Recent,
                Page::Editor,
                Page::Settings,
                Page::Shortcuts,
            ],
        }
    }
}

impl Layout {
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        match std::fs::read(path) {
            Ok(bytes) => {
                let layout: Self = serde_json::from_slice(&bytes)?;
                anyhow::ensure!(
                    unique(&layout.dashboard) && unique(&layout.pages),
                    "Layout contains duplicate modules or pages"
                );
                Ok(layout)
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(error) => Err(error.into()),
        }
    }

    pub fn save(&self, path: &Path) -> anyhow::Result<()> {
        crate::persistence::atomic_write(path, &serde_json::to_vec_pretty(self)?)
    }

    pub fn navigation(&self, editor_open: bool, compact: bool) -> String {
        let mut items = Vec::new();
        if !compact {
            items.push(r#"t:"Workspace""#.into());
        }
        let default_order = self.pages == Self::default().pages;
        for page in self.pages {
            if page == Page::Editor && !editor_open {
                continue;
            }
            if page == Page::Settings && default_order {
                items.push("gap:16".into());
            }
            let (label, icon, id) = page.item();
            items.push(format!("item[ico:{icon} > {label}@page_{id}]"));
        }
        format!("col:8[{}]", items.join(" > "))
    }

    pub fn controls(&mut self, ui: &mut egui::Ui) {
        ui.heading("Layout");
        ui.weak("Arrange the dashboard and sidebar. Changes save automatically.");
        ui.add_space(8.0);
        ui.strong("Dashboard");
        ui.weak("The first module gets the wider column. On smaller windows, modules stack in this order.");
        ui.horizontal_wrapped(|ui| {
            ui.label("Wider column");
            ui.selectable_value(&mut self.primary_on_right, false, "Left");
            ui.selectable_value(&mut self.primary_on_right, true, "Right");
        });
        ui.push_id("dashboard_layout", |ui| {
            reorder(ui, &mut self.dashboard, Module::label);
        });
        ui.separator();
        ui.strong("Sidebar");
        ui.weak("Editor appears when a document is open. New window stays at the bottom.");
        ui.push_id("sidebar_layout", |ui| {
            reorder(ui, &mut self.pages, |page| page.item().0);
        });
        ui.separator();
        if crate::widgets::button(ui, "Reset layout").clicked() {
            *self = Self::default();
        }
    }
}

fn unique<T: PartialEq>(items: &[T]) -> bool {
    items
        .iter()
        .enumerate()
        .all(|(i, item)| !items[..i].contains(item))
}

fn reorder<T: Copy>(ui: &mut egui::Ui, items: &mut [T], label: impl Fn(T) -> &'static str) {
    let width = ui.available_width().min(560.0);
    let mut swap = None;
    for (index, item) in items.iter().enumerate() {
        ui.push_id(index, |ui| {
            egui::Frame::NONE
                .fill(ui.visuals().faint_bg_color)
                .corner_radius(4)
                .inner_margin(8)
                .show(ui, |ui| {
                    ui.set_width(width - 16.0);
                    ui.horizontal(|ui| {
                        let name = label(*item);
                        ui.label(name);
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let down = ui.add_enabled_ui(index + 1 < items.len(), |ui| {
                                button(ui, Icon::Down, &format!("Move {name} down"))
                            });
                            let up = ui.add_enabled_ui(index > 0, |ui| {
                                button(ui, Icon::Up, &format!("Move {name} up"))
                            });
                            if down.inner.clicked() {
                                swap = Some((index, index + 1));
                            } else if up.inner.clicked() {
                                swap = Some((index, index - 1));
                            }
                        });
                    });
                });
        });
    }
    if let Some((from, to)) = swap {
        items.swap(from, to);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn custom_layout_survives_restart_and_routes_sidebar_in_order() -> anyhow::Result<()> {
        let folder = tempfile::tempdir()?;
        let path = folder.path().join("layout.json");
        let mut layout = Layout::default();
        layout.dashboard.swap(0, 2);
        layout.pages.swap(0, 3);
        layout.primary_on_right = true;
        layout.save(&path)?;
        let restored = Layout::load(&path)?;
        assert_eq!(restored, layout);
        let navigation = restored.navigation(false, false);
        assert!(navigation.find("Files@page_7") < navigation.find("Dashboard@page_0"));
        assert!(!navigation.contains("Editor@page_5"));
        assert!(restored.navigation(true, true).contains("Editor@page_5"));
        terminal_ui_language::Document::parse(&navigation).unwrap();
        Ok(())
    }
}
