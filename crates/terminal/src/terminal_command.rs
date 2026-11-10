//! Completion and syntax operate on the live terminal line; there is no second input widget.
use eframe::egui::{self, Event, Key};
#[derive(Default)]
pub struct Line {
    pub text: String,
    pub cursor: usize,
    pub anchor: Option<(u16, u16)>,
    pub valid: bool,
    pub syntax: bool,
}
impl Line {
    pub fn observe(&mut self, event: &Event, anchor: (u16, u16)) {
        if self.anchor.is_none() {
            self.anchor = Some(anchor);
            self.valid = true;
        }
        if matches!(
            event,
            Event::Key {
                key: Key::Enter,
                pressed: true,
                ..
            }
        ) {
            *self = Self {
                syntax: self.syntax,
                ..Self::default()
            };
            return;
        }
        if !self.valid {
            return;
        }
        match event {
            Event::Text(s) | Event::Paste(s) if !s.contains(['\r', '\n']) => {
                self.text.insert_str(self.cursor, s);
                self.cursor += s.len();
            }
            Event::Key {
                key,
                pressed: true,
                modifiers,
                ..
            } => match key {
                Key::Enter => {
                    *self = Self {
                        syntax: self.syntax,
                        ..Self::default()
                    };
                }
                Key::ArrowUp | Key::ArrowDown | Key::Escape => self.valid = false,
                Key::ArrowLeft if !modifiers.ctrl => {
                    self.cursor = self.text[..self.cursor]
                        .char_indices()
                        .last()
                        .map(|(i, _)| i)
                        .unwrap_or(0);
                }
                Key::ArrowRight if !modifiers.ctrl => {
                    if self.cursor < self.text.len() {
                        self.cursor += self.text[self.cursor..].chars().next().unwrap().len_utf8();
                    }
                }
                Key::Home => self.cursor = 0,
                Key::End => self.cursor = self.text.len(),
                Key::Backspace if !modifiers.ctrl => {
                    if let Some((i, _)) = self.text[..self.cursor].char_indices().last() {
                        self.text.replace_range(i..self.cursor, "");
                        self.cursor = i;
                    }
                }
                Key::Delete if !modifiers.ctrl => {
                    if let Some(c) = self.text[self.cursor..].chars().next() {
                        self.text
                            .replace_range(self.cursor..self.cursor + c.len_utf8(), "");
                    }
                }
                Key::Tab => self.valid = false,
                _ if modifiers.ctrl && !modifiers.alt => self.valid = false,
                _ => {}
            },
            Event::Copy | Event::Cut => {
                *self = Self {
                    syntax: self.syntax,
                    ..Self::default()
                };
            }
            _ => {}
        }
    }
    pub fn replacement(&mut self, old: &str, new: &str, new_cursor: usize) -> Option<Vec<u8>> {
        if !self.valid
            || self.text != old
            || !old.is_char_boundary(self.cursor)
            || !new.is_char_boundary(new_cursor)
            || new.contains(['\r', '\n', '\x1b'])
        {
            return None;
        }
        let old_cursor = self.cursor;
        let mut prefix = 0;
        for (a, b) in old.chars().zip(new.chars()) {
            if a != b {
                break;
            }
            prefix += a.len_utf8();
        }
        if prefix > old_cursor {
            prefix = old_cursor;
        }
        let mut bytes = vec![];
        for _ in old[prefix..old_cursor].chars() {
            bytes.extend_from_slice(b"\x1b[D");
        }
        for _ in old[prefix..].chars() {
            bytes.extend_from_slice(b"\x1b[3~");
        }
        bytes.extend_from_slice(&new.as_bytes()[prefix..]);
        for _ in new[new_cursor..].chars() {
            bytes.extend_from_slice(b"\x1b[D");
        }
        self.text = new.into();
        self.cursor = new_cursor;
        Some(bytes)
    }
}
pub fn process(
    ui: &mut egui::Ui,
    session: &mut crate::session::Session,
    rect: egui::Rect,
    cell: egui::Vec2,
    events: &[Event],
) -> anyhow::Result<Vec<Event>> {
    let mut remaining = vec![];
    session.interaction.completion.set_context(ui.ctx());
    session.interaction.completion.poll();
    for event in events {
        let mut consumed = false;
        if let Event::Key {
            key,
            pressed: true,
            modifiers,
            ..
        } = event
        {
            let open = !session.interaction.completion.items.is_empty();
            if open {
                match key {
                    Key::ArrowDown => {
                        let c = &mut session.interaction.completion;
                        c.selected = (c.selected + 1) % c.items.len();
                        consumed = true;
                    }
                    Key::ArrowUp => {
                        let c = &mut session.interaction.completion;
                        c.selected = (c.selected + c.items.len() - 1) % c.items.len();
                        consumed = true;
                    }
                    Key::Escape => {
                        session.interaction.completion.dismiss();
                        consumed = true;
                    }
                    Key::Tab => {
                        accept(session)?;
                        consumed = true;
                    }
                    _ => {}
                }
            }
            if *key == Key::Space
                && modifiers.ctrl
                && session.interaction.line.valid
                && !session.parser.screen().alternate_screen()
            {
                let line = &session.interaction.line;
                session.interaction.completion.request(
                    &line.text,
                    line.cursor,
                    crate::completion::Context::Commands {
                        shell: session.shell.clone(),
                        cwd: session.cwd.clone(),
                        environment: session.environment.clone(),
                    },
                    true,
                );
                consumed = true;
            }
        }
        if !consumed {
            if !session.parser.screen().alternate_screen() {
                session
                    .interaction
                    .line
                    .observe(event, session.parser.screen().cursor_position());
            }
            if matches!(
                event,
                Event::Text(_) | Event::Paste(_) | Event::Key { pressed: true, .. }
            ) {
                session.interaction.completion.dismiss();
            }
            remaining.push(event.clone());
        }
    }
    if !session.interaction.completion.items.is_empty() {
        let (r, c) = session.parser.screen().cursor_position();
        let position = rect.min + egui::vec2(c as f32 * cell.x, (r + 1) as f32 * cell.y + 8.0);
        let position = egui::pos2(
            position.x.min(rect.right() - 240.0).max(rect.left()),
            position.y.min(rect.bottom() - 200.0).max(rect.top()),
        );
        let mut text = session.interaction.line.text.clone();
        let old = text.clone();
        egui::Area::new(ui.id().with("terminal_completion"))
            .order(egui::Order::Foreground)
            .fixed_pos(position)
            .show(ui.ctx(), |ui| {
                egui::Frame::NONE
                    .inner_margin(12.0)
                    .fill(ui.visuals().window_fill)
                    .show(ui, |ui| {
                        ui.set_max_width(480.0);
                        session.interaction.completion.choices(ui, &mut text);
                    });
            });
        if text != old {
            if let Some(cursor) = session.interaction.completion.applied_cursor.take() {
                if let Some(bytes) = session.interaction.line.replacement(&old, &text, cursor) {
                    session.send(&bytes)?;
                }
                session.interaction.focus_requested = true;
            }
        }
    }
    Ok(remaining)
}
fn accept(session: &mut crate::session::Session) -> anyhow::Result<()> {
    let old = session.interaction.line.text.clone();
    let mut text = old.clone();
    if session.interaction.completion.apply(&mut text) {
        if let Some(cursor) = session.interaction.completion.applied_cursor.take() {
            if let Some(bytes) = session.interaction.line.replacement(&old, &text, cursor) {
                session.send(&bytes)?;
            }
        }
    }
    Ok(())
}
pub fn colors(
    session: &crate::session::Session,
    light: bool,
    font_size: f32,
) -> std::collections::HashMap<(u16, u16), egui::Color32> {
    let mut colors = std::collections::HashMap::new();
    let line = &session.interaction.line;
    if !line.syntax || !line.valid || session.parser.screen().alternate_screen() {
        return colors;
    }
    let Some((mut row, mut col)) = line.anchor else {
        return colors;
    };
    let language = if session.shell.to_lowercase().contains("pwsh")
        || session.shell.to_lowercase().contains("powershell")
    {
        "powershell"
    } else if session.shell.to_lowercase().contains("cmd") {
        "cmd"
    } else {
        "bash"
    };
    let job = crate::highlight::layout_sized(&line.text, language, light, font_size);
    let screen = session.parser.screen();
    let (rows, cols) = screen.size();
    let mut byte = 0;
    while byte < line.text.len() && row < rows {
        if col >= cols {
            row += 1;
            col = 0;
            continue;
        }
        let Some(cell) = screen.cell(row, col) else {
            break;
        };
        let text = cell.contents();
        if text.is_empty() || !line.text[byte..].starts_with(text) {
            break;
        }
        if let Some(section) = job.sections.iter().find(|s| s.byte_range.contains(&byte)) {
            colors.insert((row, col), section.format.color);
        }
        byte += text.len();
        col += if cell.is_wide() { 2 } else { 1 };
    }
    colors
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn completion_edits_the_live_line() {
        let mut line = Line {
            valid: true,
            text: "Get-Chil".into(),
            cursor: 8,
            ..Default::default()
        };
        let bytes = line.replacement("Get-Chil", "Get-ChildItem", 13).unwrap();
        assert!(bytes.ends_with(b"dItem"));
        assert_eq!(line.text, "Get-ChildItem");
    }
    #[test]
    fn completion_rejects_invalid_utf8_cursor_positions() {
        let mut line = Line {
            valid: true,
            text: "é".into(),
            cursor: 2,
            ..Default::default()
        };
        assert!(line.replacement("é", "été", 1).is_none());
        assert!(line.replacement("é", "été", 99).is_none());
        assert_eq!(line.text, "é");
        line.cursor = 1;
        assert!(line.replacement("é", "été", 5).is_none());
    }
    #[test]
    fn input_is_not_a_second_buffer() {
        let mut l = Line::default();
        l.observe(&Event::Text("echo hi".into()), (4, 12));
        assert_eq!(l.anchor, Some((4, 12)));
        assert_eq!(l.text, "echo hi");
        l.observe(
            &Event::Key {
                key: Key::Enter,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            },
            (4, 19),
        );
        assert!(l.anchor.is_none());
    }
}
