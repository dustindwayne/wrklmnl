use eframe::egui::{self, text::LayoutJob, FontFamily, FontId, TextFormat};
use pulldown_cmark::{CodeBlockKind, Event, Options, Parser, Tag, TagEnd};

pub fn parse(text: &str) -> Vec<Event<'static>> {
    let options =
        Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS | Options::ENABLE_TABLES;
    Parser::new_ext(text, options)
        .map(Event::into_static)
        .collect()
}

pub fn show(ui: &mut egui::Ui, events: &[Event<'static>], syntax: bool) {
    let mut job = LayoutJob::default();
    let mut code: Option<(&str, String)> = None;
    let mut lists: Vec<Option<u64>> = Vec::new();
    let mut heading = 0;
    let mut bold = 0;
    let mut italic = 0;
    let mut strike = 0;
    let size = egui::TextStyle::Body.resolve(ui.style()).size;
    let color = ui.visuals().text_color();
    for event in events {
        match event {
            Event::Start(Tag::Heading { level, .. }) => heading = *level as u8,
            Event::End(TagEnd::Heading(_)) => {
                flush(ui, &mut job);
                heading = 0;
            }
            Event::Start(Tag::Strong) => bold += 1,
            Event::End(TagEnd::Strong) => bold -= 1,
            Event::Start(Tag::Emphasis) => italic += 1,
            Event::End(TagEnd::Emphasis) => italic -= 1,
            Event::Start(Tag::Strikethrough) => strike += 1,
            Event::End(TagEnd::Strikethrough) => strike -= 1,
            Event::Start(Tag::List(start)) => {
                flush(ui, &mut job);
                lists.push(*start);
            }
            Event::End(TagEnd::List(_)) => {
                lists.pop();
            }
            Event::Start(Tag::Item) => {
                let indent = "    ".repeat(lists.len().saturating_sub(1).min(12));
                let bullet = match lists.last_mut() {
                    Some(Some(number)) => {
                        let bullet = format!("{number}.  ");
                        *number = number.saturating_add(1);
                        bullet
                    }
                    _ => "•  ".into(),
                };
                job.append(
                    &format!("{indent}{bullet}"),
                    0.0,
                    TextFormat::simple(FontId::proportional(size), color),
                );
            }
            Event::Start(Tag::BlockQuote(_)) => {
                job.append(
                    "│  ",
                    0.0,
                    TextFormat::simple(FontId::proportional(size), color),
                );
            }
            Event::Start(Tag::CodeBlock(kind)) => {
                flush(ui, &mut job);
                let language = match kind {
                    CodeBlockKind::Fenced(info) => info.split_whitespace().next().unwrap_or(""),
                    CodeBlockKind::Indented => "",
                };
                code = Some((crate::highlight::language(language), String::new()));
            }
            Event::End(TagEnd::CodeBlock) => {
                if let Some((language, text)) = code.take() {
                    crate::highlight::show_code(ui, text.trim_end_matches('\n'), language, syntax);
                }
            }
            Event::Start(Tag::TableCell) if !job.text.is_empty() => {
                job.append(
                    "  │  ",
                    0.0,
                    TextFormat::simple(FontId::proportional(size), color),
                );
            }
            Event::End(
                TagEnd::Paragraph
                | TagEnd::Item
                | TagEnd::BlockQuote(_)
                | TagEnd::TableHead
                | TagEnd::TableRow,
            ) => {
                flush(ui, &mut job);
            }
            Event::Rule => {
                flush(ui, &mut job);
                ui.separator();
            }
            Event::Text(text) if code.is_some() => {
                if let Some((_, body)) = &mut code {
                    body.push_str(text);
                }
            }
            Event::Text(_)
            | Event::Code(_)
            | Event::Html(_)
            | Event::InlineHtml(_)
            | Event::SoftBreak
            | Event::HardBreak
            | Event::TaskListMarker(_) => {
                let (text, mono) = match event {
                    Event::Text(text) | Event::Html(text) | Event::InlineHtml(text) => {
                        (text.as_ref(), false)
                    }
                    Event::Code(text) => (text.as_ref(), true),
                    Event::TaskListMarker(true) => ("☑  ", false),
                    Event::TaskListMarker(false) => ("☐  ", false),
                    Event::HardBreak => ("\n", false),
                    _ => (" ", false),
                };
                let family = if mono {
                    FontFamily::Monospace
                } else if bold > 0 || heading > 0 {
                    FontFamily::Name("scratchpad-bold".into())
                } else {
                    FontFamily::Proportional
                };
                let font_size = size
                    * match heading {
                        1 => 1.6,
                        2 => 1.4,
                        3..=6 => 1.2,
                        _ => 1.0,
                    };
                job.append(
                    text,
                    0.0,
                    TextFormat {
                        font_id: FontId::new(font_size, family),
                        color,
                        italics: italic > 0,
                        strikethrough: if strike > 0 {
                            egui::Stroke::new(1.0_f32, color)
                        } else {
                            egui::Stroke::NONE
                        },
                        background: if mono {
                            ui.visuals().code_bg_color
                        } else {
                            egui::Color32::TRANSPARENT
                        },
                        ..Default::default()
                    },
                );
            }
            _ => {}
        }
    }
    flush(ui, &mut job);
}

fn flush(ui: &mut egui::Ui, job: &mut LayoutJob) {
    if !job.text.is_empty() {
        ui.add(
            egui::Label::new(std::mem::take(job))
                .wrap()
                .selectable(true),
        );
    }
}
