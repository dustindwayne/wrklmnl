use crate::icons::{self, Icon};
use eframe::egui::{self, Color32, Rect, Sense, Vec2};

const BAR_MARGIN: egui::Margin = egui::Margin::symmetric(16, 6);
const ROW_HEIGHT: f32 = 32.0;

pub fn workspace_fill(light: bool) -> Color32 {
    Color32::from_gray(if light { 250 } else { 17 })
}

pub fn sidebar_fill(light: bool) -> Color32 {
    Color32::from_gray(if light { 243 } else { 22 })
}

pub fn frame(light: bool) -> egui::Frame {
    egui::Frame::NONE
        .fill(workspace_fill(light))
        .inner_margin(BAR_MARGIN)
}

pub fn row<T>(
    ui: &mut egui::Ui,
    contents: impl FnOnce(&mut egui::Ui) -> T,
) -> egui::InnerResponse<T> {
    ui.allocate_ui_with_layout(
        Vec2::new(ui.available_width(), ROW_HEIGHT),
        egui::Layout::right_to_left(egui::Align::Center),
        contents,
    )
}

pub fn bounds(row: Rect) -> Rect {
    row + BAR_MARGIN
}

pub fn divider(ui: &egui::Ui, bounds: Rect, sidebar_width: Option<f32>) {
    let y = bounds.bottom() - 0.5;
    let start = bounds.left() + sidebar_width.unwrap_or(0.0);
    let color = Color32::from_gray(if ui.visuals().dark_mode { 36 } else { 224 });
    ui.painter().line_segment(
        [egui::pos2(start, y), egui::pos2(bounds.right(), y)],
        egui::Stroke::new(1.0_f32, color),
    );
}

pub fn title(ui: &mut egui::Ui, title: &str) {
    let (rect, response) = ui.allocate_exact_size(
        Vec2::new(ui.available_width().max(0.0), ROW_HEIGHT),
        Sense::click_and_drag(),
    );
    ui.painter().with_clip_rect(rect).text(
        rect.left_center(),
        egui::Align2::LEFT_CENTER,
        title,
        egui::TextStyle::Body.resolve(ui.style()),
        ui.visuals().text_color(),
    );
    if response.drag_started() {
        ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
    }
    if response.double_clicked() {
        maximize(ui.ctx());
    }
}

fn maximize(ctx: &egui::Context) {
    let (fullscreen, maximized) = ctx.input(|i| {
        (
            i.viewport().fullscreen.unwrap_or(false),
            i.viewport().maximized.unwrap_or(false),
        )
    });
    ctx.send_viewport_cmd(if fullscreen {
        egui::ViewportCommand::Fullscreen(false)
    } else {
        egui::ViewportCommand::Maximized(!maximized)
    });
}

pub fn controls(ui: &mut egui::Ui) -> bool {
    ui.spacing_mut().item_spacing.x = 2.0;
    let close = caption_button(ui, Icon::Close, "Close window", true);
    let (fullscreen, maximized) = ui.input(|i| {
        (
            i.viewport().fullscreen.unwrap_or(false),
            i.viewport().maximized.unwrap_or(false),
        )
    });
    let (icon, label) = match (fullscreen, maximized) {
        (true, _) => (Icon::Restore, "Exit full screen"),
        (_, true) => (Icon::Restore, "Restore window"),
        _ => (Icon::Maximize, "Maximize window"),
    };
    if caption_button(ui, icon, label, false) {
        maximize(ui.ctx());
    }
    if caption_button(ui, Icon::Minimize, "Minimize window", false) {
        ui.ctx()
            .send_viewport_cmd(egui::ViewportCommand::Minimized(true));
    }
    ui.add_space(8.0);
    close
}

fn caption_button(ui: &mut egui::Ui, icon: Icon, label: &str, close: bool) -> bool {
    let (rect, response) = ui.allocate_exact_size(Vec2::new(40.0, ROW_HEIGHT), Sense::click());
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), label)
    });
    let pressed = response.is_pointer_button_down_on();
    let hovered = response.hovered();
    let amount = crate::widgets::hover(ui, response.id, hovered || pressed);
    let surface = rect.shrink(1.0);
    let fg = ui.visuals().text_color();
    let accent = ui.visuals().hyperlink_color;
    let (hover_fill, hover_icon) = if close && ui.visuals().dark_mode {
        (
            Color32::from_rgb(67, 43, 42),
            Color32::from_rgb(238, 199, 191),
        )
    } else if close {
        (
            Color32::from_rgb(237, 219, 215),
            Color32::from_rgb(110, 56, 51),
        )
    } else {
        (fg.gamma_multiply(0.10), fg)
    };
    let fill = if pressed {
        if close {
            hover_fill
        } else {
            ui.visuals().selection.bg_fill
        }
    } else {
        hover_fill.gamma_multiply(amount)
    };
    ui.painter().rect_filled(surface, 4, fill);
    if response.has_focus() {
        ui.painter().rect_stroke(
            surface,
            4,
            egui::Stroke::new(1.0_f32, accent),
            egui::StrokeKind::Inside,
        );
    }
    icons::paint(
        ui.painter(),
        Rect::from_center_size(rect.center(), Vec2::splat(18.0)),
        icon,
        if hovered || pressed { hover_icon } else { fg },
    );
    response.on_hover_text(label).clicked()
}

