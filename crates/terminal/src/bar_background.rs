use eframe::egui;
use serde::{Deserialize, Serialize};
use std::{path::PathBuf, sync::mpsc};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Bar {
    Top,
    Side,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Preset {
    #[default]
    Basic,
    PixelTech,
}

impl Preset {
    pub fn label(self) -> &'static str {
        match self {
            Self::Basic => "Basic",
            Self::PixelTech => "pixel_tech",
        }
    }
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub path: Option<PathBuf>,
    pub use_builtin: bool,
    pub intensity: f32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            path: None,
            use_builtin: true,
            intensity: 0.28,
        }
    }
}

impl Settings {
    fn uses_builtin(&self, preset: Preset) -> bool {
        self.path.is_none() && self.use_builtin && preset == Preset::PixelTech
    }

    pub fn has_image(&self, preset: Preset) -> bool {
        self.path.is_some() || self.uses_builtin(preset)
    }

    pub fn choose(&mut self, path: PathBuf) {
        self.path = Some(path);
        self.use_builtin = false;
    }

    pub fn remove(&mut self) {
        self.path = None;
        self.use_builtin = false;
    }
}

impl Bar {
    fn default_art(self) -> &'static [u8] {
        match self {
            Self::Top => include_bytes!("../assets/future-tech-top.png"),
            Self::Side => include_bytes!("../assets/future-tech-sidebar.png"),
        }
    }

    fn load_default(self) -> anyhow::Result<egui::ColorImage> {
        let image = image::load_from_memory(self.default_art())?.into_rgba8();
        Ok(egui::ColorImage::from_rgba_unmultiplied(
            [image.width() as usize, image.height() as usize],
            image.as_raw(),
        ))
    }
}

#[derive(Default)]
pub struct ImageSlot {
    path: Option<PathBuf>,
    builtin: Option<Bar>,
    pending: Option<mpsc::Receiver<anyhow::Result<egui::ColorImage>>>,
    texture: Option<egui::TextureHandle>,
    error: Option<String>,
}

impl ImageSlot {
    pub fn update(&mut self, ctx: &egui::Context, bar: Bar, preset: Preset, settings: &Settings) {
        let builtin = settings.uses_builtin(preset).then_some(bar);
        if self.path != settings.path || self.builtin != builtin {
            self.path.clone_from(&settings.path);
            self.builtin = builtin;
            self.texture = None;
            self.error = None;
            self.pending = settings.has_image(preset).then(|| {
                let path = self.path.clone();
                let (tx, rx) = mpsc::channel();
                let ctx = ctx.clone();
                std::thread::spawn(move || {
                    let result = match path {
                        Some(path) => crate::file_preview::image_preview(&path, 1400),
                        None => bar.load_default(),
                    };
                    let _ = tx.send(result);
                    ctx.request_repaint();
                });
                rx
            });
        }
        let Some(rx) = &self.pending else {
            return;
        };
        match rx.try_recv() {
            Ok(result) => {
                self.pending = None;
                match result {
                    Ok(image) => {
                        self.texture = Some(ctx.load_texture(
                            "bar_background",
                            image,
                            if self.builtin.is_some() {
                                egui::TextureOptions::NEAREST
                            } else {
                                egui::TextureOptions::LINEAR
                            },
                        ));
                    }
                    Err(error) => {
                        self.error = Some(format!("Could not load background image: {error:#}"));
                    }
                }
            }
            Err(mpsc::TryRecvError::Empty) => {}
            Err(mpsc::TryRecvError::Disconnected) => {
                self.pending = None;
                self.error = Some("Could not load background image".into());
            }
        }
    }

    pub fn reload(&mut self) {
        *self = Self::default();
    }

    pub fn ready(&self, preset: Preset, settings: &Settings) -> bool {
        !settings.has_image(preset)
            || (self.path == settings.path
                && self.builtin.is_some() == settings.uses_builtin(preset)
                && self.texture.is_some())
    }

    pub fn preview(&self, ui: &mut egui::Ui, bar: Bar, preset: Preset, settings: &Settings) {
        let size = match bar {
            Bar::Top => egui::vec2(ui.available_width().min(400.0), 46.0),
            Bar::Side => egui::vec2(84.0, 110.0),
        };
        let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
        let painter = ui.painter();
        painter.rect_filled(rect, 4, ui.visuals().extreme_bg_color);
        let index = painter.add(egui::Shape::Noop);
        self.paint(painter, index, rect.shrink(2.0), preset, settings);
        painter.rect_stroke(
            rect,
            4,
            egui::Stroke::new(1.0_f32, ui.visuals().weak_text_color().gamma_multiply(0.35)),
            egui::StrokeKind::Inside,
        );
    }

    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    pub fn paint(
        &self,
        painter: &egui::Painter,
        index: egui::layers::ShapeIdx,
        rect: egui::Rect,
        preset: Preset,
        settings: &Settings,
    ) {
        if let Some(texture) = self
            .texture
            .as_ref()
            .filter(|_| settings.has_image(preset) && self.ready(preset, settings))
        {
            let centered = cover_uv(texture.size_vec2(), rect.size());
            let uv = if let Some(bar) = self.builtin {
                let center = match bar {
                    Bar::Top => egui::pos2(0.5, 0.32),
                    Bar::Side => egui::pos2(0.60, 0.5),
                };
                let half = centered.size() * 0.5;
                let center = center.clamp(egui::Pos2::ZERO + half, egui::pos2(1.0, 1.0) - half);
                egui::Rect::from_center_size(center, centered.size())
            } else {
                centered
            };
            let opacity = (settings.intensity.clamp(0.0, 0.35) * 255.0).round() as u8;
            painter.set(
                index,
                egui::Shape::image(
                    texture.id(),
                    rect,
                    uv,
                    egui::Color32::from_white_alpha(opacity),
                ),
            );
        }
    }
}

fn cover_uv(image: egui::Vec2, target: egui::Vec2) -> egui::Rect {
    let scale = (target.x / image.x)
        .max(target.y / image.y)
        .max(f32::EPSILON);
    let visible = (target / scale / image).min(egui::Vec2::splat(1.0));
    egui::Rect::from_center_size(egui::pos2(0.5, 0.5), visible)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_and_legacy_paths_keep_their_sources() -> anyhow::Result<()> {
        let mut settings: Settings = serde_json::from_str(r#"{"path":null,"intensity":0.22}"#)?;
        assert!(!settings.has_image(Preset::Basic));
        assert!(settings.uses_builtin(Preset::PixelTech));
        settings.choose("custom.png".into());
        assert!(settings.has_image(Preset::Basic));
        assert!(!settings.uses_builtin(Preset::PixelTech));
        settings.remove();
        let saved: Settings = serde_json::from_str(&serde_json::to_string(&settings)?)?;
        assert!(!saved.has_image(Preset::PixelTech));
        let top = Bar::Top.load_default()?;
        let side = Bar::Side.load_default()?;
        assert!(top.width() > top.height());
        assert!(side.height() > side.width());
        Ok(())
    }

    #[test]
    fn images_cover_both_bar_shapes_without_stretching() {
        let image = egui::vec2(100.0, 100.0);
        let top = cover_uv(image, egui::vec2(400.0, 40.0));
        let side = cover_uv(image, egui::vec2(40.0, 400.0));
        assert_eq!(top.width(), 1.0);
        assert_eq!(side.height(), 1.0);
        assert!((top.height() - 0.1).abs() < 0.001);
        assert!((side.width() - 0.1).abs() < 0.001);
        assert_eq!(top.center(), egui::pos2(0.5, 0.5));
    }
}
