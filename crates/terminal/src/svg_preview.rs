use anyhow::{ensure, Context, Result};
use eframe::egui::{ColorImage, FontDefinitions, FontFamily};
use resvg::{tiny_skia, usvg};
use std::{
    fs::File,
    io::Read,
    path::Path,
    sync::{Arc, OnceLock},
};

const SVG_LIMIT: u64 = 512 * 1024;

pub fn is_svg(path: &Path) -> bool {
    path.extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("svg"))
}

pub fn render(path: &Path, max: u32) -> Result<(ColorImage, (u32, u32))> {
    let mut source = String::new();
    File::open(path)?
        .take(SVG_LIMIT + 1)
        .read_to_string(&mut source)?;
    ensure!(
        source.len() as u64 <= SVG_LIMIT,
        "SVG preview is limited to 512 KiB"
    );

    static FONTS: OnceLock<(Arc<usvg::fontdb::Database>, String)> = OnceLock::new();
    let (fonts, family) = FONTS.get_or_init(|| {
        let bundled = FontDefinitions::default();
        let mut fonts = usvg::fontdb::Database::new();
        if let Some(font) = bundled
            .families
            .get(&FontFamily::Proportional)
            .and_then(|names| names.first())
            .and_then(|name| bundled.font_data.get(name))
        {
            fonts.load_font_data(font.font.to_vec());
        }
        let family = fonts
            .faces()
            .next()
            .and_then(|face| face.families.first())
            .map(|(name, _)| name.clone())
            .unwrap_or_default();
        fonts.set_serif_family(&family);
        fonts.set_sans_serif_family(&family);
        fonts.set_monospace_family(&family);
        (Arc::new(fonts), family)
    });
    let options = usvg::Options {
        font_family: family.clone(),
        fontdb: Arc::clone(fonts),
        // Render the document without following image references or decoding embedded raster data.
        image_href_resolver: usvg::ImageHrefResolver {
            resolve_data: Box::new(|_, _, _| None),
            resolve_string: Box::new(|_, _| None),
        },
        ..Default::default()
    };
    let tree = usvg::Tree::from_str(source.trim_start_matches('\u{feff}'), &options)?;
    let size = tree.size();
    let scale = (max as f32 / size.width().max(size.height())).min(1.0);
    let width = (size.width() * scale).round().max(1.0) as u32;
    let height = (size.height() * scale).round().max(1.0) as u32;
    let mut pixels = tiny_skia::Pixmap::new(width, height).context("Invalid SVG preview size")?;
    resvg::render(
        &tree,
        tiny_skia::Transform::from_scale(scale, scale),
        &mut pixels.as_mut(),
    );
    Ok((
        ColorImage::from_rgba_premultiplied([width as usize, height as usize], pixels.data()),
        (size.width().ceil() as u32, size.height().ceil() as u32),
    ))
}
