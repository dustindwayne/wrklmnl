//! Font adapters supply glyphs; the core neither loads font files nor allocates caches.
use crate::{Canvas, Color, Target};
pub struct Characters<'a> {
    chars: core::str::Chars<'a>,
}
pub fn characters(text: &str) -> Characters<'_> {
    Characters {
        chars: text.chars(),
    }
}
fn hex(chars: &mut core::str::Chars<'_>) -> Option<u16> {
    let mut value = 0;
    for _ in 0..4 {
        value = value * 16 + chars.next()?.to_digit(16)? as u16;
    }
    Some(value)
}
fn escape(chars: &mut core::str::Chars<'_>) -> Option<char> {
    Some(match chars.next()? {
        'n' => '\n',
        'r' => '\r',
        't' => '\t',
        'b' => '\u{0008}',
        'f' => '\u{000c}',
        '"' => '"',
        '\\' => '\\',
        '/' => '/',
        'u' => {
            let first = hex(chars)?;
            let code = if (0xd800..=0xdbff).contains(&first) {
                if chars.next() != Some('\\') || chars.next() != Some('u') {
                    return None;
                }
                let second = hex(chars)?;
                if !(0xdc00..=0xdfff).contains(&second) {
                    return None;
                }
                0x10000 + (u32::from(first - 0xd800) << 10) + u32::from(second - 0xdc00)
            } else {
                u32::from(first)
            };
            char::from_u32(code)?
        }
        _ => return None,
    })
}
impl Iterator for Characters<'_> {
    type Item = char;
    fn next(&mut self) -> Option<char> {
        let ch = self.chars.next()?;
        Some(if ch == '\\' {
            escape(&mut self.chars).unwrap_or('\u{fffd}')
        } else {
            ch
        })
    }
}
pub fn valid_escapes(text: &str) -> bool {
    let mut chars = text.chars();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            if escape(&mut chars).is_none() {
                return false;
            }
        } else if ch < '\u{20}' {
            return false;
        }
    }
    true
}
pub struct Glyph<'a> {
    pub width: u8,
    pub height: u8,
    pub advance: u8,
    pub rows: &'a [u8],
}
pub trait Font {
    fn glyph(&self, character: char) -> Option<Glyph<'_>>;
    fn line_height(&self) -> u16;
}
pub fn draw<T: Target>(
    canvas: &mut Canvas<'_, T>,
    font: &impl Font,
    origin: (i32, i32),
    text: &str,
    color: Color,
) {
    let (mut x, mut y) = origin;
    for ch in text.chars() {
        if ch == '\n' {
            x = origin.0;
            y = y.saturating_add(i32::from(font.line_height()));
            continue;
        }
        if let Some(g) = font.glyph(ch).or_else(|| font.glyph('?')) {
            let stride = usize::from(g.width).div_ceil(8);
            for row in 0..g.height {
                for col in 0..g.width {
                    if g.rows
                        .get(usize::from(row) * stride + usize::from(col) / 8)
                        .is_some_and(|b| b & (0x80 >> (col % 8)) != 0)
                    {
                        canvas.pixel(
                            x.saturating_add(i32::from(col)),
                            y.saturating_add(i32::from(row)),
                            color,
                        );
                    }
                }
            }
            x = x.saturating_add(i32::from(g.advance));
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::Rgb565;
    struct TinyFont;
    impl Font for TinyFont {
        fn glyph(&self, _: char) -> Option<Glyph<'_>> {
            Some(Glyph {
                width: 2,
                height: 2,
                advance: 3,
                rows: &[0x80, 0x40],
            })
        }
        fn line_height(&self) -> u16 {
            3
        }
    }
    #[test]
    fn glyphs_are_clipped_and_newlines_advance() {
        let mut data = [0u16; 16];
        {
            let mut t = Rgb565::new(&mut data, 4, 4, 4).unwrap();
            draw(
                &mut Canvas::new(&mut t),
                &TinyFont,
                (0, 0),
                "a\nb",
                Color(255, 255, 255),
            );
        }
        assert_eq!(data.iter().filter(|&&p| p != 0).count(), 3);
        assert_ne!(data[12], 0);
    }
}
