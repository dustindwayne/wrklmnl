//! Responsive geometry independent of the drawing backend and authoring syntax.
use crate::Rect;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Length {
    Pixels(u16),
    Remaining,
    Percent(u8),
}
impl Length {
    pub fn resolve(self, available: u16) -> u16 {
        match self {
            Self::Pixels(n) => n.min(available),
            Self::Remaining => available,
            Self::Percent(p) => (u32::from(available) * u32::from(p.min(100)) / 100) as u16,
        }
    }
}
/// Insets consume space rather than scaling the content. Remaining dimensions grow with the viewport.
pub fn anchored(
    viewport: Rect,
    left: u16,
    top: u16,
    right: u16,
    bottom: u16,
    width: Length,
    height: Length,
) -> Rect {
    let available_width = viewport.width.saturating_sub(left).saturating_sub(right);
    let available_height = viewport.height.saturating_sub(top).saturating_sub(bottom);
    Rect::new(
        viewport
            .x
            .saturating_add(i32::from(left.min(viewport.width))),
        viewport
            .y
            .saturating_add(i32::from(top.min(viewport.height))),
        width.resolve(available_width),
        height.resolve(available_height),
    )
}
/// Tracks damage without allocation. Call `take` after sending the changed region to the display.
#[derive(Default, Debug)]
pub struct Damage {
    bounds: Option<Rect>,
}
impl Damage {
    pub fn add(&mut self, rect: Rect) {
        if rect.width == 0 || rect.height == 0 {
            return;
        }
        self.bounds = Some(match self.bounds {
            None => rect,
            Some(old) => {
                let x = old.x.min(rect.x);
                let y = old.y.min(rect.y);
                let right = (i64::from(old.x) + i64::from(old.width))
                    .max(i64::from(rect.x) + i64::from(rect.width));
                let bottom = (i64::from(old.y) + i64::from(old.height))
                    .max(i64::from(rect.y) + i64::from(rect.height));
                // Damage coordinates are display coordinates. Out-of-range unions fall back to the full supported extent.
                if right - i64::from(x) > i64::from(u16::MAX)
                    || bottom - i64::from(y) > i64::from(u16::MAX)
                {
                    Rect::new(0, 0, u16::MAX, u16::MAX)
                } else {
                    Rect::new(
                        x,
                        y,
                        (right - i64::from(x)) as u16,
                        (bottom - i64::from(y)) as u16,
                    )
                }
            }
        });
    }
    pub fn take(&mut self) -> Option<Rect> {
        self.bounds.take()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn remaining_width_adapts_without_changing_height() {
        for width in [128, 320, 1920] {
            let r = anchored(
                Rect::new(0, 0, width, 64),
                8,
                4,
                8,
                4,
                Length::Remaining,
                Length::Pixels(24),
            );
            assert_eq!(r.width, width - 16);
            assert_eq!(r.height, 24);
        }
    }
    #[test]
    fn cramped_viewport_saturates() {
        assert_eq!(
            anchored(
                Rect::new(0, 0, 8, 8),
                12,
                12,
                12,
                12,
                Length::Remaining,
                Length::Remaining
            ),
            Rect::new(8, 8, 0, 0)
        );
    }
    #[test]
    fn damage_merges_and_resets() {
        let mut d = Damage::default();
        d.add(Rect::new(2, 3, 4, 5));
        d.add(Rect::new(4, 1, 8, 2));
        assert_eq!(d.take(), Some(Rect::new(2, 1, 10, 7)));
        assert_eq!(d.take(), None);
    }
}
