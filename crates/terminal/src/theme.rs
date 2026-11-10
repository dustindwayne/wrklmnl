use eframe::egui::{self, Color32, Stroke, Vec2};
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
    fn accent_updates_common_focus_and_selection_colors() {
        let ctx = egui::Context::default();
        let raw = [220, 150, 100];
        for light in [false, true] {
            apply(&ctx, light, raw);
            let style = ctx.style();
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
