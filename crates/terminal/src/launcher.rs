use crate::{
    file_preview::{self, Preview},
    file_properties::{self, Properties},
};
use eframe::egui;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

pub enum Action {
    Terminal(PathBuf),
    Open(PathBuf),
    Editor(PathBuf),
}

type ThumbnailJob = (PathBuf, std::sync::mpsc::Receiver<Option<egui::ColorImage>>);

pub struct Workspace {
    settings: PathBuf,
    layout_file: PathBuf,
    positions: BTreeMap<PathBuf, [f32; 2]>,
    root: PathBuf,
    folder: PathBuf,
    input: String,
    entries: Vec<(PathBuf, bool)>,
    error: String,
    loaded: bool,
    selected: Option<PathBuf>,
    drag_offset: egui::Vec2,
    back: Vec<PathBuf>,
    forward: Vec<PathBuf>,
    filter: Option<String>,
    new_folder: Option<String>,
    properties: Option<Properties>,
    preview: Option<Preview>,
    show_contents: bool,
    thumbnails: BTreeMap<PathBuf, Option<egui::TextureHandle>>,
    thumbnail_jobs: Vec<ThumbnailJob>,
    import: Option<std::sync::mpsc::Receiver<Vec<String>>>,
}

impl Workspace {
    pub fn new(state: &Path, fallback: &Path) -> Self {
        let settings = state.with_extension("workspace.json");
        let saved = std::fs::read(&settings)
            .ok()
            .and_then(|data| serde_json::from_slice::<PathBuf>(&data).ok());
        let desktop = desktop();
        let organized = desktop
            .as_ref()
            .map(|path| path.join("Organized Desktop"))
            .filter(|path| path.is_dir());
        let root = saved
            .or(organized)
            .or(desktop)
            .unwrap_or_else(|| fallback.to_path_buf());
        let layout_file = state.with_extension("desktop-layout.json");
        let positions = std::fs::read(&layout_file)
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default();
        Self {
            settings,
            layout_file,
            positions,
            folder: root.clone(),
            input: root.display().to_string(),
            root,
            entries: Vec::new(),
            error: String::new(),
            loaded: false,
            selected: None,
            drag_offset: ICON_SIZE * 0.5,
            back: Vec::new(),
            forward: Vec::new(),
            filter: None,
            new_folder: None,
            properties: None,
            preview: None,
            show_contents: true,
            thumbnails: BTreeMap::new(),
            thumbnail_jobs: Vec::new(),
            import: None,
        }
    }

    #[cfg(feature = "capture")]
    pub fn capture_select(&mut self, path: PathBuf, ctx: &egui::Context) {
        self.browse(self.root.clone());
        self.select_preview(path, ctx, false);
    }

    pub fn has_unsaved_preview(&self) -> bool {
        self.preview
            .as_ref()
            .is_some_and(Preview::has_unsaved_changes)
    }

    fn can_leave_preview(&mut self) -> bool {
        if self.has_unsaved_preview() {
            self.error = "Save or cancel the current preview edit first.".into();
            return false;
        }
        true
    }

