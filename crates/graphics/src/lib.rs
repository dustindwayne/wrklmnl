//! Allocation-free pixel drawing. No operating system, window toolkit, or image decoder.
#![no_std]
#![forbid(unsafe_code)]
#[cfg(feature = "font-basic")]
pub mod basic_font;
pub mod icons;
pub mod layout;
pub mod paint;
pub mod program;
pub mod text;
pub mod widgets;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Color(pub u8, pub u8, pub u8);
impl Color {
    pub const fn rgb565(self) -> u16 {
        ((self.0 as u16 >> 3) << 11) | ((self.1 as u16 >> 2) << 5) | (self.2 as u16 >> 3)
    }
    pub const fn monochrome(self) -> bool {
        self.0 as u32 * 299 + self.1 as u32 * 587 + self.2 as u32 * 114 >= 128000
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: u16,
    pub height: u16,
}
impl Rect {
    pub const fn new(x: i32, y: i32, width: u16, height: u16) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }
    pub fn intersection(self, other: Self) -> Self {
        let x = i64::from(self.x).max(i64::from(other.x));
        let y = i64::from(self.y).max(i64::from(other.y));
        let right = (i64::from(self.x) + i64::from(self.width))
            .min(i64::from(other.x) + i64::from(other.width));
        let bottom = (i64::from(self.y) + i64::from(self.height))
            .min(i64::from(other.y) + i64::from(other.height));
        Self::new(
            x as i32,
            y as i32,
            (right - x).max(0) as u16,
            (bottom - y).max(0) as u16,
        )
    }
}

