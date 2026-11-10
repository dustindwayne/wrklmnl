use super::Values;
use terminal_ui_kit::{Direction, Element, View};

#[derive(Clone, Debug, PartialEq)]
enum Kind {
    Graphics(u16, String),
    Word(String),
    L,
    R,
    Colon,
    At,
    Eq,
    Horizontal,
    Vertical,
    Flex,
}
#[derive(Clone, Debug)]
struct Token {
    kind: Kind,
    line: usize,
    col: usize,
}
struct Parser {
    tokens: Vec<Token>,
    at: usize,
}
fn lex(source: &str) -> Result<Vec<Token>, String> {
    let mut tokens = vec![];
    let chars: Vec<char> = source.chars().collect();
    let (mut i, mut line, mut col) = (0, 1, 1);
    while i < chars.len() {
        let c = chars[i];
        if c == '\n' {
            line += 1;
            col = 1;
            i += 1;
            continue;
        }
        if c.is_whitespace() {
            col += 1;
            i += 1;
            continue;
        }
        if c == '#' {
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
                col += 1;
            }
            continue;
        }
        let begin = col;
        let kind = if c == 'g' && matches!(chars.get(i + 1), Some('[') | Some(':')) {
            i += 1;
            col += 1;
            let height = if chars.get(i) == Some(&':') {
                i += 1;
                col += 1;
                let start = i;
                while chars.get(i).is_some_and(|c| c.is_ascii_digit()) {
                    i += 1;
                    col += 1;
                }
                chars[start..i]
                    .iter()
                    .collect::<String>()
                    .parse::<u16>()
                    .map_err(|_| {
                        format!("Line {line}:{col}: graphics height requires an integer")
                    })?
            } else {
                160
            };
            if chars.get(i) != Some(&'[') {
                return Err(format!("Line {line}:{col}: expected graphics [ ... ]"));
            }
            i += 1;
            col += 1;
            let start = i;
            let mut depth = 1;
            let mut quoted = false;
            let mut escaped = false;
            let mut comment = false;
            while i < chars.len() {
                let ch = chars[i];
                if ch == '\n' {
                    comment = false;
                }
                if ch == '#' && !quoted {
                    comment = true;
                }
                if ch == '"' && !escaped && !comment {
                    quoted = !quoted;
                }
                if !quoted && !comment {
                    if ch == '[' {
                        depth += 1;
                    }
                    if ch == ']' {
                        depth -= 1;
                        if depth == 0 {
                            break;
                        }
                    }
                }
                if ch == '\\' && !escaped && quoted {
                    escaped = true;
                } else {
                    escaped = false;
                }
                if ch == '\n' {
                    line += 1;
                    col = 1;
                } else {
                    col += 1;
                }
                i += 1;
            }
            if depth != 0 {
                return Err(format!("Line {line}:{col}: unclosed graphics surface"));
            }
            let source = chars[start..i].iter().collect();
            i += 1;
            col += 1;
            Kind::Graphics(height, source)
        } else if c == '"' {
            i += 1;
            col += 1;
            let mut s = String::new();
            let mut closed = false;
            while i < chars.len() {
                let c = chars[i];
                i += 1;
                col += 1;
                if c == '"' {
                    closed = true;
                    break;
                }
                if c == '\\' {
                    let n = *chars
                        .get(i)
                        .ok_or_else(|| format!("Line {line}:{col}: unfinished escape"))?;
                    i += 1;
                    col += 1;
                    match n {
                        'n' => s.push('\n'),
                        't' => s.push('\t'),
                        '"' => s.push('"'),
                        '\\' => s.push('\\'),
                        _ => {
                            s.push('\\');
                            s.push(n);
                        }
                    }
                } else {
                    if c == '\n' {
                        line += 1;
                        col = 1;
                    }
                    s.push(c);
                }
            }
            if !closed {
                return Err(format!("Line {line}:{begin}: unfinished string"));
            }
            Kind::Word(s)
        } else {
            i += 1;
            col += 1;
            match c {
                '[' => Kind::L,
                ']' => Kind::R,
                ':' => Kind::Colon,
                '@' => Kind::At,
                '=' => Kind::Eq,
                '|' => Kind::Horizontal,
                '>' => Kind::Vertical,
                '<' if chars.get(i) == Some(&'>') => {
                    i += 1;
                    col += 1;
                    Kind::Flex
                }
                '<' if chars.get(i) == Some(&'-') && chars.get(i + 1) == Some(&'>') => {
                    i += 2;
                    col += 2;
                    Kind::Flex
                }
                '-' if chars.get(i) == Some(&'>') => {
                    i += 1;
                    col += 1;
                    Kind::Vertical
                }
                _ => {
                    let mut s = c.to_string();
                    while i < chars.len()
                        && !chars[i].is_whitespace()
                        && !"[]:@=|><".contains(chars[i])
                        && !(chars[i] == '-' && chars.get(i + 1) == Some(&'>'))
                    {
                        s.push(chars[i]);
                        i += 1;
                        col += 1;
                    }
                    Kind::Word(s)
                }
            }
        };
        tokens.push(Token {
            kind,
            line,
            col: begin,
        });
        if tokens.len() > 8192 {
            return Err("Definition has too many tokens".into());
        }
    }
    Ok(tokens)
}
impl Parser {
    fn error(&self, message: &str) -> String {
        self.tokens
            .get(self.at)
            .map(|t| format!("Line {}:{}: {message}", t.line, t.col))
            .unwrap_or_else(|| format!("End of source: {message}"))
    }
    fn is(&self, k: &Kind) -> bool {
        self.tokens.get(self.at).is_some_and(|t| &t.kind == k)
    }
    fn take(&mut self, k: &Kind) -> bool {
        if self.is(k) {
            self.at += 1;
            true
        } else {
            false
        }
    }
    fn word(&mut self) -> Result<String, String> {
        match self.tokens.get(self.at).map(|t| t.kind.clone()) {
            Some(Kind::Word(w)) => {
                self.at += 1;
                Ok(w)
            }
            _ => Err(self.error("expected a value; quote labels containing spaces")),
        }
    }
    fn expect(&mut self, k: Kind) -> Result<(), String> {
        if self.take(&k) {
            Ok(())
        } else {
            Err(self.error(&format!("expected {k:?}")))
        }
    }
    fn expression(&mut self, depth: usize) -> Result<Element, String> {
        if depth > 16 {
            return Err(self.error("at most 16 layout levels"));
        }
        let mut children = vec![self.row(depth)?];
        while self.take(&Kind::Vertical) {
            children.push(self.row(depth)?);
        }
        Ok(if children.len() == 1 {
            children.remove(0)
        } else {
            Element::Stack {
                role: None,
                direction: Direction::Column,
                gap: 20.0,
                children,
            }
        })
    }
    fn row(&mut self, depth: usize) -> Result<Element, String> {
        let mut children = vec![self.object(depth)?];
        loop {
            if self.take(&Kind::Flex) {
                children.push(Element::Flex);
            } else if !self.take(&Kind::Horizontal) {
                break;
            }
            children.push(self.object(depth)?);
        }
        Ok(if children.len() == 1 {
            children.remove(0)
        } else {
            Element::Stack {
                role: None,
                direction: Direction::Row,
                gap: 12.0,
                children,
            }
        })
    }
    fn group(&mut self, depth: usize) -> Result<Element, String> {
        self.expect(Kind::L)?;
        let e = if self.take(&Kind::R) {
            return Ok(Element::Stack {
                direction: Direction::Column,
                gap: 20.0,
                children: vec![],
                role: None,
            });
        } else {
            self.expression(depth + 1)?
        };
        self.expect(Kind::R)?;
        Ok(e)
    }
    fn object(&mut self, depth: usize) -> Result<Element, String> {
        if let Some(Kind::Graphics(height, source)) =
            self.tokens.get(self.at).map(|t| t.kind.clone())
        {
            self.at += 1;
            return Ok(Element::Graphics { height, source });
        }
        let tag = self.word()?;
        if self.take(&Kind::Colon) {
            let value = self.word()?;
            return Ok(match tag.as_str() {
                "h" | "hdr" => Element::Heading { text: value },
                "t" | "txt" | "text" | "status" => Element::Text { text: value },
                "ico" => Element::Icon { name: value },
                "in" | "input" => {
                    let default = if self.take(&Kind::Eq) {
                        self.word()?
                    } else {
                        String::new()
                    };
                    let label = if self.take(&Kind::Colon) {
                        self.word()?
                    } else {
                        value.replace('_', " ")
                    };
                    Element::Input {
                        id: value.clone(),
                        label,
                        value: default,
                    }
                }
                "p" | "progress" => Element::Progress {
                    label: if self.take(&Kind::Colon) {
                        self.word()?
                    } else {
                        "Progress".into()
                    },
                    value: value
                        .parse()
                        .map_err(|_| self.error("progress needs a number from 0 to 1"))?,
                },
                "code" => Element::Code {
                    language: value,
                    text: self.word()?,
                },
                "list" => Element::List { items: vec![value] },
                "gap" => Element::Space {
                    size: value
                        .parse()
                        .map_err(|_| self.error("gap needs a number"))?,
                },
                "at" => {
                    let r = value
                        .split(',')
                        .map(str::parse::<f32>)
                        .collect::<Result<Vec<_>, _>>()
                        .map_err(|_| self.error("at requires x,y,width,height"))?;
                    if r.len() != 4 {
                        return Err(self.error("at requires x,y,width,height"));
                    }
                    Element::Place {
                        x: r[0],
                        y: r[1],
                        width: r[2],
                        height: r[3],
                        child: Box::new(self.group(depth)?),
                    }
                }
                "row" | "col" | "nav" | "bx" | "ftr" | "content" | "grid" | "tile"
                | "tile_disabled" | "fold" | "item" | "item_selected" => {
                    let gap = value
                        .parse()
                        .map_err(|_| self.error("layout gap needs a number"))?;
                    let child = self.group(depth)?;
                    let direction = if matches!(tag.as_str(), "row" | "nav" | "ftr") {
                        Direction::Row
                    } else {
                        Direction::Column
                    };
                    let children = if let Element::Stack {
                        direction: d,
                        children,
                        ..
                    } = &child
                    {
                        if matches!(
                            (direction, d),
                            (Direction::Row, Direction::Row)
                                | (Direction::Column, Direction::Column)
                        ) {
                            children.clone()
                        } else {
                            vec![child]
                        }
                    } else {
                        vec![child]
                    };
                    Element::Stack {
                        direction,
                        gap,
                        children,
                        role: if matches!(tag.as_str(), "row" | "col") {
                            None
                        } else {
                            Some(tag)
                        },
                    }
                }
                _ => return Err(self.error(&format!("unknown object type '{tag}'"))),
            });
        }
        if self.is(&Kind::L) {
            if tag == "tbl" || tag == "table" {
                self.at += 1;
                let mut rows = vec![];
                loop {
                    let mut cells = vec![self.word()?];
                    while self.take(&Kind::Horizontal) {
                        cells.push(self.word()?);
                    }
                    rows.push(cells);
                    if !self.take(&Kind::Vertical) {
                        break;
                    }
                }
                self.expect(Kind::R)?;
                let columns = rows.remove(0);
                return Ok(Element::Table { columns, rows });
            }
            let child = self.group(depth)?;
            let direction = match tag.as_str() {
                "nav" | "ftr" | "row" => Direction::Row,
                "bx" | "content" | "col" | "column" | "grid" | "tile" | "tile_disabled"
                | "fold" | "item" | "item_selected" => Direction::Column,
                _ => return Err(self.error(&format!("unknown layout '{tag}'"))),
            };
            if let Element::Stack {
                direction: d,
                role: None,
                ..
            } = &child
            {
                if matches!(
                    (direction, d),
                    (Direction::Row, Direction::Row) | (Direction::Column, Direction::Column)
                ) {
                    if let Element::Stack { children, .. } = child {
                        return Ok(Element::Stack {
                            role: Some(tag),
                            direction,
                            gap: if matches!(direction, Direction::Row) {
                                12.0
                            } else {
                                20.0
                            },
                            children,
                        });
                    }
                }
            }
            return Ok(Element::Stack {
                role: Some(tag),
                direction,
                gap: if matches!(direction, Direction::Row) {
                    12.0
                } else {
                    20.0
                },
                children: vec![child],
            });
        }
        if tag == "flex" {
            return Ok(Element::Flex);
        }
        if tag.starts_with('$') {
            return Ok(Element::Text { text: tag });
        }
        let id = if self.take(&Kind::At) {
            self.word()?
        } else {
            tag.to_lowercase().replace(' ', "_")
        };
        Ok(Element::Action { id, label: tag })
    }
}
pub fn parse(source: &str) -> Result<View, String> {
    if terminal_ui_graphics::program::validate(source).is_ok() && !source.trim().is_empty() {
        let mut view = View::new("document", "Interface");
        view.elements.push(Element::Graphics {
            height: 160,
            source: source.into(),
        });
        return Ok(view);
    }
    let mut p = Parser {
        tokens: lex(source)?,
        at: 0,
    };
    if p.tokens.is_empty() {
        return Err("Definition is empty".into());
    }
    let element = p.expression(0)?;
    if p.at != p.tokens.len() {
        return Err(p.error("use | for a row, > for a column, or <> for a flexible gap"));
    }
    let mut view = View::new("document", "Interface");
    view.elements = match element {
        Element::Stack {
            direction: Direction::Column,
            children,
            gap: _,
            role: None,
        } if children.iter().all(|e| matches!(e, Element::Place { .. })) => children,
        other => vec![other],
    };
    Ok(view)
}
fn value(text: &str, data: &Values) -> Result<String, String> {
    if let Some(literal) = text.strip_prefix("$$") {
        return Ok(format!("${literal}"));
    }
    if let Some(key) = text
        .strip_prefix('$')
        .filter(|key| key.starts_with(|c: char| c.is_alphabetic() || c == '_'))
    {
        data.get(key)
            .cloned()
            .ok_or_else(|| format!("Unknown binding ${key}"))
    } else {
        super::interpolate(text, data)
    }
}
pub fn bind(view: &View, data: &Values) -> Result<View, String> {
    let mut data = data.clone();
    fn defaults(e: &Element, d: &mut Values) {
        match e {
            Element::Input { id, value, .. } => {
                d.entry(id.clone()).or_insert(value.clone());
            }
            Element::Stack { children, .. } => {
                for c in children {
                    defaults(c, d)
                }
            }
            Element::Place { child, .. } => defaults(child, d),
            _ => {}
        }
    }
    for e in &view.elements {
        defaults(e, &mut data);
    }
    fn walk(e: &Element, d: &Values) -> Result<Element, String> {
        Ok(match e {
            Element::Graphics { height, source } => {
                let mut replacements = vec![];
                let mut error = None;
                terminal_ui_graphics::program::visit_spanned(
                    source,
                    terminal_ui_graphics::paint::Paint::solid(terminal_ui_graphics::Color(
                        245, 245, 245,
                    )),
                    |start, end, operation| {
                        if let terminal_ui_graphics::program::Operation::Text(..) = operation {
                            let instruction = &source[start..end];
                            if let (Some(a), Some(b)) =
                                (instruction.find('"'), instruction.rfind('"'))
                            {
                                if b > a {
                                    let quoted = &instruction[a..=b];
                                    match serde_json::from_str::<String>(quoted)
                                        .map_err(|e| e.to_string())
                                        .and_then(|s| value(&s, d))
                                        .and_then(|s| {
                                            serde_json::to_string(&s).map_err(|e| e.to_string())
                                        }) {
                                        Ok(text) => {
                                            replacements.push((start + a, start + b + 1, text))
                                        }
                                        Err(e) => error = Some(e),
                                    }
                                }
                            }
                        }
                    },
                )
                .map_err(|e| e.to_string())?;
                if let Some(e) = error {
                    return Err(e);
                }
                let mut source = source.clone();
                for (a, b, text) in replacements.into_iter().rev() {
                    source.replace_range(a..b, &text);
                }
                Element::Graphics {
                    height: *height,
                    source,
                }
            }
            Element::Icon { name } => Element::Icon {
                name: value(name, d)?,
            },
            Element::Text { text } => Element::Text {
                text: value(text, d)?,
            },
            Element::Heading { text } => Element::Heading {
                text: value(text, d)?,
            },
            Element::Input {
                id,
                label,
                value: default,
            } => Element::Input {
                id: id.clone(),
                label: value(label, d)?,
                value: d.get(id).cloned().unwrap_or(default.clone()),
            },
            Element::Action { id, label } => Element::Action {
                id: id.clone(),
                label: value(label, d)?,
            },
            Element::Stack {
                direction,
                role,
                gap,
                children,
            } => Element::Stack {
                role: role.clone(),
                direction: *direction,
                gap: *gap,
                children: children
                    .iter()
                    .map(|e| walk(e, d))
                    .collect::<Result<_, _>>()?,
            },
            Element::Place {
                x,
                y,
                width,
                height,
                child,
            } => Element::Place {
                x: *x,
                y: *y,
                width: *width,
                height: *height,
                child: Box::new(walk(child, d)?),
            },
            Element::List { items } => Element::List {
                items: items
                    .iter()
                    .map(|s| value(s, d))
                    .collect::<Result<Vec<_>, _>>()?
                    .iter()
                    .flat_map(|s| s.lines().map(str::to_string))
                    .collect(),
            },
            Element::Table { columns, rows } => Element::Table {
                columns: columns
                    .iter()
                    .map(|s| value(s, d))
                    .collect::<Result<_, _>>()?,
                rows: rows
                    .iter()
                    .map(|r| r.iter().map(|s| value(s, d)).collect::<Result<Vec<_>, _>>())
                    .collect::<Result<_, _>>()?,
            },
            _ => e.clone(),
        })
    }
    let mut result = view.clone();
    result.elements = view
        .elements
        .iter()
        .map(|e| walk(e, &data))
        .collect::<Result<_, _>>()?;
    result.validate()?;
    Ok(result)
}
fn quote(s: &str) -> String {
    if !s.is_empty()
        && s.chars()
            .all(|c| !c.is_whitespace() && !"[]:@=|><\"#\\".contains(c))
    {
        s.into()
    } else {
        format!(
            "\"{}\"",
            s.replace('\\', "\\\\")
                .replace('"', "\\\"")
                .replace('\n', "\\n")
                .replace('\t', "\\t")
        )
    }
}
fn format_element(e: &Element) -> String {
    match e {
        Element::Graphics { height, source } => format!("g:{height}[{source}]"),
        Element::Icon { name } => format!("ico:{}", quote(name)),
        Element::Text { text } => format!("t:{}", quote(text)),
        Element::Heading { text } => format!("h:{}", quote(text)),
        Element::Input { id, label, value } => format!(
            "in:{}={}{}",
            quote(id),
            quote(value),
            if label == &id.replace('_', " ") {
                String::new()
            } else {
                format!(":{}", quote(label))
            }
        ),
        Element::Action { id, label } => format!("{}@{}", quote(label), quote(id)),
        Element::Progress { label, value } => format!(
            "p:{value}{}",
            if label == "Progress" {
                String::new()
            } else {
                format!(":{}", quote(label))
            }
        ),
        Element::Code { language, text } => format!("code:{} {}", quote(language), quote(text)),
        Element::Stack {
            direction,
            role,
            gap,
            children,
        } => {
            let tag = if matches!(direction, Direction::Row) {
                "row"
            } else {
                "col"
            };
            let tag = role
                .as_ref()
                .map(|role| format!("{role}:{gap}"))
                .unwrap_or_else(|| format!("{tag}:{gap}"));
            let sep = if matches!(direction, Direction::Row) {
                "|"
            } else {
                " > "
            };
            let s = children
                .iter()
                .map(format_element)
                .collect::<Vec<_>>()
                .join(sep);
            format!("{tag}[{s}]")
        }

        Element::Table { columns, rows } => {
            let mut r = vec![columns
                .iter()
                .map(|s| quote(s))
                .collect::<Vec<_>>()
                .join("|")];
            r.extend(
                rows.iter()
                    .map(|r| r.iter().map(|s| quote(s)).collect::<Vec<_>>().join("|")),
            );
            format!("tbl[{}]", r.join(" > "))
        }
        Element::Space { size } => format!("gap:{size}"),
        Element::Flex => "flex".into(),
        Element::List { items } => format!("list:{}", quote(&items.join("\n"))),
        Element::Place {
            x,
            y,
            width,
            height,
            child,
        } => format!("at:{x},{y},{width},{height}[{}]", format_element(child)),
    }
}
pub fn source(view: &View) -> String {
    view.elements
        .iter()
        .map(format_element)
        .collect::<Vec<_>>()
        .join("\n>\n")
        + "\n"
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn graphics_roundtrip_and_safe_text_bindings() {
        let doc = super::super::Document::parse(
            "g:96[blue/75[r 8,12 *,32; t 8,48 \"Hello, {name}\"]] > in:name=world",
        )
        .unwrap();
        let rendered = doc
            .render(&Values::from([(
                "name".into(),
                "Alice\"; r 0,0 *,* red".into(),
            )]))
            .unwrap();
        let Element::Stack { children, .. } = &rendered.elements[0] else {
            panic!()
        };
        let Element::Graphics { source, .. } = &children[0] else {
            panic!()
        };
        let mut n = 0;
        terminal_ui_graphics::program::visit(
            source,
            terminal_ui_graphics::paint::Paint::solid(terminal_ui_graphics::Color(0, 0, 0)),
            |_| n += 1,
        )
        .unwrap();
        assert_eq!(n, 2);
        let canonical = doc.source();
        assert_eq!(
            super::super::Document::parse(&canonical).unwrap().source(),
            canonical
        );
    }
    #[test]
    fn graphics_comments_and_bare_instructions() {
        super::super::Document::parse("g:64[# ignored ]\n t 0,0 \"Ready\"]").unwrap();
        let doc =
            super::super::Document::parse("r 8,12 *,32 blue/75\nt 16,20 \"Ready\" white").unwrap();
        assert!(matches!(
            doc.template().elements[0],
            Element::Graphics { .. }
        ));
    }
    #[test]
    fn compact_spatial_syntax() {
        let v=parse("nav[ico:term <> Home|Sessions|+@new] > bx[hdr:Recent > list:$recent > ftr[$status <> Open@open]]").unwrap();
        v.validate().unwrap();
        assert!(source(&v).contains("@new"));
    }
    #[test]
    fn compact_round_trip() {
        let a = parse("bx[at:0,24,300,80[h:Recent]] > in:name=world > \"Update greeting\"@greet")
            .unwrap();
        let b = parse(&source(&a)).unwrap();
        b.validate().unwrap();
        assert_eq!(source(&a), source(&b));
    }
    #[test]
    fn actionable_error() {
        assert!(parse("bx[t:Hello t:World]")
            .unwrap_err()
            .contains("Line 1:"));
    }
}