    fn select_preview(&mut self, path: PathBuf, ctx: &egui::Context, expanded: bool) {
        if !self.can_leave_preview() {
            return;
        }
        self.show_contents = true;
        self.selected = Some(path.clone());
        self.preview = Some(Preview::new(path, ctx, expanded));
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    fn select(&mut self, path: PathBuf) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.can_leave_preview(),
            "Save or cancel the current preview edit first"
        );
        anyhow::ensure!(path.is_absolute(), "Enter an absolute folder path");
        let _ = std::fs::read_dir(&path)?;
        crate::persistence::atomic_write(&self.settings, &serde_json::to_vec_pretty(&path)?)?;
        self.input = path.display().to_string();
        self.root = path.clone();
        self.back.clear();
        self.forward.clear();
        self.properties = None;
        self.preview = None;
        self.browse(path);
        Ok(())
    }

    pub fn reveal(&mut self, path: PathBuf, file: bool, ctx: &egui::Context) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.can_leave_preview(),
            "Save or cancel the current preview edit first"
        );
        if file {
            anyhow::ensure!(path.is_file(), "The linked file is unavailable");
            let parent = path
                .parent()
                .ok_or_else(|| anyhow::anyhow!("The linked file has no parent folder"))?;
            self.navigate(parent.to_path_buf());
            self.select_preview(path, ctx, false);
        } else {
            anyhow::ensure!(path.is_dir(), "The linked folder is unavailable");
            self.navigate(path);
        }
        Ok(())
    }

    fn navigate(&mut self, path: PathBuf) {
        if path == self.folder || !self.can_leave_preview() {
            return;
        }
        self.back.push(self.folder.clone());
        self.forward.clear();
        self.enter_folder(path);
    }

    fn enter_folder(&mut self, path: PathBuf) {
        self.filter = None;
        self.new_folder = None;
        self.properties = None;
        self.preview = None;
        self.browse(path);
    }

    fn browse(&mut self, path: PathBuf) {
        self.loaded = true;
        self.folder = path;
        self.error.clear();
        self.entries.clear();
        self.thumbnails.clear();
        self.thumbnail_jobs.clear();
        self.selected = None;
        match std::fs::read_dir(&self.folder) {
            Ok(entries) => {
                for entry in entries {
                    match entry {
                        Ok(entry) => {
                            let path = entry.path();
                            self.entries.push((path.clone(), path.is_dir()));
                        }
                        Err(error) => self.error = error.to_string(),
                    }
                }
                self.entries.sort_by_key(|(path, folder)| {
                    (
                        !folder,
                        path.file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .to_lowercase(),
                    )
                });
            }
            Err(error) => self.error = error.to_string(),
        }
    }

    pub fn settings(&mut self, ui: &mut egui::Ui) {
        ui.heading("Workspace location");
        ui.label("Choose the folder this launcher opens. Files stay where they are.");
        ui.horizontal(|ui| {
            let field = ui.add(
                egui::TextEdit::singleline(&mut self.input)
                    .desired_width(420.0)
                    .hint_text("Absolute folder path"),
            );
            let enter = field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            let path = if crate::widgets::button(ui, "Use folder").clicked() || enter {
                Some(PathBuf::from(self.input.trim()))
            } else if crate::widgets::button(ui, "Use Desktop").clicked() {
                desktop()
            } else {
                None
            };
            if let Some(path) = path {
                if let Err(error) = self.select(path) {
                    self.error = error.to_string();
                }
            }
        });
        ui.weak(self.root.display().to_string());
        if !self.error.is_empty() {
            ui.label(&self.error);
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui) -> Option<Action> {
        if !self.loaded {
            self.browse(self.root.clone());
        }
        if let Some(Ok(errors)) = self.import.as_ref().map(|rx| rx.try_recv()) {
            self.import = None;
            self.browse(self.folder.clone());
            self.error = errors.join("\n");
        }
        use crate::icons::{button, Icon};
        self.thumbnail_jobs
            .retain(|(path, rx)| match rx.try_recv() {
                Ok(image) => {
                    let texture = image.map(|pixels| {
                        ui.ctx().load_texture(
                            path.to_string_lossy(),
                            pixels,
                            egui::TextureOptions::LINEAR,
                        )
                    });
                    self.thumbnails.insert(path.clone(), texture);
                    false
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => false,
                Err(std::sync::mpsc::TryRecvError::Empty) => true,
            });
        ui.heading("Files");
        let mut action = None;
        let mut dropped = None;
        let external =
            ui.input(|i| !i.raw.hovered_files.is_empty() || !i.raw.dropped_files.is_empty());
        let mut import_to = self.folder.clone();
        let fullscreen = ui.input(|input| input.viewport().fullscreen.unwrap_or(false));
        egui::Frame::NONE
            .inner_margin(egui::Margin::symmetric(0, 4))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled_ui(!self.back.is_empty(), |ui| button(ui, Icon::Back, "Back"))
                        .inner
                        .clicked()
                    {
                        if self.can_leave_preview() {
                            if let Some(path) = self.back.pop() {
                                self.forward.push(self.folder.clone());
                                self.enter_folder(path);
                            }
                        }
                    }
                    if ui
                        .add_enabled_ui(!self.forward.is_empty(), |ui| {
                            button(ui, Icon::Forward, "Forward")
                        })
                        .inner
                        .clicked()
                    {
                        if self.can_leave_preview() {
                            if let Some(path) = self.forward.pop() {
                                self.back.push(self.folder.clone());
                                self.enter_folder(path);
                            }
                        }
                    }
                    let parent = self
                        .folder
                        .parent()
                        .filter(|path| {
                            !self.folder.starts_with(&self.root) || path.starts_with(&self.root)
                        })
                        .map(Path::to_path_buf);
                    let up = ui
                        .add_enabled_ui(parent.is_some(), |ui| {
                            button(ui, Icon::Up, "Up one folder")
                        })
                        .inner;
                    if let Some(parent) = parent {
                        if let Some(path) = folder_drop(ui, &up, &parent) {
                            dropped = Some((path, parent.clone()));
                        }
                        if up.clicked() {
                            self.navigate(parent);
                        }
                    }
                    let root = button(ui, Icon::Home, "Workspace root");
                    if let Some(path) = folder_drop(ui, &root, &self.root) {
                        dropped = Some((path, self.root.clone()));
                    }
                    if root.clicked() {
                        self.navigate(self.root.clone());
                    }
                    if button(ui, Icon::Refresh, "Refresh").clicked() && self.can_leave_preview() {
                        self.preview = None;
                        self.browse(self.folder.clone());
                    }
                    ui.separator();
                    if button(ui, Icon::Plus, "New folder").clicked() {
                        self.new_folder = if self.new_folder.is_some() {
                            None
                        } else {
                            Some(String::new())
                        };
                        self.filter = None;
                    }
                    if button(ui, Icon::Search, "Find in this folder").clicked() {
                        self.filter = if self.filter.is_some() {
                            None
                        } else {
                            Some(String::new())
                        };
                        self.new_folder = None;
                    }
                    let contents = button(
                        ui,
                        Icon::Preview,
                        if self.show_contents {
                            "Hide contents"
                        } else {
                            "Show contents"
                        },
                    );
                    if self.show_contents {
                        ui.painter().line_segment(
                            [contents.rect.left_bottom(), contents.rect.right_bottom()],
                            egui::Stroke::new(1.0_f32, ui.visuals().hyperlink_color),
                        );
                    }
                    if contents.clicked() {
                        self.show_contents = !self.show_contents;
                    }
                    if button(ui, Icon::Terminal, "Open terminal here").clicked() {
                        action = Some(Action::Terminal(self.folder.clone()));
                    }
                    if button(ui, Icon::Folder, "Open in system file manager").clicked() {
                        action = Some(Action::Open(self.folder.clone()));
                    }
                    if button(
                        ui,
                        if fullscreen {
                            Icon::Restore
                        } else {
                            Icon::Maximize
                        },
                        if fullscreen {
                            "Exit full screen"
                        } else {
                            "Full screen"
                        },
                    )
                    .clicked()
                        || (fullscreen && ui.input(|input| input.key_pressed(egui::Key::Escape)))
                    {
                        ui.ctx()
                            .send_viewport_cmd(egui::ViewportCommand::Fullscreen(!fullscreen));
                    }
                });
            });
        let mut breadcrumbs: Vec<_> = self
            .folder
            .ancestors()
            .take_while(|path| !self.folder.starts_with(&self.root) || path.starts_with(&self.root))
            .map(Path::to_path_buf)
            .collect();
        breadcrumbs.reverse();
        egui::ScrollArea::horizontal()
            .id_salt("file_breadcrumbs")
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    for (index, path) in breadcrumbs.into_iter().enumerate() {
                        if index > 0 {
                            ui.weak("›");
                        }
                        let name = path
                            .file_name()
                            .map(|name| name.to_string_lossy().into_owned())
                            .unwrap_or_else(|| path.display().to_string());
                        let crumb = crate::widgets::button(ui, name)
                            .on_hover_text(path.display().to_string());
                        if let Some(source) = folder_drop(ui, &crumb, &path) {
                            dropped = Some((source, path.clone()));
                        }
                        if crumb.clicked() {
                            self.navigate(path);
                        }
                    }
                });
            });
        if let Some(filter) = self.filter.as_mut() {
            ui.add(egui::TextEdit::singleline(filter).hint_text("Find files and folders by name"));
        }
        if let Some(name) = self.new_folder.as_mut() {
            let create = ui
                .horizontal(|ui| {
                    let field =
                        ui.add(egui::TextEdit::singleline(name).hint_text("New folder name"));
                    ui.button("Create").clicked()
                        || (field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)))
                })
                .inner;
            if create {
                let result = file_properties::validate_name(name).and_then(|()| {
                    std::fs::create_dir(self.folder.join(&*name)).map_err(Into::into)
                });
                match result {
                    Ok(()) => {
                        self.new_folder = None;
                        self.browse(self.folder.clone());
                    }
                    Err(error) => self.error = format!("Could not create folder: {error:#}"),
                }
            }
        }
        ui.separator();
        if self.import.is_some() {
            ui.weak("Copying dropped files…");
        }
        if !self.error.is_empty() {
            ui.label(&self.error);
        }
        let mut next = None;
        if self.show_contents && self.preview.is_none() {
            self.preview = Some(Preview::new(self.folder.clone(), ui.ctx(), false));
        }
        let expanded = self.show_contents && self.preview.as_ref().is_some_and(Preview::expanded);
        let mut preview_action = None;
        if let Some(preview) = self.preview.as_mut().filter(|_| self.show_contents) {
            let tint = if ui.visuals().dark_mode {
                egui::Color32::from_gray(25)
            } else {
                egui::Color32::from_gray(242)
            };
            let frame = egui::Frame::NONE.fill(tint).inner_margin(16);
            if expanded {
                frame.show(ui, |ui| {
                    preview_action = preview.show(ui);
                });
            } else if ui.available_width() >= 650.0 {
                let width = (ui.available_width() * 0.39).clamp(280.0, 570.0);
                egui::SidePanel::right("file_preview")
                    .resizable(true)
                    .default_width(width)
                    .min_width(260.0)
                    .show_separator_line(false)
                    .frame(frame)
                    .show_inside(ui, |ui| {
                        preview_action = preview.show(ui);
                    });
            } else {
                let height = (ui.available_height() * 0.48).max(130.0);
                egui::TopBottomPanel::bottom("file_preview")
                    .default_height(height)
                    .min_height(100.0)
                    .resizable(true)
                    .show_separator_line(false)
                    .frame(frame)
                    .show_inside(ui, |ui| {
                        preview_action = preview.show(ui);
                    });
            }
        } else {
            ui.weak("Use the Contents icon to open the folder and file viewer.");
        }
        match preview_action {
            Some(file_preview::Action::Close) => {
                self.show_contents = false;
                self.preview = None;
                self.selected = None;
            }
            Some(file_preview::Action::Browse(path)) => next = Some(path),
            Some(file_preview::Action::Open(path)) => action = Some(Action::Open(path)),
            Some(file_preview::Action::Editor(path)) => action = Some(Action::Editor(path)),
            Some(file_preview::Action::Properties(path)) => {
                self.properties = Some(Properties::new(path))
            }
            None => {}
        }
        let mut inspect = None;
        let filter = self.filter.as_deref().unwrap_or_default().to_lowercase();
        if !expanded {
            egui::ScrollArea::both()
                .drag_to_scroll(false)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    let viewport = ui.available_size();
                    let layout = desktop_layout(&self.entries, &self.positions, viewport.x);
                    let canvas_size = layout.iter().fold(viewport, |size, rect| {
                        size.max(rect.max.to_vec2() + egui::vec2(8.0, 8.0))
                    });
                    let (canvas, _) = ui.allocate_exact_size(canvas_size, egui::Sense::hover());
                    for ((path, directory), rect) in self.entries.iter().zip(&layout) {
                        let rect = rect.translate(canvas.min.to_vec2());
                        if !ui.is_rect_visible(rect) {
                            continue;
                        }
                        let name = path.file_name().unwrap_or_default().to_string_lossy();
                        if !filter.is_empty() && !name.to_lowercase().contains(&filter) {
                            continue;
                        }
                        let (icon, _) = file_kind(path, *directory);
                        if icon == "image"
                            && self.thumbnails.len() < 64
                            && self.thumbnail_jobs.len() < 2
                            && !self.thumbnails.contains_key(path)
                            && !self
                                .thumbnail_jobs
                                .iter()
                                .any(|(pending, _)| pending == path)
                        {
                            let (tx, rx) = std::sync::mpsc::channel();
                            self.thumbnail_jobs.push((path.clone(), rx));
                            let path = path.clone();
                            let ctx = ui.ctx().clone();
                            std::thread::spawn(move || {
                                let _ = tx.send(file_preview::thumbnail(&path).ok());
                                ctx.request_repaint();
                            });
                        }
                        let response = crate::tiles::desktop_icon(
                            ui,
                            ui.id().with(path),
                            rect,
                            icon,
                            self.thumbnails.get(path).and_then(Option::as_ref),
                            &name,
                            self.selected.as_ref() == Some(path),
                        )
                        .on_hover_text(path.display().to_string());
                        if response.drag_started() {
                            let press = ui
                                .input(|input| input.pointer.press_origin())
                                .unwrap_or(rect.center());
                            self.drag_offset =
                                (press - rect.min).clamp(egui::Vec2::ZERO, ICON_SIZE);
                        }
                        response.dnd_set_drag_payload(path.clone());
                        if *directory {
                            if external && response.contains_pointer() {
                                import_to = path.clone();
                                mark_target(ui, &response);
                            }
                            if let Some(source) = folder_drop(ui, &response, path) {
                                dropped = Some((source, path.clone()));
                            }
                        }
                        if response.clicked() || response.secondary_clicked() {
                            self.selected = Some(path.clone());
                            inspect = Some((path.clone(), response.secondary_clicked(), false));
                        }
                        if response.double_clicked()
                            || (response.has_focus()
                                && ui.input(|i| i.key_pressed(egui::Key::Enter)))
                        {
                            if *directory {
                                next = Some(path.clone());
                            } else {
                                inspect = Some((path.clone(), false, true));
                            }
                        }
                    }
                    if dropped.is_none() {
                        if let Some(pointer) = ui
                            .ctx()
                            .pointer_interact_pos()
                            .filter(|pos| canvas.intersect(ui.clip_rect()).contains(*pos))
                        {
                            if let Some(source) = egui::DragAndDrop::payload::<PathBuf>(ui.ctx()) {
                                let desired =
                                    (pointer - canvas.min - self.drag_offset).max(egui::Vec2::ZERO);
                                let obstacles: Vec<_> = self
                                    .entries
                                    .iter()
                                    .zip(&layout)
                                    .filter_map(|((path, _), rect)| {
                                        (path != source.as_ref()).then_some(*rect)
                                    })
                                    .collect();
                                let placement = nearest_open_spot(desired, &obstacles);
                                if let Some(position) = placement {
                                    ui.painter().rect_stroke(
                                        egui::Rect::from_min_size(canvas.min + position, ICON_SIZE)
                                            .shrink(3.0),
                                        0.0,
                                        egui::Stroke::new(
                                            1.0_f32,
                                            ui.visuals().hyperlink_color.gamma_multiply(0.45),
                                        ),
                                        egui::StrokeKind::Inside,
                                    );
                                }
                                if ui.input(|input| input.pointer.primary_released()) {
                                    let _ = egui::DragAndDrop::take_payload::<PathBuf>(ui.ctx());
                                    if let Some(position) = placement {
                                        self.positions.insert(
                                            source.as_ref().clone(),
                                            [position.x, position.y],
                                        );
                                        let result = serde_json::to_vec(&self.positions)
                                            .map_err(anyhow::Error::from)
                                            .and_then(|bytes| {
                                                crate::persistence::atomic_write(
                                                    &self.layout_file,
                                                    &bytes,
                                                )
                                            });
                                        if let Err(error) = result {
                                            self.error =
                                                format!("Could not save icon position: {error:#}");
                                        }
                                    } else {
                                        self.error =
                                            "No room to place the icon near that spot".into();
                                    }
                                }
                            }
                        }
                    }
                });
        }
        let files = ui.input(|i| {
            i.raw
                .dropped_files
                .iter()
                .filter_map(|file| file.path.clone())
                .collect::<Vec<_>>()
        });
        if !files.is_empty() {
            if self.import.is_some() {
                self.error = "Wait for the current file copy to finish".into();
            } else {
                let (tx, rx) = std::sync::mpsc::channel();
                self.import = Some(rx);
                let root = self.root.clone();
                let ctx = ui.ctx().clone();
                std::thread::spawn(move || {
                    let errors = files
                        .iter()
                        .filter_map(|file| {
                            file_properties::import_file(file, &import_to, &root)
                                .err()
                                .map(|error| format!("{}: {error:#}", file.display()))
                        })
                        .collect();
                    let _ = tx.send(errors);
                    ctx.request_repaint();
                });
            }
        }
        if let Some((source, target)) = dropped.filter(|_| self.can_leave_preview()) {
            match file_properties::move_into(&source, &target, &self.root) {
                Ok(()) => {
                    self.properties = None;
                    self.preview = None;
                    self.browse(self.folder.clone());
                }
                Err(error) => self.error = format!("Could not move item: {error:#}"),
            }
        }
        if let Some((path, properties, expanded)) = inspect {
            if properties {
                self.properties = Some(Properties::new(path.clone()));
            }
            self.select_preview(path, ui.ctx(), expanded);
        }
        if let Some(path) = next {
            self.properties = None;
            self.navigate(path);
        }
        if action.is_some() && !self.has_unsaved_preview() {
            self.properties = None;
        }
        if let Some(properties) = self.properties.as_mut() {
            let (open, changed, launch) = properties.show(ui.ctx());
            if !open {
                self.properties = None;
            }
            if changed {
                self.browse(self.folder.clone());
            }
            if let Some(path) = launch {
                self.properties = None;
                if path.is_dir() {
                    self.navigate(path);
                } else {
                    action = Some(Action::Open(path));
                }
            }
        }
        if let Some(source) = egui::DragAndDrop::payload::<PathBuf>(ui.ctx()) {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
            if let Some(pointer) = ui.ctx().pointer_interact_pos() {
                let name = source.file_name().unwrap_or_default().to_string_lossy();
                let ghost = ui.ctx().layer_painter(egui::LayerId::new(
                    egui::Order::Tooltip,
                    egui::Id::new("file_drag"),
                ));
                let origin = pointer - self.drag_offset;
                let rect = egui::Rect::from_min_size(origin, ICON_SIZE);
                let color = ui.visuals().hyperlink_color;
                let icon = file_kind(
                    source.as_ref(),
                    self.entries
                        .iter()
                        .any(|(path, directory)| path == source.as_ref() && *directory),
                )
                .0;
                crate::tiles::paint_icon(
                    &ghost,
                    icon,
                    egui::pos2(rect.center().x - 20.0, origin.y + 7.0),
                    2.0,
                    color,
                );
                ghost.text(
                    egui::pos2(rect.center().x, origin.y + 52.0),
                    egui::Align2::CENTER_TOP,
                    name,
                    egui::FontId::proportional(13.0),
                    color,
                );
            }
        }
        action
    }
}

