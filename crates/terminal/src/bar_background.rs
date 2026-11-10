use eframe::egui;
use serde::{Deserialize, Serialize};
use std::{path::PathBuf, sync::mpsc};

#[derive(Clone, Copy)]
pub enum Bar {
    Top,
    Side,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub path: Option<PathBuf>,
    pub intensity: f32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            path: None,
            intensity: 0.22,
        }
    }
}

#[derive(Default)]
pub struct ImageSlot {
    path: Option<PathBuf>,
    pending: Option<mpsc::Receiver<anyhow::Result<egui::ColorImage>>>,
    texture: Option<egui::TextureHandle>,
}

impl ImageSlot {
    pub fn update(&mut self, ctx: &egui::Context, settings: &Settings) -> Option<String> {
        if self.path != settings.path {
            self.path.clone_from(&settings.path);
            self.texture = None;
            self.pending = self.path.clone().map(|path| {
                let (tx, rx) = mpsc::channel();
                let ctx = ctx.clone();
                std::thread::spawn(move || {
                    let result = crate::file_preview::image_preview(&path, 1400);
                    let _ = tx.send(result);
                    ctx.request_repaint();
                });
                rx
            });
        }
        let rx = self.pending.as_ref()?;
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
                        None
                    }
                    Err(error) => Some(format!("Could not load background image: {error:#}")),
                }
            }
            Err(mpsc::TryRecvError::Empty) => None,
            Err(mpsc::TryRecvError::Disconnected) => {
                self.pending = None;
                Some("Could not load background image".into())
            }
        }
    }

    pub fn paint(
        &self,
        painter: &egui::Painter,
        index: egui::layers::ShapeIdx,
        rect: egui::Rect,
        settings: &Settings,
    ) {
        if let Some(texture) = &self.texture {
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
