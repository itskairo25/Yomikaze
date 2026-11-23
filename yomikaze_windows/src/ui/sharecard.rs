// share card: a 1080x1920 picture of a series for the clipboard or a file

use std::path::PathBuf;

use ab_glyph::{Font, FontVec, PxScale, ScaleFont, point};
use egui::{RichText, Ui, vec2};
use crate::icons as icon;
use image::{Rgba, RgbaImage, imageops::FilterType};
use poll_promise::Promise;

use super::widgets;
use crate::app::{Shared, spawn};
use crate::images::{Kind, State, disk};
use crate::source;
use crate::theme::{self, Palette};
use crate::i18n::{t, tf};

const W: u32 = 1080;
const H: u32 = 1920;

pub struct CardInfo {
    pub manga_id: String,
    pub title: String,
    pub authors: String,
    pub chapter_count: usize,
    pub status: String,
    pub kind: String,
    pub year: String,
    pub tags: Vec<String>,
    pub accent: [u8; 3],
}

#[derive(Default)]
pub struct ShareState {
    request: Option<Promise<Result<PathBuf, String>>>,
    result: Option<Result<PathBuf, String>>,
    open: bool,
}

impl ShareState {
    pub fn start(&mut self, ctx: &egui::Context, info: CardInfo) {
        self.open = true;
        self.result = None;
        self.request = Some(spawn(ctx, move || render(&info)));
    }
}

pub fn show_modal(ui: &mut Ui, state: &mut ShareState, shared: &mut Shared, palette: &Palette) {
    if !state.open {
        return;
    }
    if let Some(promise) = &state.request {
        if let Some(result) = promise.ready() {
            state.result = Some(result.clone());
            state.request = None;
        }
    }

    let mut close = false;
    let modal = egui::Modal::new(egui::Id::new("share-card"))
        .frame(theme::modal_frame(palette))
        .show(ui.ctx(), |ui| {
            ui.set_width(420.0);
            ui.horizontal(|ui| {
                ui.label(RichText::new(t("Share card")).font(theme::bold_font(22.0)));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if widgets::icon_button(ui, icon::X, 34.0, None).clicked() {
                        close = true;
                    }
                });
            });
            ui.add_space(10.0);
            match &state.result {
                None => {
                    ui.vertical_centered(|ui| {
                        ui.add_space(120.0);
                        ui.add(egui::Spinner::new().size(30.0));
                        ui.label(RichText::new(t("Drawing the card...")).color(palette.secondary));
                        ui.add_space(120.0);
                    });
                }
                Some(Err(error)) => {
                    widgets::empty_state(ui, icon::WARNING_CIRCLE, t("Couldn't make the card"), error);
                }
                Some(Ok(path)) => {
                    let url = format!("file:{}", path.display());
                    let preview_height = 560.0;
                    let size = vec2(preview_height * W as f32 / H as f32, preview_height);
                    ui.vertical_centered(|ui| {
                        let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
                        match shared.images.get(&url, Kind::Wide) {
                            State::Ready(loaded) => {
                                egui::Image::from_texture((loaded.first().id(), loaded.tiles[0].1))
                                    .corner_radius(egui::CornerRadius::same(18))
                                    .paint_at(ui, rect);
                            }
                            _ => {
                                ui.painter().rect_filled(rect, egui::CornerRadius::same(18), palette.quaternary);
                            }
                        }
                    });
                    ui.add_space(14.0);
                    ui.horizontal(|ui| {
                        if widgets::primary_button(ui, t("Copy image"), Some(icon::COPY), Some(170.0), true).clicked() {
                            match copy_to_clipboard(path) {
                                Ok(()) => shared.router.toast(t("Card copied, paste it into any chat")),
                                Err(e) => shared.router.toast_warning(tf("Couldn't copy: {error}", &[("error", &e)])),
                            }
                        }
                        if widgets::glass_button(ui, t("Save as..."), Some(icon::FLOPPY_DISK), Some(120.0)).clicked() {
                            if let Some(target) = rfd::FileDialog::new()
                                .set_file_name(path.file_name().and_then(|n| n.to_str()).unwrap_or("card.png"))
                                .add_filter(t("PNG image"), &["png"])
                                .save_file()
                            {
                                let _ = std::fs::copy(path, target);
                            }
                        }
                        if widgets::icon_button(ui, icon::FOLDER_OPEN, 40.0, None).on_hover_text(t("Show in folder")).clicked() {
                            if let Some(dir) = path.parent() {
                                let _ = open::that(dir);
                            }
                        }
                    });
                    ui.label(RichText::new(tf("Saved to {path}", &[("path", &path.display())])).size(11.5).color(palette.tertiary));
                }
            }
        });
    if close || modal.should_close() {
        state.open = false;
        state.result = None;
    }
}

