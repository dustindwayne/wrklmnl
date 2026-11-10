use crate::{Color, Mono, Rgb565, Target};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Paint {
    pub color: Color,
    pub opacity: u8,
}
impl Paint {
    pub const fn solid(color: Color) -> Self {
        Self {
            color,
            opacity: 255,
        }
    }
    pub fn parse(value: &str) -> Option<Self> {
        let (name, alpha) = match value.split_once('/') {
            Some((n, a)) => (n, a.parse::<u8>().ok()?),
            None => (value, 100),
        };
        if alpha > 100 {
            return None;
        }
        let color = match name {
            "black" => Color(0, 0, 0),
            "white" => Color(255, 255, 255),
            "red" => Color(255, 0, 0),
            "green" => Color(0, 128, 0),
            "blue" => Color(0, 0, 255),
            "gray" | "grey" => Color(128, 128, 128),
            "silver" => Color(192, 192, 192),
            "charcoal" => Color(26, 26, 26),
            "pink" => Color(255, 0, 119),
            "orange" => Color(255, 165, 0),
            "yellow" => Color(255, 255, 0),
            "purple" => Color(128, 0, 128),
            "cyan" => Color(0, 255, 255),
            "teal" => Color(0, 128, 128),
            "navy" => Color(0, 0, 128),
            "lime" => Color(0, 255, 0),
            "magenta" => Color(255, 0, 255),
            "transparent" => {
                return Some(Self {
                    color: Color(0, 0, 0),
                    opacity: 0,
                })
            }
            _ => return None,
        };
        Some(Self {
            color,
            opacity: ((u16::from(alpha) * 255 + 50) / 100) as u8,
        })
    }
}
/// Transparency requires readable pixels or an equivalent backing store. Drivers choose storage.
pub trait ReadTarget: Target {
    fn read(&self, x: u16, y: u16) -> Color;
    fn blend(&mut self, x: u16, y: u16, paint: Paint) {
        let (w, h) = self.size();
        if x >= w || y >= h || paint.opacity == 0 {
            return;
        }
        if paint.opacity == 255 {
            self.pixel(x, y, paint.color);
            return;
        }
        let old = self.read(x, y);
        let a = u32::from(paint.opacity);
        let channel = |foreground: u8, background: u8| {
            ((u32::from(foreground) * a + u32::from(background) * (255 - a) + 127) / 255) as u8
        };
        self.pixel(
            x,
            y,
            Color(
                channel(paint.color.0, old.0),
                channel(paint.color.1, old.1),
                channel(paint.color.2, old.2),
            ),
        );
    }
}
pub struct Painted<'a, T: ReadTarget> {
    pub target: &'a mut T,
    pub paint: Paint,
}
impl<T: ReadTarget> Target for Painted<'_, T> {
    fn size(&self) -> (u16, u16) {
        self.target.size()
    }
    fn pixel(&mut self, x: u16, y: u16, color: Color) {
        self.target.blend(
            x,
            y,
            Paint {
                color,
                ..self.paint
            },
        );
    }
    fn span(&mut self, x: u16, y: u16, length: u16, color: Color) {
        if self.paint.opacity == 255 {
            self.target.span(x, y, length, color);
            return;
        }
        let (w, h) = self.size();
        if x >= w || y >= h {
            return;
        }
        for offset in 0..length.min(w - x) {
            self.pixel(x + offset, y, color);
        }
    }
}
impl ReadTarget for Rgb565<'_> {
    fn read(&self, x: u16, y: u16) -> Color {
        if x >= self.width || y >= self.height {
            return Color(0, 0, 0);
        }
        let p = self.pixels[usize::from(y) * self.stride + usize::from(x)];
        let r = ((p >> 11) & 31) as u8;
        let g = ((p >> 5) & 63) as u8;
        let b = (p & 31) as u8;
        Color(
            (r << 3) | (r >> 2),
            (g << 2) | (g >> 4),
            (b << 3) | (b >> 2),
        )
    }
}
impl ReadTarget for Mono<'_> {
    fn read(&self, x: u16, y: u16) -> Color {
        if x >= self.width || y >= self.height {
            return Color(0, 0, 0);
        }
        if self.bytes[usize::from(y) * self.stride + usize::from(x) / 8] & (0x80 >> (x % 8)) != 0 {
            Color(255, 255, 255)
        } else {
            Color(0, 0, 0)
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn opacity_is_percentage() {
        assert_eq!(Paint::parse("red/20").unwrap().opacity, 51);
        assert!(Paint::parse("red/101").is_none());
        assert!(Paint::parse("unknown").is_none());
    }
    #[test]
    fn blends_with_existing_surface() {
        let mut data = [Color(255, 255, 255).rgb565()];
        let mut target = Rgb565::new(&mut data, 1, 1, 1).unwrap();
        target.blend(0, 0, Paint::parse("red/20").unwrap());
        let c = target.read(0, 0);
        assert_eq!(c.0, 255);
        assert!((198..=210).contains(&c.1));
        assert!((198..=210).contains(&c.2));
        let before = c;
        target.blend(0, 0, Paint::parse("blue/0").unwrap());
        assert_eq!(target.read(0, 0), before);
    }
}
