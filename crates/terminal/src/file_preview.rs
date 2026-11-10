use eframe::egui;
use std::{
    fs,
    io::{BufReader, Read},
    path::{Path, PathBuf},
    sync::mpsc,
};

const BYTE_LIMIT: u64 = 96 * 1024;
const ITEM_LIMIT: usize = 160;

struct FolderItem {
    path: PathBuf,
    directory: bool,
}

enum Content {
    Folder(Vec<FolderItem>, bool),
    Text(String, bool),
    Table(Vec<Vec<String>>, bool),
    Json(serde_json::Value),
    Image(egui::ColorImage, (u32, u32)),
    Binary(String),
    Link,
}

struct Loaded {
    content: Content,
    size: Option<u64>,
}

pub enum Action {
    Close,
    Properties(PathBuf),
    Open(PathBuf),
    Editor(PathBuf),
    Browse(PathBuf),
}

pub struct Preview {
    path: PathBuf,
    trail: Vec<PathBuf>,
    pending: mpsc::Receiver<anyhow::Result<Loaded>>,
    loaded: Option<anyhow::Result<Loaded>>,
    texture: Option<egui::TextureHandle>,
    expanded: bool,
    larger_image: bool,
    draft: Option<crate::preview_edit::Draft>,
    edit_status: String,
}

impl Preview {
    pub fn new(path: PathBuf, ctx: &egui::Context, expanded: bool) -> Self {
        Self {
            pending: request(path.clone(), ctx),
            path,
            trail: Vec::new(),
            loaded: None,
            texture: None,
            expanded,
            larger_image: false,
            draft: None,
            edit_status: String::new(),
        }
    }

    pub fn expanded(&self) -> bool {
        self.expanded
    }

    pub fn has_unsaved_changes(&self) -> bool {
        self.draft
            .as_ref()
            .is_some_and(crate::preview_edit::Draft::dirty)
    }

    fn change(&mut self, path: PathBuf, ctx: &egui::Context) {
        self.pending = request(path.clone(), ctx);
        self.path = path;
        self.loaded = None;
        self.texture = None;
        self.larger_image = false;
        self.draft = None;
        self.edit_status.clear();
    }

