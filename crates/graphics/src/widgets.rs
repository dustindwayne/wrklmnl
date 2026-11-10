//! Optional-use, allocation-free controls for small displays. The application owns values and actions.
use crate::{
    paint::{Paint, Painted, ReadTarget},
    program::{Bounds, TextRenderer},
    Canvas,
};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind<'a> {
    Label(&'a str),
    Button(&'a str),
    Input(&'a str),
    Progress(u8),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Widget<'a> {
    pub id: &'a str,
    pub bounds: Bounds,
    pub kind: Kind<'a>,
    pub enabled: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    Next,
    Previous,
    Activate,
    PointerDown(u16, u16),
    PointerUp(u16, u16),
    Character(char),
    Backspace,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action<'a> {
    Activate(&'a str),
    Insert(&'a str, char),
    Backspace(&'a str),
}
#[derive(Default, Debug)]
pub struct State {
    pub focused: Option<usize>,
    pressed: Option<usize>,
}
fn interactive(widget: &Widget<'_>) -> bool {
    widget.enabled && matches!(widget.kind, Kind::Button(_) | Kind::Input(_))
}
impl State {
    pub fn event<'a>(
        &mut self,
        widgets: &[Widget<'a>],
        viewport: (u16, u16),
        event: Event,
    ) -> Option<Action<'a>> {
        if self
            .focused
            .is_some_and(|i| widgets.get(i).is_none_or(|w| !interactive(w)))
        {
            self.focused = None;
        }
        match event {
            Event::Next | Event::Previous => {
                let n = widgets.len();
                if n == 0 {
                    return None;
                }
                let reverse = event == Event::Previous;
                let start = self.focused.unwrap_or(if reverse { 0 } else { n - 1 });
                for offset in 1..=n {
                    let i = if reverse {
                        (start + n - offset % n) % n
                    } else {
                        (start + offset) % n
                    };
                    if interactive(&widgets[i]) {
                        self.focused = Some(i);
                        break;
                    }
                }
            }
            Event::PointerDown(x, y) | Event::PointerUp(x, y) => {
                let hit = widgets
                    .iter()
                    .enumerate()
                    .rev()
                    .find(|(_, w)| {
                        if !interactive(w) {
                            return false;
                        }
                        let r = w.bounds.resolve(viewport.0, viewport.1);
                        let (x, y) = (i64::from(x), i64::from(y));
                        x >= i64::from(r.x)
                            && y >= i64::from(r.y)
                            && x < i64::from(r.x) + i64::from(r.width)
                            && y < i64::from(r.y) + i64::from(r.height)
                    })
                    .map(|(i, _)| i);
                if matches!(event, Event::PointerDown(..)) {
                    self.pressed = hit;
                    self.focused = hit;
                } else {
                    let pressed = self.pressed.take();
                    if hit.is_some() && pressed == hit {
                        let i = hit.unwrap();
                        self.focused = Some(i);
                        if matches!(widgets[i].kind, Kind::Button(_)) {
                            return Some(Action::Activate(widgets[i].id));
                        }
                    }
                }
            }
            Event::Activate => {
                if let Some(i) = self.focused {
                    if matches!(widgets[i].kind, Kind::Button(_)) {
                        return Some(Action::Activate(widgets[i].id));
                    }
                }
            }
            Event::Character(ch) => {
                if let Some(i) = self.focused {
                    if matches!(widgets[i].kind, Kind::Input(_)) && !ch.is_control() {
                        return Some(Action::Insert(widgets[i].id, ch));
                    }
                }
            }
            Event::Backspace => {
                if let Some(i) = self.focused {
                    if matches!(widgets[i].kind, Kind::Input(_)) {
                        return Some(Action::Backspace(widgets[i].id));
                    }
                }
            }
        }
        None
    }
    pub fn draw<T: ReadTarget>(
        &self,
        widgets: &[Widget<'_>],
        target: &mut T,
        text: &mut impl TextRenderer<T>,
        foreground: Paint,
        accent: Paint,
        text_size: u16,
    ) {
        let (w, h) = target.size();
        for (index, widget) in widgets.iter().enumerate() {
            let bounds = widget.bounds.resolve(w, h);
            let paint = if widget.enabled {
                foreground
            } else {
                Paint {
                    opacity: foreground.opacity / 2,
                    ..foreground
                }
            };
            if self.focused == Some(index) {
                let tint = Paint {
                    opacity: 40,
                    ..accent
                };
                let mut p = Painted {
                    target,
                    paint: tint,
                };
                Canvas::new(&mut p).fill(bounds, tint.color);
            }
            match widget.kind {
                Kind::Label(value) | Kind::Input(value) | Kind::Button(value) => text.text(
                    target,
                    (bounds.x.saturating_add(4), bounds.y.saturating_add(4)),
                    text_size,
                    value,
                    if matches!(widget.kind, Kind::Button(_)) {
                        accent
                    } else {
                        paint
                    },
                ),
                Kind::Progress(value) => {
                    let mut bar = bounds;
                    let mut p = Painted {
                        target,
                        paint: Paint {
                            opacity: 32,
                            ..foreground
                        },
                    };
                    Canvas::new(&mut p).fill(bar, foreground.color);
                    bar.width = (u32::from(bar.width) * u32::from(value.min(100)) / 100) as u16;
                    let mut p = Painted {
                        target,
                        paint: accent,
                    };
                    Canvas::new(&mut p).fill(bar, accent.color);
                }
            }
        }
    }
}
/// Caller-owned UTF-8 input storage. Insertion is append-only; Backspace removes one Unicode scalar.
pub struct TextBuffer<const N: usize> {
    bytes: [u8; N],
    len: usize,
}
impl<const N: usize> Default for TextBuffer<N> {
    fn default() -> Self {
        Self {
            bytes: [0; N],
            len: 0,
        }
    }
}
impl<const N: usize> TextBuffer<N> {
    pub fn as_str(&self) -> &str {
        core::str::from_utf8(&self.bytes[..self.len]).unwrap_or("")
    }
    pub fn push(&mut self, ch: char) -> bool {
        let mut buffer = [0; 4];
        let encoded = ch.encode_utf8(&mut buffer);
        if self.len + encoded.len() > N {
            return false;
        }
        self.bytes[self.len..self.len + encoded.len()].copy_from_slice(encoded.as_bytes());
        self.len += encoded.len();
        true
    }
    pub fn backspace(&mut self) {
        if let Some((at, _)) = self.as_str().char_indices().last() {
            self.len = at;
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::Length;
    fn bounds() -> Bounds {
        Bounds {
            x: 0,
            y: 0,
            width: Length::Pixels(32),
            height: Length::Pixels(16),
        }
    }
    #[test]
    fn focus_and_activation_use_controls_only() {
        let widgets = [
            Widget {
                id: "label",
                bounds: bounds(),
                kind: Kind::Label("Label"),
                enabled: true,
            },
            Widget {
                id: "go",
                bounds: bounds(),
                kind: Kind::Button("Go"),
                enabled: true,
            },
            Widget {
                id: "disabled",
                bounds: bounds(),
                kind: Kind::Button("No"),
                enabled: false,
            },
        ];
        let mut state = State::default();
        state.event(&widgets, (128, 64), Event::Next);
        assert_eq!(state.focused, Some(1));
        assert_eq!(
            state.event(&widgets, (128, 64), Event::Activate),
            Some(Action::Activate("go"))
        );
        state.event(&widgets, (128, 64), Event::PointerDown(2, 2));
        assert_eq!(
            state.event(&widgets, (128, 64), Event::PointerUp(90, 60)),
            None
        );
    }
    #[test]
    fn fixed_input_keeps_utf8_and_capacity() {
        let mut b = TextBuffer::<5>::default();
        assert!(b.push('é'));
        assert!(b.push('猫'));
        assert!(!b.push('x'));
        b.backspace();
        assert_eq!(b.as_str(), "é");
        b.backspace();
        assert_eq!(b.as_str(), "");
    }
}