fn mark_target(ui: &egui::Ui, response: &egui::Response) {
    ui.painter().rect_stroke(
        response.rect.shrink(2.0),
        0.0,
        egui::Stroke::new(1.5_f32, ui.visuals().hyperlink_color),
        egui::StrokeKind::Inside,
    );
}

fn folder_drop(
    ui: &egui::Ui,
    response: &egui::Response,
    folder: &Path,
) -> Option<std::sync::Arc<PathBuf>> {
    if !response.contains_pointer() {
        return None;
    }
    let source = egui::DragAndDrop::payload::<PathBuf>(ui.ctx())?;
    if source.as_path() == folder
        || source.parent() == Some(folder)
        || folder.starts_with(source.as_path())
    {
        return None;
    }
    mark_target(ui, response);
    response.dnd_release_payload::<PathBuf>()
}

fn nearest_open_spot(desired: egui::Vec2, obstacles: &[egui::Rect]) -> Option<egui::Vec2> {
    let free = |point: egui::Vec2| {
        let rect = egui::Rect::from_min_size(egui::pos2(point.x, point.y), ICON_SIZE);
        !obstacles.iter().any(|other| rect.intersects(*other))
    };
    if free(desired) {
        return Some(desired);
    }
    for offset in [8.0, 16.0, 24.0, 32.0, 40.0, 48.0, 56.0, 64.0] {
        for (x, y) in [
            (1., 0.),
            (-1., 0.),
            (0., 1.),
            (0., -1.),
            (1., 1.),
            (1., -1.),
            (-1., 1.),
            (-1., -1.),
        ] {
            let candidate = (desired + egui::vec2(x, y) * offset).max(egui::Vec2::ZERO);
            if free(candidate) {
                return Some(candidate);
            }
        }
    }
    None
}

