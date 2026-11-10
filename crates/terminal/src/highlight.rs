//! Lightweight default lexer. Replace or extend this module with full grammars later.
use eframe::egui::{
    text::{LayoutJob, TextFormat},
    Color32, FontId,
};

#[cfg(test)]
pub fn layout(text: &str, language: &str, light: bool) -> LayoutJob {
    layout_sized(text, language, light, 15.0)
}
pub fn layout_sized(text: &str, language: &str, light: bool, size: f32) -> LayoutJob {
    let mut job = LayoutJob::default();
    let base = if light {
        Color32::from_rgb(23, 23, 23)
    } else {
        Color32::from_rgb(245, 245, 245)
    };
    if language == "plain" {
        return LayoutJob::simple(text.into(), FontId::monospace(size), base, f32::INFINITY);
    }
    let keyword = if light {
        // Semantic highlighting stays restrained in both themes.
        Color32::from_rgb(132, 42, 97)
    } else {
        Color32::from_rgb(211, 138, 175)
    };
    let string = if light {
        Color32::from_rgb(42, 100, 70)
    } else {
        Color32::from_rgb(156, 190, 160)
    };
    let variable = if light {
        Color32::from_rgb(37, 83, 137)
    } else {
        Color32::from_rgb(148, 177, 204)
    };
    let muted = if light {
        Color32::from_rgb(98, 98, 98)
    } else {
        Color32::from_rgb(163, 163, 163)
    };
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let mut i = 0;
    while i < chars.len() {
        let start = i;
        let c = chars[i].1;
        let mut color = base;
        if (c == '#' && matches!(language, "powershell" | "bash" | "python" | "ui"))
            || (c == '/'
                && chars.get(i + 1).is_some_and(|(_, c)| *c == '/')
                && matches!(language, "rust" | "javascript" | "typescript"))
        {
            color = muted;
            while i < chars.len() && chars[i].1 != '\n' {
                i += 1;
            }
        } else if (c == '\'' || c == '"')
            && !(language == "rust"
                && c == '\''
                && chars.get(i + 1).is_some_and(|(_, c)| c.is_alphabetic())
                && !chars.get(i + 2).is_some_and(|(_, c)| *c == '\''))
        {
            color = string;
            i += 1;
            while i < chars.len() {
                let now = chars[i].1;
                if now == '\n' {
                    break;
                }
                i += 1;
                if now == '\\' || now == '`' {
                    i = (i + 1).min(chars.len());
                } else if now == c {
                    break;
                }
            }
        } else if c == '$' || (c == '%' && language == "cmd") {
            color = variable;
            i += 1;
            while i < chars.len() && (chars[i].1.is_alphanumeric() || "_{}:%".contains(chars[i].1))
            {
                i += 1;
            }
        } else if c.is_ascii_digit() {
            color = variable;
            i += 1;
            while i < chars.len() && (chars[i].1.is_ascii_digit() || chars[i].1 == '.') {
                i += 1;
            }
        } else if c.is_alphabetic() || c == '_' || c == '-' {
            i += 1;
            while i < chars.len() && (chars[i].1.is_alphanumeric() || "_-".contains(chars[i].1)) {
                i += 1;
            }
            let end = chars.get(i).map(|x| x.0).unwrap_or(text.len());
            let token = &text[chars[start].0..end];
            let keywords: &[&str] = match language {
                "rust" => &[
                    "fn", "let", "mut", "pub", "use", "impl", "struct", "enum", "match", "if",
                    "else", "for", "in", "while", "return", "self", "Self", "trait", "mod",
                    "const", "async", "await", "move", "true", "false",
                ],
                "python" => &[
                    "def", "class", "import", "from", "if", "elif", "else", "for", "in", "while",
                    "return", "True", "False", "None", "with", "as", "try", "except", "lambda",
                    "async", "await",
                ],
                "javascript" | "typescript" => &[
                    "function",
                    "const",
                    "let",
                    "var",
                    "class",
                    "import",
                    "export",
                    "if",
                    "else",
                    "for",
                    "of",
                    "in",
                    "while",
                    "return",
                    "true",
                    "false",
                    "null",
                    "undefined",
                    "async",
                    "await",
                    "interface",
                    "type",
                ],
                "powershell" => &[
                    "function", "param", "foreach", "if", "else", "return", "while", "try", "catch",
                ],
                "bash" => &[
                    "if", "then", "else", "fi", "for", "in", "do", "done", "while", "function",
                    "case", "esac", "echo", "cd",
                ],
                "cmd" => &[
                    "echo", "cd", "dir", "rem", "REM", "if", "for", "set", "call", "exit",
                ],
                "ui" => &[
                    "nav",
                    "bx",
                    "hdr",
                    "ftr",
                    "row",
                    "col",
                    "ico",
                    "tbl",
                    "code",
                    "list",
                    "at",
                    "h",
                    "t",
                    "p",
                    "in",
                    "gap",
                    "flex",
                    "g",
                    "r",
                    "l",
                    "c",
                    "i",
                    "white",
                    "black",
                    "gray",
                    "grey",
                    "charcoal",
                    "red",
                    "blue",
                    "green",
                    "pink",
                    "orange",
                    "yellow",
                    "purple",
                    "cyan",
                    "teal",
                    "navy",
                    "lime",
                    "magenta",
                    "transparent",
                ],
                "json" => &["true", "false", "null"],
                _ => &[],
            };
            if keywords.contains(&token)
                || matches!(language, "powershell" | "bash") && token.contains('-')
            {
                color = keyword;
            }
        } else {
            i += 1;
            if "|;&<>=[]:@".contains(c) {
                color = keyword;
            }
        }
        let a = chars[start].0;
        let b = chars.get(i).map(|x| x.0).unwrap_or(text.len());
        job.append(
            &text[a..b],
            0.0,
            TextFormat {
                font_id: FontId::monospace(size),
                color,
                ..Default::default()
            },
        );
    }
    job
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preserves_unicode_and_colors_syntax() {
        let s = "Get-ChildItem $HOME 'café' | echo 42 # comment";
        let j = layout(s, "powershell", false);
        assert_eq!(j.text, s);
        assert!(j.sections.len() > 5);
        assert_ne!(j.sections[0].format.color, j.sections[1].format.color);
    }
}

#[cfg(test)]
mod review_tests {
    use super::*;
    #[test]
    fn rust_attributes_and_lifetimes_dont_consume_file() {
        let source = "#[derive(Clone)]\nfn read<'a>(v: &'a str) -> &'a str { v }";
        let job = layout(source, "rust", false);
        let fn_section = job
            .sections
            .iter()
            .find(|s| &job.text[s.byte_range.clone()] == "fn")
            .unwrap();
        assert_ne!(fn_section.format.color, Color32::from_rgb(163, 163, 163));
        assert_eq!(job.text, source);
    }
    #[test]
    fn ui_words_are_not_python_keywords() {
        let job = layout("row col list code p t", "python", false);
        assert_eq!(
            job.sections[0].format.color,
            Color32::from_rgb(245, 245, 245)
        );
    }
}
