use eframe::egui::{Key, Modifiers};
pub fn key(key: Key, m: Modifiers, application: bool) -> Vec<u8> {
    let modifier = 1
        + u8::from(m.shift)
        + 2 * u8::from(m.alt)
        + 4 * u8::from(m.ctrl)
        + 8 * u8::from(m.mac_cmd);
    if m.ctrl && key >= Key::A && key <= Key::Z {
        return vec![key as u8 - Key::A as u8 + 1];
    }
    let cursor = match key {
        Key::ArrowUp => Some('A'),
        Key::ArrowDown => Some('B'),
        Key::ArrowRight => Some('C'),
        Key::ArrowLeft => Some('D'),
        Key::Home => Some('H'),
        Key::End => Some('F'),
        _ => None,
    };
    if let Some(c) = cursor {
        return if modifier > 1 {
            format!("\x1b[1;{modifier}{c}").into_bytes()
        } else if application {
            format!("\x1bO{c}").into_bytes()
        } else {
            format!("\x1b[{c}").into_bytes()
        };
    }
    let f = match key {
        Key::F1 => Some(1),
        Key::F2 => Some(2),
        Key::F3 => Some(3),
        Key::F4 => Some(4),
        Key::F5 => Some(5),
        Key::F6 => Some(6),
        Key::F7 => Some(7),
        Key::F8 => Some(8),
        Key::F9 => Some(9),
        Key::F10 => Some(10),
        Key::F11 => Some(11),
        Key::F12 => Some(12),
        Key::F13 => Some(13),
        Key::F14 => Some(14),
        Key::F15 => Some(15),
        Key::F16 => Some(16),
        Key::F17 => Some(17),
        Key::F18 => Some(18),
        Key::F19 => Some(19),
        Key::F20 => Some(20),
        _ => None,
    };
    if let Some(f) = f {
        if f <= 4 {
            let c = (b'P' + f - 1) as char;
            return if modifier == 1 {
                format!("\x1bO{c}").into_bytes()
            } else {
                format!("\x1b[1;{modifier}{c}").into_bytes()
            };
        }
        let n = [
            15, 17, 18, 19, 20, 21, 23, 24, 25, 26, 28, 29, 31, 32, 33, 34,
        ][(f - 5) as usize];
        return if modifier == 1 {
            format!("\x1b[{n}~").into_bytes()
        } else {
            format!("\x1b[{n};{modifier}~").into_bytes()
        };
    }
    let seq = match key {
        Key::Enter => vec![13],
        Key::Tab if m.shift => b"\x1b[Z".to_vec(),
        Key::Tab => vec![9],
        Key::Escape => vec![27],
        Key::Backspace if m.ctrl => vec![23],
        Key::Backspace => vec![127],
        Key::Space if m.ctrl => vec![0],
        Key::Insert => b"\x1b[2~".to_vec(),
        Key::Delete => b"\x1b[3~".to_vec(),
        Key::PageUp => b"\x1b[5~".to_vec(),
        Key::PageDown => b"\x1b[6~".to_vec(),
        _ => vec![],
    };
    if modifier > 1 && matches!(key, Key::Insert | Key::Delete | Key::PageUp | Key::PageDown) {
        let n = match key {
            Key::Insert => 2,
            Key::Delete => 3,
            Key::PageUp => 5,
            _ => 6,
        };
        return format!("\x1b[{n};{modifier}~").into_bytes();
    }
    if m.alt && !seq.is_empty() {
        [vec![27], seq].concat()
    } else {
        seq
    }
}
pub fn mouse(
    button: u8,
    col: u16,
    row: u16,
    release: bool,
    encoding: vt100::MouseProtocolEncoding,
) -> Vec<u8> {
    match encoding {
        vt100::MouseProtocolEncoding::Sgr => format!(
            "\x1b[<{button};{};{}{}",
            col + 1,
            row + 1,
            if release { 'm' } else { 'M' }
        )
        .into_bytes(),
        vt100::MouseProtocolEncoding::Utf8 => {
            let mut s = "\x1b[M".to_string();
            for v in [
                u32::from(if release { 3 } else { button }) + 32,
                u32::from(col) + 33,
                u32::from(row) + 33,
            ] {
                if let Some(c) = char::from_u32(v) {
                    s.push(c);
                }
            }
            s.into_bytes()
        }
        _ => vec![
            27,
            b'[',
            b'M',
            if release {
                35
            } else {
                button.saturating_add(32)
            },
            (col + 33).min(255) as u8,
            (row + 33).min(255) as u8,
        ],
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn keys() {
        assert_eq!(key(Key::F12, Modifiers::NONE, false), b"\x1b[24~");
        assert_eq!(key(Key::ArrowUp, Modifiers::CTRL, false), b"\x1b[1;5A");
        assert_eq!(key(Key::Tab, Modifiers::SHIFT, false), b"\x1b[Z");
    }
    #[test]
    fn mouse_sgr() {
        assert_eq!(
            mouse(0, 5, 3, false, vt100::MouseProtocolEncoding::Sgr),
            b"\x1b[<0;6;4M"
        );
        assert_eq!(
            mouse(0, 5, 3, true, vt100::MouseProtocolEncoding::Sgr),
            b"\x1b[<0;6;4m"
        );
    }
}
