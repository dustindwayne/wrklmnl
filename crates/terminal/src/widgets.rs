use eframe::egui::{self, Response, Widget};
pub fn add(ui: &mut egui::Ui, widget: impl Widget) -> Response {
    let index = ui.painter().add(egui::Shape::Noop);
    let r = ui.add(widget);
    let active = r.has_focus() || r.is_pointer_button_down_on();
    let amount = hover(ui, r.id, r.hovered() || active);
    if amount > 0.0 {
        let fill = if active {
            ui.visuals().selection.bg_fill
        } else {
            ui.visuals().text_color().gamma_multiply(0.07)
        };
        ui.painter().set(
            index,
            egui::Shape::rect_filled(r.rect, 0.0, fill.gamma_multiply(amount)),
        );
    }
    r
}
pub fn hover(ui: &egui::Ui, id: egui::Id, hovering: bool) -> f32 {
    ui.ctx()
        .animate_bool_with_time(id.with("surface_hover"), hovering, 0.13)
}

pub fn button(ui: &mut egui::Ui, label: impl Into<egui::WidgetText>) -> Response {
    add(ui, egui::Button::new(label).frame(false))
}
pub fn text_tab(ui: &mut egui::Ui, label: &str, selected: bool) -> Response {
    let color = if selected {
        ui.visuals().hyperlink_color
    } else {
        ui.visuals().weak_text_color()
    };
    add(
        ui,
        egui::Button::new(egui::RichText::new(label).color(color)).frame(false),
    )
}
