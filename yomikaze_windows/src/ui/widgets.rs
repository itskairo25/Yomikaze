use egui::{
    Align2, Color32, CornerRadius, Pos2, Rect, Response, RichText, Sense, Shape, Stroke, StrokeKind, TextureHandle, Ui,
    Vec2, pos2, vec2,
};
use crate::icons as icon;

use crate::images::{Images, Kind, Loaded, State};
use crate::model::MangaSummary;
use crate::source;
use crate::theme::{self, Palette};
use crate::i18n::t;

// cropped to fill rect
pub fn paint_fill(ui: &Ui, texture: &TextureHandle, texture_size: Vec2, rect: Rect, radius: u8, tint: Color32) {
    let target = rect.width() / rect.height().max(1.0);
    let source = texture_size.x / texture_size.y.max(1.0);
    let uv = if source > target {
        let span = target / source;
        Rect::from_min_max(pos2(0.5 - span / 2.0, 0.0), pos2(0.5 + span / 2.0, 1.0))
    } else {
        let span = source / target;
        Rect::from_min_max(pos2(0.0, 0.5 - span / 2.0), pos2(1.0, 0.5 + span / 2.0))
    };
    egui::Image::from_texture((texture.id(), texture_size))
        .uv(uv)
        .corner_radius(CornerRadius::same(radius))
        .tint(tint)
        .paint_at(ui, rect);
}

pub fn paint_image(ui: &Ui, state: State<'_>, rect: Rect, radius: u8, tint: Color32) {
    let palette = Palette::of(ui.ctx());
    match state {
        State::Ready(loaded) => paint_loaded(ui, loaded, rect, radius, tint),
        _ => {
            ui.painter().rect_filled(rect, CornerRadius::same(radius), palette.quaternary);
        }
    }
}

pub fn paint_loaded(ui: &Ui, loaded: &Loaded, rect: Rect, radius: u8, tint: Color32) {
    paint_fill(ui, loaded.first(), loaded.tiles[0].1, rect, radius, tint);
}

pub fn paint_line(painter: &egui::Painter, pos: Pos2, text: &str, font: egui::FontId, color: Color32, max_width: f32) {
    let mut job = egui::text::LayoutJob::single_section(text.to_string(), egui::TextFormat::simple(font, color));
    job.wrap = egui::text::TextWrapping {
        max_width,
        max_rows: 1,
        break_anywhere: true,
        overflow_character: Some('…'),
    };
    let galley = painter.layout_job(job);
    painter.galley(pos, galley, color);
}

pub fn cover(ui: &mut Ui, images: &mut Images, manga_id: &str, width: f32, radius: u8, small: bool) -> Response {
    let size = vec2(width, width * 1.5);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let hover = ui.ctx().animate_bool_with_time(response.id.with("hover"), response.hovered(), 0.18);
    // the cover lifts a little under the mouse
    let lift = 4.0 * theme::ease_out_cubic(hover);
    let shown = rect.translate(vec2(0.0, -lift));
    if hover > 0.0 {
        theme::soft_shadow(ui.painter(), shown, radius, 6.0 + lift, 20.0, Color32::from_black_alpha((80.0 * hover) as u8));
    }
    paint_cover(ui, images, manga_id, shown, radius, small);
    if hover > 0.0 {
        let palette = Palette::of(ui.ctx());
        ui.painter().rect_filled(shown, CornerRadius::same(radius), Color32::from_white_alpha((10.0 * hover) as u8));
        ui.painter().rect_stroke(shown, CornerRadius::same(radius), Stroke::new(1.0, palette.accent.gamma_multiply(0.8 * hover)), StrokeKind::Inside);
    }
    response
}

