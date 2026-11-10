use eframe::egui::{self, FontData, FontDefinitions, FontFamily, FontId, TextStyle};
use serde::{Deserialize, Serialize};
use std::{path::Path, sync::Arc};

#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Appearance {
    pub ui_size: f32,
    pub terminal_size: f32,
    pub ui_font: String,
    pub terminal_font: String,
    pub syntax: bool,
    pub light: bool,
    pub accent: [u8; 3],
    pub top_background: crate::bar_background::Settings,
    pub side_background: crate::bar_background::Settings,
}
impl Default for Appearance {
    fn default() -> Self {
        Self {
            ui_size: 15.0,
            terminal_size: 15.0,
            ui_font: String::new(),
            terminal_font: String::new(),
            syntax: true,
            light: false,
            accent: [182, 182, 178],
            top_background: Default::default(),
            side_background: Default::default(),
        }
    }
}
impl Appearance {
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        match std::fs::read(path) {
            Ok(bytes) => {
                let value: Self = serde_json::from_slice(&bytes)?;
                value.validate()?;
                Ok(value)
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(e.into()),
        }
    }
    pub fn validate(&self) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.ui_size.is_finite() && (12.0..=28.0).contains(&self.ui_size),
            "Interface text size must be 12-28"
        );
        anyhow::ensure!(
            self.terminal_size.is_finite() && (10.0..=36.0).contains(&self.terminal_size),
            "Terminal text size must be 10-36"
        );
        Ok(())
    }
    pub fn apply(&self, ctx: &egui::Context) -> anyhow::Result<()> {
        self.validate()?;
        let mut warnings = vec![];
        let mut fonts = FontDefinitions::default();
        for (name, path, family) in [
            ("user-interface", &self.ui_font, FontFamily::Proportional),
            ("user-terminal", &self.terminal_font, FontFamily::Monospace),
        ] {
            if path.trim().is_empty() {
                continue;
            }
            let bytes = match (|| -> anyhow::Result<Vec<u8>> {
                anyhow::ensure!(
                    std::fs::metadata(path)?.len() <= 16 * 1024 * 1024,
                    "Font exceeds 16 MiB"
                );
                let bytes = std::fs::read(path)?;
                ab_glyph::FontArc::try_from_vec(bytes.clone())
                    .map_err(|_| anyhow::anyhow!("Invalid font"))?;
                Ok(bytes)
            })() {
                Ok(b) => b,
                Err(e) => {
                    warnings.push(format!("{path}: {e}; using bundled font"));
                    continue;
                }
            };
            fonts
                .font_data
                .insert(name.into(), Arc::new(FontData::from_owned(bytes)));
            fonts
                .families
                .entry(family)
                .or_default()
                .insert(0, name.into());
        }
        let candidates: Vec<std::path::PathBuf> = if cfg!(windows) {
            let dir = std::env::var_os("WINDIR")
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|| "C:/Windows".into())
                .join("Fonts");
            vec![dir.join("msyh.ttc"), dir.join("seguiemj.ttf")]
        } else if cfg!(target_os = "macos") {
            vec![
                "/System/Library/Fonts/PingFang.ttc".into(),
                "/System/Library/Fonts/Apple Symbols.ttf".into(),
            ]
        } else {
            vec![
                "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc".into(),
                "/usr/share/fonts/truetype/noto/NotoSansCJK-Regular.ttf".into(),
                "/usr/share/fonts/truetype/noto/NotoEmoji-Regular.ttf".into(),
            ]
        };
        for (i, path) in candidates.iter().enumerate() {
            if std::fs::metadata(path).is_ok_and(|m| m.len() <= 40 * 1024 * 1024) {
                if let Ok(bytes) = std::fs::read(path) {
                    if ab_glyph::FontArc::try_from_vec(bytes.clone()).is_ok() {
                        let name = format!("system-fallback-{i}");
                        fonts
                            .font_data
                            .insert(name.clone(), Arc::new(FontData::from_owned(bytes)));
                        for family in [FontFamily::Monospace, FontFamily::Proportional] {
                            fonts.families.entry(family).or_default().push(name.clone());
                        }
                    }
                }
            }
        }
        if let Some(name) = fonts
            .families
            .get(&FontFamily::Proportional)
            .and_then(|v| v.first())
        {
            if let Some(data) = fonts.font_data.get(name) {
                if let Ok(font) = ab_glyph::FontArc::try_from_vec(data.font.to_vec()) {
                    crate::pixel_surface::set_font(ctx, font);
                }
            }
        }
        let mut bold_family = fonts
            .families
            .get(&FontFamily::Proportional)
            .cloned()
            .unwrap_or_default();
        let bold_candidates: Vec<std::path::PathBuf> = if cfg!(windows) {
            vec![std::env::var_os("WINDIR")
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|| "C:/Windows".into())
                .join("Fonts/segoeuib.ttf")]
        } else if cfg!(target_os = "macos") {
            vec!["/System/Library/Fonts/Supplemental/Arial Bold.ttf".into()]
        } else {
            vec![
                "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf".into(),
                "/usr/share/fonts/truetype/liberation2/LiberationSans-Bold.ttf".into(),
            ]
        };
        if let Some(bytes) = bold_candidates.iter().find_map(|path| {
            let bytes = std::fs::read(path).ok()?;
            ab_glyph::FontArc::try_from_vec(bytes.clone()).ok()?;
            Some(bytes)
        }) {
            fonts.font_data.insert(
                "scratchpad-bold".into(),
                Arc::new(FontData::from_owned(bytes)),
            );
            bold_family.insert(0, "scratchpad-bold".into());
        }
        fonts
            .families
            .insert(FontFamily::Name("scratchpad-bold".into()), bold_family);
        ctx.set_fonts(fonts);
        ctx.style_mut(|style| {
            for (kind, size, family) in [
                (TextStyle::Body, self.ui_size, FontFamily::Proportional),
                (TextStyle::Button, self.ui_size, FontFamily::Proportional),
                (
                    TextStyle::Small,
                    self.ui_size * 0.85,
                    FontFamily::Proportional,
                ),
                (
                    TextStyle::Heading,
                    self.ui_size * 1.6,
                    FontFamily::Proportional,
                ),
                (
                    TextStyle::Monospace,
                    self.terminal_size,
                    FontFamily::Monospace,
                ),
            ] {
                style.text_styles.insert(kind, FontId::new(size, family));
            }
        });
        if warnings.is_empty() {
            Ok(())
        } else {
            anyhow::bail!("{}", warnings.join("; "))
        }
    }
    pub fn save(&self, path: &Path) -> anyhow::Result<()> {
        self.validate()?;
        if let Ok(bytes) = std::fs::read(path) {
            if serde_json::from_slice::<Self>(&bytes)
                .ok()
                .is_none_or(|a| a.validate().is_err())
            {
                let backup = path.with_extension(format!(
                    "invalid-{}.json",
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_nanos()
                ));
                crate::persistence::atomic_write(&backup, &bytes)?;
            }
        }
        crate::persistence::atomic_write(path, &serde_json::to_vec_pretty(self)?)
    }
    pub fn commit(&self, ctx: &egui::Context, path: &Path) -> anyhow::Result<String> {
        self.validate()?;
        let warning = self.apply(ctx).err();
        self.save(path)?;
        Ok(warning
            .map(|e| format!("Appearance saved / {e}"))
            .unwrap_or_else(|| "Theme, accent, fonts, sizes and syntax preference saved".into()))
    }

    pub fn controls(
        &mut self,
        ui: &mut egui::Ui,
        choose: &mut Option<crate::bar_background::Bar>,
    ) -> bool {
        ui.set_max_width(980.0);
        let mut apply = false;
        ui.horizontal(|ui| {
            ui.heading("Appearance");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                apply = crate::widgets::add(
                    ui,
                    egui::Button::new(
                        egui::RichText::new("Apply changes").color(ui.visuals().hyperlink_color),
                    )
                    .frame(false)
                    .fill(ui.visuals().selection.bg_fill),
                )
                .clicked();
            });
        });
        ui.label(
            egui::RichText::new("Make the workspace comfortable to read.")
                .color(ui.visuals().weak_text_color()),
        );
        ui.add_space(24.0);
        let id = ui.id().with("appearance_section");
        let mut section = ui.ctx().data_mut(|d| d.get_temp::<u8>(id).unwrap_or(0));
        ui.horizontal_wrapped(|ui| {
            for (i, label) in [
                (0, "Theme & color"),
                (1, "Typography"),
                (2, "Terminal"),
                (3, "Backgrounds"),
            ] {
                ui.selectable_value(&mut section, i, label);
            }
        });
        ui.ctx().data_mut(|d| d.insert_temp(id, section));
        ui.add_space(28.0);
        let wide = ui.available_width() > 720.0;
        if section == 3 {
            ui.label("Images are cropped to fit each bar. Low intensity keeps controls readable.");
            ui.add_space(8.0);
            if background_controls(ui, "Top bar", &mut self.top_background) {
                *choose = Some(crate::bar_background::Bar::Top);
            }
            ui.add_space(18.0);
            if background_controls(ui, "Left sidebar", &mut self.side_background) {
                *choose = Some(crate::bar_background::Bar::Side);
            }
        } else if wide {
            ui.columns(2, |columns| {
                self.settings_section(&mut columns[0], section);
                columns[1].add_space(4.0);
                self.preview(&mut columns[1]);
            });
        } else {
            self.settings_section(ui, section);
            ui.add_space(28.0);
            self.preview(ui);
        }
        ui.add_space(32.0);
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(
                    "Accent and backgrounds preview immediately. Apply changes to save.",
                )
                .small()
                .color(ui.visuals().weak_text_color()),
            );
            if crate::widgets::button(ui, "Reset defaults").clicked() {
                *self = Self::default();
            }
        });
        apply
    }
    fn settings_section(&mut self, ui: &mut egui::Ui, section: u8) {
        match section {
            0 => {
                ui.label(egui::RichText::new("Theme").strong());
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut self.light, false, "Dark");
                    ui.selectable_value(&mut self.light, true, "Light");
                });
                ui.add_space(28.0);
                ui.label(egui::RichText::new("Accent color").strong());
                ui.label(
                    egui::RichText::new("Selection, focus, and primary actions.")
                        .small()
                        .color(ui.visuals().weak_text_color()),
                );
                ui.add_space(12.0);
                ui.horizontal_wrapped(|ui| {
                    for (color, label) in [
                        ([154, 154, 154], "Slate"),
                        ([182, 182, 178], "Silver"),
                        ([219, 219, 216], "Pearl"),
                    ] {
                        let (rect, r) =
                            ui.allocate_exact_size(egui::vec2(36.0, 36.0), egui::Sense::click());
                        ui.painter().circle_filled(
                            rect.center(),
                            if self.accent == color { 11.0 } else { 8.0 },
                            egui::Color32::from_rgb(color[0], color[1], color[2]),
                        );
                        r.widget_info(|| {
                            egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label)
                        });
                        if r.on_hover_text(label).clicked() {
                            self.accent = color;
                        }
                    }
                });
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    ui.label("Custom");
                    ui.color_edit_button_srgb(&mut self.accent);
                    ui.label(
                        egui::RichText::new(format!(
                            "#{:02X}{:02X}{:02X}",
                            self.accent[0], self.accent[1], self.accent[2]
                        ))
                        .monospace()
                        .small(),
                    );
                });
            }
            1 => {
                ui.label(egui::RichText::new("Interface").strong());
                ui.label(
                    egui::RichText::new("Navigation, labels, and controls.")
                        .small()
                        .color(ui.visuals().weak_text_color()),
                );
                ui.add_space(8.0);
                ui.add(
                    egui::Slider::new(&mut self.ui_size, 12.0..=28.0)
                        .text("Text size")
                        .suffix(" px"),
                );
                ui.add_space(24.0);
                ui.label(egui::RichText::new("Terminal & code").strong());
                ui.label(
                    egui::RichText::new("A separate size for monospaced text.")
                        .small()
                        .color(ui.visuals().weak_text_color()),
                );
                ui.add_space(8.0);
                ui.add(
                    egui::Slider::new(&mut self.terminal_size, 10.0..=36.0)
                        .text("Text size")
                        .suffix(" px"),
                );
                ui.add_space(24.0);
                egui::CollapsingHeader::new("Custom font files").show(ui, |ui| {
                    ui.label("Interface font");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.ui_font)
                            .hint_text("Bundled font")
                            .desired_width(ui.available_width()),
                    );
                    ui.add_space(8.0);
                    ui.label("Terminal font");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.terminal_font)
                            .hint_text("Bundled monospace")
                            .desired_width(ui.available_width()),
                    );
                    ui.label(
                        egui::RichText::new("Leave blank to use the bundled family.")
                            .small()
                            .color(ui.visuals().weak_text_color()),
                    );
                });
            }
            _ => {
                ui.label(egui::RichText::new("Terminal rendering").strong());
                ui.add_space(12.0);
                ui.checkbox(&mut self.syntax, "Syntax highlighting");
                ui.label(egui::RichText::new("Color the live command line and code editor. Shell-provided colors are preserved.").small().color(ui.visuals().weak_text_color()));
                ui.add_space(28.0);
                ui.label(egui::RichText::new("Terminal text size").strong());
                ui.add_space(8.0);
                ui.add(egui::Slider::new(&mut self.terminal_size, 10.0..=36.0).suffix(" px"));
            }
        }
    }
    fn preview(&self, ui: &mut egui::Ui) {
        let bg = if self.light {
            egui::Color32::from_rgb(250, 250, 250)
        } else {
            egui::Color32::from_rgb(22, 22, 22)
        };
        let fg = if self.light {
            egui::Color32::from_rgb(23, 23, 23)
        } else {
            egui::Color32::from_rgb(245, 245, 245)
        };
        let muted = if self.light {
            egui::Color32::from_rgb(98, 98, 98)
        } else {
            egui::Color32::from_rgb(163, 163, 163)
        };
        ui.label(
            egui::RichText::new("Preview")
                .small()
                .color(ui.visuals().weak_text_color()),
        );
        ui.add_space(12.0);
        egui::Frame::NONE
            .inner_margin(24.0)
            .fill(bg)
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.set_min_height(240.0);
                ui.label(
                    egui::RichText::new("Your workspace")
                        .size(self.ui_size * 1.55)
                        .strong()
                        .color(fg),
                );
                ui.add_space(8.0);
                ui.label(
                    egui::RichText::new("Interface text and secondary details")
                        .size(self.ui_size)
                        .color(muted),
                );
                ui.add_space(32.0);
                ui.label(egui::RichText::new("Shell prompt").small().color(muted));
                ui.add_space(8.0);
                ui.label(
                    egui::RichText::new("~/workspace $")
                        .monospace()
                        .size(self.terminal_size)
                        .color(fg),
                );
                ui.label(crate::highlight::layout_sized(
                    "echo \"Hello, world\"",
                    if self.syntax { "bash" } else { "plain" },
                    self.light,
                    self.terminal_size,
                ));
                ui.add_space(20.0);
                ui.label(
                    egui::RichText::new("Selected action")
                        .size(self.ui_size)
                        .color(crate::theme::accent(self.light, self.accent)),
                );
            });
    }
}
fn background_controls(
    ui: &mut egui::Ui,
    label: &str,
    settings: &mut crate::bar_background::Settings,
) -> bool {
    ui.heading(label);
    let mut choose = false;
    ui.horizontal_wrapped(|ui| {
        choose = crate::widgets::button(ui, "Choose image…").clicked();
        if let Some(path) = settings.path.as_ref() {
            ui.label(
                path.file_name()
                    .unwrap_or(path.as_os_str())
                    .to_string_lossy(),
            )
            .on_hover_text(path.display().to_string());
            if crate::widgets::button(ui, "Remove image").clicked() {
                settings.path = None;
            }
        } else {
            ui.weak("Solid background");
        }
    });
    if settings.path.is_some() {
        ui.add(
            egui::Slider::new(&mut settings.intensity, 0.0..=0.35)
                .text("Image intensity")
                .custom_formatter(|value, _| format!("{:.0}%", value * 100.0)),
        );
    }
    choose
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reject_unreadable_sizes() {
        assert!(Appearance::default().validate().is_ok());
        assert!(Appearance {
            terminal_size: f32::NAN,
            ..Default::default()
        }
        .validate()
        .is_err());
        assert!(Appearance {
            ui_size: 2.0,
            ..Default::default()
        }
        .validate()
        .is_err());
    }
}

