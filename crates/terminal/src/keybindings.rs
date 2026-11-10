use eframe::egui::{self, Key, Modifiers};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::Path};

pub const ACTIONS: &[(&str, &str, &str)] = &[
    ("new_window", "New window", "Ctrl+Shift+N"),
    (
        "toggle_terminal_view",
        "Toggle grid / splits",
        "Ctrl+Shift+Space",
    ),
    ("terminal_grid", "Show terminal grid", "Ctrl+Alt+1"),
    ("terminal_splits", "Show terminal splits", "Ctrl+Alt+2"),
    ("split_right", "Split right", "Ctrl+Shift+D"),
    ("split_below", "Split below", "Ctrl+Alt+D"),
    ("close_pane", "Close active pane", "Ctrl+Shift+W"),
    ("restart_pane", "Restart active pane", "Ctrl+Shift+R"),
    ("next_pane", "Next pane", "Ctrl+Alt+ArrowRight"),
    ("previous_pane", "Previous pane", "Ctrl+Alt+ArrowLeft"),
    ("detach_pane", "Detach active pane", "Ctrl+Shift+E"),
    ("dock_pane", "Dock active pane", "Ctrl+Shift+B"),
    ("terminal_focus", "Focus terminal canvas", "Ctrl+Shift+T"),
    ("keybindings", "Open keybindings", "Ctrl+Shift+K"),
    ("dashboard", "Open dashboard", "Ctrl+Shift+H"),
    (
        "copy_terminal",
        "Copy terminal selection or screen",
        "Ctrl+Shift+C",
    ),
];
#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Bindings {
    pub keys: BTreeMap<String, String>,
    #[serde(skip)]
    pub message: String,
    #[serde(skip)]
    query: String,
    #[serde(skip)]
    recording: Option<String>,
}
impl Default for Bindings {
    fn default() -> Self {
        Self {
            keys: ACTIONS
                .iter()
                .map(|(a, _, k)| (a.to_string(), k.to_string()))
                .collect(),
            message: String::new(),
            query: String::new(),
            recording: None,
        }
    }
}
fn parse(text: &str) -> Result<(Modifiers, Key), String> {
    let mut mods = Modifiers::NONE;
    let mut key = None;
    for part in text.split('+').map(str::trim) {
        match part.to_ascii_lowercase().as_str() {
            "ctrl" => mods.ctrl = true,
            "shift" => mods.shift = true,
            "alt" => mods.alt = true,
            "cmd" => mods.mac_cmd = true,
            _ => {
                if key.is_some() {
                    return Err("Use one key per shortcut".into());
                }
                key = Key::from_name(part);
                if key.is_none() {
                    return Err(format!("Unknown key: {part}"));
                }
            }
        }
    }
    Ok((mods, key.ok_or("A shortcut needs a key")?))
}
impl Bindings {
    pub fn load(path: &Path) -> Self {
        match std::fs::read(path) {
            Ok(b) => match serde_json::from_slice::<Self>(&b) {
                Ok(mut v) => {
                    v.add_missing_defaults();
                    match v.validate() {
                        Ok(()) => v,
                        Err(e) => Self {
                            message: format!("Keybindings preserved; defaults loaded: {e}"),
                            ..Self::default()
                        },
                    }
                }
                Err(e) => Self {
                    message: format!("Keybindings preserved; defaults loaded: {e}"),
                    ..Self::default()
                },
            },
            Err(_) => Self::default(),
        }
    }
    fn validate(&self) -> Result<(), String> {
        let mut used = std::collections::HashSet::new();
        for (a, _, _) in ACTIONS {
            if let Some(text) = self.keys.get(*a).filter(|s| !s.trim().is_empty()) {
                let (m, k) = parse(text)?;
                if !m.ctrl && !m.mac_cmd && !m.alt {
                    return Err("Application shortcuts need Ctrl, Cmd or Alt so terminal keys stay available".into());
                }
                let id = format!("{m:?}:{k:?}");
                if !used.insert(id) {
                    return Err(format!("Duplicate shortcut: {text}"));
                }
                if (m.ctrl || m.mac_cmd)
                    && !m.shift
                    && !m.alt
                    && matches!(k, Key::C | Key::V | Key::X | Key::L)
                {
                    return Err(format!("{text} is reserved for terminal and text editing"));
                }
            }
        }
        Ok(())
    }
    fn add_missing_defaults(&mut self) {
        for (action, _, default) in ACTIONS {
            if self.keys.contains_key(*action) {
                continue;
            }
            let conflict = self
                .keys
                .values()
                .any(|v| parse(v).ok() == parse(default).ok());
            self.keys.insert(
                (*action).into(),
                if conflict {
                    String::new()
                } else {
                    (*default).into()
                },
            );
        }
    }
    pub fn take(&self, ctx: &egui::Context, action: &str) -> bool {
        if self.recording.is_some() || self.validate().is_err() {
            return false;
        }
        let Some(text) = self.keys.get(action).filter(|s| !s.is_empty()) else {
            return false;
        };
        let Ok((mods, key)) = parse(text) else {
            return false;
        };
        ctx.input_mut(|i| {
            if key == Key::C
                && (mods.ctrl || mods.mac_cmd)
                && i.modifiers.matches_exact(mods)
                && i.events.iter().any(|e| matches!(e, egui::Event::Copy))
            {
                i.events.retain(|e| !matches!(e, egui::Event::Copy));
                true
            } else {
                i.consume_key(mods, key)
            }
        })
    }
    pub fn show(&mut self, ui: &mut egui::Ui, path: &Path) {
        self.capture_pending(ui);
        ui.heading("Shortcuts");
        ui.weak("Select a key binding to change it.");
        ui.add_space(12.0);
        ui.horizontal(|ui| {
            let filter_width = (ui.available_width() - 170.0).max(90.0);
            ui.add_sized(
                [filter_width, 36.0],
                egui::TextEdit::singleline(&mut self.query)
                    .hint_text("Search actions or keys")
                    .margin(egui::Margin::symmetric(10, 8)),
            );
            if ui
                .add(
                    egui::Button::new("Save changes")
                        .fill(ui.visuals().selection.bg_fill)
                        .min_size(egui::vec2(100.0, 36.0)),
                )
                .clicked()
            {
                self.message = match self.validate().map_err(anyhow::Error::msg).and_then(|_| {
                    crate::persistence::atomic_write(path, &serde_json::to_vec_pretty(self)?)
                }) {
                    Ok(()) => "Shortcuts saved".into(),
                    Err(error) => error.to_string(),
                };
            }
            crate::icons::menu(ui, "Shortcut settings", |ui| {
                if ui.button("Restore all defaults").clicked() {
                    self.keys = Self::default().keys;
                    self.recording = None;
                    self.message = "Defaults restored. Save changes to keep them.".into();
                    ui.close_menu();
                }
            });
        });
        if let Some(action) = self.recording.as_deref() {
            let label = ACTIONS
                .iter()
                .find(|(id, _, _)| *id == action)
                .map_or(action, |(_, label, _)| *label);
            ui.horizontal_wrapped(|ui| {
                ui.label(
                    egui::RichText::new(format!("Recording · {label}"))
                        .color(ui.visuals().hyperlink_color),
                );
                ui.weak("Press the new keys. Esc cancels. Backspace clears.");
            });
        }
        if !self.message.is_empty() {
            ui.weak(&self.message);
        }
        ui.separator();

        let query = self.query.trim().to_lowercase();
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            let mut count = 0;
            for (group, name) in [("navigation", "Workspace navigation"), ("panes", "Windows and terminal panes"), ("clipboard", "Clipboard")] {
                let mut header = false;
                for &(action, label, default) in ACTIONS {
                    let shortcut = self.keys.get(action).cloned().unwrap_or_else(|| default.into());
                    let action_group = if action.starts_with("copy") { "clipboard" }
                        else if ["keybindings", "dashboard", "terminal_focus"].contains(&action) { "navigation" }
                        else { "panes" };
                    if action_group != group || !format!("{label} {shortcut}").to_lowercase().contains(&query) { continue; }
                    if !header {
                        ui.add_space(12.0);
                        ui.label(egui::RichText::new(name).strong().color(ui.visuals().weak_text_color()));
                        header = true;
                    }
                    count += 1;
                    ui.push_id(action, |ui| self.shortcut_row(ui, action, label, default, &shortcut));
                    ui.separator();
                }
            }
            if count == 0 { ui.weak("No matching shortcuts."); }
            if query.is_empty() {
                ui.add_space(14.0);
                ui.label(egui::RichText::new("Built-in controls").strong());
                ui.weak("Terminal: Tab completes. Ctrl+C interrupts when there is no selection. Ctrl+Space opens suggestions.");
                ui.weak("Editor: Tab indents when completion is closed.");
            }
        });
    }

    fn shortcut_row(
        &mut self,
        ui: &mut egui::Ui,
        action: &str,
        label: &str,
        default: &str,
        shortcut: &str,
    ) {
        ui.horizontal(|ui| {
            let binding_width = (ui.available_width() * 0.37).clamp(130.0, 225.0);
            let label_width = (ui.available_width() - binding_width - 65.0).max(50.0);
            ui.add_sized(
                [label_width, 36.0],
                egui::Label::new(label).truncate().halign(egui::Align::LEFT),
            )
            .on_hover_text(label);
            let recording = self.recording.as_deref() == Some(action);
            let keys = if recording {
                "Press keys…".into()
            } else if shortcut.is_empty() {
                "Unassigned".into()
            } else {
                shortcut
                    .replace("ArrowRight", "→")
                    .replace("ArrowLeft", "←")
                    .replace('+', " + ")
            };
            let color = if recording {
                ui.visuals().hyperlink_color
            } else if shortcut.is_empty() {
                ui.visuals().weak_text_color()
            } else {
                ui.visuals().text_color()
            };
            let fill = if recording {
                ui.visuals().selection.bg_fill
            } else {
                ui.visuals().extreme_bg_color
            };
            let binding = ui.add_sized(
                [binding_width, 34.0],
                egui::Button::new(egui::RichText::new(keys).monospace().color(color)).fill(fill),
            );
            if binding.on_hover_text("Record a shortcut").clicked() {
                self.recording = Some(action.into());
                self.message.clear();
            }
            crate::icons::menu(ui, "Binding actions", |ui| {
                for (title, value, enabled) in [
                    ("Reset to default", default, shortcut != default),
                    ("Clear binding", "", !shortcut.is_empty()),
                ] {
                    if ui.add_enabled(enabled, egui::Button::new(title)).clicked() {
                        let previous = self.keys.insert(action.into(), value.into());
                        match self.validate() {
                            Ok(()) => {
                                self.recording = None;
                                self.message = "Shortcut updated. Save changes to keep it.".into();
                            }
                            Err(error) => {
                                if let Some(previous) = previous {
                                    self.keys.insert(action.into(), previous);
                                } else {
                                    self.keys.remove(action);
                                }
                                self.message = error;
                            }
                        }
                        ui.close_menu();
                    }
                }
            });
        });
    }

    fn capture_pending(&mut self, ui: &egui::Ui) {
        if let Some(action) = self.recording.clone() {
            for event in ui.input(|i| i.events.clone()) {
                let shortcut = match event {
                    egui::Event::Key {
                        key: Key::Escape,
                        pressed: true,
                        ..
                    } => {
                        self.recording = None;
                        None
                    }
                    egui::Event::Key {
                        key: Key::Backspace | Key::Delete,
                        pressed: true,
                        modifiers,
                        ..
                    } if modifiers.is_none() => Some(String::new()),
                    egui::Event::Key {
                        key,
                        pressed: true,
                        modifiers,
                        ..
                    } => capture(modifiers, key),
                    egui::Event::Copy => capture(ui.input(|i| i.modifiers), Key::C),
                    egui::Event::Cut => capture(ui.input(|i| i.modifiers), Key::X),
                    _ => None,
                };
                if let Some(shortcut) = shortcut {
                    let old = self.keys.insert(action.clone(), shortcut.clone());
                    match self.validate() {
                        Ok(()) => {
                            self.recording = None;
                            self.message = "Shortcut updated; save to keep it.".into();
                        }
                        Err(e) => {
                            if let Some(old) = old {
                                self.keys.insert(action.clone(), old);
                            }
                            self.message = e;
                        }
                    }
                    break;
                }
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reject_collisions_and_terminal_conflicts() {
        let mut b = Bindings::default();
        b.validate().unwrap();
        b.keys.insert("terminal_focus".into(), "Ctrl+L".into());
        assert!(b.validate().is_err());
        b.keys
            .insert("terminal_focus".into(), "Ctrl+Shift+N".into());
        assert!(b.validate().is_err());
    }
}

#[cfg(test)]
mod review_tests {
    use super::*;
    #[test]
    fn new_view_shortcuts_preserve_custom_and_disabled_bindings() {
        let mut bindings = Bindings::default();
        bindings.keys.remove("toggle_terminal_view");
        bindings.keys.remove("terminal_splits");
        bindings.keys.insert("terminal_grid".into(), String::new());
        bindings
            .keys
            .insert("new_window".into(), "Ctrl+Shift+Space".into());
        bindings.add_missing_defaults();
        assert_eq!(bindings.keys["toggle_terminal_view"], "");
        assert_eq!(bindings.keys["terminal_grid"], "");
        assert_eq!(bindings.keys["terminal_splits"], "Ctrl+Alt+2");
        assert_eq!(bindings.keys["new_window"], "Ctrl+Shift+Space");
        bindings.validate().unwrap();
    }
    #[test]
    fn view_shortcut_is_consumed_before_terminal_input() {
        let ctx = egui::Context::default();
        let bindings = Bindings::default();
        let modifiers = Modifiers::CTRL | Modifiers::SHIFT;
        let input = egui::RawInput {
            modifiers,
            events: vec![egui::Event::Key {
                key: Key::Space,
                physical_key: Some(Key::Space),
                pressed: true,
                repeat: false,
                modifiers,
            }],
            ..Default::default()
        };
        let _ = ctx.run(input, |ctx| {
            assert!(bindings.take(ctx, "toggle_terminal_view"));
            assert!(!ctx.input(|i| i.key_pressed(Key::Space)));
        });
    }
    #[test]
    fn copy_shortcut_uses_clipboard_event() {
        let ctx = egui::Context::default();
        let bindings = Bindings::default();
        let mut input = egui::RawInput::default();
        input.modifiers = Modifiers::CTRL | Modifiers::SHIFT;
        input.events = vec![egui::Event::Copy];
        let _ = ctx.run(input, |ctx| {
            assert!(bindings.take(ctx, "copy_terminal"));
            assert!(!bindings.take(ctx, "copy_terminal"));
        });
    }
}

fn capture(modifiers: Modifiers, key: Key) -> Option<String> {
    if !modifiers.ctrl && !modifiers.alt && !modifiers.mac_cmd {
        return None;
    }
    let mut parts = vec![];
    if modifiers.ctrl {
        parts.push("Ctrl".to_string());
    }
    if modifiers.mac_cmd {
        parts.push("Cmd".to_string());
    }
    if modifiers.alt {
        parts.push("Alt".to_string());
    }
    if modifiers.shift {
        parts.push("Shift".to_string());
    }
    parts.push(key.name().to_string());
    Some(parts.join("+"))
}
#[cfg(test)]
mod capture_tests {
    use super::*;
    #[test]
    fn captured_chords_parse_and_plain_typing_is_ignored() {
        let chord = capture(Modifiers::CTRL | Modifiers::SHIFT, Key::N).unwrap();
        assert_eq!(chord, "Ctrl+Shift+N");
        parse(&chord).unwrap();
        assert!(capture(Modifiers::NONE, Key::A).is_none());
    }
}