fn copy_to_clipboard(path: &PathBuf) -> Result<(), String> {
    let image = image::open(path).map_err(|e| e.to_string())?.to_rgba8();
    let (width, height) = image.dimensions();
    let mut clipboard = arboard::Clipboard::new().map_err(|e| e.to_string())?;
    clipboard
        .set_image(arboard::ImageData { width: width as usize, height: height as usize, bytes: image.into_raw().into() })
        .map_err(|e| e.to_string())
}

struct Fonts {
    regular: Vec<FontVec>,
    bold: Vec<FontVec>,
    heavy: Vec<FontVec>,
}

fn load_fonts() -> Fonts {
    let dir = PathBuf::from(std::env::var("WINDIR").unwrap_or_else(|_| "C:\\Windows".into())).join("Fonts");
    let load = |files: &[&str]| -> Vec<FontVec> {
        files
            .iter()
            .filter_map(|f| std::fs::read(dir.join(f)).ok())
            .filter_map(|bytes| FontVec::try_from_vec_and_index(bytes, 0).ok())
            .collect()
    };
    Fonts {
        regular: load(&["segoeui.ttf", "YuGothM.ttc", "seguiemj.ttf"]),
        bold: load(&["seguisb.ttf", "YuGothB.ttc", "seguiemj.ttf"]),
        heavy: load(&["segoeuib.ttf", "YuGothB.ttc", "seguiemj.ttf"]),
    }
}

fn pick(fonts: &[FontVec], c: char) -> Option<&FontVec> {
    fonts.iter().find(|f| f.glyph_id(c).0 != 0).or_else(|| fonts.first())
}

fn text_width(fonts: &[FontVec], text: &str, size: f32, tracking: f32) -> f32 {
    text.chars()
        .filter_map(|c| pick(fonts, c).map(|f| f.as_scaled(PxScale::from(size)).h_advance(f.glyph_id(c)) + tracking))
        .sum::<f32>()
        - if text.is_empty() { 0.0 } else { tracking }
}

fn draw_text(img: &mut RgbaImage, fonts: &[FontVec], text: &str, x: f32, y: f32, size: f32, tracking: f32, color: [f32; 4]) {
    let mut cursor = x;
    for c in text.chars() {
        let Some(font) = pick(fonts, c) else { continue };
        let scaled = font.as_scaled(PxScale::from(size));
        let id = font.glyph_id(c);
        let glyph = id.with_scale_and_position(PxScale::from(size), point(cursor, y + scaled.ascent()));
        if let Some(outline) = font.outline_glyph(glyph) {
            let bounds = outline.px_bounds();
            outline.draw(|gx, gy, coverage| {
                let px = bounds.min.x as i32 + gx as i32;
                let py = bounds.min.y as i32 + gy as i32;
                blend(img, px, py, color, coverage);
            });
        }
        cursor += scaled.h_advance(id) + tracking;
    }
}

