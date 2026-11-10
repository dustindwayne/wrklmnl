# Window control icons

Source: [Lucide](https://github.com/lucide-icons/lucide), retrieved 2026-10-18.

Minimize, maximize, restore and close geometry is compiled directly into `crates/terminal/src/icons.rs`. Unused SVG source copies were removed from the build.

The Rust adapter renders their geometry as vector strokes, so no icon font, emoji font, SVG runtime dependency, or OS title bar is needed. The restore path is approximated with short line segments. Stroke scale and color follow the app's presentation.

The full upstream ISC and Feather-derived MIT notices are in `LICENSE` and embedded in the executable. Icon outlines communicate window actions; they do not frame content regions.