pub fn paint_cover(ui: &Ui, images: &mut Images, manga_id: &str, rect: Rect, radius: u8, small: bool) {
    let palette = Palette::of(ui.ctx());
    let url = source::cover_url(manga_id, small);
    let state = images.get(&url, Kind::Cover);
    // covers that arrive from the network fade in, cached ones show at once
    let ready = matches!(state, State::Ready(_));
    let fade = ui.ctx().animate_bool_with_time(egui::Id::new(("cover-fade", &url)), ready, 0.3);
    if ready && fade < 1.0 {
        ui.painter().rect_filled(rect, CornerRadius::same(radius), palette.quaternary);
    }
    paint_image(ui, state, rect, radius, Color32::WHITE.gamma_multiply(fade));
    ui.painter().rect_stroke(
        rect,
        CornerRadius::same(radius),
        Stroke::new(1.0, palette.glass_stroke),
        StrokeKind::Inside,
    );
}

pub fn paint_blurred_cover(ui: &Ui, images: &mut Images, manga_id: &str, rect: Rect, radius: u8, alpha: f32) {
    let url = source::cover_url(manga_id, true);
    if let State::Ready(loaded) = images.get(&url, Kind::Blur) {
        paint_fill(ui, loaded.first(), loaded.tiles[0].1, rect, radius, Color32::from_white_alpha((255.0 * alpha) as u8));
    }
}

