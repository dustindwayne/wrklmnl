use eframe::egui::{self, Rect, Sense, Vec2};
pub fn title(ui: &mut egui::Ui, title: &str) {
    let (rect, r) = ui.allocate_exact_size(
        Vec2::new(ui.available_width().max(80.0), 28.0),
        Sense::click_and_drag(),
    );
    ui.painter().with_clip_rect(rect).text(
        rect.left_center(),
        egui::Align2::LEFT_CENTER,
        title,
        egui::TextStyle::Body.resolve(ui.style()),
        ui.visuals().text_color(),
    );
    let r = r.on_hover_text(title);
    if r.drag_started() {
        ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
    }
    if r.double_clicked() {
        maximize(ui.ctx());
    }
}
pub fn maximize(ctx: &egui::Context) {
    let value = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
    ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!value));
}
pub fn controls(ui: &mut egui::Ui) -> bool {
    use crate::icons::{button, Icon};
    let close = button(ui, Icon::Close, "Close window").clicked();
    let max = ui.input(|i| i.viewport().maximized.unwrap_or(false));
    if button(
        ui,
        if max { Icon::Restore } else { Icon::Maximize },
        "Maximize or restore",
    )
    .clicked()
    {
        maximize(ui.ctx());
    }
    if button(ui, Icon::Minimize, "Minimize").clicked() {
        ui.ctx()
            .send_viewport_cmd(egui::ViewportCommand::Minimized(true));
    }
    close
}
pub fn resize(ctx: &egui::Context) {
    if ctx.input(|i| i.viewport().maximized.unwrap_or(false)) {
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
