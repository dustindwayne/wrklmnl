use eframe::egui::{self, Color32, Stroke, Vec2};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Gradient {
    Velvet,
    Ember,
    Prism,
    Afterglow,
}

impl Gradient {
    pub const ALL: [Self; 4] = [Self::Velvet, Self::Ember, Self::Prism, Self::Afterglow];

    pub fn label(self) -> &'static str {
        match self {
            Self::Velvet => "Velvet",
            Self::Ember => "Ember",
            Self::Prism => "Prism",
            Self::Afterglow => "Afterglow",
        }
    }

    fn colors(self) -> [[u8; 3]; 4] {
        match self {
            Self::Velvet => [[178, 12, 141], [95, 8, 151], [217, 17, 88], [131, 8, 104]],
            Self::Ember => [[232, 64, 9], [187, 12, 34], [108, 7, 68], [218, 95, 8]],
            Self::Prism => [[191, 13, 152], [225, 25, 71], [216, 90, 10], [117, 9, 162]],
            Self::Afterglow => [[224, 32, 98], [164, 12, 112], [104, 8, 132], [218, 80, 10]],
        }
    }

    fn pattern(self, x: f32, y: f32) -> (f32, f32) {
        match self {
            Self::Velvet => {
                let fold = x * 2.7 + (y * 3.8).sin() * 0.28;
                (
                    fold,
                    0.60 + 0.25 * (fold * std::f32::consts::TAU).sin().abs(),
                )
            }
            Self::Ember => {
                let height = 1.0 - y;
                let center = 0.24 + height * 0.36 + (height * 8.0).sin() * 0.07;
                let plume = (x - center).abs() / (0.12 + height * 0.45);
                (plume * 0.55 + y * 0.22, 0.82 - plume.min(1.0) * 0.24)
            }
            Self::Prism => {
                let spectrum = x * 1.25 + y * 0.70;
                (spectrum, 0.70 + (x * 4.0 - y * 3.0).sin().abs() * 0.12)
            }
            Self::Afterglow => {
                let radius = ((x - 0.76).powi(2) + ((y - 0.63) * 1.6).powi(2)).sqrt();
                (radius * 2.6, 0.64 + 0.20 * (radius * 15.0).cos().abs())
            }
        }
    }

    fn sample(self, x: f32, y: f32, light: bool) -> Color32 {
        let (phase, strength) = self.pattern(x, y);
        let t = phase.rem_euclid(1.0) * 4.0;
        let index = t.floor() as usize;
        let blend = t.fract();
        let blend = blend * blend * (3.0 - 2.0 * blend);
        let colors = self.colors();
        let a = colors[index % 4];
        let b = colors[(index + 1) % 4];
        let strength = if light { strength * 0.65 } else { strength };
        let base = if light { 255.0 } else { 0.0 };
        let channel = |i| {
            let value = a[i] as f32 * (1.0 - blend) + b[i] as f32 * blend;
            (base * (1.0 - strength) + value * strength).round() as u8
        };
        Color32::from_rgb(channel(0), channel(1), channel(2))
    }
}

#[derive(Clone)]
struct GradientMesh {
    gradient: Gradient,
    light: bool,
    rect: egui::Rect,
    mesh: Arc<egui::Mesh>,
}