const ICON_SIZE: egui::Vec2 = egui::vec2(105.0, 88.0);
const CELL_SIZE: egui::Vec2 = egui::vec2(115.0, 98.0);

fn desktop_layout(
    entries: &[(PathBuf, bool)],
    positions: &BTreeMap<PathBuf, [f32; 2]>,
    width: f32,
) -> Vec<egui::Rect> {
    let mut reserved: Vec<_> = entries
        .iter()
        .filter_map(|(path, _)| positions.get(path))
        .map(|&[x, y]| egui::Rect::from_min_size(egui::pos2(x, y), ICON_SIZE))
        .collect();
    let columns = (width / CELL_SIZE.x).floor().max(1.0) as usize;
    let mut slot = 0;
    entries
        .iter()
        .map(|(path, _)| {
            if let Some(&[x, y]) = positions.get(path) {
                return egui::Rect::from_min_size(egui::pos2(x, y), ICON_SIZE);
            }
            loop {
                let origin = egui::pos2(
                    5.0 + (slot % columns) as f32 * CELL_SIZE.x,
                    5.0 + (slot / columns) as f32 * CELL_SIZE.y,
                );
                slot += 1;
                let candidate = egui::Rect::from_min_size(origin, ICON_SIZE);
                if !reserved.iter().any(|rect| rect.intersects(candidate)) {
                    reserved.push(candidate);
                    return candidate;
                }
            }
        })
        .collect()
}

