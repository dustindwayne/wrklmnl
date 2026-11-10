use eframe::egui;

#[cfg(windows)]
pub fn notices() -> &'static str {
    include_str!("../../../THIRD_PARTY_NOTICES.txt")
}

#[cfg(not(windows))]
pub fn notices() -> &'static str {
    crate::icons::LICENSE
}

pub fn show(ui: &mut egui::Ui) {
    ui.heading("Licenses");
    ui.weak("Third-party components are covered by their own licenses.");
    ui.add_space(8.0);
    ui.label("The release includes dependency and font notices, Rust standard library notices, and the source archives for the MPL-covered components.");
    ui.hyperlink_to(
        "Open release notices and source",
        "https://github.com/dustindwayne/wrklmnl/releases",
    );
    #[cfg(windows)]
    if crate::widgets::button(ui, "Copy Windows third-party notices").clicked() {
        ui.ctx().copy_text(notices().into());
    }
}
