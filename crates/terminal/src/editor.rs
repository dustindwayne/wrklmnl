use crate::{
    completion::{byte_cursor, Completion, Context},
    highlight,
};
use eframe::egui;
use std::path::{Path, PathBuf};
pub struct Editor {
    text: String,
    saved: String,
    language: String,
    path: String,
    opened: Option<PathBuf>,
    completion: Completion,
    message: String,
    cursor: usize,
    pending_open: Option<PathBuf>,
    pending_save: Option<PathBuf>,
    pending_close: bool,
}
impl Default for Editor {
    fn default() -> Self {
        Self {
            text: String::new(),
            saved: String::new(),
            language: "rust".into(),
            path: String::new(),
            opened: None,
            completion: Completion::default(),
            message: "Open a file or choose a language. Ctrl+Space requests completion.".into(),
            cursor: 0,
            pending_open: None,
            pending_save: None,
            pending_close: false,
        }
    }
}
impl Editor {
    pub fn has_document(&self) -> bool {
        self.opened.is_some() || self.pending_open.is_some()
    }
    pub fn request_open(&mut self, path: PathBuf) -> Result<(), String> {
        let meta = std::fs::metadata(&path).map_err(|e| e.to_string())?;
        if !meta.is_file() || meta.len() > 8 * 1024 * 1024 {
            return Err("Choose a text/code file up to 8 MiB".into());
        }
        let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
        if bytes.contains(&0) || std::str::from_utf8(&bytes).is_err() {
            return Err("This file is binary or is not UTF-8 text".into());
        }
        if self.dirty() {
            self.pending_open = Some(path);
        } else {
            self.open(path);
        }
        Ok(())
    }
    pub fn dirty(&self) -> bool {
        self.text != self.saved
    }
    fn resolve(&self, cwd: &Path) -> PathBuf {
        let path = PathBuf::from(&self.path);
        if path.is_absolute() {
            path
        } else {
            cwd.join(path)
        }
    }
    fn open(&mut self, path: PathBuf) {
        match std::fs::read_to_string(&path) {
            Ok(text) => {
                self.text = text;
                self.saved = self.text.clone();
                self.language = language_for(&path).into();
                self.opened = Some(path.clone());
                self.path = path.display().to_string();
                self.completion.dismiss();
                self.message = format!("Opened {}", path.display());
            }
            Err(e) => self.message = e.to_string(),
        }
    }
    fn save(&mut self, path: PathBuf) {
        match crate::persistence::atomic_write(&path, self.text.as_bytes()) {
            Ok(()) => {
                self.saved = self.text.clone();
                self.opened = Some(path.clone());
                self.path = path.display().to_string();
                self.message = format!("Saved {}", path.display());
            }
            Err(e) => self.message = e.to_string(),
        }
    }
    pub fn show(&mut self, ui: &mut egui::Ui, light: bool, size: f32, syntax: bool, cwd: &Path) {
        ui.horizontal(|ui| {
            if let Some(path) = &self.opened {
                ui.label(path.file_name().unwrap_or_default().to_string_lossy());
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if crate::widgets::button(ui, "Close file").clicked() {
                    if self.dirty() {
                        self.pending_close = true;
                    } else {
                        *self = Self::default();
                    }
                }
            });
        });
        if self.pending_close {
            ui.horizontal(|ui| {
                ui.label("Discard unsaved changes?");
                if crate::widgets::button(ui, "Discard and close").clicked() {
                    *self = Self::default();
                }
                if crate::widgets::button(ui, "Keep editing").clicked() {
                    self.pending_close = false;
                }
            });
        }
        if !self.has_document() {
            return;
        }
        ui.horizontal_wrapped(|ui| {
            ui.heading(if self.dirty() {
                "Code editor / Unsaved"
            } else {
                "Code editor"
            });
            crate::icons::combo_menu_icon(egui::ComboBox::from_id_salt("code_language"))
                .selected_text(&self.language)
                .show_ui(ui, |ui| {
                    for lang in [
                        "plain",
                        "markdown",
                        "rust",
                        "python",
                        "javascript",
                        "typescript",
                        "json",
                        "powershell",
                        "bash",
                        "cmd",
                        "ui",
                    ] {
                        ui.selectable_value(&mut self.language, lang.into(), lang);
                    }
                });
        });
        ui.horizontal_wrapped(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut self.path)
                    .hint_text("File path")
                    .desired_width(440.0),
            );
            if crate::widgets::button(ui, "Open").clicked() {
                let path = self.resolve(cwd);
                if self.dirty() {
                    self.pending_open = Some(path);
                } else {
                    self.open(path);
                }
            }
            let save = crate::widgets::button(ui, "Save").clicked()
                || ui.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::S));
            if save {
                if self.path.trim().is_empty() {
                    self.message = "Enter a file path".into();
                } else {
                    let path = self.resolve(cwd);
                    if path.exists() && self.opened.as_ref() != Some(&path) {
                        self.pending_save = Some(path);
                    } else {
                        self.save(path);
                    }
                }
            }
        });
        if let Some(path) = self.pending_open.clone() {
            ui.horizontal_wrapped(|ui| {
                ui.label(format!(
                    "Discard unsaved changes and open {}?",
                    path.display()
                ));
                if crate::widgets::button(ui, "Discard and open").clicked() {
                    self.pending_open = None;
                    self.open(path);
                }
                if crate::widgets::button(ui, "Cancel").clicked() {
                    self.pending_open = None;
                }
            });
        }
        if let Some(path) = self.pending_save.clone() {
            ui.horizontal_wrapped(|ui| {
                ui.label(format!("Replace {}?", path.display()));
                if crate::widgets::button(ui, "Replace file").clicked() {
                    self.pending_save = None;
                    self.save(path);
                }
                if crate::widgets::button(ui, "Cancel").clicked() {
                    self.pending_save = None;
                }
            });
        }
        self.completion.poll();
        let id = egui::Id::new("code_editor");
        let focus = ui.memory(|m| m.has_focus(id));
        self.completion.keyboard(ui, focus);
        let trigger =
            focus && ui.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, egui::Key::Space));
        let tab = focus
            && !self.completion.items.is_empty()
            && ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Tab));
        if tab {
            self.completion.apply(&mut self.text);
            self.completion.restore_cursor(ui, &self.text, id);
        }
        let lang = self.language.clone();
        let mut layouter = |ui: &egui::Ui, text: &str, width: f32| {
            let mut job =
                highlight::layout_sized(text, if syntax { &lang } else { "plain" }, light, size);
            job.wrap.max_width = width;
            ui.fonts(|f| f.layout_job(job))
        };
        let result = egui::ScrollArea::both()
            .id_salt("editor_scroll")
            .show(ui, |ui| {
                egui::TextEdit::multiline(&mut self.text)
                    .id(id)
                    .code_editor()
                    .desired_width(f32::INFINITY)
                    .desired_rows(((ui.available_height() - 48.0) / (size * 1.4)).max(5.0) as usize)
                    .layouter(&mut layouter)
                    .show(ui)
            })
            .inner;
        if let Some(range) = result.cursor_range {
            self.cursor = byte_cursor(&self.text, range.primary.ccursor.index);
        }
        if result.response.changed() || trigger {
            let root = self
                .opened
                .as_ref()
                .and_then(|p| p.parent())
                .unwrap_or(cwd)
                .to_path_buf();
            let context = if matches!(self.language.as_str(), "powershell" | "cmd" | "bash") {
                Context::Commands {
                    shell: match self.language.as_str() {
                        "powershell" => {
                            if cfg!(windows) {
                                "powershell.exe"
                            } else {
                                "pwsh"
                            }
                        }
                        "cmd" => "cmd.exe",
                        _ => "bash",
                    }
                    .into(),
                    cwd: root,
                    environment: std::env::var_os("CONDA_PREFIX").map(PathBuf::from),
                }
            } else {
                Context::Code {
                    language: self.language.clone(),
                    cwd: root,
                    file: self.opened.clone(),
                }
            };
            self.completion
                .request(&self.text, self.cursor, context, trigger);
        }
        self.completion
            .popup(ui, &mut self.text, result.response.rect, id);
        ui.label(egui::RichText::new(&self.message).small());
    }
}
fn language_for(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "rs" => "rust",
        "py" => "python",
        "js" | "jsx" => "javascript",
        "ts" | "tsx" => "typescript",
        "json" => "json",
        "ps1" | "psm1" => "powershell",
        "bat" | "cmd" => "cmd",
        "sh" | "bash" => "bash",
        "md" => "markdown",
        _ => "plain",
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn file_routes_are_contextual_and_preserve_dirty_documents() {
        let base = std::env::temp_dir().join(format!(
            "dterm-editor-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&base).unwrap();
        let first = base.join("first.rs");
        let next = base.join("next.txt");
        let binary = base.join("binary.dat");
        std::fs::write(&first, "fn main() {}\n").unwrap();
        std::fs::write(&next, "Text\n").unwrap();
        std::fs::write(&binary, [0, 159, 255]).unwrap();
        let mut editor = Editor::default();
        assert!(!editor.has_document());
        editor.request_open(first.clone()).unwrap();
        assert!(editor.has_document());
        assert_eq!(editor.language, "rust");
        editor.text.push_str("// unsaved");
        editor.request_open(next.clone()).unwrap();
        assert_eq!(editor.opened.as_ref(), Some(&first));
        assert_eq!(editor.pending_open.as_ref(), Some(&next));
        assert!(editor.dirty());
        assert!(editor.request_open(binary.clone()).is_err());
        for file in [first, next, binary] {
            std::fs::remove_file(file).unwrap();
        }
        std::fs::remove_dir(base).unwrap();
    }
    #[test]
    fn unknown_files_are_plain() {
        assert_eq!(language_for(Path::new("x.txt")), "plain");
        assert_eq!(language_for(Path::new("x.md")), "markdown");
    }
    #[test]
    fn paths_resolve_under_active_root() {
        let mut e = Editor::default();
        e.path = "file.rs".into();
        assert_eq!(
            e.resolve(Path::new("project")),
            Path::new("project/file.rs")
        );
    }
}