pub fn file_kind(path: &Path, folder: bool) -> (&'static str, &'static str) {
    if folder {
        return ("folder", "Folder");
    }
    match path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "svg" | "bmp" | "ico" | "tiff" => {
            ("image", "Image")
        }
        "mp3" | "flac" | "wav" | "ogg" | "m4a" | "aac" => ("audio", "Audio"),
        "mp4" | "mov" | "mkv" | "avi" | "webm" => ("video", "Video"),
        "zip" | "7z" | "tar" | "gz" | "rar" | "xz" => ("archive", "Archive"),
        "xls" | "xlsx" | "ods" | "csv" | "tsv" => ("spreadsheet", "Spreadsheet"),
        "rs" | "py" | "js" | "ts" | "c" | "cpp" | "h" | "html" | "css" | "toml" | "json"
        | "yaml" | "yml" | "ps1" | "sh" => ("code", "Code or configuration"),
        "txt" | "md" | "pdf" | "doc" | "docx" | "odt" | "rtf" => ("document", "Document"),
        "exe" | "msi" | "lnk" | "app" | "desktop" => ("app", "Application or shortcut"),
        _ => ("file", "File"),
    }
}

fn desktop() -> Option<PathBuf> {
    directories::UserDirs::new()?
        .desktop_dir()
        .map(Path::to_path_buf)
}

