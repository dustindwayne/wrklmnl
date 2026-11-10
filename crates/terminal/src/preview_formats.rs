use eframe::egui;
use serde_json::Value;

pub fn show_json(ui: &mut egui::Ui, name: &str, value: &Value, budget: &mut usize) {
    if *budget == 0 {
        return;
    }
    *budget -= 1;
    match value {
        Value::Array(items) => {
            egui::CollapsingHeader::new(format!("{name}  [{}]", items.len()))
                .id_salt(name)
                .default_open(name == "Root")
                .show(ui, |ui| {
                    for (i, item) in items.iter().enumerate() {
                        if *budget == 0 {
                            ui.weak("Preview limit reached");
                            break;
                        }
                        ui.push_id(i, |ui| show_json(ui, &i.to_string(), item, budget));
                    }
                });
        }
        Value::Object(fields) => {
            egui::CollapsingHeader::new(format!("{name}  {{{}}}", fields.len()))
                .id_salt(name)
                .default_open(name == "Root")
                .show(ui, |ui| {
                    for (key, item) in fields {
                        if *budget == 0 {
                            ui.weak("Preview limit reached");
                            break;
                        }
                        ui.push_id(key, |ui| show_json(ui, key, item, budget));
                    }
                });
        }
        _ => {
            ui.add(
                egui::Label::new(egui::RichText::new(format!("{name}: {value}")).monospace())
                    .wrap()
                    .selectable(true),
            );
        }
    }
}

pub fn table(text: &str, delimiter: char, truncated: bool) -> (Vec<Vec<String>>, bool) {
    const MAX_ROWS: usize = 80;
    const MAX_COLUMNS: usize = 20;
    let mut rows = Vec::new();
    let mut row = Vec::new();
    let mut field = String::new();
    let mut quoted = false;
    let mut field_started = false;
    let mut limited = truncated;
    let mut chars = text.chars().peekable();
    let finish_field = |row: &mut Vec<String>, field: &mut String, limited: &mut bool| {
        if row.len() < MAX_COLUMNS {
            row.push(std::mem::take(field));
        } else {
            *limited = true;
            field.clear();
        }
    };
    while let Some(ch) = chars.next() {
        match ch {
            '"' if quoted && chars.peek() == Some(&'"') => {
                chars.next();
                field.push('"');
            }
            '"' if quoted || !field_started => {
                quoted = !quoted;
                field_started = true;
            }
            ch if !quoted && ch == delimiter => {
                finish_field(&mut row, &mut field, &mut limited);
                field_started = false;
            }
            '\n' | '\r' if !quoted => {
                if ch == '\r' && chars.peek() == Some(&'\n') {
                    chars.next();
                }
                finish_field(&mut row, &mut field, &mut limited);
                rows.push(std::mem::take(&mut row));
                field_started = false;
                if rows.len() == MAX_ROWS {
                    return (rows, limited || chars.peek().is_some());
                }
            }
            _ => {
                field.push(ch);
                field_started = true;
            }
        }
    }
    if !truncated && !quoted && (field_started || !row.is_empty()) {
        finish_field(&mut row, &mut field, &mut limited);
        rows.push(row);
    }
    (rows, limited || quoted)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tables_preserve_quoted_values_and_ignore_a_truncated_last_row() {
        let (rows, limited) = table(
            "name,value\r\n\"first, last\",\"two\"\" words\"\n\"wrapped\nfield\",end",
            ',',
            false,
        );
        assert!(!limited);
        assert_eq!(rows[1], ["first, last", "two\" words"]);
        assert_eq!(rows[2], ["wrapped\nfield", "end"]);
        assert_eq!(
            table("\"\"", ',', false),
            (vec![vec![String::new()]], false)
        );
        assert_eq!(table("a,b\nx,y", ',', true).0.len(), 1);
    }
}
