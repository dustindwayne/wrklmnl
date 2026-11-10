use eframe::egui;
use serde::{Deserialize, Serialize};
use std::{path::PathBuf, sync::mpsc};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Bar {
    Top,
    Side,
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub path: Option<PathBuf>,
    pub intensity: f32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            path: None,
            intensity: 0.28,
        }
    }
}

impl Settings {
    pub fn choose(&mut self, path: PathBuf) {
        self.path = Some(path);
    }

    pub fn remove(&mut self) {
        self.path = None;
    }
}

#[derive(Default)]
pub struct ImageSlot {
    path: Option<PathBuf>,
    pending: Option<mpsc::Receiver<anyhow::Result<egui::ColorImage>>>,
    texture: Option<egui::TextureHandle>,
    error: Option<String>,
}

impl ImageSlot {
    pub fn update(&mut self, ctx: &egui::Context, settings: &Settings) {
        if self.path != settings.path {
            self.path.clone_from(&settings.path);
            self.texture = None;
            self.error = None;
            self.pending = self.path.clone().map(|path| {
                let (tx, rx) = mpsc::channel();
                let ctx = ctx.clone();
                std::thread::spawn(move || {
                    let _ = tx.send(crate::file_preview::image_preview(&path, 1400));
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
                            egui::TextureOptions::LINEAR,
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

    pub fn ready(&self, settings: &Settings) -> bool {
        settings.path.is_none() || (self.path == settings.path && self.texture.is_some())
    }

    pub fn preview(&self, ui: &mut egui::Ui, bar: Bar, settings: &Settings) {
        let size = match bar {
            Bar::Top => egui::vec2(ui.available_width().min(400.0), 46.0),
            Bar::Side => egui::vec2(84.0, 110.0),
        };
        let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
        let painter = ui.painter();
        painter.rect_filled(rect, 4, ui.visuals().extreme_bg_color);
        let index = painter.add(egui::Shape::Noop);
        self.paint(painter, index, rect.shrink(2.0), settings);
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
        settings: &Settings,
    ) {
        if let Some(texture) = self
            .texture
            .as_ref()
            .filter(|_| settings.path.is_some() && self.ready(settings))
        {
            let uv = cover_uv(texture.size_vec2(), rect.size());
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
    fn legacy_image_settings_keep_custom_paths() -> anyhow::Result<()> {
        let mut settings: Settings =
            serde_json::from_str(r#"{"path":"custom.png","use_builtin":true,"intensity":0.22}"#)?;
        assert_eq!(settings.path, Some("custom.png".into()));
        assert_eq!(settings.intensity, 0.22);
        settings.remove();
        assert!(settings.path.is_none());
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
