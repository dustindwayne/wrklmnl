use std::io::{BufRead, Write};
use terminal_ui_kit::{Action, View};
pub fn emit(view: Option<&View>) -> anyhow::Result<()> {
    let mut out = std::io::stdout().lock();
    if let Some(view) = view {
        view.validate().map_err(anyhow::Error::msg)?;
        let bytes = serde_json::to_vec(view)?;
        write!(out, "\x1b]777;ui-begin\x07")?;
        for chunk in bytes.chunks(400) {
            write!(out, "\x1b]777;ui-part;")?;
            for b in chunk {
                write!(out, "{b:02x}")?;
            }
            write!(out, "\x07")?;
        }
        write!(out, "\x1b]777;ui-end\x07")?;
    } else {
        write!(out, "\x1b]777;ui-close\x07")?;
    }
    out.flush()?;
    Ok(())
}
pub fn decode_hex(text: &[u8]) -> Option<Vec<u8>> {
    if text.len() % 2 != 0 {
        return None;
    }
    text.chunks_exact(2)
        .map(|b| u8::from_str_radix(std::str::from_utf8(b).ok()?, 16).ok())
        .collect()
}
pub fn run(source: &str, data: &std::collections::BTreeMap<String, String>) -> anyhow::Result<()> {
    anyhow::ensure!(
        std::env::var_os("TERMINAL_UI").is_some(),
        "Run this command inside a session of this terminal app to render its graphical UI"
    );
    let document = terminal_ui_language::Document::parse(source).map_err(anyhow::Error::msg)?;
    let mut values = data.clone();
    values.entry("status".into()).or_insert("Ready".into());
    values.entry("recent".into()).or_default();
    emit(Some(&document.render(&values).map_err(anyhow::Error::msg)?))?;
    for line in std::io::stdin().lock().lines() {
        let line = line?;
        let Ok(action) = serde_json::from_str::<Action>(&line) else {
            continue;
        };
        if action.action == "_terminal.close" {
            break;
        }
        values.extend(action.values);
        values.insert("status".into(), format!("Action: {}", action.action));
        emit(Some(&document.render(&values).map_err(anyhow::Error::msg)?))?;
    }
    emit(None)?;
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn chunks_are_safe_and_lossless() {
        assert_eq!(decode_hex(b"7b22c3a9227d").unwrap(), "{\"é\"}".as_bytes());
        assert!(decode_hex(b"zz").is_none());
    }
}
