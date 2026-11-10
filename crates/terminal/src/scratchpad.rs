use eframe::egui;
use serde::{Deserialize, Serialize};
use std::{
    ops::Range,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

const LIMIT: usize = 8192;
const DELAY: Duration = Duration::from_millis(450);

#[derive(Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
struct Format {
    size: u8,
    bold: bool,
    color: Option<[u8; 3]>,
}

#[derive(Serialize, Deserialize)]
struct Span {
    start: usize,
    end: usize,
    format: Format,
}

#[derive(Clone, Serialize, Deserialize)]
pub enum Target {
    Folder(PathBuf),
    File(PathBuf),
    Page(u8),
}

#[derive(Serialize, Deserialize)]
struct Link {
    label: String,
    target: Target,
}

#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
struct Note {
    text: String,
    spans: Vec<Span>,
    links: Vec<Link>,
}

#[derive(Default)]
struct LinkDraft {
    label: String,
    path: String,
    kind: u8,
    page: u8,
}

#[derive(Default)]
pub struct Scratchpad {
    note: Note,
    file: PathBuf,
    pending: Option<Instant>,
    error: String,
    blocked: bool,
    typing: Format,
    selection: Option<Range<usize>>,
    link: Option<LinkDraft>,
}

const PAGES: &[(u8, &str)] = &[
    (0, "Dashboard"),
    (1, "Terminals"),
    (3, "Connections"),
    (7, "Files"),
    (8, "Recent locations"),
    (4, "Settings"),
    (6, "Shortcuts"),
];

impl Scratchpad {
    pub fn load(file: PathBuf) -> Self {
        let mut result = Self {
            file,
            ..Default::default()
        };
        match std::fs::read(&result.file) {
            Ok(bytes) => match serde_json::from_slice::<Note>(&bytes) {
                Ok(mut note) => {
                    note.text = note.text.chars().take(LIMIT).collect();
                    note.links.truncate(32);
                    result.note = note;
                }
                Err(error) => {
                    result.error = format!("Could not read the saved scratchpad: {error}");
                    result.blocked = true;
                }
            },
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                result.error = format!("Could not read the saved scratchpad: {error}");
                result.blocked = true;
            }
        }
        result
    }

    fn changed(&mut self, ctx: &egui::Context) {
        self.pending = Some(Instant::now());
        ctx.request_repaint_after(DELAY);
    }

    pub fn tick(&mut self, ctx: &egui::Context) {
        if let Some(pending) = self.pending {
            if pending.elapsed() >= DELAY {
                self.flush();
            } else {
                ctx.request_repaint_after(DELAY.saturating_sub(pending.elapsed()));
            }
        }
    }

    pub fn flush(&mut self) {
        if self.blocked || self.pending.take().is_none() {
            return;
        }
        self.error = serde_json::to_vec(&self.note)
            .map_err(anyhow::Error::from)
            .and_then(|bytes| crate::persistence::atomic_write(&self.file, &bytes))
            .err()
            .map_or_else(String::new, |error| {
                format!("Could not store the scratchpad: {error:#}")
            });
    }

    pub fn show(&mut self, ui: &mut egui::Ui, root: &Path, page_height: f32) -> Option<Target> {
        let top = ui.next_widget_position().y;
        ui.heading("Scratchpad");
        ui.weak("Type a note. Changes are kept automatically.");
        let default_size = egui::TextStyle::Body.resolve(ui.style()).size.round() as u8;
        let mut next_format = self.typing;
        ui.horizontal_wrapped(|ui| {
            if ui
                .selectable_label(next_format.bold, "Bold")
                .on_hover_text("Bold for selected text or new typing")
                .clicked()
            {
                next_format.bold = !next_format.bold;
            }
            let mut size = if next_format.size == 0 {
                default_size
            } else {
                next_format.size
            };
            if ui
                .add(egui::DragValue::new(&mut size).range(10..=32).suffix(" px"))
                .changed()
            {
                next_format.size = size;
            }
            let base = ui.visuals().text_color();
            let mut color = next_format.color.unwrap_or([base.r(), base.g(), base.b()]);
            if ui
                .color_edit_button_srgb(&mut color)
                .on_hover_text("Text color")
                .changed()
            {
                next_format.color = Some(color);
            }
            if crate::widgets::button(ui, "Clear formatting").clicked() {
                next_format = Format::default();
            }
            if crate::icons::button(ui, crate::icons::Icon::Plus, "Add a link").clicked() {
                if self.note.links.len() < 32 {
                    let label = self
                        .selection
                        .as_ref()
                        .map(|range| {
                            self.note
                                .text
                                .chars()
                                .skip(range.start)
                                .take(range.len().min(72))
                                .collect::<String>()
                        })
                        .unwrap_or_default();
                    self.link = Some(LinkDraft {
                        label,
                        page: 7,
                        ..Default::default()
                    });
                }
            }
        });
        let old = self.note.text.clone();
        let mut formats = formats(old.chars().count(), &self.note.spans);
        if next_format != self.typing {
            self.typing = next_format;
            if let Some(range) = self
                .selection
                .clone()
                .filter(|range| range.end <= formats.len())
            {
                formats[range].fill(next_format);
                self.note.spans = spans(&formats);
                self.changed(ui.ctx());
            }
        }
        let typing = self.typing;
        let mut layouter = |ui: &egui::Ui, text: &str, width: f32| {
            let next = rebase(&old, text, &formats, typing);
            let mut job = egui::text::LayoutJob::default();
            job.wrap.max_width = width;
            let mut start = 0;
            let mut previous = next.first().copied().unwrap_or(typing);
            for (index, (offset, _)) in text.char_indices().enumerate() {
                let current = next[index];
                if current != previous {
                    append(&mut job, ui, &text[start..offset], previous);
                    start = offset;
                    previous = current;
                }
            }
            append(&mut job, ui, &text[start..], previous);
            ui.fonts(|fonts| fonts.layout_job(job))
        };
        self.link_editor(ui, root);
        let mut result = None;
        let mut remove = None;
        if !self.note.links.is_empty() {
            ui.add_space(4.0);
            ui.horizontal_wrapped(|ui| {
                for (index, link) in self.note.links.iter().enumerate() {
                    ui.push_id(index, |ui| {
                        let label =
                            egui::RichText::new(&link.label).color(ui.visuals().hyperlink_color);
                        let response = crate::widgets::button(ui, label);
                        if response.clicked() {
                            result = Some(link.target.clone());
                        }
                        response.context_menu(|ui| {
                            if ui.button("Remove link").clicked() {
                                remove = Some(index);
                                ui.close_menu();
                            }
                        });
                    });
                }
            });
        }
        if let Some(index) = remove {
            self.note.links.remove(index);
            self.changed(ui.ctx());
        }
        if !self.error.is_empty() {
            ui.colored_label(ui.visuals().error_fg_color, &self.error);
        }
        ui.separator();
        let writing_height = (page_height - (ui.next_widget_position().y - top)).max(100.0);
        let output = egui::TextEdit::multiline(&mut self.note.text)
            .id_salt("scratchpad_page")
            .hint_text("Write a note…")
            .char_limit(LIMIT)
            .frame(false)
            .desired_rows(1)
            .desired_width(f32::INFINITY)
            .min_size(egui::vec2(0.0, writing_height))
            .margin(egui::Margin::ZERO)
            .layouter(&mut layouter)
            .show(ui);
        if let Some(range) = output.cursor_range {
            let range = range.as_sorted_char_range();
            if output.response.has_focus() || output.response.changed() {
                self.selection = (!range.is_empty()).then_some(range);
            }
        }
        if output.response.changed() {
            self.note.spans = spans(&rebase(&old, &self.note.text, &formats, self.typing));
            self.changed(ui.ctx());
        }
        result
    }

    fn link_editor(&mut self, ui: &mut egui::Ui, root: &Path) {
        let Some(draft) = self.link.as_mut() else {
            return;
        };
        let mut added = None;
        let mut cancel = false;
        ui.add_space(6.0);
        ui.strong("Add link");
        ui.horizontal_wrapped(|ui| {
            for (kind, label) in [(0, "Folder"), (1, "File"), (2, "App page")] {
                ui.selectable_value(&mut draft.kind, kind, label);
            }
        });
        ui.add(egui::TextEdit::singleline(&mut draft.label).hint_text("Link label"));
        if draft.kind == 2 {
            crate::icons::combo_menu_icon(egui::ComboBox::from_id_salt("scratchpad_page"))
                .selected_text(
                    PAGES
                        .iter()
                        .find(|(id, _)| *id == draft.page)
                        .map_or("Files", |(_, name)| *name),
                )
                .show_ui(ui, |ui| {
                    for &(page, name) in PAGES {
                        ui.selectable_value(&mut draft.page, page, name);
                    }
                });
        } else {
            ui.add(
                egui::TextEdit::singleline(&mut draft.path)
                    .hint_text("Absolute path or path from your workspace")
                    .desired_width(f32::INFINITY),
            );
        }
        ui.horizontal(|ui| {
            if crate::widgets::button(ui, "Add link").clicked() {
                let label = draft.label.trim();
                let path = root.join(draft.path.trim());
                let target = match draft.kind {
                    2 => Some(Target::Page(draft.page)),
                    0 if !draft.path.trim().is_empty() && path.is_dir() => {
                        Some(Target::Folder(path))
                    }
                    1 if path.is_file() => Some(Target::File(path)),
                    _ => None,
                };
                if let Some(target) = target {
                    let label = if !label.is_empty() {
                        label.into()
                    } else {
                        match &target {
                            Target::Page(page) => PAGES
                                .iter()
                                .find(|(id, _)| id == page)
                                .map_or("Page", |(_, name)| *name)
                                .into(),
                            Target::Folder(path) | Target::File(path) => path
                                .file_name()
                                .unwrap_or(path.as_os_str())
                                .to_string_lossy()
                                .into_owned(),
                        }
                    };
                    added = Some(Link { label, target });
                } else if !self.blocked {
                    self.error = "Choose an existing file or folder.".into();
                }
            }
            cancel = crate::widgets::button(ui, "Cancel").clicked();
        });
        if let Some(link) = added {
            self.note.links.push(link);
            self.link = None;
            self.changed(ui.ctx());
        } else if cancel {
            self.link = None;
        }
    }
}

