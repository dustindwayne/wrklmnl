//! Lucide SVG geometry adapted to native strokes. See assets/icons/LICENSE (ISC/MIT).
use eframe::egui::{self, Color32, Pos2, Rect, Response, Sense, Stroke, Vec2};
pub enum Icon {
    Home,
    Back,
    Forward,
    Up,
    Down,
    Refresh,
    History,
    Serial,
    Transfer,
    Folder,
    Preview,
    Search,
    Terminal,
    Code,
    Palette,
    Settings,
    Keys,
    Grid,
    More,
    Plus,
    Minimize,
    Maximize,
    Restore,
    Close,
}
/// Shell identifiers use text and geometry only, with no image assets.
pub fn shell(p: &egui::Painter, rect: Rect, label: &str, color: Color32) {
    let lower = label.to_lowercase();
    let mark = if lower.contains("python") {
        "Py"
    } else if lower.contains("conda") {
        "Co"
    } else if lower.contains("node") {
        "JS"
    } else if lower.contains("nushell") {
        "Nu"
    } else if lower.contains("command prompt") {
        "C:"
    } else if lower.contains("powershell") {
        ">_"
    } else if lower.contains("bash")
        || lower.contains("posix")
        || lower.contains("zsh")
        || lower.contains("fish")
    {
        "$_"
    } else {
        ">_"
    };
    p.text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        mark,
        egui::FontId::monospace(rect.height() * 0.7),
        color,
    );
}
pub fn button(ui: &mut egui::Ui, icon: Icon, label: &str) -> Response {
    let response = crate::widgets::add(
        ui,
        egui::Button::new("")
            .frame(false)
            .min_size(Vec2::new(32.0, 28.0))
            .sense(Sense::click()),
    );
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), label)
    });
    let color = if !ui.is_enabled() {
        ui.visuals().weak_text_color().gamma_multiply(0.5)
    } else if response.hovered() || response.has_focus() {
        ui.visuals().hyperlink_color
    } else {
        ui.visuals().text_color()
    };
    let rect = Rect::from_center_size(response.rect.center(), Vec2::splat(17.0));
    paint(ui.painter(), rect, icon, color);
    response.on_hover_text(label)
}
pub fn menu<R>(ui: &mut egui::Ui, label: &str, items: impl FnOnce(&mut egui::Ui) -> R) {
    #[cfg(feature = "capture")]
    if std::env::var_os("TERMINAL_CAPTURE_TO").is_some()
        && std::env::var("DTERM_CAPTURE_MENU").ok().as_deref() == Some(label)
    {
        let response = button(ui, Icon::More, label);
        let anchor = response.rect.left_bottom() + Vec2::new(0.0, 3.0);
        let _ = egui::menu::MenuRoot::new(anchor, response.id).show(&response, items);
        return;
    }
    let menu = ui.menu_button("    ", items);
    let response = menu.response;
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label));
    let rect = Rect::from_center_size(response.rect.center(), Vec2::splat(17.0));
    paint(ui.painter(), rect, Icon::More, ui.visuals().text_color());
    response.on_hover_text(label);
}

pub fn combo_menu_icon(combo: egui::ComboBox) -> egui::ComboBox {
    combo.icon(|ui, rect, visuals, _, _| {
        paint(ui.painter(), rect, Icon::More, visuals.fg_stroke.color)
    })
}