    pub fn show(&mut self, ui: &mut egui::Ui) -> Option<Action> {
        use crate::icons::{button, Icon};
        if self.loaded.is_none() {
            match self.pending.try_recv() {
                Ok(result) => self.loaded = Some(result),
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.loaded = Some(Err(anyhow::anyhow!("Preview worker stopped")))
                }
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        let mut action = None;
        ui.horizontal(|ui| {
            if !self.trail.is_empty()
                && !self.has_unsaved_changes()
                && button(ui, Icon::Back, "Back inside preview").clicked()
            {
                if let Some(parent) = self.trail.pop() {
                    self.change(parent, ui.ctx());
                }
            }
            ui.weak("CONTENTS");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if button(ui, Icon::Close, "Close preview").clicked() {
                    if self.has_unsaved_changes() {
                        self.edit_status = "Save or cancel your changes before closing.".into();
                    } else {
                        action = Some(Action::Close);
                    }
                }
                if button(
                    ui,
                    if self.expanded {
                        Icon::Restore
                    } else {
                        Icon::Maximize
                    },
                    if self.expanded {
                        "Restore preview"
                    } else {
                        "Expand preview"
                    },
                )
                .clicked()
                {
                    self.expanded = !self.expanded;
                }
            });
        });
        let title = self.path.file_name().unwrap_or_default().to_string_lossy();
        ui.heading(title.as_ref());
        let directory = matches!(
            &self.loaded,
            Some(Ok(Loaded {
                content: Content::Folder(..),
                ..
            }))
        );
        if let Some(Ok(Loaded {
            size: Some(size), ..
        })) = &self.loaded
        {
            ui.weak(format!(
                "{} · {size} bytes",
                crate::launcher::file_kind(&self.path, false).1
            ));
        }
        let can_inline_edit = matches!(
            &self.loaded,
            Some(Ok(Loaded {
                content: Content::Text(_, false),
                ..
            }))
        );
        let can_open_editor = matches!(
            &self.loaded,
            Some(Ok(Loaded {
                content: Content::Text(..) | Content::Json(..) | Content::Table(..),
                ..
            }))
        );
        if self.draft.is_some() {
            self.show_editor(ui);
            return action;
        }
        if !self.edit_status.is_empty() {
            ui.label(&self.edit_status);
        }
        ui.horizontal_wrapped(|ui| {
            if crate::widgets::button(
                ui,
                if directory {
                    "Enter folder"
                } else {
                    "Open with default app"
                },
            )
            .clicked()
            {
                action = Some(if directory {
                    Action::Browse(self.path.clone())
                } else {
                    Action::Open(self.path.clone())
                });
            }
            if can_inline_edit && crate::widgets::button(ui, "Edit here").clicked() {
                match crate::preview_edit::Draft::load(&self.path) {
                    Ok(draft) => {
                        self.draft = Some(draft);
                        self.edit_status.clear();
                    }
                    Err(error) => self.edit_status = format!("{error:#}"),
                }
            }
            if can_open_editor && crate::widgets::button(ui, "Open in editor").clicked() {
                action = Some(Action::Editor(self.path.clone()));
            }
            if crate::widgets::button(ui, "Properties").clicked() {
                action = Some(Action::Properties(self.path.clone()));
            }
        });
        ui.separator();
        let mut next = None;
        egui::ScrollArea::both()
            .auto_shrink([false, false])
            .id_salt(&self.path)
            .show(ui, |ui| match self.loaded.as_ref() {
                None => {
                    ui.label("Loading preview…");
                }
                Some(Err(error)) => {
                    ui.label(format!("Preview unavailable: {error:#}"));
                }
                Some(Ok(loaded)) => match &loaded.content {
                    Content::Folder(items, more) => {
                        ui.weak(format!(
                            "{}{} items",
                            items.len(),
                            if *more { "+" } else { "" }
                        ));
                        if items.is_empty() {
                            ui.label("This folder is empty");
                        }
                        for item in items {
                            let name = item.path.file_name().unwrap_or_default().to_string_lossy();
                            let (icon, kind) =
                                crate::launcher::file_kind(&item.path, item.directory);
                            ui.horizontal(|ui| {
                                let size = egui::vec2(20.0, 20.0);
                                let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
                                crate::tiles::paint_icon(
                                    ui.painter(),
                                    icon,
                                    rect.min,
                                    0.85,
                                    ui.visuals().text_color(),
                                );
                                let response =
                                    crate::widgets::button(ui, name.as_ref()).on_hover_text(kind);
                                if response.clicked() {
                                    next = Some(item.path.clone());
                                }
                                if item.directory {
                                    ui.weak("›");
                                }
                            });
                        }
                        if *more {
                            ui.weak("Enter the folder to see the rest.");
                        }
                    }
                    Content::Text(text, more) => {
                        if *more {
                            ui.weak("Showing the first 96 KiB.");
                        }
                        if text.is_empty() {
                            ui.weak("Empty file");
                        }
                        ui.add(
                            egui::Label::new(egui::RichText::new(text).monospace())
                                .wrap()
                                .selectable(true),
                        );
                    }
                    Content::Table(rows, limited) => {
                        ui.weak(if *limited {
                            "Table preview · limited rows or columns"
                        } else {
                            "Table preview"
                        });
                        egui::Grid::new("file_table")
                            .striped(true)
                            .spacing([14.0, 5.0])
                            .show(ui, |ui| {
                                for (index, row) in rows.iter().enumerate() {
                                    ui.weak((index + 1).to_string());
                                    for field in row {
                                        ui.add_sized(
                                            [160.0, 20.0],
                                            egui::Label::new(field.as_str())
                                                .truncate()
                                                .selectable(true),
                                        )
                                        .on_hover_text(field);
                                    }
                                    ui.end_row();
                                }
                            });
                    }
                    Content::Json(value) => {
                        ui.weak("JSON structure");
                        crate::preview_formats::show_json(ui, "Root", value, &mut 250);
                    }
                    Content::Image(image, (width, height)) => {
                        ui.horizontal_wrapped(|ui| {
                            ui.weak(format!("{width} × {height} px"));
                            ui.selectable_value(&mut self.larger_image, false, "Fit");
                            ui.selectable_value(&mut self.larger_image, true, "Larger");
                        });
                        let texture = self.texture.get_or_insert_with(|| {
                            ui.ctx().load_texture(
                                "file_preview",
                                image.clone(),
                                egui::TextureOptions::LINEAR,
                            )
                        });
                        let mut size = texture.size_vec2();
                        if !self.larger_image {
                            size *= (ui.available_width() / size.x).clamp(0.02, 1.0);
                        }
                        ui.image((texture.id(), size));
                    }
                    Content::Binary(bytes) => {
                        ui.weak("File header · hexadecimal");
                        ui.add(
                            egui::Label::new(egui::RichText::new(bytes).monospace())
                                .selectable(true),
                        );
                    }
                    Content::Link => {
                        ui.weak("This item is a link or a system-managed file.");
                    }
                },
            });
        if let Some(path) = next {
            self.trail.push(self.path.clone());
            self.change(path, ui.ctx());
        }
        action
    }
    fn show_editor(&mut self, ui: &mut egui::Ui) {
        let mut save =
            ui.input_mut(|input| input.consume_key(egui::Modifiers::COMMAND, egui::Key::S));
        let mut cancel = false;
        ui.horizontal_wrapped(|ui| {
            save |= crate::widgets::button(ui, "Save").clicked();
            cancel = crate::widgets::button(ui, "Cancel edits").clicked();
            ui.weak("Ctrl+S to save");
        });
        if !self.edit_status.is_empty() {
            ui.label(&self.edit_status);
        }
        let Some(draft) = self.draft.as_mut() else {
            return;
        };
        if cancel {
            self.draft = None;
            self.edit_status.clear();
            return;
        }
        if save {
            match draft.save(&self.path) {
                Ok(()) => {
                    self.change(self.path.clone(), ui.ctx());
                    self.edit_status = "Saved".into();
                    return;
                }
                Err(error) => self.edit_status = format!("Could not save: {error:#}"),
            }
        }
        egui::ScrollArea::vertical()
            .id_salt("inline_text_editor")
            .show(ui, |ui| {
                ui.add(
                    egui::TextEdit::multiline(&mut draft.text)
                        .code_editor()
                        .desired_width(ui.available_width())
                        .desired_rows(12),
                );
            });
    }
}

