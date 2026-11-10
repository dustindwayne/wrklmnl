use crate::session::Session;
use eframe::egui::{self, PointerButton, Rect, Vec2};
#[derive(Default)]
pub struct State {
    pub line: crate::terminal_command::Line,
    pub completion: crate::completion::Completion,
    pub selection: Option<((u16, u16), (u16, u16))>,
    pub active: bool,
    pub focus_requested: bool,
    scroll_delta: f32,
    drag_anchor: Option<(u16, u16)>,
    last_mouse: Option<(u16, u16)>,
}
impl State {
    pub fn selected(&self, screen: &vt100::Screen) -> Option<String> {
        let (a, b) = self.selection?;
        let (start, end) = if a <= b { (a, b) } else { (b, a) };
        Some(screen.contents_between(start.0, start.1, end.0, end.1))
    }
    pub fn contains(&self, row: u16, col: u16) -> bool {
        self.selection.is_some_and(|(a, b)| {
            let (a, b) = if a <= b { (a, b) } else { (b, a) };
            (row, col) >= a && (row, col) < b
        })
    }
}
pub fn pointer(
    ui: &mut egui::Ui,
    session: &mut Session,
    rect: Rect,
    cell: Vec2,
    response: &egui::Response,
) -> anyhow::Result<()> {
    let (rows, cols) = session.parser.screen().size();
    let at = |pos: egui::Pos2| {
        (
            ((pos.y - rect.min.y) / cell.y)
                .floor()
                .clamp(0.0, (rows - 1) as f32) as u16,
            ((pos.x - rect.min.x) / cell.x)
                .floor()
                .clamp(0.0, (cols - 1) as f32) as u16,
        )
    };
    let mode = session.parser.screen().mouse_protocol_mode();
    let encoding = session.parser.screen().mouse_protocol_encoding();
    let report = mode != vt100::MouseProtocolMode::None && !ui.input(|i| i.modifiers.shift);
    for event in ui.input(|i| i.events.clone()) {
        match event {
            egui::Event::PointerButton {
                pos,
                button,
                pressed,
                modifiers,
            } if rect.contains(pos) && report => {
                if !pressed && mode == vt100::MouseProtocolMode::Press {
                    continue;
                }
                let (r, c) = at(pos);
                let b = match button {
                    PointerButton::Primary => 0,
                    PointerButton::Middle => 1,
                    PointerButton::Secondary => 2,
                    _ => continue,
                };
                let b = b
                    + 4 * u8::from(modifiers.shift)
                    + 8 * u8::from(modifiers.alt)
                    + 16 * u8::from(modifiers.ctrl);
                session.send(&crate::input::mouse(b, c, r, !pressed, encoding))?;
            }
            egui::Event::PointerMoved(pos) if rect.contains(pos) && report => {
                let (r, c) = at(pos);
                let buttons = ui.input(|i| {
                    (
                        i.pointer.primary_down(),
                        i.pointer.secondary_down(),
                        i.pointer.middle_down(),
                    )
                });
                let held = buttons.0 || buttons.1 || buttons.2;
                if session.interaction.last_mouse != Some((r, c))
                    && (mode == vt100::MouseProtocolMode::AnyMotion
                        || (held && mode == vt100::MouseProtocolMode::ButtonMotion))
                {
                    let b = 32
                        + if buttons.0 {
                            0
                        } else if buttons.2 {
                            1
                        } else if buttons.1 {
                            2
                        } else {
                            3
                        };
                    session.send(&crate::input::mouse(b, c, r, false, encoding))?;
                }
                session.interaction.last_mouse = Some((r, c));
            }
            _ => {}
        }
    }
    if response.hovered() {
        session.interaction.scroll_delta += ui.input(|i| i.smooth_scroll_delta.y);
        let lines = (session.interaction.scroll_delta / 15.0).trunc();
        let delta = lines * 15.0;
        session.interaction.scroll_delta -= delta;
        if delta.abs() > 0.0 {
            if report {
                for _ in 0..((delta.abs() / 20.0).ceil() as usize).clamp(1, 6) {
                    if let Some(pos) = ui.input(|i| i.pointer.hover_pos()) {
                        let (r, c) = at(pos);
                        session.send(&crate::input::mouse(
                            if delta > 0.0 { 64 } else { 65 },
                            c,
                            r,
                            false,
                            encoding,
                        ))?;
                    }
                }
            } else {
                if session.parser.screen().alternate_screen() {
                    let key = if delta > 0.0 {
                        egui::Key::ArrowUp
                    } else {
                        egui::Key::ArrowDown
                    };
                    for _ in 0..lines.abs().min(20.0) as usize {
                        session.send(&crate::input::key(
                            key,
                            egui::Modifiers::NONE,
                            session.parser.screen().application_cursor(),
                        ))?;
                    }
                }
                let current = session.parser.screen().scrollback();
                session
                    .parser
                    .screen_mut()
                    .set_scrollback((current as isize + (delta / 15.0) as isize).max(0) as usize);
                session.interaction.selection = None;
            }
        }
    }
    if !report {
        if response.drag_started() {
            if let Some(origin) = ui.input(|i| i.pointer.press_origin()) {
                let p = at(origin);
                session.interaction.drag_anchor = Some(p);
                session.interaction.selection = Some((p, (p.0, (p.1 + 1).min(cols))));
            }
        }
        if response.dragged() {
            if let Some(pos) = response.interact_pointer_pos() {
                let p = at(pos);
                if let Some(anchor) = session.interaction.drag_anchor {
                    let (a, b) = if anchor <= p {
                        (anchor, p)
                    } else {
                        (p, anchor)
                    };
                    session.interaction.selection = Some((a, (b.0, (b.1 + 1).min(cols))));
                }
            }
        }
        if response.clicked() && !response.double_clicked() {
            session.interaction.selection = None;
        }
        if response.double_clicked() {
            if let Some(pos) = response.interact_pointer_pos() {
                let (r, c) = at(pos);
                let is_word = |col| {
                    session.parser.screen().cell(r, col).is_some_and(|v| {
                        v.contents()
                            .chars()
                            .any(|c| c.is_alphanumeric() || "_./\\:-".contains(c))
                    })
                };
                let mut start = c;
                let mut end = c + 1;
                while start > 0 && is_word(start - 1) {
                    start -= 1;
                }
                while end < cols && is_word(end) {
                    end += 1;
                }
                session.interaction.selection = Some(((r, start), (r, end)));
            }
        }
        if response.triple_clicked() {
            if let Some(pos) = response.interact_pointer_pos() {
                let (r, _) = at(pos);
                session.interaction.selection = Some(((r, 0), (r, cols)));
            }
        }
        if response.clicked() && ui.input(|i| i.modifiers.ctrl) {
            if let Some(pos) = response.interact_pointer_pos() {
                let (r, c) = at(pos);
                let line = session
                    .parser
                    .screen()
                    .rows(0, cols)
                    .nth(r as usize)
                    .unwrap_or_default();
                let cursor = session
                    .parser
                    .screen()
                    .contents_between(r, 0, r, c)
                    .len()
                    .min(line.len());
                let a = line[..cursor]
                    .char_indices()
                    .rev()
                    .find(|(_, c)| c.is_whitespace())
                    .map(|(i, c)| i + c.len_utf8())
                    .unwrap_or(0);
                let b = line[cursor..]
                    .find(char::is_whitespace)
                    .map(|i| i + cursor)
                    .unwrap_or(line.len());
                let token = line[a..b].trim_matches(['\'', '"', '(', ')', '[', ']', ',', ';']);
                if token.starts_with("https://") || token.starts_with("http://") {
                    ui.ctx().open_url(egui::OpenUrl::new_tab(token));
                } else {
                    let mut path = token.to_string();
                    while path.rsplit_once(':').is_some_and(|(_, tail)| {
                        !tail.is_empty() && tail.chars().all(|c| c.is_ascii_digit())
                    }) {
                        path = path.rsplit_once(':').unwrap().0.into();
                    }
                    let path = std::path::Path::new(&path);
                    let path = if path.is_absolute() {
                        path.to_path_buf()
                    } else {
                        session.cwd.join(path)
                    };
                    if path.exists() {
                        if !path.is_dir() && !safe_document(&path) {
                            anyhow::bail!("Opening executable or unrecognized files from terminal output is disabled; use the editor or an explicit shell command");
                        }
                        let mut command = std::process::Command::new(if cfg!(windows) {
                            "explorer.exe"
                        } else if cfg!(target_os = "macos") {
                            "open"
                        } else {
                            "xdg-open"
                        });
                        command.arg(path).spawn()?;
                    }
                }
            }
        }
    }
    Ok(())
}
fn safe_document(path: &std::path::Path) -> bool {
    path.extension().and_then(|s| s.to_str()).is_some_and(|s| {
        [
            "txt", "md", "rs", "py", "json", "toml", "yaml", "yml", "log", "csv", "png", "jpg",
            "jpeg", "pdf",
        ]
        .contains(&s.to_ascii_lowercase().as_str())
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn selection_copies_wide_characters() {
        let mut p = vt100::Parser::new(5, 30, 10);
        p.process("hello 世界\r\nsecond line".as_bytes());
        let s = State {
            selection: Some(((0, 0), (1, 6))),
            ..Default::default()
        };
        assert_eq!(s.selected(p.screen()).unwrap(), "hello 世界\nsecond");
        assert!(s.contains(0, 3));
        assert!(!s.contains(1, 6));
    }
}
