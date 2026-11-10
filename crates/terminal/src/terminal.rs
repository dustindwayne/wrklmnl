use crate::session::Session;
use eframe::egui::{self, Color32, FontId, Rect, Sense, Vec2};

pub fn draw(
    ui: &mut egui::Ui,
    session: &mut Session,
    light: bool,
    font_size: f32,
) -> anyhow::Result<()> {
    if let Some(view) = session.ui_view.clone() {
        session.ui_surface.light = light;
        session.ui_surface.accent_color = ui.visuals().hyperlink_color;
        session.ui_surface.size = font_size;
        session.ui_surface.ui_size = ui.text_style_height(&egui::TextStyle::Body);
        session.ui_surface.syntax = session.interaction.line.syntax;
        ui.horizontal(|ui| {
            ui.heading(&view.title);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if crate::widgets::button(ui, "Close UI").clicked() {
                    session.ui_surface.events.push(terminal_ui_kit::Action {
                        version: 1,
                        view: view.id.clone(),
                        action: "_terminal.close".into(),
                        values: session.ui_surface.values.clone(),
                    });
                }
            });
        });
        ui.add_space(16.0);
        egui::ScrollArea::both()
            .id_salt("inline_graphics")
            .show(ui, |ui| {
                session
                    .ui_surface
                    .render_elements(ui, &view, &view.elements)
            });
        let events = std::mem::take(&mut session.ui_surface.events);
        for action in events {
            let mut bytes = serde_json::to_vec(&action)?;
            bytes.push(b'\r');
            session.send(&bytes)?;
        }
        return Ok(());
    }
    let font = FontId::monospace(font_size);
    let cell = Vec2::new(ui.fonts(|f| f.glyph_width(&font, 'M')), font_size * 1.4);
    let size = ui.available_size().max(Vec2::new(cell.x, cell.y));
    let rows = ((size.y / cell.y) as u16).clamp(1, 1000);
    let cols = ((size.x / cell.x) as u16).clamp(1, 2000);
    session.resize(rows, cols)?;
    let (rect, response) = ui.allocate_exact_size(size, Sense::click_and_drag());
    if response.clicked()
        || response.drag_started()
        || response.drag_stopped()
        || session.interaction.focus_requested
    {
        session.interaction.focus_requested = false;
        response.request_focus();
    }
    if response.gained_focus() {
        ui.ctx().request_repaint();
    }
    lock_focus(ui, response.id);
    crate::terminal_state::pointer(ui, session, rect, cell, &response)?;
    let normal = if light {
        Color32::from_rgb(23, 23, 23)
    } else {
        Color32::from_rgb(245, 245, 245)
    };
    let bg = if light {
        Color32::from_rgb(250, 250, 250)
    } else {
        Color32::from_rgb(17, 17, 17)
    };
    let painter = ui.painter().with_clip_rect(rect);
    let syntax = crate::terminal_command::colors(session, light, font_size);
    let screen = session.parser.screen();
    for row in 0..rows {
        for col in 0..cols {
            let Some(c) = screen.cell(row, col) else {
                continue;
            };
            if c.is_wide_continuation() {
                continue;
            }
            let mut fg = color(c.fgcolor(), normal, light, true);
            if matches!(c.fgcolor(), vt100::Color::Default) {
                if let Some(color) = syntax.get(&(row, col)) {
                    fg = *color;
                }
            }
            let mut back = color(c.bgcolor(), bg, light, false);
            if c.inverse() {
                std::mem::swap(&mut fg, &mut back);
            }
            if c.dim() {
                fg = fg.gamma_multiply(0.65);
            }
            let pos = rect.min + Vec2::new(col as f32 * cell.x, row as f32 * cell.y);
            let width = cell.x * if c.is_wide() { 2.0 } else { 1.0 };
            if session.interaction.contains(row, col) {
                back = ui.visuals().selection.bg_fill;
            }
            if back != bg {
                painter.rect_filled(
                    Rect::from_min_size(pos, Vec2::new(width, cell.y)),
                    0.0,
                    back,
                );
            }
            if c.has_contents() {
                let format = egui::TextFormat {
                    font_id: font.clone(),
                    color: fg,
                    italics: c.italic(),
                    ..Default::default()
                };
                let mut job = egui::text::LayoutJob::default();
                job.append(c.contents(), 0.0, format);
                let galley = ui.fonts(|f| f.layout_job(job));
                painter.galley(pos, galley.clone(), fg);
                if c.bold() {
                    painter.galley(pos + Vec2::new(0.5, 0.0), galley, fg);
                }
            }
            if c.underline() {
                painter.line_segment(
                    [
                        pos + Vec2::new(0.0, cell.y - 2.0),
                        pos + Vec2::new(width, cell.y - 2.0),
                    ],
                    (1.0, fg),
                );
            }
        }
    }
    if (response.has_focus() || session.interaction.active)
        && !screen.hide_cursor()
        && screen.scrollback() == 0
    {
        let (r, c) = screen.cursor_position();
        let p = rect.min + Vec2::new(c as f32 * cell.x, r as f32 * cell.y);
        painter.rect_filled(
            Rect::from_min_size(
                p,
                Vec2::new(if response.has_focus() { 2.0 } else { 1.0 }, cell.y),
            ),
            0.0,
            if response.has_focus() {
                normal
            } else {
                normal.gamma_multiply(0.5)
            },
        );
    }
    if ui.input(|i| {
        i.pointer.any_pressed() && i.pointer.interact_pos().is_some_and(|p| !rect.contains(p))
    }) {
        session.interaction.completion.dismiss();
    }
    if response.has_focus() || !session.interaction.completion.items.is_empty() {
        let raw_events = if response.has_focus() {
            ui.input(|i| i.events.clone())
        } else {
            vec![]
        };
        let events = crate::terminal_command::process(ui, session, rect, cell, &raw_events)?;
        for input in crate::terminal_input::translate(
            &events,
            ui.input(|i| i.modifiers),
            session.interaction.selection.is_some(),
            session.parser.screen().application_cursor(),
            session.parser.screen().bracketed_paste(),
        ) {
            match input {
                crate::terminal_input::Input::Copy => {
                    ui.ctx().copy_text(session.copy_text());
                    session.interaction.selection = None;
                }
                crate::terminal_input::Input::Scroll(pages) => {
                    let current = session.parser.screen().scrollback();
                    let rows = session.parser.screen().size().0 as isize;
                    session
                        .parser
                        .screen_mut()
                        .set_scrollback((current as isize + pages * rows).max(0) as usize);
                    session.interaction.selection = None;
                }
                crate::terminal_input::Input::Bytes(bytes) => {
                    session.parser.screen_mut().set_scrollback(0);
                    session.interaction.selection = None;
                    session.send(&bytes)?;
                }
            }
        }
    }
    Ok(())
}
fn color(c: vt100::Color, default: Color32, light: bool, foreground: bool) -> Color32 {
    match c {
        vt100::Color::Default => default,
        vt100::Color::Rgb(r, g, b) => Color32::from_rgb(r, g, b),
        vt100::Color::Idx(i) => {
            const BASIC: [[u8; 3]; 16] = [
                [20, 20, 20],
                [183, 91, 91],
                [120, 169, 122],
                [190, 163, 109],
                [122, 157, 193],
                [185, 112, 158],
                [116, 173, 176],
                [205, 205, 205],
                [115, 115, 115],
                [222, 130, 130],
                [159, 202, 160],
                [218, 193, 142],
                [160, 190, 222],
                [216, 155, 193],
                [157, 205, 207],
                [245, 245, 245],
            ];
            let [r, g, b] = if i < 16 {
                if light && foreground {
                    [
                        [23, 23, 23],
                        [145, 45, 45],
                        [32, 101, 45],
                        [112, 82, 20],
                        [36, 75, 145],
                        [128, 45, 104],
                        [26, 96, 101],
                        [82, 82, 82],
                        [98, 98, 98],
                        [159, 48, 48],
                        [40, 105, 55],
                        [119, 85, 24],
                        [45, 85, 155],
                        [138, 50, 114],
                        [28, 100, 108],
                        [60, 60, 60],
                    ][i as usize]
                } else if !light && foreground && i == 0 {
                    [132, 132, 132]
                } else {
                    BASIC[i as usize]
                }
            } else if i < 232 {
                let n = i - 16;
                let step = |v: u8| if v == 0 { 0 } else { 55 + 40 * v };
                [step(n / 36), step(n / 6 % 6), step(n % 6)]
            } else {
                let v = 8 + 10 * (i - 232);
                [v, v, v]
            };
            Color32::from_rgb(r, g, b)
        }
    }
}
fn lock_focus(ui: &egui::Ui, id: egui::Id) {
    if ui.memory(|m| m.has_focus(id)) {
        ui.memory_mut(|m| {
            m.set_focus_lock_filter(
                id,
                egui::EventFilter {
                    tab: true,
                    horizontal_arrows: true,
                    vertical_arrows: true,
                    escape: true,
                },
            )
        });
    }
}
#[cfg(test)]
mod review_tests {
    use super::*;
    #[test]
    fn canvas_keeps_navigation_focus() {
        let ctx = egui::Context::default();
        let id = egui::Id::new("terminal_test_canvas");
        for frame in 0..2 {
            let _ = ctx.run(Default::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    ui.interact(ui.max_rect(), id, Sense::click());
                    if frame == 0 {
                        ui.memory_mut(|m| m.request_focus(id));
                    }
                    lock_focus(ui, id);
                    let _ = ui.button("Outside");
                });
            });
        }
        for key in [
            egui::Key::Tab,
            egui::Key::ArrowUp,
            egui::Key::ArrowLeft,
            egui::Key::Escape,
        ] {
            let mut input = egui::RawInput::default();
            input.events.push(egui::Event::Key {
                key,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            });
            let _ = ctx.run(input, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    ui.interact(ui.max_rect(), id, Sense::click());
                    lock_focus(ui, id);
                    let _ = ui.button("Outside");
                    assert!(ui.memory(|m| m.has_focus(id)), "Lost focus on {key:?}");
                });
            });
        }
    }
    #[test]
    fn light_palette_is_readable() {
        for i in 0..16 {
            let c = color(vt100::Color::Idx(i), Color32::BLACK, true, true);
            let linear = |v: u8| {
                let f = v as f64 / 255.0;
                if f <= 0.04045 {
                    f / 12.92
                } else {
                    ((f + 0.055) / 1.055).powf(2.4)
                }
            };
            let luminance =
                0.2126 * linear(c.r()) + 0.7152 * linear(c.g()) + 0.0722 * linear(c.b());
            assert!(
                (0.955973 + 0.05) / (luminance + 0.05) >= 4.5,
                "Unreadable ANSI color {i}"
            );
        }
    }
}
