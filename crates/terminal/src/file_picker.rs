use crate::icons::{button, Icon};
use eframe::egui;
use std::path::PathBuf;

#[derive(Default)]
pub struct Picker {
    pub open: bool,
    folder: PathBuf,
    home: PathBuf,
    location: String,
    query: String,
    error: String,
    selected: Option<PathBuf>,
    entries: Vec<(bool, String, PathBuf)>,
    mode: Mode,
}

#[derive(Default, PartialEq, Eq)]
enum Mode {
    #[default]
    File,
    Image,
    Executable,
}

impl Picker {
    pub fn start(&mut self, folder: PathBuf) {
        self.begin(folder, Mode::File);
    }

    pub fn start_image(&mut self, workspace: PathBuf, current: Option<&std::path::Path>) {
        self.begin(workspace, Mode::Image);
        if let Some(parent) = current
            .and_then(std::path::Path::parent)
            .filter(|p| p.is_dir())
        {
            self.navigate(parent.to_path_buf());
        }
    }

    pub fn start_executable(&mut self, folder: PathBuf) {
        self.begin(folder, Mode::Executable);
    }

    fn begin(&mut self, folder: PathBuf, mode: Mode) {
        self.mode = mode;
        self.home = folder.clone();
        self.open = true;
        self.navigate(folder);
    }

    fn navigate(&mut self, folder: PathBuf) {
        self.location = folder.display().to_string();
        self.folder = folder;
        self.query.clear();
        self.error.clear();
        self.selected = None;
        self.refresh();
    }

    fn refresh(&mut self) {
        self.error.clear();
        match std::fs::read_dir(&self.folder) {
            Ok(entries) => {
                self.entries = entries
                    .filter_map(Result::ok)
                    .filter_map(|entry| {
                        let path = entry.path();
                        let folder = path.is_dir();
                        (folder || self.accepts(&path)).then(|| {
                            (
                                folder,
                                entry.file_name().to_string_lossy().into_owned(),
                                path,
                            )
                        })
                    })
                    .collect();
                self.entries
                    .sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
            }
            Err(error) => {
                self.entries.clear();
                self.error = format!("Could not open folder: {error}");
            }
        }
        if self
            .selected
            .as_ref()
            .is_some_and(|path| !self.entries.iter().any(|entry| &entry.2 == path))
        {
            self.selected = None;
        }
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Option<PathBuf> {
        let mut chosen = None;
        let mut cancelled = false;
        let modal = egui::Modal::new(egui::Id::new("file_picker"))
            .frame(
                egui::Frame::popup(&ctx.style())
                    .inner_margin(20)
                    .corner_radius(4),
            )
            .show(ctx, |ui| {
                ui.set_width((ctx.screen_rect().width() - 72.0).clamp(300.0, 620.0));
                ui.heading(match self.mode {
                    Mode::Image => "Choose a background image",
                    Mode::Executable => "Choose a shell executable",
                    Mode::File => "Open a text or code file",
                });
                ui.horizontal(|ui| {
                    if button(ui, Icon::Up, "Parent folder").clicked() {
                        if let Some(parent) = self.folder.parent() {
                            self.navigate(parent.into());
                        }
                    }
                    if button(ui, Icon::Home, "Starting folder").clicked() {
                        self.navigate(self.home.clone());
                    }
                    if button(ui, Icon::Refresh, "Refresh folder").clicked() {
                        self.refresh();
                    }
                    let path_width = ui.available_width();
                    let address = ui
                        .add(
                            egui::TextEdit::singleline(&mut self.location)
                                .desired_width(path_width)
                                .margin(egui::Margin::symmetric(10, 7))
                                .hint_text("Folder path"),
                        )
                        .on_hover_text("Enter a folder path and press Enter");
                    if address.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        let path = self.folder.join(self.location.trim());
                        if path.is_dir() {
                            self.navigate(path);
                        } else {
                            self.error = "Folder not found.".into();
                        }
                    }
                });
                ui.add(
                    egui::TextEdit::singleline(&mut self.query)
                        .desired_width(f32::INFINITY)
                        .hint_text("Filter files and folders")
                        .margin(egui::Margin::symmetric(10, 7)),
                );
                ui.separator();
                let mut next_folder = None;
                let filter = self.query.to_lowercase();
                egui::ScrollArea::vertical()
                    .id_salt(&self.folder)
                    .min_scrolled_height(150.0)
                    .max_height((ctx.screen_rect().height() * 0.48).clamp(150.0, 360.0))
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        let mut matched = false;
                        for (folder, name, path) in &self.entries {
                            if !name.to_lowercase().contains(&filter) {
                                continue;
                            }
                            matched = true;
                            let selected = self.selected.as_ref() == Some(path);
                            let mut row_padding = ui.spacing().button_padding;
                            row_padding.x = 28.0;
                            let row = ui
                                .scope(|ui| {
                                    ui.spacing_mut().button_padding = row_padding;
                                    ui.add(
                                        egui::Button::new(name)
                                            .selected(selected)
                                            .truncate()
                                            .min_size(egui::vec2(ui.available_width(), 30.0)),
                                    )
                                })
                                .inner;
                            let icon = if *folder { Icon::Folder } else { Icon::Preview };
                            let icon_rect = egui::Rect::from_center_size(
                                egui::pos2(row.rect.right() - 18.0, row.rect.center().y),
                                egui::Vec2::splat(16.0),
                            );
                            crate::icons::paint(
                                ui.painter(),
                                icon_rect,
                                icon,
                                ui.visuals().weak_text_color(),
                            );
                            if row.clicked() {
                                if *folder {
                                    next_folder = Some(path.clone());
                                } else {
                                    self.selected = Some(path.clone());
                                    if row.double_clicked() {
                                        chosen = Some(path.clone());
                                    }
                                }
                            }
                        }
                        if !matched && self.error.is_empty() {
                            ui.weak("No matching files or folders.");
                        }
                    });
                if let Some(folder) = next_folder {
                    self.navigate(folder);
                }
                if !self.error.is_empty() {
                    ui.colored_label(ui.visuals().error_fg_color, &self.error);
                }
                ui.separator();
                ui.horizontal(|ui| {
                    if let Some(name) = self.selected.as_ref().and_then(|path| path.file_name()) {
                        let width = ui.available_width() * 0.55;
                        ui.add_sized(
                            [width, 0.0],
                            egui::Label::new(name.to_string_lossy()).truncate(),
                        );
                    } else {
                        ui.weak("Select a file");
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let label = if self.mode == Mode::Image {
                            "Use image"
                        } else {
                            "Open"
                        };
                        if ui
                            .add_enabled(
                                self.selected.is_some(),
                                egui::Button::new(label).fill(ui.visuals().selection.bg_fill),
                            )
                            .clicked()
                        {
                            chosen = self.selected.clone();
                        }
                        cancelled = crate::widgets::button(ui, "Cancel").clicked();
                    });
                });
            });
        if chosen.is_some() || cancelled || modal.should_close() {
            self.open = false;
        }
        chosen
    }

    fn accepts(&self, path: &std::path::Path) -> bool {
        match self.mode {
            Mode::File => true,
            Mode::Executable => crate::custom_shells::is_executable(path),
            Mode::Image => path
                .extension()
                .and_then(|ext| ext.to_str())
                .is_some_and(|ext| {
                    matches!(
                        ext.to_ascii_lowercase().as_str(),
                        "png" | "jpg" | "jpeg" | "webp" | "gif" | "bmp"
                    )
                }),
        }
    }
}