fn draw_centered(img: &mut RgbaImage, fonts: &[FontVec], text: &str, y: f32, size: f32, tracking: f32, color: [f32; 4]) {
    let width = text_width(fonts, text, size, tracking);
    draw_text(img, fonts, text, (W as f32 - width) / 2.0, y, size, tracking, color);
}

// shrinks the size when needed to fit max_lines
fn fit_lines(fonts: &[FontVec], text: &str, mut size: f32, width: f32, max_lines: usize, min_size: f32) -> (Vec<String>, f32) {
    loop {
        let mut lines: Vec<String> = Vec::new();
        let mut current = String::new();
        for word in text.split_whitespace() {
            let candidate = if current.is_empty() { word.to_string() } else { format!("{current} {word}") };
            if text_width(fonts, &candidate, size, 0.0) <= width || current.is_empty() {
                current = candidate;
            } else {
                lines.push(std::mem::take(&mut current));
                current = word.to_string();
            }
        }
        if !current.is_empty() {
            lines.push(current);
        }
        // text of only spaces still gives one (empty) line, callers draw lines[0]
        if lines.is_empty() {
            lines.push(String::new());
        }
        let fits = lines.len() <= max_lines && lines.iter().all(|l| text_width(fonts, l, size, 0.0) <= width);
        if fits || size <= min_size {
            if lines.len() > max_lines {
                lines.truncate(max_lines);
                if let Some(last) = lines.last_mut() {
                    while !last.is_empty() && text_width(fonts, &format!("{last}…"), size, 0.0) > width {
                        last.pop();
                    }
                    last.push('…');
                }
            }
            return (lines, size);
        }
        size *= 0.92;
    }
}

fn blend(img: &mut RgbaImage, x: i32, y: i32, color: [f32; 4], coverage: f32) {
    if x < 0 || y < 0 || x >= img.width() as i32 || y >= img.height() as i32 {
        return;
    }
    let alpha = (color[3] * coverage).clamp(0.0, 1.0);
    if alpha <= 0.0 {
        return;
    }
    let pixel = img.get_pixel_mut(x as u32, y as u32);
    for i in 0..3 {
        let dst = pixel[i] as f32 / 255.0;
        pixel[i] = ((color[i] * alpha + dst * (1.0 - alpha)) * 255.0).round() as u8;
    }
    let dst_a = pixel[3] as f32 / 255.0;
    pixel[3] = ((alpha + dst_a * (1.0 - alpha)) * 255.0).round() as u8;
}