fn append(job: &mut egui::text::LayoutJob, ui: &egui::Ui, text: &str, format: Format) {
    let mut font_id = egui::TextStyle::Body.resolve(ui.style());
    if format.size > 0 {
        font_id.size = format.size.clamp(10, 32) as f32;
    }
    if format.bold {
        font_id.family = egui::FontFamily::Name("scratchpad-bold".into());
    }
    let color = format.color.map_or(ui.visuals().text_color(), |[r, g, b]| {
        egui::Color32::from_rgb(r, g, b)
    });
    job.append(
        text,
        0.0,
        egui::TextFormat {
            font_id,
            color,
            ..Default::default()
        },
    );
}

fn formats(len: usize, spans: &[Span]) -> Vec<Format> {
    let mut result = vec![Format::default(); len];
    for span in spans {
        if let Some(range) = result.get_mut(span.start..span.end) {
            range.fill(span.format);
        }
    }
    result
}

fn spans(formats: &[Format]) -> Vec<Span> {
    let mut result = Vec::new();
    let mut start = 0;
    while start < formats.len() {
        let format = formats[start];
        let mut end = start + 1;
        while end < formats.len() && formats[end] == format {
            end += 1;
        }
        if format != Format::default() {
            result.push(Span { start, end, format });
        }
        start = end;
    }
    result
}

