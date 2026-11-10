//! Compact spatial UI syntax; naming and file extension remain undecided.
mod compact;
use std::collections::BTreeMap;
use terminal_ui_kit::View;

pub type Values = BTreeMap<String, String>;

#[derive(Debug, Clone)]
pub struct Document {
    template: View,
}

impl Document {
    pub fn parse(source: &str) -> Result<Self, String> {
        if source.len() > terminal_ui_kit::MAX_MESSAGE_BYTES {
            return Err("Definition exceeds 256 KiB".into());
        }
        let template = compact::parse(source)?;
        template.validate()?;
        Ok(Self { template })
    }
    pub fn render(&self, values: &Values) -> Result<View, String> {
        compact::bind(&self.template, values)
    }
    pub fn from_view(template: View) -> Self {
        Self { template }
    }
    pub fn template(&self) -> View {
        self.template.clone()
    }
    pub fn source(&self) -> String {
        compact::source(&self.template)
    }
}

fn interpolate(text: &str, values: &Values) -> Result<String, String> {
    let mut output = String::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '{' if chars.peek() == Some(&'{') => {
                chars.next();
                output.push('{');
            }
            '}' if chars.peek() == Some(&'}') => {
                chars.next();
                output.push('}');
            }
            '{' => {
                let mut key = String::new();
                let mut closed = false;
                for c in chars.by_ref() {
                    if c == '}' {
                        closed = true;
                        break;
                    }
                    key.push(c);
                }
                if !closed {
                    return Err("Unclosed binding; use {{ for a literal brace".into());
                }
                let key = key.trim_start_matches('$');
                output.push_str(
                    values
                        .get(key)
                        .ok_or_else(|| format!("Unknown binding {{{key}}}"))?,
                );
            }
            _ => output.push(c),
        }
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn binding_defaults_and_updates() {
        let d = Document::parse("hdr:\"Hello, {name}\" > in:name=world > Update@greet").unwrap();
        assert!(format!("{:?}", d.render(&Values::new()).unwrap()).contains("Hello, world"));
        let mut values = Values::new();
        values.insert("name".into(), "Ada".into());
        assert!(format!("{:?}", d.render(&values).unwrap()).contains("Hello, Ada"));
    }
    #[test]
    fn sample_is_valid() {
        Document::parse(include_str!("../../../examples/ui-example.txt"))
            .unwrap()
            .render(&Values::new())
            .unwrap()
            .validate()
            .unwrap();
    }
}

#[cfg(test)]
mod review_tests {
    use super::*;
    #[test]
    fn literal_money_braces_and_unknown_bindings() {
        let d = Document::parse("t:\"$5.00\" > t:\"$$name\" > t:\"{{literal}}\"").unwrap();
        let v = d.render(&Values::new()).unwrap();
        let s = format!("{v:?}");
        assert!(s.contains("$5.00"));
        assert!(s.contains("$name"));
        assert!(s.contains("{literal}"));
        assert!(Document::parse("t:$typo")
            .unwrap()
            .render(&Values::new())
            .is_err());
    }
    #[test]
    fn shorthand_roles_icons_and_flex_round_trip() {
        let source = "nav[ico:term <> a <> b] > bx[hdr:Hello->t:World > ftr[flex|a|flex|b|flex]]";
        let d = Document::parse(source).unwrap();
        let out = d.source();
        assert!(out.contains("nav:"));
        assert!(out.contains("ico:term"));
        assert!(out.contains("bx:"));
        assert!(out.contains("ftr:"));
        let again = Document::parse(&out).unwrap();
        assert_eq!(out, again.source());
        assert!(!out.contains("Hello-"));
    }
}
