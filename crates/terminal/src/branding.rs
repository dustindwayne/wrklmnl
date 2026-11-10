use eframe::egui;
use std::sync::{Arc, OnceLock};

pub fn icon() -> Arc<egui::IconData> {
    static ICON: OnceLock<Arc<egui::IconData>> = OnceLock::new();
    ICON.get_or_init(|| {
        let image = image::load_from_memory(include_bytes!("../assets/icon.png"))
            .expect("valid WRKLMNL icon")
            .resize(256, 256, image::imageops::FilterType::Lanczos3)
            .into_rgba8();
        Arc::new(egui::IconData {
            width: image.width(),
            height: image.height(),
            rgba: image.into_raw(),
        })
    })
    .clone()
}
