use eframe::egui;
use std::path::PathBuf;
#[derive(Default)]
pub struct Picker {
    pub open: bool,
    folder: PathBuf,
    query: String,
    error: String,
    images_only: bool,
    choose_executable: bool,
}
impl Picker {
    pub fn start(&mut self, folder: PathBuf) {
        self.images_only = false;
        self.choose_executable = false;
        self.folder = folder;
        self.open = true;
        self.query.clear();
        self.error.clear();
    }
    pub fn start_image(&mut self, folder: PathBuf) {
        self.start(folder);
        self.images_only = true;
    }

    pub fn start_executable(&mut self, folder: PathBuf) {
        self.start(folder);
        self.choose_executable = true;
    }

    pub fn show(&mut self, ui: &mut egui::Ui) -> Option<PathBuf> {
        let mut selected = None;
        ui.heading(if self.choose_executable {
            "Choose a shell executable"
        } else if self.images_only {
            "Choose a background image"
        } else {
            "Open a text or code file"
        });
        ui.horizontal(|ui| {
            if crate::widgets::button(ui, "Up").clicked() {
                if let Some(parent) = self.folder.parent() {
                    self.folder = parent.into();
                }
            }
            if crate::widgets::button(ui, "Cancel").clicked() {
                self.open = false;
            }
            ui.label(self.folder.display().to_string());
        });
        ui.add(egui::TextEdit::singleline(&mut self.query).hint_text("Filter files"));
        ui.add_space(12.0);
        match std::fs::read_dir(&self.folder) {
            Ok(entries) => {
                let mut entries: Vec<_> = entries.filter_map(Result::ok).collect();
                entries.sort_by_key(|e| (!e.path().is_dir(), e.file_name()));
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for entry in entries {
                        let path = entry.path();
                        if self.images_only
                            && !path.is_dir()
                            && !path
                                .extension()
                                .and_then(|ext| ext.to_str())
                                .is_some_and(|ext| {
                                    matches!(
                                        ext.to_ascii_lowercase().as_str(),
                                        "png" | "jpg" | "jpeg" | "webp" | "gif" | "bmp"
                                    )
                                })
                        {
                            continue;
                        }
                        if self.choose_executable
                            && !path.is_dir()
                            && !crate::custom_shells::is_executable(&path)
                        {
                            continue;
                        }
                        let name = entry.file_name().to_string_lossy().into_owned();
                        if !name.to_lowercase().contains(&self.query.to_lowercase()) {
                            continue;
                        }
                        let label = if path.is_dir() {
                            format!("{name} /")
                        } else {
                            name
                        };
                        if crate::widgets::button(ui, label).clicked() {
                            if path.is_dir() {
                                self.folder = path;
                                self.query.clear();
                            } else {
                                self.open = false;
                                selected = Some(path);
                            }
                        }
                    }
                });
            }
            Err(e) => self.error = e.to_string(),
        }
        if !self.error.is_empty() {
            ui.colored_label(ui.visuals().error_fg_color, &self.error);
        }
        selected
    }
}