// signed distance to a rounded box, p relative to the centre
fn rounded_box_sdf(px: f32, py: f32, half_w: f32, half_h: f32, radius: f32) -> f32 {
    let qx = px.abs() - half_w + radius;
    let qy = py.abs() - half_h + radius;
    let outside = (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt();
    outside + qx.max(qy).min(0.0) - radius
}

struct Shape {
    cx: f32,
    cy: f32,
    w: f32,
    h: f32,
    radius: f32,
    // degrees
    angle: f32,
}

impl Shape {
    fn new(x: f32, y: f32, w: f32, h: f32, radius: f32) -> Self {
        Self { cx: x + w / 2.0, cy: y + h / 2.0, w, h, radius, angle: 0.0 }
    }

    fn rotated(mut self, degrees: f32, dx: f32, dy: f32) -> Self {
        self.angle = degrees;
        self.cx += dx;
        self.cy += dy;
        self
    }

    fn for_each(&self, margin: f32, mut visit: impl FnMut(i32, i32, f32)) {
        let reach = ((self.w / 2.0).powi(2) + (self.h / 2.0).powi(2)).sqrt() + margin;
        let (sin, cos) = (-self.angle.to_radians()).sin_cos();
        let (x0, x1) = ((self.cx - reach).floor() as i32, (self.cx + reach).ceil() as i32);
        let (y0, y1) = ((self.cy - reach).floor() as i32, (self.cy + reach).ceil() as i32);
        for y in y0.max(0)..y1.min(H as i32) {
            for x in x0.max(0)..x1.min(W as i32) {
                let (dx, dy) = (x as f32 + 0.5 - self.cx, y as f32 + 0.5 - self.cy);
                let (lx, ly) = (dx * cos - dy * sin, dx * sin + dy * cos);
                visit(x, y, rounded_box_sdf(lx, ly, self.w / 2.0, self.h / 2.0, self.radius));
            }
        }
    }

    fn fill(&self, img: &mut RgbaImage, color: [f32; 4]) {
        self.for_each(1.0, |x, y, d| blend(img, x, y, color, (0.5 - d).clamp(0.0, 1.0)));
    }

    fn stroke(&self, img: &mut RgbaImage, width: f32, color: [f32; 4]) {
        self.for_each(1.0, |x, y, d| {
            let coverage = (width / 2.0 + 0.5 - (d + width / 2.0).abs()).clamp(0.0, 1.0);
            blend(img, x, y, color, coverage);
        });
    }

    fn shadow(&self, img: &mut RgbaImage, softness: f32, color: [f32; 4]) {
        self.for_each(softness, |x, y, d| {
            let t = (1.0 - (d / softness).clamp(0.0, 1.0)).powi(2);
            blend(img, x, y, color, t);
        });
    }
}

fn white(alpha: f32) -> [f32; 4] {
    [1.0, 1.0, 1.0, alpha]
}

#[cfg(test)]
mod tests {
    // renders a real card for a manual look, needs a connection
    #[test]
    #[ignore]
    fn renders_card() {
        let info = super::CardInfo {
            manga_id: "01J76XY7E9FNDZ1DBBM6PBJPFK".into(),
            title: "One Piece".into(),
            authors: "ODA Eiichiro".into(),
            chapter_count: 1198,
            status: "Ongoing".into(),
            kind: "Manga".into(),
            year: "1997".into(),
            tags: vec!["Action".into(), "Adventure".into(), "Comedy".into()],
            accent: [125, 115, 255],
        };
        let path = super::render(&info).expect("card renders");
        println!("CARD {}", path.display());
    }
}

fn render(info: &CardInfo) -> Result<PathBuf, String> {
    let fonts = load_fonts();
    if fonts.regular.is_empty() {
        return Err("No system fonts found.".to_string());
    }
    let accent = [info.accent[0] as f32 / 255.0, info.accent[1] as f32 / 255.0, info.accent[2] as f32 / 255.0];

    let cover = disk::bytes(&source::cover_url(&info.manga_id, false), false)
        .ok()
        .and_then(|bytes| image::load_from_memory(&bytes).ok());

    let mut img = RgbaImage::from_pixel(W, H, Rgba([16, 13, 31, 255]));
    if let Some(cover) = &cover {
        let small = cover.resize_to_fill(108, 192, FilterType::Triangle).blur(3.0);
        let big = small.resize_exact(W, H, FilterType::Triangle).to_rgba8();
        for (x, y, pixel) in big.enumerate_pixels() {
            let c = [pixel[0] as f32 / 255.0, pixel[1] as f32 / 255.0, pixel[2] as f32 / 255.0, 0.8];
            blend(&mut img, x as i32, y as i32, c, 1.0);
        }
    }
    let (gx, gy, gr) = (W as f32 * 0.5, H as f32 * 0.3, 780.0);
    for y in 0..H {
        let t = y as f32 / H as f32;
        let dark = if t < 0.45 { 0.25 + (t / 0.45) * 0.10 } else { 0.35 + ((t - 0.45) / 0.55) * 0.53 };
        for x in 0..W {
            let distance = ((x as f32 - gx).powi(2) + (y as f32 - gy).powi(2)).sqrt();
            let glow = (1.0 - distance / gr).clamp(0.0, 1.0);
            if glow > 0.0 {
                blend(&mut img, x as i32, y as i32, [accent[0], accent[1], accent[2], 0.45 * glow * glow], 1.0);
            }
            blend(&mut img, x as i32, y as i32, [0.0, 0.0, 0.0, dark], 1.0);
        }
    }

    let margin = 72.0;
    draw_text(&mut img, &fonts.heavy, "読", margin, 62.0, 52.0, 0.0, [accent[0], accent[1], accent[2], 1.0]);
    draw_text(&mut img, &fonts.heavy, "YOMIKAZE", margin + 70.0, 76.0, 33.0, 6.0, white(0.85));
    if !info.kind.is_empty() {
        let label = info.kind.to_uppercase();
        let width = text_width(&fonts.heavy, &label, 30.0, 3.0) + 60.0;
        Shape::new(W as f32 - margin - width, 64.0, width, 64.0, 32.0).fill(&mut img, [accent[0], accent[1], accent[2], 1.0]);
        draw_text(&mut img, &fonts.heavy, &label, W as f32 - margin - width + 30.0, 77.0, 30.0, 3.0, white(1.0));
    }

    let (title_lines, title_size) = fit_lines(&fonts.heavy, &info.title, 81.0, W as f32 - 156.0, 2, 54.0);
    let line_height = title_size * 1.18;
    let facts: Vec<(String, &str)> = [
        ((info.chapter_count > 0).then(|| info.chapter_count.to_string()), "CHAPTERS"),
        ((!info.status.is_empty()).then(|| info.status.clone()), "STATUS"),
        ((!info.year.is_empty()).then(|| info.year.clone()), "YEAR"),
    ]
    .into_iter()
    .filter_map(|(v, l)| v.map(|v| (v, l)))
    .collect();
    let tags: Vec<&String> = info.tags.iter().take(3).collect();

    let cover_w = 588.0;
    let cover_h = 882.0;
    let mut content = cover_h + 66.0 + line_height * title_lines.len() as f32;
    if !info.authors.is_empty() {
        content += 60.0;
    }
    if !facts.is_empty() {
        content += 48.0 + 150.0;
    }
    if !tags.is_empty() {
        content += 36.0 + 78.0;
    }
    let top = 170.0;
    let bottom = H as f32 - 210.0;
    let mut y = top + ((bottom - top) - content).max(0.0) / 2.0;

    let cover_x = (W as f32 - cover_w) / 2.0;
    Shape::new(cover_x, y, cover_w, cover_h, 60.0).rotated(-8.0, -42.0, 18.0).fill(&mut img, white(0.10));
    Shape::new(cover_x, y, cover_w, cover_h, 60.0).rotated(6.0, 36.0, 12.0).fill(&mut img, white(0.16));
    let cover_shape = Shape::new(cover_x, y, cover_w, cover_h, 60.0);
    Shape::new(cover_x, y + 42.0, cover_w, cover_h, 60.0).shadow(&mut img, 90.0, [0.0, 0.0, 0.0, 0.55]);
    if let Some(cover) = &cover {
        let scaled = cover.resize_to_fill(cover_w as u32, cover_h as u32, FilterType::Lanczos3).to_rgba8();
        cover_shape.for_each(1.0, |x, yy, d| {
            let coverage = (0.5 - d).clamp(0.0, 1.0);
            if coverage <= 0.0 {
                return;
            }
            let sx = (x as f32 - cover_x) as i32;
            let sy = (yy as f32 - y) as i32;
            if sx >= 0 && sy >= 0 && (sx as u32) < scaled.width() && (sy as u32) < scaled.height() {
                let p = scaled.get_pixel(sx as u32, sy as u32);
                blend(&mut img, x, yy, [p[0] as f32 / 255.0, p[1] as f32 / 255.0, p[2] as f32 / 255.0, 1.0], coverage);
            }
        });
    } else {
        cover_shape.fill(&mut img, white(0.12));
    }
    cover_shape.stroke(&mut img, 3.0, white(0.22));
    y += cover_h + 66.0;

    for line in &title_lines {
        draw_centered(&mut img, &fonts.heavy, line, y, title_size, 0.0, white(1.0));
        y += line_height;
    }
    if !info.authors.is_empty() {
        let (lines, size) = fit_lines(&fonts.regular, &info.authors, 39.0, W as f32 - 156.0, 1, 30.0);
        draw_centered(&mut img, &fonts.regular, &lines[0], y + 6.0, size, 0.0, white(0.7));
        y += 60.0;
    }

    if !facts.is_empty() {
        y += 48.0;
        let panel_x = margin;
        let panel_w = W as f32 - margin * 2.0;
        let panel = Shape::new(panel_x, y, panel_w, 150.0, 54.0);
        panel.fill(&mut img, white(0.10));
        panel.stroke(&mut img, 3.0, white(0.14));
        let cell = panel_w / facts.len() as f32;
        for (index, (value, label)) in facts.iter().enumerate() {
            let center = panel_x + cell * (index as f32 + 0.5);
            if index > 0 {
                Shape::new(panel_x + cell * index as f32 - 1.5, y + 36.0, 3.0, 78.0, 1.5).fill(&mut img, white(0.18));
            }
            let (lines, size) = fit_lines(&fonts.bold, value, 48.0, cell - 30.0, 1, 30.0);
            let vw = text_width(&fonts.bold, &lines[0], size, 0.0);
            draw_text(&mut img, &fonts.bold, &lines[0], center - vw / 2.0, y + 28.0, size, 0.0, white(1.0));
            let lw = text_width(&fonts.bold, label, 27.0, 3.6);
            draw_text(&mut img, &fonts.bold, label, center - lw / 2.0, y + 92.0, 27.0, 3.6, white(0.55));
        }
        y += 150.0;
    }

    if !tags.is_empty() {
        y += 36.0;
        let gap = 18.0;
        let widths: Vec<f32> = tags.iter().map(|t| text_width(&fonts.bold, t, 33.0, 0.0) + 66.0).collect();
        let total: f32 = widths.iter().sum::<f32>() + gap * (tags.len() as f32 - 1.0);
        let mut x = (W as f32 - total) / 2.0;
        for (tag, width) in tags.iter().zip(widths) {
            Shape::new(x, y, width, 78.0, 39.0).stroke(&mut img, 3.0, white(0.3));
            draw_text(&mut img, &fonts.bold, tag, x + 33.0, y + 18.0, 33.0, 0.0, white(0.9));
            x += width + gap;
        }
    }

    Shape::new((W as f32 - 132.0) / 2.0, H as f32 - 160.0, 132.0, 12.0, 6.0).fill(&mut img, [accent[0], accent[1], accent[2], 1.0]);
    draw_centered(&mut img, &fonts.heavy, "READ ON YOMIKAZE", H as f32 - 124.0, 30.0, 6.6, white(0.6));

    let card = Shape::new(0.0, 0.0, W as f32, H as f32, 102.0);
    card.for_each(0.0, |x, y, d| {
        let coverage = (0.5 - d).clamp(0.0, 1.0);
        if coverage < 1.0 {
            let pixel = img.get_pixel_mut(x as u32, y as u32);
            pixel[3] = (pixel[3] as f32 * coverage).round() as u8;
        }
    });

    let dir = dirs::picture_dir().unwrap_or_else(crate::store::data_dir).join("Yomikaze");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let safe: String = info
        .title
        .chars()
        .map(|c| if c.is_alphanumeric() || c == ' ' || c == '-' { c } else { '_' })
        .collect::<String>()
        .trim()
        .chars()
        .take(60)
        .collect();
    let path = dir.join(format!("{} - Yomikaze.png", if safe.is_empty() { "Manga" } else { &safe }));
    img.save(&path).map_err(|e| e.to_string())?;
    Ok(path)
}