pub fn resize(ctx: &egui::Context) {
    if ctx.input(|i| {
        i.viewport().maximized.unwrap_or(false) || i.viewport().fullscreen.unwrap_or(false)
    }) {
        return;
    }
    let rect = ctx.screen_rect();
    let t = 5.0;
    for (name, area, direction, cursor) in [
        (
            "left",
            Rect::from_min_size(rect.min, Vec2::new(t, rect.height())),
            egui::ResizeDirection::West,
            egui::CursorIcon::ResizeHorizontal,
        ),
        (
            "right",
            Rect::from_min_size(
                egui::pos2(rect.max.x - t, rect.min.y),
                Vec2::new(t, rect.height()),
            ),
            egui::ResizeDirection::East,
            egui::CursorIcon::ResizeHorizontal,
        ),
        (
            "top",
            Rect::from_min_size(rect.min, Vec2::new(rect.width(), t)),
            egui::ResizeDirection::North,
            egui::CursorIcon::ResizeVertical,
        ),
        (
            "bottom",
            Rect::from_min_size(
                egui::pos2(rect.min.x, rect.max.y - t),
                Vec2::new(rect.width(), t),
            ),
            egui::ResizeDirection::South,
            egui::CursorIcon::ResizeVertical,
        ),
        (
            "nw",
            Rect::from_min_size(rect.min, Vec2::splat(12.0)),
            egui::ResizeDirection::NorthWest,
            egui::CursorIcon::ResizeNwSe,
        ),
        (
            "ne",
            Rect::from_min_size(egui::pos2(rect.max.x - 12.0, rect.min.y), Vec2::splat(12.0)),
            egui::ResizeDirection::NorthEast,
            egui::CursorIcon::ResizeNeSw,
        ),
        (
            "sw",
            Rect::from_min_size(egui::pos2(rect.min.x, rect.max.y - 12.0), Vec2::splat(12.0)),
            egui::ResizeDirection::SouthWest,
            egui::CursorIcon::ResizeNeSw,
        ),
        (
            "se",
            Rect::from_min_size(rect.max - Vec2::splat(12.0), Vec2::splat(12.0)),
            egui::ResizeDirection::SouthEast,
            egui::CursorIcon::ResizeNwSe,
        ),
    ] {
        egui::Area::new(egui::Id::new(("window_edge", name)))
            .order(egui::Order::Foreground)
            .fixed_pos(area.min)
            .show(ctx, |ui| {
                let (_, r) = ui.allocate_exact_size(area.size(), Sense::drag());
                let r = r.on_hover_cursor(cursor);
                if r.drag_started() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::BeginResize(direction));
                }
            });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn caption_buttons_send_separate_window_commands() {
        for (x, expected) in [
            (364.0, egui::ViewportCommand::Close),
            (322.0, egui::ViewportCommand::Maximized(true)),
            (280.0, egui::ViewportCommand::Minimized(true)),
        ] {
            let ctx = egui::Context::default();
            let pos = egui::pos2(x, 22.0);
            let run = |events| {
                ctx.run(
                    egui::RawInput {
                        screen_rect: Some(Rect::from_min_size(
                            egui::Pos2::ZERO,
                            Vec2::new(400.0, 160.0),
                        )),
                        events,
                        ..Default::default()
                    },
                    |ctx| {
                        egui::TopBottomPanel::top("caption_test")
                            .frame(frame(false))
                            .show(ctx, |ui| {
                                row(ui, |ui| {
                                    if controls(ui) {
                                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                                    }
                                    title(ui, "WRKLMNL");
                                });
                            });
                    },
                )
            };
            let pointer = |pressed| egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            };
            run(vec![egui::Event::PointerMoved(pos)]);
            run(vec![pointer(true)]);
            let output = run(vec![pointer(false)]);
            assert!(output.viewport_output[&egui::ViewportId::ROOT]
                .commands
                .contains(&expected));
        }
    }

    #[test]
    fn maximize_control_restores_maximized_and_fullscreen_windows() {
        for (fullscreen, maximized, expected) in [
            (false, false, egui::ViewportCommand::Maximized(true)),
            (false, true, egui::ViewportCommand::Maximized(false)),
            (true, false, egui::ViewportCommand::Fullscreen(false)),
        ] {
            let ctx = egui::Context::default();
            let mut input = egui::RawInput::default();
            let viewport = input.viewports.get_mut(&egui::ViewportId::ROOT).unwrap();
            viewport.fullscreen = Some(fullscreen);
            viewport.maximized = Some(maximized);
            let output = ctx.run(input, maximize);
            assert!(output.viewport_output[&egui::ViewportId::ROOT]
                .commands
                .contains(&expected));
        }
    }
}