fn rebase(old: &str, new: &str, previous: &[Format], typing: Format) -> Vec<Format> {
    let prefix = old
        .chars()
        .zip(new.chars())
        .take_while(|(a, b)| a == b)
        .count();
    let new_len = new.chars().count();
    let suffix = old
        .chars()
        .rev()
        .zip(new.chars().rev())
        .take(previous.len().min(new_len).saturating_sub(prefix))
        .take_while(|(a, b)| a == b)
        .count();
    let mut result = vec![typing; new_len];
    result[..prefix].copy_from_slice(&previous[..prefix]);
    result[new_len - suffix..].copy_from_slice(&previous[previous.len() - suffix..]);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unicode_formatting_and_links_survive_restart() -> anyhow::Result<()> {
        let root = tempfile::tempdir_in(std::env::current_dir()?)?;
        let file = root.path().join("scratchpad.json");
        let mut pad = Scratchpad::load(file.clone());
        let bold = Format {
            bold: true,
            ..Default::default()
        };
        let changed = rebase("aé", "a🙂é", &[Format::default(), bold], Format::default());
        assert_eq!(changed.len(), 3);
        assert!(changed[2].bold);
        pad.note.text = "a🙂é".into();
        pad.note.spans = spans(&changed);
        pad.note.links.push(Link {
            label: "Files".into(),
            target: Target::Page(7),
        });
        pad.pending = Some(Instant::now());
        pad.flush();
        let restored = Scratchpad::load(file);
        assert_eq!(restored.note.text, "a🙂é");
        assert!(formats(3, &restored.note.spans)[2].bold);
        assert!(matches!(restored.note.links[0].target, Target::Page(7)));
        Ok(())
    }
}
