//! Small, renderer-independent UI vocabulary. One JSON document per stdout line.
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashSet},
    io::{self, Write},
};

fn default_gap() -> f32 {
    12.0
}
pub const VERSION: u32 = 1;
pub const MAX_MESSAGE_BYTES: usize = 256 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct View {
    pub version: u32,
    pub id: String,
    pub title: String,
    pub elements: Vec<Element>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Element {
    Graphics {
        height: u16,
        source: String,
    },
    Icon {
        name: String,
    },
    Text {
        text: String,
    },
    Heading {
        text: String,
    },
    Table {
        columns: Vec<String>,
        rows: Vec<Vec<String>>,
    },
    Input {
        id: String,
        label: String,
        value: String,
    },
    Action {
        id: String,
        label: String,
    },
    Progress {
        label: String,
        value: f32,
    },
    Code {
        language: String,
        text: String,
    },
    Stack {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        role: Option<String>,
        direction: Direction,
        #[serde(default = "default_gap")]
        gap: f32,
        children: Vec<Element>,
    },
    Space {
        size: f32,
    },
    Flex,
    List {
        items: Vec<String>,
    },
    Place {
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        child: Box<Element>,
    },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    Row,
    Column,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Action {
    pub version: u32,
    pub view: String,
    pub action: String,
    pub values: BTreeMap<String, String>,
}

impl View {
    pub fn new(id: impl Into<String>, title: impl Into<String>) -> Self {
        Self {
            version: VERSION,
            id: id.into(),
            title: title.into(),
            elements: vec![],
        }
    }
    pub fn text(mut self, text: impl Into<String>) -> Self {
        self.elements.push(Element::Text { text: text.into() });
        self
    }
    pub fn heading(mut self, text: impl Into<String>) -> Self {
        self.elements.push(Element::Heading { text: text.into() });
        self
    }
    pub fn input(
        mut self,
        id: impl Into<String>,
        label: impl Into<String>,
        value: impl Into<String>,
    ) -> Self {
        self.elements.push(Element::Input {
            id: id.into(),
            label: label.into(),
            value: value.into(),
        });
        self
    }
    pub fn action(mut self, id: impl Into<String>, label: impl Into<String>) -> Self {
        self.elements.push(Element::Action {
            id: id.into(),
            label: label.into(),
        });
        self
    }
    pub fn table(mut self, columns: Vec<String>, rows: Vec<Vec<String>>) -> Self {
        self.elements.push(Element::Table { columns, rows });
        self
    }
    pub fn progress(mut self, label: impl Into<String>, value: f32) -> Self {
        self.elements.push(Element::Progress {
            label: label.into(),
            value,
        });
        self
    }
    pub fn code(mut self, language: impl Into<String>, text: impl Into<String>) -> Self {
        self.elements.push(Element::Code {
            language: language.into(),
            text: text.into(),
        });
        self
    }
    pub fn validate(&self) -> Result<(), String> {
        if self.version != VERSION {
            return Err("Unsupported UI protocol version".into());
        }
        if self.id.is_empty() || self.title.is_empty() {
            return Err("View ID and title are required".into());
        }
        if self.elements.len() > 256 {
            return Err("At most 256 elements per view".into());
        }
        validate_placement_group(&self.elements, "elements")?;
        let mut ids = HashSet::new();
        let mut pending: Vec<_> = self
            .elements
            .iter()
            .enumerate()
            .map(|(i, e)| (e, 0usize, format!("elements[{i}]")))
            .collect();
        let mut count = 0;
        while let Some((e, depth, path)) = pending.pop() {
            count += 1;
            if count > 256 || depth > 16 {
                return Err(format!("{path}: At most 256 elements and 16 layout levels"));
            }
            match e {
                Element::Graphics { height, source } => {
                    if !(16..=2048).contains(height) {
                        return Err(format!("{path}: Graphics height must be 16-2048"));
                    }
                    terminal_ui_graphics::program::validate(source)
                        .map_err(|e| format!("{path}: {e}"))?;
                }
                Element::Input { id, .. } => {
                    if id.is_empty() || !ids.insert(id) {
                        return Err(format!("{path}: Input IDs must be nonempty and unique"));
                    }
                }
                Element::Action { id, .. } if id.is_empty() => {
                    return Err(format!("{path}: Action IDs must not be empty"))
                }
                Element::Progress { value, .. }
                    if !value.is_finite() || !(0.0..=1.0).contains(value) =>
                {
                    return Err(format!("{path}: Progress must be between 0 and 1"))
                }
                Element::Table { columns, rows }
                    if columns.is_empty()
                        || columns.len() > 32
                        || rows.len() > 1000
                        || rows.iter().any(|r| r.len() != columns.len()) =>
                {
                    return Err(format!("{path}: Table dimensions are invalid"))
                }
                Element::Stack { gap, children, .. } => {
                    if !gap.is_finite() || !(0.0..=128.0).contains(gap) {
                        return Err(format!("{path}: Layout gap must be 0-128"));
                    }
                    pending.extend(
                        children
                            .iter()
                            .enumerate()
                            .map(|(i, e)| (e, depth + 1, format!("{path}.children[{i}]"))),
                    );
                }
                Element::Space { size } if !size.is_finite() || !(0.0..=128.0).contains(size) => {
                    return Err(format!("{path}: Space must be 0-128"))
                }
                Element::Place {
                    x,
                    y,
                    width,
                    height,
                    child,
                } => {
                    if ![x, y, width, height].iter().all(|v| v.is_finite())
                        || *x < 0.0
                        || *y < 0.0
                        || *width < 24.0
                        || *height < 24.0
                        || *x > 10000.0
                        || *y > 10000.0
                        || *width > 10000.0
                        || *height > 10000.0
                    {
                        return Err(format!("{path}: Invalid object placement"));
                    }
                    pending.push((child, depth + 1, format!("{path}.child")));
                }
                _ => {}
            }
        }
        Ok(())
    }
    pub fn emit(&self) -> io::Result<()> {
        self.validate()
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;
        let bytes = serde_json::to_vec(self)?;
        if bytes.len() > MAX_MESSAGE_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "UI document exceeds 256 KiB",
            ));
        }
        let mut out = io::stdout().lock();
        out.write_all(&bytes)?;
        out.write_all(b"\n")?;
        out.flush()
    }
}