pub fn manga_card(ui: &mut Ui, images: &mut Images, manga: &MangaSummary, subtitle: Option<&str>, width: f32) -> Response {
    let palette = Palette::of(ui.ctx());
    let response = ui
        .vertical(|ui| {
            ui.set_width(width);
            ui.spacing_mut().item_spacing.y = 4.0;
            let cover = cover(ui, images, &manga.id, width, 14, false);
            let title = ui.add(
                egui::Label::new(RichText::new(&manga.title).font(theme::bold_font(13.5)))
                    .truncate()
                    .sense(Sense::click()),
            );
            if let Some(subtitle) = subtitle.filter(|s| !s.is_empty()) {
                ui.add(egui::Label::new(RichText::new(subtitle).size(12.0).color(palette.secondary)).truncate());
            }
            cover.union(title)
        })
        .inner;
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

pub fn manga_row(ui: &mut Ui, images: &mut Images, manga: &MangaSummary, detail: Option<&str>) -> Response {
    let palette = Palette::of(ui.ctx());
    let width = ui.available_width();
    let (rect, response) = ui.allocate_exact_size(vec2(width, 96.0), Sense::click());
    let hover = ui.ctx().animate_bool_with_time(response.id.with("hover"), response.hovered(), 0.15);
    if hover > 0.0 {
        ui.painter().rect_filled(rect.expand(4.0), CornerRadius::same(16), palette.quaternary.gamma_multiply(hover));
    }
    let cover_rect = Rect::from_min_size(rect.min + vec2(4.0, 2.0), vec2(61.0, 92.0));
    paint_cover(ui, images, &manga.id, cover_rect, 10, true);

    let text_left = cover_rect.right() + 16.0;
    let text_width = rect.right() - text_left - 8.0;
    let title_galley = ui.painter().layout(
        manga.title.clone(),
        theme::bold_font(16.0),
        palette.text,
        text_width,
    );
    let line = detail.map(str::to_string).unwrap_or_else(|| manga.meta_line());
    let detail_galley = ui.painter().layout(line, theme::font(13.0), palette.secondary, text_width);
    let total = title_galley.size().y + 4.0 + detail_galley.size().y;
    let top = rect.center().y - total / 2.0;
    ui.painter().galley(pos2(text_left, top), title_galley.clone(), palette.text);
    ui.painter().galley(pos2(text_left, top + title_galley.size().y + 4.0), detail_galley, palette.secondary);
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

pub fn chip(ui: &mut Ui, text: &str, glyph: Option<&str>) -> Response {
    let palette = Palette::of(ui.ctx());
    let label = match glyph {
        Some(g) => format!("{g}  {text}"),
        None => text.to_string(),
    };
    let galley = ui.painter().layout_no_wrap(label, theme::bold_font(12.5), palette.text);
    let size = galley.size() + vec2(24.0, 12.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::hover());
    ui.painter().rect_filled(rect, CornerRadius::same(255), palette.quaternary);
    ui.painter().galley(rect.center() - galley.size() / 2.0, galley, palette.text);
    response
}

pub fn pill(ui: &mut Ui, text: &str, glyph: Option<&str>, selected: bool, min_width: f32) -> Response {
    let palette = Palette::of(ui.ctx());
    let label = match glyph {
        Some(g) => format!("{g}  {text}"),
        None => text.to_string(),
    };
    let fg = if selected { theme::readable_on(palette.accent) } else { palette.text };
    let galley = ui.painter().layout_no_wrap(label, theme::bold_font(14.0), fg);
    let size = vec2((galley.size().x + 32.0).max(min_width), 38.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let t = ui.ctx().animate_bool_with_time(response.id.with("sel"), selected, 0.18);
    let hover = ui.ctx().animate_bool_with_time(response.id.with("hov"), response.hovered() && !selected, 0.12);
    let base = palette.glass_strong.gamma_multiply(0.85);
    if t > 0.0 {
        theme::soft_shadow(ui.painter(), rect, 255, 3.0, 12.0, palette.accent.gamma_multiply(0.25 * t));
    }
    ui.painter().rect_filled(rect, CornerRadius::same(255), lerp_color(base, palette.accent, t));
    if hover > 0.0 {
        ui.painter().rect_filled(rect, CornerRadius::same(255), palette.quaternary.gamma_multiply(1.6 * hover));
    }
    if t < 1.0 {
        ui.painter().rect_stroke(rect, CornerRadius::same(255), Stroke::new(1.0, palette.glass_stroke.gamma_multiply(1.0 - t)), StrokeKind::Inside);
    }
    ui.painter().galley(rect.center() - galley.size() / 2.0, galley, fg);
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

// a smaller selectable pill, for long rows of choices
pub fn chip_button(ui: &mut Ui, text: &str, selected: bool) -> Response {
    let palette = Palette::of(ui.ctx());
    let fg = if selected { theme::readable_on(palette.accent) } else { palette.text };
    let galley = ui.painter().layout_no_wrap(text.to_string(), theme::bold_font(12.5), fg);
    let (rect, response) = ui.allocate_exact_size(galley.size() + vec2(24.0, 14.0), Sense::click());
    let t = ui.ctx().animate_bool_with_time(response.id.with("sel"), selected, 0.15);
    let hover = ui.ctx().animate_bool_with_time(response.id.with("hov"), response.hovered() && !selected, 0.12);
    ui.painter().rect_filled(rect, CornerRadius::same(255), lerp_color(palette.quaternary, palette.accent, t));
    if hover > 0.0 {
        ui.painter().rect_filled(rect, CornerRadius::same(255), palette.quaternary.gamma_multiply(1.4 * hover));
    }
    ui.painter().galley(rect.center() - galley.size() / 2.0, galley, fg);
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

pub fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let mix = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    // Color32 is premultiplied, so mixing the stored values is the correct blend
    Color32::from_rgba_premultiplied(mix(a.r(), b.r()), mix(a.g(), b.g()), mix(a.b(), b.b()), mix(a.a(), b.a()))
}

// a button label and the button's width. a given width is a minimum, so longer translations
// make the button wider. if even that doesn't fit the row, the label is cut with "…".
fn fit_label(ui: &Ui, label: String, font: egui::FontId, color: Color32, width: Option<f32>, padding: f32) -> (std::sync::Arc<egui::Galley>, f32) {
    let natural = ui.painter().layout_no_wrap(label.clone(), font.clone(), color);
    let wanted = (natural.size().x + padding).max(width.unwrap_or(0.0));
    let room = ui.available_width().max(padding + 40.0);
    if wanted <= room {
        return (natural, wanted);
    }
    let mut job = egui::text::LayoutJob::single_section(label, egui::TextFormat::simple(font, color));
    job.wrap = egui::text::TextWrapping {
        max_width: room - padding,
        max_rows: 1,
        break_anywhere: true,
        overflow_character: Some('…'),
    };
    (ui.painter().layout_job(job), room)
}

pub fn primary_button(ui: &mut Ui, text: &str, glyph: Option<&str>, width: Option<f32>, enabled: bool) -> Response {
    let palette = Palette::of(ui.ctx());
    let fg = theme::readable_on(palette.accent);
    let label = match glyph {
        Some(g) => format!("{g}   {text}"),
        None => text.to_string(),
    };
    let (galley, button_width) = fit_label(ui, label, theme::bold_font(15.5), fg, width, 40.0);
    let size = vec2(button_width, 44.0);
    let sense = if enabled { Sense::click() } else { Sense::hover() };
    let (rect, response) = ui.allocate_exact_size(size, sense);
    let hover = ui.ctx().animate_bool_with_time(response.id.with("hover"), response.hovered() && enabled, 0.15);
    let mut fill = lerp_color(palette.accent, palette.accent.gamma_multiply(1.15), hover);
    if !enabled {
        fill = fill.gamma_multiply(0.45);
    } else {
        // a soft glow in the accent colour that grows a little under the mouse
        theme::soft_shadow(ui.painter(), rect, 255, 4.0 + 2.0 * hover, 14.0 + 6.0 * hover, palette.accent.gamma_multiply(0.28 + 0.14 * hover));
    }
    ui.painter().rect_filled(rect, CornerRadius::same(255), fill);
    let sheen = Rect::from_min_max(rect.min, pos2(rect.max.x, rect.center().y));
    ui.painter().rect_filled(
        sheen.shrink2(vec2(6.0, 2.0)),
        CornerRadius { nw: 20, ne: 20, sw: 4, se: 4 },
        Color32::from_white_alpha(18),
    );
    let text_pos = Align2::CENTER_CENTER.anchor_size(rect.center(), galley.size()).min;
    ui.painter().galley(text_pos, galley, fg);
    if enabled { response.on_hover_cursor(egui::CursorIcon::PointingHand) } else { response }
}

pub fn glass_button(ui: &mut Ui, text: &str, glyph: Option<&str>, width: Option<f32>) -> Response {
    let palette = Palette::of(ui.ctx());
    let label = match glyph {
        Some(g) => format!("{g}   {text}"),
        None => text.to_string(),
    };
    let (galley, button_width) = fit_label(ui, label, theme::bold_font(14.5), palette.text, width, 36.0);
    let size = vec2(button_width, 40.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let hover = ui.ctx().animate_bool_with_time(response.id.with("hover"), response.hovered(), 0.15);
    // fairly solid so it reads well over banners and pictures
    ui.painter().rect_filled(rect, CornerRadius::same(255), palette.glass_strong.gamma_multiply(0.85));
    ui.painter().rect_filled(rect, CornerRadius::same(255), palette.quaternary.gamma_multiply(1.4 * hover));
    ui.painter().rect_stroke(rect, CornerRadius::same(255), Stroke::new(1.0, palette.glass_stroke), StrokeKind::Inside);
    ui.painter().galley(rect.center() - galley.size() / 2.0, galley, palette.text);
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

pub fn icon_button(ui: &mut Ui, glyph: &str, size: f32, tint: Option<Color32>) -> Response {
    let font = theme::icon_font(size * 0.45);
    let palette = Palette::of(ui.ctx());
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(size), Sense::click());
    let hover = ui.ctx().animate_bool_with_time(response.id.with("hover"), response.hovered(), 0.15);
    ui.painter().circle_filled(rect.center(), size / 2.0, palette.glass_strong.gamma_multiply(0.85));
    ui.painter().circle_filled(rect.center(), size / 2.0, palette.quaternary.gamma_multiply(1.4 * hover));
    ui.painter().circle_stroke(rect.center(), size / 2.0 - 0.5, Stroke::new(1.0, palette.glass_stroke));
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        glyph,
        font,
        tint.unwrap_or(palette.text),
    );
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

// track with an accent fill and a round white knob that grows under the mouse.
// step snaps the value, logarithmic suits ranges that span a lot.
pub fn slider(ui: &mut Ui, value: &mut f32, range: std::ops::RangeInclusive<f32>, step: Option<f32>, logarithmic: bool) -> Response {
    let palette = Palette::of(ui.ctx());
    let (low, high) = (*range.start(), *range.end());
    let width = ui.spacing().slider_width;
    let (rect, mut response) = ui.allocate_exact_size(vec2(width, 28.0), Sense::click_and_drag());
    let track = Rect::from_min_max(pos2(rect.left() + 10.0, rect.center().y - 2.5), pos2(rect.right() - 10.0, rect.center().y + 2.5));
    let position = |v: f32| {
        if logarithmic {
            (v.max(low).ln() - low.ln()) / (high.ln() - low.ln())
        } else {
            (v - low) / (high - low)
        }
    };
    let value_at = |t: f32| if logarithmic { (low.ln() + t * (high.ln() - low.ln())).exp() } else { low + t * (high - low) };
    if response.is_pointer_button_down_on() || response.dragged() {
        if let Some(pointer) = response.interact_pointer_pos() {
            let mut next = value_at(((pointer.x - track.left()) / track.width()).clamp(0.0, 1.0));
            if let Some(step) = step {
                next = low + ((next - low) / step).round() * step;
            }
            next = next.clamp(low, high);
            if next != *value {
                *value = next;
                response.mark_changed();
            }
        }
    }
    let t = position(*value).clamp(0.0, 1.0);
    let active = ui.ctx().animate_bool_with_time(response.id.with("knob"), response.hovered() || response.dragged(), 0.12);
    let painter = ui.painter();
    let dark = ui.visuals().dark_mode;
    painter.rect_filled(track, CornerRadius::same(255), if dark { Color32::from_white_alpha(34) } else { Color32::from_black_alpha(26) });
    let filled = Rect::from_min_max(track.min, pos2(track.left() + track.width() * t, track.max.y));
    painter.rect_filled(filled, CornerRadius::same(255), palette.accent);
    let knob = pos2(track.left() + track.width() * t, track.center().y);
    let radius = 8.5 + 1.5 * active;
    painter.circle_filled(knob + vec2(0.0, 1.0), radius + 0.5, Color32::from_black_alpha(45));
    painter.circle_filled(knob, radius, Color32::WHITE);
    if active > 0.0 {
        painter.circle_stroke(knob, radius + 3.0, Stroke::new(2.0, palette.accent.gamma_multiply(0.35 * active)));
    }
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

// returns true when the value changed
pub fn toggle_row(ui: &mut Ui, value: &mut bool, label: &str) -> bool {
    let palette = Palette::of(ui.ctx());
    let width = ui.available_width();
    let label_width = width - 70.0;
    let galley = ui.painter().layout(label.to_string(), theme::font(15.0), palette.text, label_width);
    let height = galley.size().y.max(30.0) + 8.0;
    let (rect, response) = ui.allocate_exact_size(vec2(width, height), Sense::click());
    ui.painter().galley(pos2(rect.left(), rect.center().y - galley.size().y / 2.0), galley, palette.text);

    let track = Rect::from_center_size(pos2(rect.right() - 26.0, rect.center().y), vec2(50.0, 30.0));
    let t = ui.ctx().animate_bool_with_time(response.id, *value, 0.16);
    let off = if palette.dark { Color32::from_gray(72) } else { Color32::from_gray(205) };
    ui.painter().rect_filled(track, CornerRadius::same(255), lerp_color(off, palette.accent, t));
    let knob = pos2(track.left() + 15.0 + t * 20.0, track.center().y);
    ui.painter().circle_filled(knob + vec2(0.0, 1.0), 12.5, Color32::from_black_alpha(40));
    ui.painter().circle_filled(knob, 12.0, Color32::WHITE);

    if response.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
        *value = !*value;
        return true;
    }
    false
}

pub fn link_button(ui: &mut Ui, text: &str) -> Response {
    let palette = Palette::of(ui.ctx());
    ui.add(egui::Button::new(RichText::new(text).font(theme::bold_font(14.0)).color(palette.accent)).frame(false))
        .on_hover_cursor(egui::CursorIcon::PointingHand)
}

pub fn progress_bar(ui: &mut Ui, fraction: f32, height: f32) {
    let palette = Palette::of(ui.ctx());
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(vec2(width, height), Sense::hover());
    paint_progress_bar(ui, rect, fraction, palette);
}

pub fn paint_progress_bar(ui: &Ui, rect: Rect, fraction: f32, palette: Palette) {
    ui.painter().rect_filled(rect, CornerRadius::same(255), palette.quaternary);
    let filled = Rect::from_min_size(rect.min, vec2(rect.width() * fraction.clamp(0.0, 1.0), rect.height()));
    if filled.width() > 0.5 {
        ui.painter().rect_filled(filled, CornerRadius::same(255), palette.accent);
    }
}

pub fn progress_ring(ui: &mut Ui, fraction: f32, size: f32, line: f32) {
    let palette = Palette::of(ui.ctx());
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    let center = rect.center();
    let radius = size / 2.0 - line / 2.0;
    ui.painter().circle_stroke(center, radius, Stroke::new(line, palette.quaternary));
    let fraction = fraction.clamp(0.0, 1.0);
    if fraction > 0.0 {
        let steps = (64.0 * fraction).max(2.0) as usize;
        let points: Vec<Pos2> = (0..=steps)
            .map(|i| {
                let angle = -std::f32::consts::FRAC_PI_2 + std::f32::consts::TAU * fraction * i as f32 / steps as f32;
                center + Vec2::angled(angle) * radius
            })
            .collect();
        ui.painter().add(Shape::line(points, Stroke::new(line, palette.accent)));
    }
    ui.painter().text(
        center,
        Align2::CENTER_CENTER,
        format!("{}%", (fraction * 100.0).round() as i32),
        theme::bold_font(size * 0.22),
        palette.text,
    );
}

// own vertical block so it lines up with the page edge even inside a row
pub fn page_title(ui: &mut Ui, title: &str) {
    ui.vertical(|ui| {
        ui.add_space(6.0);
        ui.label(RichText::new(title).font(theme::heavy_font(34.0)));
        ui.add_space(8.0);
    });
}

pub fn empty_state(ui: &mut Ui, glyph: &str, title: &str, message: &str) {
    let palette = Palette::of(ui.ctx());
    ui.vertical_centered(|ui| {
        ui.add_space(40.0);
        ui.label(RichText::new(glyph).size(46.0).color(palette.secondary));
        ui.add_space(6.0);
        ui.label(RichText::new(title).font(theme::bold_font(20.0)));
        ui.add_space(2.0);
        ui.add(egui::Label::new(RichText::new(message).size(14.0).color(palette.secondary)).wrap());
        ui.add_space(10.0);
    });
}

// returns true when the user asks to retry
pub fn error_state(ui: &mut Ui, message: &str) -> bool {
    let mut retry = false;
    empty_state(ui, icon::WIFI_X, t("Couldn't load"), message);
    ui.vertical_centered(|ui| {
        retry = primary_button(ui, t("Try again"), None, Some(140.0), true).clicked();
    });
    retry
}

pub fn offline_banner(ui: &mut Ui, text: &str) {
    let palette = Palette::of(ui.ctx());
    egui::Frame::new()
        .fill(palette.quaternary)
        .corner_radius(CornerRadius::same(14))
        .inner_margin(egui::Margin::symmetric(14, 10))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new(format!("{}   {text}", icon::WIFI_SLASH)).size(13.5).color(palette.secondary));
        });
}

pub fn spinner(ui: &mut Ui) {
    ui.vertical_centered(|ui| {
        ui.add_space(30.0);
        ui.add(egui::Spinner::new().size(28.0));
        ui.add_space(30.0);
    });
}

// as many columns as fit, each at least min wide
pub fn grid_columns(available: f32, min: f32, spacing: f32) -> (usize, f32) {
    let columns = ((available + spacing) / (min + spacing)).floor().max(1.0) as usize;
    let width = (available - spacing * (columns as f32 - 1.0)) / columns as f32;
    (columns, width)
}
