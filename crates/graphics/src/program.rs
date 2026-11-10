//! Borrowed, allocation-free graphics instructions shared by desktop and embedded backends.
use crate::{
    icons::Icon,
    layout::Length,
    paint::{Paint, Painted, ReadTarget},
    Canvas, Color, Rect,
};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Error {
    pub offset: usize,
    pub message: &'static str,
}
impl core::fmt::Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "Graphics byte {}: {}", self.offset, self.message)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Bounds {
    pub x: i32,
    pub y: i32,
    pub width: Length,
    pub height: Length,
}
impl Bounds {
    pub fn resolve(self, w: u16, h: u16) -> Rect {
        let x = if self.x < 0 {
            i32::from(w).saturating_add(self.x)
        } else {
            self.x
        };
        let y = if self.y < 0 {
            i32::from(h).saturating_add(self.y)
        } else {
            self.y
        };
        Rect::new(
            x,
            y,
            self.width
                .resolve(w.saturating_sub(x.max(0).min(i32::from(w)) as u16)),
            self.height
                .resolve(h.saturating_sub(y.max(0).min(i32::from(h)) as u16)),
        )
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Operation<'a> {
    Rectangle(Bounds, Paint),
    Pixel(i32, i32, Paint),
    Line((i32, i32), (i32, i32), Paint),
    Circle((i32, i32), u16, Paint),
    Text((i32, i32), u16, &'a str, Paint),
    Icon((i32, i32), u16, Icon, Paint),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Token<'a> {
    Word(&'a str),
    Text(&'a str),
    Open,
    Close,
}
struct Parser<'a> {
    source: &'a str,
    at: usize,
}
impl<'a> Parser<'a> {
    fn error(&self, message: &'static str) -> Error {
        Error {
            offset: self.at,
            message,
        }
    }
    fn next(&mut self) -> Result<Option<Token<'a>>, Error> {
        let bytes = self.source.as_bytes();
        loop {
            while bytes
                .get(self.at)
                .is_some_and(|b| b.is_ascii_whitespace() || *b == b';')
            {
                self.at += 1;
            }
            if bytes.get(self.at) == Some(&b'#') {
                while bytes.get(self.at).is_some_and(|b| *b != b'\n') {
                    self.at += 1;
                }
            } else {
                break;
            }
        }
        let Some(&b) = bytes.get(self.at) else {
            return Ok(None);
        };
        self.at += 1;
        if b == b'[' {
            return Ok(Some(Token::Open));
        }
        if b == b']' {
            return Ok(Some(Token::Close));
        }
        if b == b'"' {
            let start = self.at;
            let mut escaped = false;
            while let Some(&b) = bytes.get(self.at) {
                self.at += 1;
                if b == b'"' && !escaped {
                    let text = &self.source[start..self.at - 1];
                    if !crate::text::valid_escapes(text) {
                        return Err(self.error("invalid quoted text escape"));
                    }
                    return Ok(Some(Token::Text(text)));
                }
                if b == b'\\' && !escaped {
                    escaped = true;
                } else {
                    escaped = false;
                }
            }
            return Err(self.error("unfinished text string"));
        }
        let start = self.at - 1;
        while bytes
            .get(self.at)
            .is_some_and(|b| !b.is_ascii_whitespace() && !b";[]\"".contains(b))
        {
            self.at += 1;
        }
        Ok(Some(Token::Word(&self.source[start..self.at])))
    }
    fn word(&mut self) -> Result<&'a str, Error> {
        match self.next()? {
            Some(Token::Word(w)) => Ok(w),
            _ => Err(self.error("expected positional argument")),
        }
    }
    fn point(&mut self) -> Result<(i32, i32), Error> {
        let word = self.word()?;
        let Some((x, y)) = word.split_once(',') else {
            return Err(self.error("expected x,y"));
        };
        let x = x.parse().map_err(|_| self.error("invalid x coordinate"))?;
        let y = y.parse().map_err(|_| self.error("invalid y coordinate"))?;
        Ok((x, y))
    }
    fn paint(&mut self, inherited: Paint) -> Result<Paint, Error> {
        let before = self.at;
        match self.next()? {
            Some(Token::Word(w)) if Paint::parse(w).is_some() => {
                let after_color = self.at;
                if self.next()? == Some(Token::Open) {
                    self.at = before;
                    Ok(inherited)
                } else {
                    self.at = after_color;
                    Ok(Paint::parse(w).unwrap())
                }
            }
            _ => {
                self.at = before;
                Ok(inherited)
            }
        }
    }
}
fn length(text: &str) -> Option<Length> {
    if text == "*" {
        Some(Length::Remaining)
    } else if let Some(p) = text.strip_suffix('%') {
        let p = p.parse::<u8>().ok()?;
        if p > 100 {
            None
        } else {
            Some(Length::Percent(p))
        }
    } else {
        text.parse().ok().map(Length::Pixels)
    }
}
pub fn visit<'a>(
    source: &'a str,
    default: Paint,
    mut visitor: impl FnMut(Operation<'a>),
) -> Result<(), Error> {
    visit_spanned(source, default, |_, _, operation| visitor(operation))
}
pub fn visit_spanned<'a>(
    source: &'a str,
    default: Paint,
    mut visitor: impl FnMut(usize, usize, Operation<'a>),
) -> Result<(), Error> {
    let mut p = Parser { source, at: 0 };
    let mut paints = [default; 16];
    let mut depth = 0;
    let mut count = 0;
    while let Some(token) = p.next()? {
        let tag = match token {
            Token::Close => {
                if depth == 0 {
                    return Err(p.error("unexpected closing group"));
                }
                depth -= 1;
                continue;
            }
            Token::Word(w) => w,
            _ => return Err(p.error("expected drawing instruction")),
        };
        if let Some(color) = Paint::parse(tag) {
            if p.next()? != Some(Token::Open) {
                return Err(p.error("a color group requires [ ... ]"));
            }
            if depth == 15 {
                return Err(p.error("at most 16 color levels"));
            }
            depth += 1;
            paints[depth] = color;
            continue;
        }
        let start = tag.as_ptr() as usize - source.as_ptr() as usize;
        count += 1;
        if count > 256 {
            return Err(p.error("at most 256 drawing instructions per surface"));
        }
        let inherited = paints[depth];
        let operation = match tag {
            "r" => {
                let (x, y) = p.point()?;
                let size = p.word()?;
                let Some((w, h)) = size.split_once(',') else {
                    return Err(p.error("rectangle requires width,height"));
                };
                let width = length(w)
                    .ok_or_else(|| p.error("width requires pixels, a percentage, or *"))?;
                let height = length(h)
                    .ok_or_else(|| p.error("height requires pixels, a percentage, or *"))?;
                Operation::Rectangle(
                    Bounds {
                        x,
                        y,
                        width,
                        height,
                    },
                    p.paint(inherited)?,
                )
            }
            "p" => {
                let (x, y) = p.point()?;
                Operation::Pixel(x, y, p.paint(inherited)?)
            }
            "l" => {
                let from = p.point()?;
                let to = p.point()?;
                Operation::Line(from, to, p.paint(inherited)?)
            }
            "c" => {
                let center = p.point()?;
                let radius = p
                    .word()?
                    .parse::<u16>()
                    .map_err(|_| p.error("circle requires an integer radius"))?;
                if radius > 4096 {
                    return Err(p.error("circle radius must be at most 4096"));
                }
                Operation::Circle(center, radius, p.paint(inherited)?)
            }
            "t" => {
                let pos = p.point()?;
                let mut token = p.next()?;
                let mut size = 16;
                if let Some(Token::Word(w)) = token {
                    size = w.parse::<u16>().map_err(|_| {
                        p.error("text requires a quoted string, optionally preceded by size")
                    })?;
                    token = p.next()?;
                }
                if !(4..=128).contains(&size) {
                    return Err(p.error("text size must be 4-128"));
                }
                let Some(Token::Text(text)) = token else {
                    return Err(p.error("text requires a quoted string"));
                };
                Operation::Text(pos, size, text, p.paint(inherited)?)
            }
            "i" => {
                let pos = p.point()?;
                let mut name = p.word()?;
                let mut size = 16;
                if let Ok(n) = name.parse::<u16>() {
                    size = n;
                    name = p.word()?;
                }
                if !(2..=256).contains(&size) {
                    return Err(p.error("icon size must be 2-256"));
                }
                let icon = match name {
                    "close" => Icon::Close,
                    "plus" => Icon::Plus,
                    "minus" => Icon::Minus,
                    "maximize" => Icon::Maximize,
                    "right" => Icon::ChevronRight,
                    "down" => Icon::ChevronDown,
                    "terminal" | "term" => Icon::Terminal,
                    _ => return Err(p.error("unknown vector icon")),
                };
                Operation::Icon(pos, size, icon, p.paint(inherited)?)
            }
            _ => return Err(p.error("unknown instruction or invalid named color/opacity")),
        };
        visitor(start, p.at, operation);
    }
    if depth != 0 {
        return Err(p.error("unclosed color group"));
    }
    Ok(())
}
pub fn validate(source: &str) -> Result<(), Error> {
    visit(source, Paint::solid(Color(245, 245, 245)), |_| {})
}
pub trait TextRenderer<T: ReadTarget> {
    fn text(&mut self, target: &mut T, position: (i32, i32), size: u16, text: &str, paint: Paint);
}
fn position(point: (i32, i32), w: u16, h: u16) -> (i32, i32) {
    (
        if point.0 < 0 {
            i32::from(w).saturating_add(point.0)
        } else {
            point.0
        },
        if point.1 < 0 {
            i32::from(h).saturating_add(point.1)
        } else {
            point.1
        },
    )
}
pub fn draw<T: ReadTarget>(
    source: &str,
    target: &mut T,
    text: &mut impl TextRenderer<T>,
    default: Paint,
) -> Result<(), Error> {
    draw_in_viewport(source, target, text, default, target.size())
}
/// Layout uses logical pixels; rasterization uses the display's native pixel density.
pub fn draw_in_viewport<T: ReadTarget>(
    source: &str,
    target: &mut T,
    text: &mut impl TextRenderer<T>,
    default: Paint,
    logical: (u16, u16),
) -> Result<(), Error> {
    validate(source)?;
    let (w, h) = logical;
    if w == 0 || h == 0 {
        return Ok(());
    }
    let (pw, ph) = target.size();
    let sx = |x: i32| {
        ((i128::from(x) * i128::from(pw)) / i128::from(w))
            .clamp(i128::from(i32::MIN), i128::from(i32::MAX)) as i32
    };
    let sy = |y: i32| {
        ((i128::from(y) * i128::from(ph)) / i128::from(h))
            .clamp(i128::from(i32::MIN), i128::from(i32::MAX)) as i32
    };
    let point = |p| {
        let (x, y) = position(p, w, h);
        (sx(x), sy(y))
    };
    let rect = |r: Rect| {
        Rect::new(
            sx(r.x),
            sy(r.y),
            ((u32::from(r.width) * u32::from(pw)) / u32::from(w)).min(65535) as u16,
            ((u32::from(r.height) * u32::from(ph)) / u32::from(h)).min(65535) as u16,
        )
    };
    visit(source, default, |op| {
        if let Operation::Text(pos, size, value, paint) = op {
            text.text(
                target,
                point(pos),
                ((u32::from(size) * u32::from(ph)) / u32::from(h)).clamp(1, 512) as u16,
                value,
                paint,
            );
            return;
        }
        let paint = match op {
            Operation::Rectangle(_, p)
            | Operation::Pixel(_, _, p)
            | Operation::Line(_, _, p)
            | Operation::Circle(_, _, p)
            | Operation::Icon(_, _, _, p) => p,
            _ => return,
        };
        let mut painted = Painted { target, paint };
        let mut canvas = Canvas::new(&mut painted);
        match op {
            Operation::Rectangle(bounds, _) => canvas.fill(rect(bounds.resolve(w, h)), paint.color),
            Operation::Pixel(x, y, _) => {
                let (x, y) = position((x, y), w, h);
                canvas.fill(rect(Rect::new(x, y, 1, 1)), paint.color);
            }
            Operation::Line(a, b, _) => canvas.line(point(a), point(b), paint.color),
            Operation::Icon(pos, size, icon, _) => {
                let (x, y) = position(pos, w, h);
                crate::icons::draw(
                    &mut canvas,
                    icon,
                    rect(Rect::new(x, y, size, size)),
                    paint.color,
                );
            }
            Operation::Circle(center, radius, _) => {
                let (cx, cy) = point(center);
                let mut x = ((u32::from(radius) * u32::from(ph)) / u32::from(h)).min(65535) as i32;
                let mut y = 0;
                let mut error = 1 - x;
                while x >= y {
                    let points = [
                        (x, y),
                        (y, x),
                        (-y, x),
                        (-x, y),
                        (-x, -y),
                        (-y, -x),
                        (y, -x),
                        (x, -y),
                    ];
                    for (index, (dx, dy)) in points.iter().copied().enumerate() {
                        if points[..index].contains(&(dx, dy)) {
                            continue;
                        }
                        canvas.pixel(cx.saturating_add(dx), cy.saturating_add(dy), paint.color);
                    }
                    y += 1;
                    if error < 0 {
                        error += 2 * y + 1;
                    } else {
                        x -= 1;
                        error += 2 * (y - x) + 1;
                    }
                }
            }
            _ => {}
        }
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{paint::ReadTarget, Rgb565};
    struct NoText;
    #[test]
    fn native_density_preserves_layout() {
        let mut pixels = [0; 64];
        let mut target = Rgb565::new(&mut pixels, 8, 8, 8).unwrap();
        draw_in_viewport(
            "r 1,1 *,1 white",
            &mut target,
            &mut NoText,
            Paint::solid(Color(255, 255, 255)),
            (4, 4),
        )
        .unwrap();
        for y in 0..8 {
            for x in 0..8 {
                assert_eq!(
                    target.read(x, y) != Color(0, 0, 0),
                    x >= 2 && (y == 2 || y == 3)
                );
            }
        }
    }
    #[test]
    fn circle_symmetry_does_not_stack_alpha() {
        let mut pixels = [0; 25];
        let mut target = Rgb565::new(&mut pixels, 5, 5, 5).unwrap();
        draw(
            "c 2,2 0 red/20",
            &mut target,
            &mut NoText,
            Paint::solid(Color(0, 0, 0)),
        )
        .unwrap();
        assert!((48..=56).contains(&target.read(2, 2).0));
    }
    impl<T: ReadTarget> TextRenderer<T> for NoText {
        fn text(&mut self, _: &mut T, _: (i32, i32), _: u16, _: &str, _: Paint) {}
    }
    #[test]
    fn inherited_colors_are_scoped() {
        let mut colors = [Paint::solid(Color(0, 0, 0)); 3];
        let mut n = 0;
        visit(
            "blue/75[r 0,0 *,2; red/20[p 1,1]; p 2,1]",
            Paint::solid(Color(255, 255, 255)),
            |o| {
                if let Operation::Rectangle(_, c) | Operation::Pixel(_, _, c) = o {
                    colors[n] = c;
                    n += 1;
                }
            },
        )
        .unwrap();
        assert_eq!(colors[0], Paint::parse("blue/75").unwrap());
        assert_eq!(colors[1], Paint::parse("red/20").unwrap());
        assert_eq!(colors[2], colors[0]);
    }
    #[test]
    fn remaining_size_and_alpha_render() {
        for width in [8, 16] {
            let mut pixels = [0u16; 64];
            let mut target = Rgb565::new(&mut pixels, width, 4, usize::from(width)).unwrap();
            draw(
                "r 2,0 *,2 red/20",
                &mut target,
                &mut NoText,
                Paint::solid(Color(255, 255, 255)),
            )
            .unwrap();
            assert_eq!(target.read(0, 0), Color(0, 0, 0));
            assert!(target.read(width - 1, 0).0 > 0);
            assert_eq!(target.read(width - 1, 3), Color(0, 0, 0));
        }
    }
    #[test]
    fn rejects_bad_input_before_drawing() {
        let mut pixels = [0; 4];
        let mut target = Rgb565::new(&mut pixels, 2, 2, 2).unwrap();
        assert!(draw(
            "r 0,0 *,* red; r bad",
            &mut target,
            &mut NoText,
            Paint::solid(Color(0, 0, 0))
        )
        .is_err());
        assert_eq!(pixels, [0; 4]);
        for bad in [
            "blue/101[r 0,0 1,1]",
            "blue[p 0,0",
            "i 0,0 unknown",
            "t 0,0 0 \"x\"",
            "r 0,0 200%,4",
        ] {
            assert!(validate(bad).is_err(), "{bad}");
        }
    }
}