pub fn paint_gradient(ctx: &egui::Context, gradient: Option<Gradient>, light: bool) {
    let Some(gradient) = gradient else { return };
    let rect = ctx.screen_rect();
    let key = egui::Id::new(("workspace_gradient", ctx.viewport_id()));
    let cached = ctx.data_mut(|data| data.get_temp::<GradientMesh>(key));
    let mesh = match cached {
        Some(cached)
            if cached.gradient == gradient && cached.light == light && cached.rect == rect =>
        {
            cached.mesh
        }
        _ => {
            let (columns, rows) = if gradient == Gradient::Prism {
                (12, 8)
            } else {
                (64, 40)
            };
            let mut mesh = egui::Mesh::default();
            for row in 0..rows {
                for column in 0..columns {
                    let left = column as f32 / columns as f32;
                    let right = (column + 1) as f32 / columns as f32;
                    let top = row as f32 / rows as f32;
                    let bottom = (row + 1) as f32 / rows as f32;
                    for triangle in [
                        [[left, top], [right, top], [left, bottom]],
                        [[right, top], [right, bottom], [left, bottom]],
                    ] {
                        let center_x = triangle.iter().map(|point| point[0]).sum::<f32>() / 3.0;
                        let center_y = triangle.iter().map(|point| point[1]).sum::<f32>() / 3.0;
                        let start = mesh.vertices.len() as u32;
                        for [x, y] in triangle {
                            let color = if gradient == Gradient::Prism {
                                gradient.sample(center_x, center_y, light)
                            } else {
                                gradient.sample(x, y, light)
                            };
                            mesh.colored_vertex(
                                rect.min + egui::vec2(x * rect.width(), y * rect.height()),
                                color,
                            );
                        }
                        mesh.add_triangle(start, start + 1, start + 2);
                    }
                }
            }
            let mesh = Arc::new(mesh);
            ctx.data_mut(|data| {
                data.insert_temp(
                    key,
                    GradientMesh {
                        gradient,
                        light,
                        rect,
                        mesh: mesh.clone(),
                    },
                )
            });
            mesh
        }
    };
    ctx.layer_painter(egui::LayerId::background())
        .add(egui::Shape::Mesh(mesh));
}
pub fn accent(light: bool, raw: [u8; 3]) -> Color32 {
    let mut rgb = raw;
    let luminance = |rgb: [u8; 3]| {
        let linear = |v: u8| {
            let f = v as f64 / 255.0;
            if f <= 0.04045 {
                f / 12.92
            } else {
                ((f + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * linear(rgb[0]) + 0.7152 * linear(rgb[1]) + 0.0722 * linear(rgb[2])
    };
    for _ in 0..40 {
        let l = luminance(rgb);
        let bg = luminance(if light { [250, 250, 250] } else { [17, 17, 17] });
        if (l.max(bg) + 0.05) / (l.min(bg) + 0.05) >= 4.5 {
            break;
        }
        for v in &mut rgb {
            *v = if light {
                (*v as f32 * 0.85) as u8
            } else {
                (*v as f32 + (255 - *v) as f32 * 0.15 + 1.0).min(255.0) as u8
            };
        }
    }
    Color32::from_rgb(rgb[0], rgb[1], rgb[2])
}
pub fn apply(ctx: &egui::Context, light: bool, accent: [u8; 3]) {
    ctx.set_theme(if light {
        egui::Theme::Light
    } else {
        egui::Theme::Dark
    });
    ctx.style_mut(|style| {
        style.visuals = if light {
            egui::Visuals::light()
        } else {
            egui::Visuals::dark()
        };
        style.interaction.selectable_labels = false;
        style.spacing.item_spacing = Vec2::new(12.0, 12.0);
        style.spacing.button_padding = Vec2::new(12.0, 8.0);
        style.visuals.panel_fill = if light {
            Color32::from_rgb(250, 250, 250)
        } else {
            Color32::from_rgb(17, 17, 17)
        };
        style.visuals.window_fill = if light {
            Color32::WHITE
        } else {
            Color32::from_rgb(34, 34, 34)
        };
        style.visuals.button_frame = false;
        style.visuals.window_stroke = Stroke::NONE;
        style.visuals.window_corner_radius = egui::CornerRadius::ZERO;
        style.visuals.menu_corner_radius = egui::CornerRadius::ZERO;
        style.visuals.window_shadow = egui::epaint::Shadow::NONE;
        style.visuals.popup_shadow = egui::epaint::Shadow::NONE;
        style.visuals.extreme_bg_color = if light {
            Color32::from_rgb(241, 241, 241)
        } else {
            Color32::from_rgb(26, 26, 26)
        };
        style.visuals.override_text_color = Some(if light {
            Color32::from_rgb(23, 23, 23)
        } else {
            Color32::from_rgb(245, 245, 245)
        });
        for widget in [
            &mut style.visuals.widgets.noninteractive,
            &mut style.visuals.widgets.inactive,
            &mut style.visuals.widgets.hovered,
            &mut style.visuals.widgets.active,
            &mut style.visuals.widgets.open,
        ] {
            widget.bg_stroke = Stroke::NONE;
            widget.corner_radius = egui::CornerRadius::ZERO;
        }
        style.visuals.widgets.noninteractive.fg_stroke.color = if light {
            Color32::from_rgb(23, 23, 23)
        } else {
            Color32::from_rgb(245, 245, 245)
        };
        style.visuals.widgets.inactive.fg_stroke.color =
            style.visuals.widgets.noninteractive.fg_stroke.color;
        style.visuals.widgets.hovered.fg_stroke.color =
            style.visuals.widgets.inactive.fg_stroke.color;
        set_accent(&mut style.visuals, light, accent);
    });
}

pub fn apply_accent(ctx: &egui::Context, light: bool, accent: [u8; 3]) {
    ctx.style_mut(|style| set_accent(&mut style.visuals, light, accent));
}

fn set_accent(visuals: &mut egui::Visuals, light: bool, accent: [u8; 3]) {
    visuals.hyperlink_color = self::accent(light, accent);
    let base = if light { 250.0 } else { 17.0 };
    let color = visuals.hyperlink_color;
    let blend = |v: u8| (base * 0.78 + v as f32 * 0.22) as u8;
    visuals.selection.bg_fill =
        Color32::from_rgb(blend(color.r()), blend(color.g()), blend(color.b()));
    visuals.selection.stroke = Stroke::new(1.0_f32, color);
    visuals.widgets.active.fg_stroke.color = color;
    visuals.widgets.active.weak_bg_fill = visuals.selection.bg_fill;
    visuals.widgets.open.fg_stroke.color = color;
    visuals.widgets.open.weak_bg_fill = visuals.selection.bg_fill;
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn themes_have_distinct_patterns_independent_of_color() {
        for (index, first) in Gradient::ALL.iter().enumerate() {
            for second in &Gradient::ALL[index + 1..] {
                let differences = (0..10)
                    .filter(|sample| {
                        let x = *sample as f32 / 10.0;
                        let y = 1.0 - x;
                        first.pattern(x, y) != second.pattern(x, y)
                    })
                    .count();
                assert!(differences >= 8, "{first:?} and {second:?} share a pattern");
            }
        }
    }
    #[test]
    fn gradients_keep_the_background_readable_in_both_modes() {
        for gradient in Gradient::ALL {
            for row in 0..=30 {
                for column in 0..=40 {
                    let x = column as f32 / 40.0;
                    let y = row as f32 / 30.0;
                    let dark = gradient.sample(x, y, false);
                    let light = gradient.sample(x, y, true);
                    let luminance = |color: Color32| {
                        let linear = |value: u8| {
                            let value = value as f32 / 255.0;
                            if value <= 0.04045 {
                                value / 12.92
                            } else {
                                ((value + 0.055) / 1.055).powf(2.4)
                            }
                        };
                        linear(color.r()) * 0.2126
                            + linear(color.g()) * 0.7152
                            + linear(color.b()) * 0.0722
                    };
                    let contrast = |a: Color32, b: Color32| {
                        let a = luminance(a);
                        let b = luminance(b);
                        (a.max(b) + 0.05) / (a.min(b) + 0.05)
                    };
                    assert!(
                        contrast(dark, Color32::from_gray(245)) >= 4.5,
                        "{gradient:?} at {x}, {y}: {dark:?}, contrast {}",
                        contrast(dark, Color32::from_gray(245))
                    );
                    assert!(
                        contrast(light, Color32::from_gray(23)) >= 4.5,
                        "{gradient:?} at {x}, {y}"
                    );
                }
            }
        }
    }

    #[test]
    fn accent_updates_common_focus_and_selection_colors() {
        let ctx = egui::Context::default();
        let raw = [220, 150, 100];
        for light in [false, true] {
            apply(&ctx, light, raw);
            let style = ctx.style();
            assert!(!style.interaction.selectable_labels);
            let selected = accent(light, raw);
            assert_eq!(style.visuals.hyperlink_color, selected);
            assert_eq!(style.visuals.selection.stroke.color, selected);
            assert_eq!(style.visuals.widgets.active.fg_stroke.color, selected);
            assert_eq!(
                style.visuals.widgets.open.weak_bg_fill,
                style.visuals.selection.bg_fill
            );
        }
    }
}
