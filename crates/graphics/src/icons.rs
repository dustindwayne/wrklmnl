//! Built-in vector symbols. No icon font or image assets are required.
use crate::{Canvas, Color, Rect, Target};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Icon {
    Close,
    Plus,
    Minus,
    Maximize,
    ChevronRight,
    ChevronDown,
    Terminal,
}
pub fn draw<T: Target>(canvas: &mut Canvas<'_, T>, icon: Icon, bounds: Rect, color: Color) {
    if bounds.width < 2 || bounds.height < 2 {
        return;
    }
    let x = bounds.x;
    let y = bounds.y;
    let right = x.saturating_add(i32::from(bounds.width) - 1);
    let bottom = y.saturating_add(i32::from(bounds.height) - 1);
    let mid_x = x.saturating_add(i32::from(bounds.width) / 2);
    let mid_y = y.saturating_add(i32::from(bounds.height) / 2);
    match icon {
        Icon::Close => {
            canvas.line((x, y), (right, bottom), color);
            canvas.line((right, y), (x, bottom), color);
        }
        Icon::Plus => {
            canvas.line((x, mid_y), (right, mid_y), color);
            canvas.line((mid_x, y), (mid_x, bottom), color);
        }
        Icon::Minus => canvas.line((x, mid_y), (right, mid_y), color),
        Icon::Maximize => {
            canvas.line((x, y), (right, y), color);
            canvas.line((x, bottom), (right, bottom), color);
            canvas.line((x, y), (x, bottom), color);
            canvas.line((right, y), (right, bottom), color);
        }
        Icon::ChevronRight => {
            canvas.line((x, y), (right, mid_y), color);
            canvas.line((right, mid_y), (x, bottom), color);
        }
        Icon::ChevronDown => {
            canvas.line((x, y), (mid_x, bottom), color);
            canvas.line((mid_x, bottom), (right, y), color);
        }
        Icon::Terminal => {
            canvas.line((x, y), (mid_x, mid_y), color);
            canvas.line((mid_x, mid_y), (x, bottom), color);
            canvas.line((mid_x, bottom), (right, bottom), color);
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::Rgb565;
    #[test]
    fn icons_respect_clip() {
        let mut data = [0; 16];
        {
            let mut target = Rgb565::new(&mut data, 4, 4, 4).unwrap();
            let mut canvas = Canvas::new(&mut target);
            draw(
                &mut canvas,
                Icon::Close,
                Rect::new(-2, -2, 8, 8),
                Color(255, 255, 255),
            );
        }
        assert_eq!(data.iter().filter(|&&p| p != 0).count(), 8);
    }
}