#[cfg(test)]
mod review_tests {
    use super::*;
    #[test]
    fn missing_optional_font_still_applies_sizes() {
        let a = Appearance {
            ui_size: 22.0,
            terminal_size: 24.0,
            terminal_font: "__missing_font__.ttf".into(),
            ..Default::default()
        };
        let ctx = egui::Context::default();
        assert!(a.apply(&ctx).is_err());
        assert_eq!(ctx.style().text_styles[&TextStyle::Body].size, 22.0);
        assert_eq!(ctx.style().text_styles[&TextStyle::Monospace].size, 24.0);
    }
}

#[cfg(test)]
mod persistence_review_tests {
    use super::*;
    #[test]
    fn saves_bare_filename_and_preserves_invalid_json() {
        let file = format!("appearance-review-test-{}.json", std::process::id());
        let path = Path::new(&file);
        std::fs::write(path, b"invalid json").unwrap();
        Appearance::default().save(path).unwrap();
        Appearance::load(path).unwrap();
        let stem = path.file_stem().unwrap().to_string_lossy().to_string() + ".invalid-";
        let backup = std::fs::read_dir(".")
            .unwrap()
            .filter_map(Result::ok)
            .map(|e| e.path())
            .find(|p| p.file_name().unwrap().to_string_lossy().starts_with(&stem))
            .unwrap();
        assert_eq!(std::fs::read(&backup).unwrap(), b"invalid json");
        std::fs::remove_file(path).unwrap();
        std::fs::remove_file(backup).unwrap();
    }
}
