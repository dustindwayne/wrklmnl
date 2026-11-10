# Minimal eframe 0.31.1 source patch

Upstream: https://github.com/emilk/egui/tree/0.31.1/crates/eframe

Distributed under the upstream MIT license included here.

The only runtime change replaces the embedded default PNG window icon with an
empty icon. Its image data is omitted. Cargo package include paths were adjusted
for the local MIT notice. All UI rendering, native windows and external icon
loading keep their upstream behavior. Keep this small patch when updating eframe.
