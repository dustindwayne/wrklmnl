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
    ui.hyperlink_to(
        "Third-party notices and source",
        "https://github.com/dustindwayne/wrklmnl/blob/main/docs/distribution.md#notices-and-source",
    );
    #[cfg(windows)]
    {
        use std::io::Read;

        if crate::widgets::button(ui, "Copy third-party notices").clicked() {
            ui.ctx().copy_text(notices().into());
        }
        if crate::widgets::button(ui, "Copy Rust standard library notices (HTML)").clicked() {
            let compressed = include_bytes!("../../../licenses/rust-stdlib-notices.html.gz");
            let mut html = String::new();
            match flate2::read::GzDecoder::new(compressed.as_slice()).read_to_string(&mut html) {
                Ok(_) => ui.ctx().copy_text(html),
                Err(error) => {
                    ui.label(format!("Could not read Rust notices. {error}"));
                }
            }
        }
    }
}
