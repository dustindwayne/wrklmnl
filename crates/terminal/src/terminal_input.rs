use eframe::egui::{self, Event, Key};
pub enum Input {
    Bytes(Vec<u8>),
    Copy,
    Scroll(isize),
}
pub fn translate(
    events: &[Event],
    modifiers: egui::Modifiers,
    selection: bool,
    app_cursor: bool,
    bracketed: bool,
) -> Vec<Input> {
    let altgr =
        modifiers.ctrl && modifiers.alt && events.iter().any(|e| matches!(e, Event::Text(_)));
    let mut result = vec![];
    let mut selected = selection;
    for event in events {
        let bytes = match event {
            Event::Copy => {
                if selected || modifiers.shift || modifiers.mac_cmd {
                    result.push(Input::Copy);
                    selected = false;
                    continue;
                } else {
                    vec![3]
                }
            }
            Event::Cut => {
                if selected || modifiers.mac_cmd {
                    result.push(Input::Copy);
                    selected = false;
                    continue;
                } else {
                    vec![24]
                }
            }
            Event::Text(s) => {
                if modifiers.alt && !altgr {
                    [vec![27], s.as_bytes().to_vec()].concat()
                } else {
                    s.as_bytes().to_vec()
                }
            }
            Event::Paste(s) => {
                // Escape bytes cannot terminate bracketed paste or inject terminal control sequences.
                let clean = s.replace('\x1b', "");
                if bracketed {
                    format!("\x1b[200~{clean}\x1b[201~").into_bytes()
                } else {
                    clean.into_bytes()
                }
            }
            Event::Key {
                key,
                pressed: true,
                modifiers,
                ..
            } => {
                if (*key == Key::V && (modifiers.ctrl || modifiers.mac_cmd))
                    || (*key == Key::Insert && modifiers.shift)
                    || altgr
                {
                    continue;
                }
                if modifiers.shift && matches!(key, Key::PageUp | Key::PageDown) {
                    result.push(Input::Scroll(if *key == Key::PageUp { 1 } else { -1 }));
                    continue;
                }
                if *key == Key::C && modifiers.ctrl && selected {
                    result.push(Input::Copy);
                    selected = false;
                    continue;
                }
                crate::input::key(*key, *modifiers, app_cursor)
            }
            _ => vec![],
        };
        if !bytes.is_empty() {
            result.push(Input::Bytes(bytes));
        }
    }
    result
}
#[cfg(test)]
mod tests {
    use super::*;
    fn bytes(input: Vec<Input>) -> Vec<u8> {
        input
            .into_iter()
            .flat_map(|i| match i {
                Input::Bytes(b) => b,
                _ => vec![],
            })
            .collect()
    }
    #[test]
    fn clipboard_events_preserve_interrupts() {
        assert_eq!(
            bytes(translate(
                &[Event::Copy],
                egui::Modifiers::CTRL,
                false,
                false,
                false
            )),
            vec![3]
        );
        assert!(matches!(
            translate(&[Event::Copy], egui::Modifiers::CTRL, true, false, false)[0],
            Input::Copy
        ));
        assert_eq!(
            bytes(translate(
                &[Event::Cut],
                egui::Modifiers::CTRL,
                false,
                false,
                false
            )),
            vec![24]
        );
    }
    #[test]
    fn paste_has_no_control_prefix_and_cannot_escape() {
        let events = vec![
            Event::Key {
                key: Key::V,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::CTRL,
            },
            Event::Paste("hi\x1b[201~there".into()),
        ];
        assert_eq!(
            bytes(translate(
                &events,
                egui::Modifiers::CTRL,
                false,
                false,
                true
            )),
            b"\x1b[200~hi[201~there\x1b[201~"
        );
    }
    #[test]
    fn altgr_is_text() {
        let mut m = egui::Modifiers::CTRL;
        m.alt = true;
        assert_eq!(
            bytes(translate(
                &[
                    Event::Key {
                        key: Key::Q,
                        physical_key: None,
                        pressed: true,
                        repeat: false,
                        modifiers: m
                    },
                    Event::Text("@".into())
                ],
                m,
                false,
                false,
                false
            )),
            b"@"
        );
    }
}
