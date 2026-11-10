//! Desktop adapter for the same allocation-free program/rasterizer used by embedded targets.
use ab_glyph::{Font, FontArc, ScaleFont};
use eframe::egui::{self, Color32};
use std::hash::{Hash, Hasher};
use terminal_ui_graphics::{
    paint::{Paint, ReadTarget},
    program::{self, TextRenderer},
    Color, Target,
};
struct Pixels {
    width: u16,
    height: u16,
    data: Vec<Color>,
}
impl Target for Pixels {
    fn size(&self) -> (u16, u16) {
        (self.width, self.height)
    }
    fn pixel(&mut self, x: u16, y: u16, c: Color) {
        if x < self.width && y < self.height {
            self.data[usize::from(y) * usize::from(self.width) + usize::from(x)] = c;
        }
    }
    fn span(&mut self, x: u16, y: u16, length: u16, c: Color) {
        if x >= self.width || y >= self.height {
            return;
        }
        let start = usize::from(y) * usize::from(self.width) + usize::from(x);
        self.data[start..start + usize::from(length.min(self.width - x))].fill(c);
    }
}
impl ReadTarget for Pixels {
    fn read(&self, x: u16, y: u16) -> Color {
        if x < self.width && y < self.height {
            self.data[usize::from(y) * usize::from(self.width) + usize::from(x)]
        } else {
            Color(0, 0, 0)
        }
    }
}
struct DesktopText(FontArc);
impl TextRenderer<Pixels> for DesktopText {
    fn text(
        &mut self,
        target: &mut Pixels,
        position: (i32, i32),
        size: u16,
        text: &str,
        paint: Paint,
    ) {
        let font = self.0.as_scaled(size as f32);
        let mut x = position.0 as f32;
        let mut baseline = position.1 as f32 + font.ascent();
        let mut previous = None;
        for ch in terminal_ui_graphics::text::characters(text) {
            if ch == '\n' {
                x = position.0 as f32;
                baseline += font.height() + font.line_gap();
                previous = None;
                continue;
            }
            if ch == '\t' {
                x += font.h_advance(font.glyph_id(' ')) * 4.0;
                continue;
            }
            let id = font.glyph_id(ch);
            if x + font.h_advance(id) > f32::from(target.width) && x > position.0 as f32 {
                x = position.0 as f32;
                baseline += font.height() + font.line_gap();
                previous = None;
            }
            if baseline - font.ascent() >= f32::from(target.height) {
                break;
            }
            if let Some(before) = previous {
                x += font.kern(before, id);
            }
            previous = Some(id);
            let glyph = id.with_scale_and_position(size as f32, ab_glyph::point(x, baseline));
            if let Some(outline) = self.0.outline_glyph(glyph) {
                let bounds = outline.px_bounds();
                outline.draw(|gx, gy, coverage| {
                    let px = bounds.min.x as i32 + gx as i32;
                    let py = bounds.min.y as i32 + gy as i32;
                    if px >= 0
                        && py >= 0
                        && px < i32::from(target.width)
                        && py < i32::from(target.height)
                    {
                        target.blend(
                            px as u16,
                            py as u16,
                            Paint {
                                opacity: (f32::from(paint.opacity) * coverage).round() as u8,
                                ..paint
                            },
                        );
                    }
                });
            }
            x += font.h_advance(id);
        }
    }
}
#[derive(Clone)]
struct Cache {
    hash: u64,
    texture: egui::TextureHandle,
}
pub fn set_font(ctx: &egui::Context, font: FontArc) {
    ctx.data_mut(|d| {
        let revision = d
            .get_temp::<u64>(egui::Id::new("pixel_font_revision"))
            .unwrap_or(0)
            + 1;
        d.insert_temp(egui::Id::new("pixel_font_revision"), revision);
        d.insert_temp(egui::Id::new("pixel_surface_font"), font);
    });
}
pub fn show(ui: &mut egui::Ui, source: &str, height: u16) -> Option<egui::Response> {
    let width = ui.available_width().floor().clamp(1.0, 4096.0) as u16;
    let density = ui.ctx().pixels_per_point().clamp(1.0, 4.0);
    let pixel_width = (f32::from(width) * density).round() as u16;
    let pixel_height = (f32::from(height) * density).round() as u16;
    let count = usize::from(pixel_width) * usize::from(pixel_height);
    if count > 4 * 1024 * 1024 {
        ui.colored_label(
            ui.visuals().error_fg_color,
            "Graphics surface exceeds its desktop pixel budget",
        );
        return None;
    }
    let bg = ui.visuals().panel_fill;
    let fg = ui.visuals().text_color();
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    source.hash(&mut hasher);
    width.hash(&mut hasher);
    height.hash(&mut hasher);
    density.to_bits().hash(&mut hasher);
    bg.hash(&mut hasher);
    fg.hash(&mut hasher);
    ui.ctx()
        .data_mut(|d| d.get_temp::<u64>(egui::Id::new("pixel_font_revision")))
        .hash(&mut hasher);
    let hash = hasher.finish();
    let id = ui.id().with("pixel_surface_cache");
    let old = ui.ctx().data_mut(|data| data.get_temp::<Cache>(id));
    let cache = if let Some(cache) = old.filter(|c| c.hash == hash) {
        cache
    } else {
        let font_id = egui::Id::new("pixel_surface_font");
        let font = ui
            .ctx()
            .data_mut(|d| d.get_temp::<FontArc>(font_id))
            .or_else(|| {
                let definitions = egui::FontDefinitions::default();
                let name = definitions
                    .families
                    .get(&egui::FontFamily::Proportional)?
                    .first()?;
                let bytes = &definitions.font_data.get(name)?.font;
                FontArc::try_from_vec(bytes.to_vec()).ok()
            });
        let Some(font) = font else {
            ui.label("Graphics font could not load");
            return None;
        };
        ui.ctx().data_mut(|d| d.insert_temp(font_id, font.clone()));
        let mut target = Pixels {
            width: pixel_width,
            height: pixel_height,
            data: vec![Color(bg.r(), bg.g(), bg.b()); count],
        };
        if let Err(e) = program::draw_in_viewport(
            source,
            &mut target,
            &mut DesktopText(font),
            Paint::solid(Color(fg.r(), fg.g(), fg.b())),
            (width, height),
        ) {
            ui.colored_label(ui.visuals().error_fg_color, e.to_string());
            return None;
        }
        let image = egui::ColorImage {
            size: [usize::from(pixel_width), usize::from(pixel_height)],
            pixels: target
                .data
                .iter()
                .map(|c| Color32::from_rgb(c.0, c.1, c.2))
                .collect(),
        };
        let cache = Cache {
            hash,
            texture: ui.ctx().load_texture(
                format!("graphics-{id:?}"),
                image,
                egui::TextureOptions::LINEAR,
            ),
        };
        ui.ctx().data_mut(|d| d.insert_temp(id, cache.clone()));
        cache
    };
    Some(
        ui.add(
            egui::Image::new(&cache.texture)
                .fit_to_exact_size(egui::vec2(f32::from(width), f32::from(height))),
        ),
    )
}
