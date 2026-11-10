use eframe::egui;

pub fn grid<T>(
    ui: &mut egui::Ui,
    items: &[T],
    minimum: f32,
    mut draw: impl FnMut(&mut egui::Ui, &T, f32),
) {
    let gap = ui.spacing().item_spacing.x;
    let available = ui.available_width();
    let columns = ((available + gap) / (minimum + gap)).floor().max(1.0) as usize;
    let width = (available - gap * (columns - 1) as f32) / columns as f32;
    for row in items.chunks(columns) {
        ui.horizontal(|ui| {
            for item in row {
                draw(ui, item, width);
            }
        });
        ui.add_space(6.0);
    }
}

pub fn tile(
    ui: &mut egui::Ui,
    width: f32,
    icon: &str,
    title: &str,
    detail: &str,
    selected: bool,
) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(width, 98.0), egui::Sense::click());
    let hovered = crate::widgets::hover(ui, response.id, response.hovered());
    let base = ui.visuals().window_fill;
    let dark = ui.visuals().dark_mode;
    let shift = 8
        + (hovered * 11.0) as u8
        + if selected || response.has_focus() {
            12
        } else {
            0
        };
    let adjust = |value: u8| {
        if dark {
            value.saturating_add(shift)
        } else {
            value.saturating_sub(shift)
        }
    };
    let focused = selected || response.has_focus();
    let color = if focused {
        ui.visuals().hyperlink_color
    } else {
        ui.visuals().text_color()
    };
    let fill = if focused {
        ui.visuals().selection.bg_fill
    } else {
        egui::Color32::from_rgb(adjust(base.r()), adjust(base.g()), adjust(base.b()))
    };
    ui.painter().rect_filled(rect, 0.0, fill);
    let x = rect.left() + 12.0;
    let painter = ui.painter().with_clip_rect(rect.shrink(4.0));
    paint_icon(&painter, icon, egui::pos2(x, rect.top() + 13.0), 1.0, color);
    for (text, offset, size, color) in [
        (title, 43.0, 15.0, color),
        (detail, 67.0, 12.0, ui.visuals().weak_text_color()),
    ] {
        let mut job = egui::text::LayoutJob::simple(
            text.to_owned(),
            egui::FontId::proportional(size),
            color,
            (width - 24.0).max(1.0),
        );
        job.wrap.max_rows = 1;
        painter.galley(
            egui::pos2(x, rect.top() + offset),
            ui.fonts(|fonts| fonts.layout_job(job)),
            color,
        );
    }
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, title));
    response
}

pub fn paint_icon(
    painter: &egui::Painter,
    icon: &str,
    origin: egui::Pos2,
    scale: f32,
    color: egui::Color32,
) {
    let stroke = egui::Stroke::new(1.4 * scale, color);
    let points: &[(f32, f32)] = match icon {
        "folder" => &[
            (0., 3.),
            (7., 3.),
            (10., 6.),
            (20., 6.),
            (20., 18.),
            (0., 18.),
            (0., 3.),
        ],
        "file" | "document" | "code" | "archive" | "image" | "audio" | "video" | "spreadsheet"
        | "app" => &[
            (3., 1.),
            (13., 1.),
            (18., 6.),
            (18., 20.),
            (3., 20.),
            (3., 1.),
        ],
        _ => &[
            (0., 4.),
            (20., 4.),
            (20., 16.),
            (0., 16.),
            (0., 4.),
            (4., 4.),
            (4., 16.),
        ],
    };
    painter.add(egui::Shape::line(
        points
            .iter()
            .map(|&(x, y)| origin + egui::vec2(x, y) * scale)
            .collect(),
        stroke,
    ));
    let marks: &[&[(f32, f32)]] = match icon {
        "image" => &[&[(5., 15.), (8., 11.), (10., 13.), (13., 9.), (16., 15.)]],
        "audio" => &[&[
            (7., 16.),
            (10., 16.),
            (10., 8.),
            (15., 7.),
            (15., 10.),
            (10., 11.),
        ]],
        "video" | "app" => &[&[(7., 8.), (14., 12.), (7., 16.), (7., 8.)]],
        "archive" => &[
            &[(10., 6.), (10., 16.)],
            &[(8., 9.), (12., 9.)],
            &[(8., 13.), (12., 13.)],
        ],
        "code" => &[
            &[(8., 9.), (5., 12.), (8., 15.)],
            &[(13., 9.), (16., 12.), (13., 15.)],
        ],
        "document" => &[
            &[(6., 9.), (15., 9.)],
            &[(6., 12.), (15., 12.)],
            &[(6., 15.), (12., 15.)],
        ],
        "spreadsheet" => &[
            &[(6., 8.), (15., 8.), (15., 16.), (6., 16.), (6., 8.)],
            &[(10., 8.), (10., 16.)],
            &[(6., 12.), (15., 12.)],
        ],
        _ => &[],
    };
    for mark in marks {
        painter.add(egui::Shape::line(
            mark.iter()
                .map(|&(x, y)| origin + egui::vec2(x, y) * scale)
                .collect(),
            stroke,
        ));
    }
}

pub fn desktop_icon(
    ui: &mut egui::Ui,
    id: egui::Id,
    rect: egui::Rect,
    icon: &str,
    thumbnail: Option<&egui::TextureHandle>,
    title: &str,
    selected: bool,
) -> egui::Response {
    let response = ui.interact(rect, id, egui::Sense::click_and_drag());
    let painter = ui.painter().with_clip_rect(rect);
    let focused = selected || response.has_focus();
    let color = if focused {
        ui.visuals().hyperlink_color
    } else {
        ui.visuals().text_color()
    };
    let hover = ui
        .ctx()
        .animate_bool_with_time(id, response.hovered(), 0.14);
    let origin = egui::pos2(rect.center().x - 20.0, rect.top() + 7.0);
    if let Some(texture) = thumbnail {
        let size = texture.size_vec2();
        let scale = (66.0 / size.x).min(44.0 / size.y);
        let target = egui::Rect::from_center_size(
            egui::pos2(rect.center().x, rect.top() + 26.0),
            size * scale,
        );
        painter.image(
            texture.id(),
            target,
            egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
            egui::Color32::WHITE,
        );
    } else {
        paint_icon(
            &painter,
            icon,
            origin,
            2.0,
            color.gamma_multiply(0.8 + 0.2 * hover),
        );
    }
    let mut job = egui::text::LayoutJob::simple(
        title.into(),
        egui::FontId::proportional(13.0),
        color,
        rect.width() - 8.0,
    );
    job.wrap.max_rows = 2;
    job.halign = egui::Align::Center;
    let label = ui.fonts(|fonts| fonts.layout_job(job));
    let top = rect.top() + 52.0;
    if selected || response.has_focus() {
        let highlight = egui::Rect::from_center_size(
            egui::pos2(rect.center().x, top + label.size().y / 2.0),
            label.size() + egui::vec2(10.0, 4.0),
        );
        painter.rect_filled(highlight, 2.0, ui.visuals().selection.bg_fill);
    }
    painter.galley(egui::pos2(rect.center().x, top), label, color);
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, title));
    response
}