fn text_preview(path: &Path, text: &str, more: bool) -> Content {
    let text = text.trim_start_matches('\u{feff}');
    let extension = path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or_default();
    let delimiter = if extension.eq_ignore_ascii_case("csv") {
        Some(',')
    } else if extension.eq_ignore_ascii_case("tsv") {
        Some('\t')
    } else {
        None
    };
    if let Some(delimiter) = delimiter {
        let (rows, limited) = crate::preview_formats::table(text, delimiter, more);
        return Content::Table(rows, limited);
    }
    if !more && extension.eq_ignore_ascii_case("json") {
        if let Ok(value) = serde_json::from_str(text) {
            return Content::Json(value);
        }
    }
    Content::Text(text.to_owned(), more)
}

fn request(path: PathBuf, ctx: &egui::Context) -> mpsc::Receiver<anyhow::Result<Loaded>> {
    let (tx, rx) = mpsc::channel();
    let ctx = ctx.clone();
    std::thread::spawn(move || {
        let _ = tx.send(read(&path));
        ctx.request_repaint();
    });
    rx
}

pub fn thumbnail(path: &Path) -> anyhow::Result<egui::ColorImage> {
    image_preview(path, 96)
}

pub fn image_preview(path: &Path, max: u32) -> anyhow::Result<egui::ColorImage> {
    let metadata = fs::symlink_metadata(path)?;
    anyhow::ensure!(
        metadata.is_file() && !crate::file_properties::is_reparse_point(&metadata),
        "Not a regular image"
    );
    let mut header = [0; 32];
    let mut file = fs::File::open(path)?;
    file.read_exact(&mut header)?;
    let format = image::guess_format(&header)?;
    Ok(decode_image(path, format, max)?.0)
}

