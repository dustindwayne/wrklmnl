//! Optional compact 5x7 font, authored here. Lowercase maps to uppercase; unsupported glyphs use '?'.
use crate::{
    paint::{Paint, Painted, ReadTarget},
    program::TextRenderer,
    text::{Font, Glyph},
    Canvas, Rect,
};
pub struct BasicFont;
impl Font for BasicFont {
    fn line_height(&self) -> u16 {
        8
    }
    fn glyph(&self, ch: char) -> Option<Glyph<'_>> {
        let rows: &'static [u8] = match ch.to_ascii_uppercase() {
            'A' => &[112, 136, 136, 248, 136, 136, 136],
            'B' => &[240, 136, 136, 240, 136, 136, 240],
            'C' => &[112, 136, 128, 128, 128, 136, 112],
            'D' => &[240, 136, 136, 136, 136, 136, 240],
            'E' => &[248, 128, 128, 240, 128, 128, 248],
            'F' => &[248, 128, 128, 240, 128, 128, 128],
            'G' => &[112, 136, 128, 184, 136, 136, 112],
            'H' => &[136, 136, 136, 248, 136, 136, 136],
            'I' => &[112, 32, 32, 32, 32, 32, 112],
            'J' => &[56, 16, 16, 16, 144, 144, 96],
            'K' => &[136, 144, 160, 192, 160, 144, 136],
            'L' => &[128, 128, 128, 128, 128, 128, 248],
            'M' => &[136, 216, 168, 168, 136, 136, 136],
            'N' => &[136, 200, 168, 152, 136, 136, 136],
            'O' => &[112, 136, 136, 136, 136, 136, 112],
            'P' => &[240, 136, 136, 240, 128, 128, 128],
            'Q' => &[112, 136, 136, 136, 168, 144, 104],
            'R' => &[240, 136, 136, 240, 160, 144, 136],
            'S' => &[120, 128, 128, 112, 8, 8, 240],
            'T' => &[248, 32, 32, 32, 32, 32, 32],
            'U' => &[136, 136, 136, 136, 136, 136, 112],
            'V' => &[136, 136, 136, 136, 136, 80, 32],
            'W' => &[136, 136, 136, 168, 168, 216, 136],
            'X' => &[136, 136, 80, 32, 80, 136, 136],
            'Y' => &[136, 136, 80, 32, 32, 32, 32],
            'Z' => &[248, 8, 16, 32, 64, 128, 248],
            '0' => &[112, 136, 152, 168, 200, 136, 112],
            '1' => &[32, 96, 32, 32, 32, 32, 112],
            '2' => &[112, 136, 8, 16, 32, 64, 248],
            '3' => &[240, 8, 8, 112, 8, 8, 240],
            '4' => &[16, 48, 80, 144, 248, 16, 16],
            '5' => &[248, 128, 128, 240, 8, 8, 240],
            '6' => &[112, 128, 128, 240, 136, 136, 112],
            '7' => &[248, 8, 16, 32, 64, 64, 64],
            '8' => &[112, 136, 136, 112, 136, 136, 112],
            '9' => &[112, 136, 136, 120, 8, 8, 112],
            ' ' => &[0; 7],
            '.' => &[0, 0, 0, 0, 0, 96, 96],
            ',' => &[0, 0, 0, 0, 32, 32, 64],
            ':' => &[0, 96, 96, 0, 96, 96, 0],
            '-' => &[0, 0, 0, 248, 0, 0, 0],
            '_' => &[0, 0, 0, 0, 0, 0, 248],
            '+' => &[0, 32, 32, 248, 32, 32, 0],
            '/' => &[8, 8, 16, 32, 64, 128, 128],
            '!' => &[32, 32, 32, 32, 32, 0, 32],
            '(' => &[16, 32, 64, 64, 64, 32, 16],
            ')' => &[64, 32, 16, 16, 16, 32, 64],
            '%' => &[200, 208, 16, 32, 64, 88, 152],
            '>' => &[128, 64, 32, 16, 32, 64, 128],
            '<' => &[8, 16, 32, 64, 32, 16, 8],
            '=' => &[0, 0, 248, 0, 248, 0, 0],
            _ => &[112, 136, 8, 16, 32, 0, 32],
        };
        Some(Glyph {
            width: 5,
            height: 7,
            advance: 6,
            rows,
        })
    }
}
pub struct BasicText;
impl<T: ReadTarget> TextRenderer<T> for BasicText {
    fn text(&mut self, target: &mut T, position: (i32, i32), size: u16, text: &str, paint: Paint) {
        let scale = i32::from((size / 8).max(1));
        let (w, h) = target.size();
        let (mut x, mut y) = position;
        let mut chars = crate::text::characters(text);
        let mut painted = Painted { target, paint };
        let mut canvas = Canvas::new(&mut painted);
        while let Some(ch) = chars.next() {
            if ch == '\n' {
                x = position.0;
                y = y.saturating_add(scale * 8);
                continue;
            }
            if x.saturating_add(scale * 5) > i32::from(w) && x > position.0 {
                x = position.0;
                y = y.saturating_add(scale * 8);
            }
            if y >= i32::from(h) {
                break;
            }
            if let Some(g) = BasicFont.glyph(ch) {
                for row in 0..7 {
                    for col in 0..5 {
                        if g.rows[row] & (0x80 >> col) != 0 {
                            canvas.fill(
                                Rect::new(
                                    x.saturating_add(col * scale),
                                    y.saturating_add(row as i32 * scale),
                                    scale as u16,
                                    scale as u16,
                                ),
                                paint.color,
                            );
                        }
                    }
                }
            }
            x = x.saturating_add(scale * 6);
        }
    }
}