pub fn decode(bytes: &[u8]) -> Result<View, String> {
    if bytes.len() > MAX_MESSAGE_BYTES {
        return Err("UI document exceeds 256 KiB".into());
    }
    let view: View = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
    view.validate()?;
    Ok(view)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn round_trip() {
        let v = View::new("demo", "Demo")
            .input("name", "Name", "")
            .action("save", "Save")
            .progress("Ready", 0.5);
        assert_eq!(
            decode(&serde_json::to_vec(&v).unwrap())
                .unwrap()
                .elements
                .len(),
            3
        );
    }
    #[test]
    fn duplicate_ids() {
        assert!(View::new("x", "X")
            .input("same", "Name", "")
            .input("same", "Other", "")
            .validate()
            .is_err());
        assert!(View::new("x", "X")
            .action("new", "New")
            .action("new", "New again")
            .validate()
            .is_ok());
    }
    #[test]
    fn malformed_views() {
        assert!(decode(br#"{"version":99,"id":"x","title":"X","elements":[]}"#).is_err());
        assert!(decode(&vec![b'x'; MAX_MESSAGE_BYTES + 1]).is_err());
        assert!(View::new("x", "X").progress("Bad", 2.0).validate().is_err());
    }
    #[test]
    fn malformed_table() {
        assert!(View::new("x", "X")
            .table(vec!["A".into()], vec![vec![]])
            .validate()
            .is_err());
    }
}

fn validate_placement_group(elements: &[Element], path: &str) -> Result<(), String> {
    let placed = elements
        .iter()
        .filter(|e| matches!(e, Element::Place { .. }))
        .count();
    if placed > 0 && placed < elements.len() {
        return Err(format!("{path}: positioned and flow siblings cannot be mixed; put them in separate layout groups"));
    }
    for (i, e) in elements.iter().enumerate() {
        if let Element::Stack { children, .. } = e {
            validate_placement_group(children, &format!("{path}[{i}].children"))?;
        }
    }
    Ok(())
}
