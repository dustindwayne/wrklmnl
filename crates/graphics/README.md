# Portable drawing and controls

`no_std`, dependency-free, allocation-free, unsafe-code-free Rust. This crate builds independently of the desktop terminal, designer, fonts, PTYs and JSON modules.

- Custom `Target` drivers; caller-owned RGB565 and packed monochrome buffers with validated strides.
- Clipped pixels, lines, rectangles, circles and procedural vector icons.
- Named colors, percentage opacity and scoped inheritance in a borrowed instruction parser.
- Native-density drawing after responsive logical layout; `*`, percentages and negative anchors.
- Pluggable font rasterization; optional `font-basic` with a compact 5x7 uppercase ASCII font.
- Basic label/button/input/progress controls, focus, pointer/key dispatch and fixed-capacity UTF-8 input.
- Anchored layout and damage-region tracking APIs.

The shared `g[...]` UI component is validated by this parser and drawn through this rasterizer in desktop preview, live CLI graphics and exported desktop apps. Embedded applications can feed the same drawing-body instructions to `program::draw`, with their display and font adapters. The desktop UI container/parser/designer remains a separate std-based layer.

```text
cargo check -p terminal-ui-graphics --no-default-features --locked
cargo run -p terminal-ui-graphics --example embedded_surface --features font-basic
```

The example verifies 128x64 monochrome drawing and controls with a 1024-byte framebuffer. It is a host-side test, not board firmware. `Target` permits direct drivers; correct transparency additionally needs `ReadTarget` readback or backing storage. The renderer does not automatically supply hardware drivers, a display bus, an allocator, Unicode shaping, or the full desktop terminal emulator. BasicFont maps lowercase to uppercase and unsupported characters to '?'.