pub fn paint(p: &egui::Painter, rect: Rect, icon: Icon, color: Color32) {
    let scale = rect.width() / 24.0;
    let point = |x: f32, y: f32| Pos2::new(rect.min.x + x * scale, rect.min.y + y * scale);
    let stroke = Stroke::new(2.0 * scale, color);
    let line = |a: (f32, f32), b: (f32, f32)| {
        p.line_segment([point(a.0, a.1), point(b.0, b.1)], stroke);
    };
    match icon {
        Icon::Back | Icon::Forward | Icon::Up | Icon::Down => {
            let transform = |(x, y)| match icon {
                Icon::Forward => (24.0 - x, y),
                Icon::Up => (y, x),
                Icon::Down => (y, 24.0 - x),
                _ => (x, y),
            };
            for (a, b) in [
                ((4., 12.), (20., 12.)),
                ((4., 12.), (10., 6.)),
                ((4., 12.), (10., 18.)),
            ] {
                line(transform(a), transform(b));
            }
        }
        Icon::Serial => {
            for x in [8., 16.] {
                line((x, 3.), (x, 8.));
            }
            p.add(egui::Shape::line(
                vec![
                    point(5., 8.),
                    point(19., 8.),
                    point(19., 12.),
                    point(16., 16.),
                    point(8., 16.),
                    point(5., 12.),
                    point(5., 8.),
                ],
                stroke,
            ));
            line((12., 16.), (12., 22.));
        }
        Icon::Transfer => {
            for (a, b) in [
                ((3., 8.), (21., 8.)),
                ((16., 3.), (21., 8.)),
                ((16., 13.), (21., 8.)),
                ((21., 16.), (3., 16.)),
                ((8., 11.), (3., 16.)),
                ((8., 21.), (3., 16.)),
            ] {
                line(a, b);
            }
        }
        Icon::History => {
            p.circle_stroke(point(12., 12.), 9.0 * scale, stroke);
            line((12., 6.), (12., 12.));
            line((12., 12.), (16., 14.));
        }
        Icon::Refresh => {
            p.add(egui::Shape::line(
                vec![
                    point(20., 8.),
                    point(16., 4.),
                    point(9., 4.),
                    point(4., 8.),
                    point(4., 15.),
                    point(9., 20.),
                    point(16., 20.),
                    point(20., 16.),
                ],
                stroke,
            ));
            line((20., 3.), (20., 9.));
            line((20., 9.), (14., 9.));
        }
        Icon::Preview => {
            p.rect_stroke(
                Rect::from_min_max(point(3., 5.), point(21., 19.)),
                0.0,
                stroke,
                egui::StrokeKind::Middle,
            );
            line((14., 5.), (14., 19.));
            line((17., 9.), (19., 9.));
            line((17., 12.), (19., 12.));
        }
        Icon::Folder => {
            p.add(egui::Shape::line(
                vec![
                    point(3., 6.),
                    point(9., 6.),
                    point(12., 9.),
                    point(21., 9.),
                    point(21., 19.),
                    point(3., 19.),
                    point(3., 6.),
                ],
                stroke,
            ));
        }
        Icon::Search => {
            p.circle_stroke(point(10., 10.), 6.0 * scale, stroke);
            line((14.5, 14.5), (21., 21.));
        }
        Icon::Home => {
            line((3.0, 10.0), (12.0, 3.0));
            line((12.0, 3.0), (21.0, 10.0));
            line((5.0, 10.0), (5.0, 21.0));
            line((5.0, 21.0), (19.0, 21.0));
            line((19.0, 21.0), (19.0, 10.0));
            line((10.0, 21.0), (10.0, 15.0));
            line((10.0, 15.0), (14.0, 15.0));
            line((14.0, 15.0), (14.0, 21.0));
        }
        Icon::Terminal => {
            line((4.0, 6.0), (10.0, 12.0));
            line((10.0, 12.0), (4.0, 18.0));
            line((13.0, 18.0), (21.0, 18.0));
        }
        Icon::Code => {
            line((7.0, 6.0), (2.0, 12.0));
            line((2.0, 12.0), (7.0, 18.0));
            line((17.0, 6.0), (22.0, 12.0));
            line((22.0, 12.0), (17.0, 18.0));
            line((14.0, 3.0), (10.0, 21.0));
        }
        Icon::Grid => {
            for x in [4.0, 14.0] {
                for y in [4.0, 14.0] {
                    p.rect_stroke(
                        Rect::from_min_max(point(x, y), point(x + 6.0, y + 6.0)),
                        0.0,
                        stroke,
                        egui::StrokeKind::Middle,
                    );
                }
            }
        }
        Icon::Keys => {
            p.rect_stroke(
                Rect::from_min_max(point(2.0, 5.0), point(22.0, 19.0)),
                0.0,
                stroke,
                egui::StrokeKind::Middle,
            );
            for x in [6.0, 10.0, 14.0, 18.0] {
                line((x, 9.0), (x + 0.1, 9.0));
            }
            line((6.0, 13.0), (6.1, 13.0));
            line((10.0, 13.0), (18.0, 13.0));
        }
        Icon::Settings => {
            for y in [5.0, 12.0, 19.0] {
                line((3.0, y), (21.0, y));
            }
            for (x, y) in [(8.0, 5.0), (16.0, 12.0), (10.0, 19.0)] {
                p.circle_filled(point(x, y), 2.5 * scale, color);
            }
        }
        Icon::Palette => {
            p.circle_stroke(point(12.0, 12.0), 9.0 * scale, stroke);
            for (x, y) in [(8.0, 8.0), (14.0, 6.5), (18.0, 11.0), (7.0, 15.0)] {
                p.circle_filled(point(x, y), 1.3 * scale, color);
            }
        }
        Icon::More => {
            for x in [5.0, 12.0, 19.0] {
                p.circle_filled(point(x, 12.0), 1.4 * scale, color);
            }
        }
        Icon::Plus => {
            line((12.0, 4.0), (12.0, 20.0));
            line((4.0, 12.0), (20.0, 12.0));
        }
        Icon::Minimize => line((5.0, 12.0), (19.0, 12.0)),
        Icon::Maximize => {
            p.rect_stroke(
                Rect::from_min_max(point(3.0, 3.0), point(21.0, 21.0)),
                2.0 * scale,
                stroke,
                egui::StrokeKind::Middle,
            );
        }
        Icon::Restore => {
            p.rect_stroke(
                Rect::from_min_max(point(8.0, 8.0), point(22.0, 22.0)),
                2.0 * scale,
                stroke,
                egui::StrokeKind::Middle,
            );
            p.add(egui::epaint::PathShape::line(
                vec![
                    point(4.0, 16.0),
                    point(2.6, 15.4),
                    point(2.0, 14.0),
                    point(2.0, 4.0),
                    point(2.6, 2.6),
                    point(4.0, 2.0),
                    point(14.0, 2.0),
                    point(15.4, 2.6),
                    point(16.0, 4.0),
                ],
                stroke,
            ));
        }
        Icon::Close => {
            line((18.0, 6.0), (6.0, 18.0));
            line((6.0, 6.0), (18.0, 18.0));
        }
    }
}
// Retain upstream notices in the executable as well as the source package.
pub const LICENSE: &str = include_str!("../../../assets/icons/LICENSE");
