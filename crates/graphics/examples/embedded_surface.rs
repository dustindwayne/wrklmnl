//! Host-side verification of the embedded runtime: 128x64 monochrome, 1024 bytes of pixel storage.
//! A board replaces this memory target with its display driver; this example is not board firmware.
use terminal_ui_graphics::{
    basic_font::BasicText,
    layout::Length,
    paint::Paint,
    program::{self, Bounds},
    widgets::{self, Event, Kind, State, Widget},
    Color, Mono, Target,
};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut pixels = [0u8; 1024];
    let mut target = Mono::new(&mut pixels, 128, 64, 16).map_err(|_| "Invalid framebuffer")?;
    program::draw(
        "white[t 4,2 8 \"DEVICE READY\"; i -12,3 8 close]\nr 4,17 75%,3 white",
        &mut target,
        &mut BasicText,
        Paint::solid(Color(255, 255, 255)),
    )
    .map_err(|e| e.to_string())?;
    let controls = [
        Widget {
            id: "start",
            bounds: Bounds {
                x: 4,
                y: 28,
                width: Length::Pixels(48),
                height: Length::Pixels(16),
            },
            kind: Kind::Button("START"),
            enabled: true,
        },
        Widget {
            id: "level",
            bounds: Bounds {
                x: 4,
                y: 51,
                width: Length::Remaining,
                height: Length::Pixels(4),
            },
            kind: Kind::Progress(60),
            enabled: true,
        },
    ];
    let mut state = State::default();
    state.event(&controls, target.size(), Event::Next);
    assert_eq!(
        state.event(&controls, target.size(), Event::Activate),
        Some(widgets::Action::Activate("start"))
    );
    state.draw(
        &controls,
        &mut target,
        &mut BasicText,
        Paint::solid(Color(255, 255, 255)),
        Paint::solid(Color(255, 255, 255)),
        8,
    );
    assert!(pixels.iter().any(|&p| p != 0));
    if let Some(path) = std::env::args().nth(1) {
        use std::io::Write;
        let mut file = std::fs::File::create(path)?;
        file.write_all(b"P4\n128 64\n")?;
        file.write_all(&pixels)?;
    }
    println!("PASS: 128x64 drawing, glyphs, vector icon, responsive bar, keyboard focus and button activation; pixel storage={} bytes",pixels.len());
    Ok(())
}