/// Drivers can override `span` to issue one bulk display transfer instead of per-pixel writes.
pub trait Target {
    fn size(&self) -> (u16, u16);
    fn pixel(&mut self, x: u16, y: u16, color: Color);
    fn span(&mut self, x: u16, y: u16, length: u16, color: Color) {
        let (width, height) = self.size();
        if x >= width || y >= height {
            return;
        }
        for offset in 0..length.min(width - x) {
            self.pixel(x + offset, y, color);
        }
    }
}
pub struct Canvas<'a, T: Target> {
    target: &'a mut T,
    clip: Rect,
}
impl<'a, T: Target> Canvas<'a, T> {
    pub fn new(target: &'a mut T) -> Self {
        let (w, h) = target.size();
        Self {
            target,
            clip: Rect::new(0, 0, w, h),
        }
    }
    pub fn clip(&mut self, rect: Rect) {
        self.clip = self.clip.intersection(rect);
    }
    pub fn pixel(&mut self, x: i32, y: i32, color: Color) {
        let r = self.clip;
        if i64::from(x) >= i64::from(r.x)
            && i64::from(y) >= i64::from(r.y)
            && i64::from(x) < i64::from(r.x) + i64::from(r.width)
            && i64::from(y) < i64::from(r.y) + i64::from(r.height)
        {
            self.target.pixel(x as u16, y as u16, color);
        }
    }
    pub fn fill(&mut self, rect: Rect, color: Color) {
        let r = rect.intersection(self.clip);
        if r.width == 0 || r.height == 0 {
            return;
        }
        for y in r.y..r.y + i32::from(r.height) {
            self.target.span(r.x as u16, y as u16, r.width, color);
        }
    }
    /// Clips before rasterization so off-screen coordinates cannot cause unbounded iteration.
    pub fn line(&mut self, from: (i32, i32), to: (i32, i32), color: Color) {
        let r = self.clip;
        if r.width == 0 || r.height == 0 {
            return;
        }
        let (left, top) = (i128::from(r.x), i128::from(r.y));
        let (right, bottom) = (
            left + i128::from(r.width) - 1,
            top + i128::from(r.height) - 1,
        );
        let code = |x: i128, y: i128| -> u8 {
            u8::from(x < left)
                | (u8::from(x > right) << 1)
                | (u8::from(y < top) << 2)
                | (u8::from(y > bottom) << 3)
        };
        let (mut x0, mut y0, mut x1, mut y1) = (
            i128::from(from.0),
            i128::from(from.1),
            i128::from(to.0),
            i128::from(to.1),
        );
        let mut accepted = false;
        for _ in 0..16 {
            let (a, b) = (code(x0, y0), code(x1, y1));
            if a | b == 0 {
                accepted = true;
                break;
            }
            if a & b != 0 {
                return;
            }
            let c = if a != 0 { a } else { b };
            let (x, y) = if c & 8 != 0 {
                (x0 + (x1 - x0) * (bottom - y0) / (y1 - y0), bottom)
            } else if c & 4 != 0 {
                (x0 + (x1 - x0) * (top - y0) / (y1 - y0), top)
            } else if c & 2 != 0 {
                (right, y0 + (y1 - y0) * (right - x0) / (x1 - x0))
            } else {
                (left, y0 + (y1 - y0) * (left - x0) / (x1 - x0))
            };
            if c == a {
                x0 = x;
                y0 = y;
            } else {
                x1 = x;
                y1 = y;
            }
        }
        if !accepted {
            return;
        }
        let (mut x, mut y, end_x, end_y) = (x0 as i32, y0 as i32, x1 as i32, y1 as i32);
        let dx = (end_x - x).abs();
        let dy = -(end_y - y).abs();
        let sx = if x < end_x { 1 } else { -1 };
        let sy = if y < end_y { 1 } else { -1 };
        let mut error = dx + dy;
        loop {
            self.pixel(x, y, color);
            if x == end_x && y == end_y {
                break;
            }
            let twice = 2 * error;
            if twice >= dy {
                error += dy;
                x += sx;
            }
            if twice <= dx {
                error += dx;
                y += sy;
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BufferError {
    StrideTooSmall,
    BufferTooSmall,
}
/// Caller owns the storage. Rows may include padding. Pixels outside the dimensions are ignored.
pub struct Rgb565<'a> {
    pixels: &'a mut [u16],
    width: u16,
    height: u16,
    stride: usize,
}
impl<'a> Rgb565<'a> {
    pub fn new(
        pixels: &'a mut [u16],
        width: u16,
        height: u16,
        stride: usize,
    ) -> Result<Self, BufferError> {
        if stride < usize::from(width) {
            return Err(BufferError::StrideTooSmall);
        }
        if stride
            .checked_mul(usize::from(height))
            .is_none_or(|n| n > pixels.len())
        {
            return Err(BufferError::BufferTooSmall);
        }
        Ok(Self {
            pixels,
            width,
            height,
            stride,
        })
    }
}
impl Target for Rgb565<'_> {
    fn size(&self) -> (u16, u16) {
        (self.width, self.height)
    }
    fn pixel(&mut self, x: u16, y: u16, c: Color) {
        if x < self.width && y < self.height {
            self.pixels[usize::from(y) * self.stride + usize::from(x)] = c.rgb565();
        }
    }
    fn span(&mut self, x: u16, y: u16, length: u16, c: Color) {
        if x >= self.width || y >= self.height {
            return;
        }
        let start = usize::from(y) * self.stride + usize::from(x);
        let n = usize::from(length.min(self.width - x));
        self.pixels[start..start + n].fill(c.rgb565());
    }
}
/// Packed, horizontal, most-significant-bit-first monochrome rows.
pub struct Mono<'a> {
    bytes: &'a mut [u8],
    width: u16,
    height: u16,
    stride: usize,
}
impl<'a> Mono<'a> {
    pub fn new(
        bytes: &'a mut [u8],
        width: u16,
        height: u16,
        stride: usize,
    ) -> Result<Self, BufferError> {
        if stride < usize::from(width).div_ceil(8) {
            return Err(BufferError::StrideTooSmall);
        }
        if stride
            .checked_mul(usize::from(height))
            .is_none_or(|n| n > bytes.len())
        {
            return Err(BufferError::BufferTooSmall);
        }
        Ok(Self {
            bytes,
            width,
            height,
            stride,
        })
    }
}
impl Target for Mono<'_> {
    fn size(&self) -> (u16, u16) {
        (self.width, self.height)
    }
    fn pixel(&mut self, x: u16, y: u16, c: Color) {
        if x >= self.width || y >= self.height {
            return;
        }
        let b = &mut self.bytes[usize::from(y) * self.stride + usize::from(x) / 8];
        let mask = 0x80 >> (x % 8);
        if c.monochrome() {
            *b |= mask;
        } else {
            *b &= !mask;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn clipped_fill_preserves_padding() {
        let mut data = [0; 12];
        {
            let mut t = Rgb565::new(&mut data, 3, 3, 4).unwrap();
            let mut c = Canvas::new(&mut t);
            c.fill(Rect::new(-2, -2, 4, 4), Color(255, 255, 255));
        }
        assert_eq!(data, [65535, 65535, 0, 0, 65535, 65535, 0, 0, 0, 0, 0, 0]);
    }
    #[test]
    fn extreme_line_is_clipped() {
        let mut data = [0; 16];
        {
            let mut t = Rgb565::new(&mut data, 4, 4, 4).unwrap();
            Canvas::new(&mut t).line(
                (i32::MIN, i32::MIN),
                (i32::MAX, i32::MAX),
                Color(255, 255, 255),
            );
        }
        for y in 0..4 {
            for x in 0..4 {
                assert_eq!(data[y * 4 + x], if x == y { 65535 } else { 0 });
            }
        }
    }
    #[test]
    fn packed_mono_preserves_unused_bits() {
        let mut data = [0x07; 2];
        {
            let mut t = Mono::new(&mut data, 5, 2, 1).unwrap();
            Canvas::new(&mut t).fill(Rect::new(0, 0, 5, 2), Color(255, 255, 255));
        }
        assert_eq!(data, [255, 255]);
    }
    #[test]
    fn rejects_short_storage_and_stride() {
        let mut data = [0; 4];
        assert!(matches!(
            Rgb565::new(&mut data, 3, 2, 3),
            Err(BufferError::BufferTooSmall)
        ));
        let mut bytes = [0u8; 4];
        assert!(matches!(
            Mono::new(&mut bytes, 9, 2, 1),
            Err(BufferError::StrideTooSmall)
        ));
    }
    #[test]
    fn clip_never_expands() {
        let mut data = [0; 16];
        {
            let mut t = Rgb565::new(&mut data, 4, 4, 4).unwrap();
            let mut c = Canvas::new(&mut t);
            c.clip(Rect::new(1, 1, 1, 1));
            c.clip(Rect::new(0, 0, 4, 4));
            c.fill(Rect::new(0, 0, 4, 4), Color(255, 255, 255));
        }
        assert_eq!(data.iter().filter(|&&p| p != 0).count(), 1);
    }
}