pub fn open(path: &Path) -> std::io::Result<()> {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        let path: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
        let result = unsafe {
            windows_sys::Win32::UI::Shell::ShellExecuteW(
                std::ptr::null_mut(),
                std::ptr::null(),
                path.as_ptr(),
                std::ptr::null(),
                std::ptr::null(),
                1,
            )
        };
        if result as isize <= 32 {
            return Err(std::io::Error::other(format!(
                "Windows could not open the item ({})",
                result as isize
            )));
        }
    }
    #[cfg(not(windows))]
    std::process::Command::new(if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    })
    .arg(path)
    .spawn()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn files_opens_with_a_folder_contents_view() -> anyhow::Result<()> {
        let temp = tempfile::tempdir_in(std::env::current_dir()?)?;
        let mut workspace = Workspace::new(&temp.path().join("state.json"), temp.path());
        workspace.select(temp.path().to_path_buf())?;
        let ctx = egui::Context::default();
        let output = ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1000.0, 750.0),
                )),
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    workspace.show(ui);
                });
            },
        );
        fn paints_contents(shape: &egui::Shape) -> bool {
            match shape {
                egui::Shape::Text(text) => text.galley.job.text == "CONTENTS",
                egui::Shape::Vec(items) => items.iter().any(paints_contents),
                _ => false,
            }
        }
        assert!(output
            .shapes
            .iter()
            .any(|item| paints_contents(&item.shape)));
        assert!(workspace.show_contents && workspace.preview.is_some());
        assert!(!workspace.preview.as_ref().unwrap().expanded());
        Ok(())
    }

    #[test]
    fn drop_uses_exact_position_or_finds_nearby_room() {
        let obstacle = egui::Rect::from_min_size(egui::pos2(8.0, 8.0), ICON_SIZE);
        let desired = egui::vec2(210.0, 30.0);
        assert_eq!(nearest_open_spot(desired, &[obstacle]), Some(desired));
        let adjusted = nearest_open_spot(egui::vec2(100.0, 25.0), &[obstacle]).unwrap();
        assert!(
            !egui::Rect::from_min_size(egui::pos2(adjusted.x, adjusted.y), ICON_SIZE)
                .intersects(obstacle)
        );
    }

    #[test]
    fn desktop_layout_keeps_saved_positions_and_places_new_icons_clear() {
        let pinned = PathBuf::from("pinned.txt");
        let entries = [(PathBuf::from("new.txt"), false), (pinned.clone(), false)];
        let positions = BTreeMap::from([(pinned, [37.0, 20.0])]);
        for width in [120.0, 900.0] {
            let layout = desktop_layout(&entries, &positions, width);
            assert_eq!(layout[1].min, egui::pos2(37.0, 20.0));
            assert!(!layout[0].intersects(layout[1]));
        }
    }

    #[test]
    fn existing_workspace_is_saved_and_reopened_in_place() -> anyhow::Result<()> {
        let temp = tempfile::tempdir_in(std::env::current_dir()?)?;
        let folder = temp.path().join("Organized Desk");
        std::fs::create_dir(&folder)?;
        let file = folder.join("notes.txt");
        std::fs::write(&file, "keep")?;
        let state = temp.path().join("recent.json");
        let mut workspace = Workspace::new(&state, temp.path());
        workspace.select(folder.clone())?;
        assert_eq!(workspace.entries, vec![(file.clone(), false)]);
        assert!(workspace.select(folder.join("missing")).is_err());
        assert_eq!(Workspace::new(&state, temp.path()).root(), folder);
        assert_eq!(std::fs::read_to_string(file)?, "keep");
        Ok(())
    }
}