fn decode_image(
    path: &Path,
    format: image::ImageFormat,
    max: u32,
) -> anyhow::Result<(egui::ColorImage, (u32, u32))> {
    let mut reader = image::ImageReader::with_format(BufReader::new(fs::File::open(path)?), format);
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(8192);
    limits.max_image_height = Some(8192);
    limits.max_alloc = Some(96 * 1024 * 1024);
    reader.limits(limits);
    let decoded = reader.decode()?;
    let dimensions = (decoded.width(), decoded.height());
    let thumbnail = decoded.thumbnail(max, max).into_rgba8();
    let image = egui::ColorImage::from_rgba_unmultiplied(
        [thumbnail.width() as usize, thumbnail.height() as usize],
        thumbnail.as_raw(),
    );
    Ok((image, dimensions))
}

fn read(path: &Path) -> anyhow::Result<Loaded> {
    let metadata = fs::symlink_metadata(path)?;
    let size = metadata.is_file().then_some(metadata.len());
    let content = if metadata.is_symlink() || crate::file_properties::is_reparse_point(&metadata) {
        Content::Link
    } else if metadata.is_dir() {
        let mut items = Vec::new();
        let mut more = false;
        for entry in fs::read_dir(path)? {
            if items.len() == ITEM_LIMIT {
                more = true;
                break;
            }
            let entry = entry?;
            items.push(FolderItem {
                path: entry.path(),
                directory: entry.file_type()?.is_dir(),
            });
        }
        items.sort_by_key(|item| {
            (
                !item.directory,
                item.path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_lowercase(),
            )
        });
        Content::Folder(items, more)
    } else if metadata.is_file() {
        let mut file = fs::File::open(path)?;
        let mut data = Vec::new();
        file.by_ref().take(BYTE_LIMIT + 1).read_to_end(&mut data)?;
        if let Ok(format) = image::guess_format(&data) {
            let (image, dimensions) = decode_image(path, format, 1600)?;
            Content::Image(image, dimensions)
        } else {
            let more = data.len() as u64 > BYTE_LIMIT;
            data.truncate(BYTE_LIMIT as usize);
            match std::str::from_utf8(&data) {
                Ok(text)
                    if !text
                        .chars()
                        .any(|c| c.is_control() && !matches!(c, '\t' | '\r' | '\n')) =>
                {
                    text_preview(path, text, more)
                }
                Err(error) if more && error.error_len().is_none() && !data.contains(&0) => {
                    Content::Text(
                        String::from_utf8_lossy(&data[..error.valid_up_to()]).into_owned(),
                        true,
                    )
                }
                _ => Content::Binary(
                    data.chunks(12)
                        .take(8)
                        .map(|line| {
                            line.iter()
                                .map(|byte| format!("{byte:02X}"))
                                .collect::<Vec<_>>()
                                .join(" ")
                        })
                        .collect::<Vec<_>>()
                        .join("\n"),
                ),
            }
        }
    } else {
        Content::Link
    };
    Ok(Loaded { content, size })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nested_json_routes_to_the_structure_viewer() {
        assert!(
            matches!(text_preview(Path::new("sample.JSON"), r#"{"items":[1,2]}"#, false), Content::Json(value) if value["items"][1] == 2)
        );
        assert!(matches!(
            text_preview(Path::new("sample.json"), "invalid", false),
            Content::Text(..)
        ));
    }

    #[test]
    fn preview_reads_real_contents_and_bounds_large_text() -> anyhow::Result<()> {
        let temp = tempfile::tempdir_in(std::env::current_dir()?)?;
        let text = temp.path().join("notes");
        fs::write(&text, "hello workspace")?;
        assert!(
            matches!(read(&text)?.content, Content::Text(value, false) if value == "hello workspace")
        );
        fs::write(&text, vec![b'x'; BYTE_LIMIT as usize + 1])?;
        assert!(
            matches!(read(&text)?.content, Content::Text(value, true) if value.len() == BYTE_LIMIT as usize)
        );
        let image = temp.path().join("sample.png");
        image::RgbaImage::new(8, 12).save(&image)?;
        assert!(matches!(read(&image)?.content, Content::Image(_, (8, 12))));
        assert!(
            matches!(read(temp.path())?.content, Content::Folder(items, false) if items.len() == 2)
        );
        Ok(())
    }
}
