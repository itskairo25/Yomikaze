// colours, fonts and painting helpers

use std::sync::Arc;

use egui::{
    Color32, Context, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Margin, Mesh, Painter, Pos2, Rect,
    Shadow, Stroke, TextStyle, Vec2, Visuals, pos2,
};

use crate::store::{Appearance, Settings};

pub const ACCENTS: [(&str, &str, Color32); 5] = [
    ("yoru", "Yoru", Color32::from_rgb(125, 115, 255)),
    ("kaze", "Kaze", Color32::from_rgb(41, 189, 199)),
    ("sakura", "Sakura", Color32::from_rgb(255, 107, 163)),
    ("akane", "Akane", Color32::from_rgb(255, 97, 77)),
    ("kohaku", "Kohaku", Color32::from_rgb(250, 168, 41)),
];

pub const CUSTOM_ACCENT: &str = "custom";

pub fn accent(settings: &Settings) -> Color32 {
    if settings.accent == CUSTOM_ACCENT {
        let [r, g, b] = settings.custom_accent;
        return Color32::from_rgb(r, g, b);
    }
    ACCENTS
        .iter()
        .find(|(id, _, _)| *id == settings.accent)
        .map(|(_, _, c)| *c)
        .unwrap_or(ACCENTS[0].2)
}

#[derive(Clone, Copy)]
pub struct Palette {
    pub dark: bool,
    pub accent: Color32,
    pub background: Color32,
    pub glass: Color32,
    pub glass_strong: Color32,
    pub glass_stroke: Color32,
    pub quaternary: Color32,
    pub text: Color32,
    pub secondary: Color32,
    pub tertiary: Color32,
}

impl Palette {
    pub fn new(dark: bool, accent: Color32) -> Self {
        if dark {
            Self {
                dark,
                accent,
                background: Color32::from_rgb(12, 11, 18),
                glass: Color32::from_rgba_unmultiplied(255, 255, 255, 14),
                glass_strong: Color32::from_rgba_unmultiplied(30, 28, 42, 225),
                glass_stroke: Color32::from_rgba_unmultiplied(255, 255, 255, 26),
                quaternary: Color32::from_rgba_unmultiplied(255, 255, 255, 22),
                text: Color32::from_rgb(240, 240, 246),
                secondary: Color32::from_rgb(160, 160, 176),
                tertiary: Color32::from_rgb(110, 110, 126),
            }
        } else {
            Self {
                dark,
                accent,
                background: Color32::from_rgb(246, 246, 250),
                glass: Color32::from_rgba_unmultiplied(255, 255, 255, 170),
                glass_strong: Color32::from_rgba_unmultiplied(255, 255, 255, 235),
                glass_stroke: Color32::from_rgba_unmultiplied(0, 0, 0, 18),
                quaternary: Color32::from_rgba_unmultiplied(0, 0, 0, 16),
                text: Color32::from_rgb(20, 20, 28),
                secondary: Color32::from_rgb(100, 100, 112),
                tertiary: Color32::from_rgb(150, 150, 160),
            }
        }
    }

    pub fn of(ctx: &Context) -> Self {
        let accent = ctx.data(|d| d.get_temp::<Color32>(egui::Id::new("accent"))).unwrap_or(ACCENTS[0].2);
        Self::new(ctx.global_style().visuals.dark_mode, accent)
    }
}

// cheap enough to call every frame
pub fn apply(ctx: &Context, settings: &Settings) {
    let accent = accent(settings);
    ctx.data_mut(|d| d.insert_temp(egui::Id::new("accent"), accent));

    let dark = match settings.appearance {
        Appearance::Dark => true,
        Appearance::Light => false,
        Appearance::System => ctx.system_theme().map(|t| t == egui::Theme::Dark).unwrap_or(true),
    };
    let palette = Palette::new(dark, accent);

    let mut visuals = if dark { Visuals::dark() } else { Visuals::light() };
    visuals.panel_fill = palette.background;
    visuals.window_fill = palette.glass_strong;
    visuals.extreme_bg_color = palette.quaternary;
    visuals.hyperlink_color = accent;
    visuals.selection.bg_fill = accent.gamma_multiply(0.55);
    visuals.selection.stroke = Stroke::new(1.0, accent);
    visuals.window_corner_radius = CornerRadius::same(22);
    visuals.menu_corner_radius = CornerRadius::same(14);
    visuals.window_stroke = Stroke::new(1.0, palette.glass_stroke);
    visuals.window_shadow = Shadow { offset: [0, 12], blur: 40, spread: 0, color: Color32::from_black_alpha(90) };
    visuals.popup_shadow = Shadow { offset: [0, 8], blur: 24, spread: 0, color: Color32::from_black_alpha(70) };
    visuals.override_text_color = Some(palette.text);
    for widget in [
        &mut visuals.widgets.inactive,
        &mut visuals.widgets.hovered,
        &mut visuals.widgets.active,
        &mut visuals.widgets.open,
        &mut visuals.widgets.noninteractive,
    ] {
        widget.corner_radius = CornerRadius::same(10);
    }
    visuals.widgets.inactive.weak_bg_fill = palette.quaternary;
    visuals.widgets.inactive.bg_fill = palette.quaternary;
    visuals.widgets.inactive.bg_stroke = Stroke::NONE;
    visuals.widgets.hovered.weak_bg_fill = palette.quaternary.gamma_multiply(1.8);
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, accent.gamma_multiply(0.6));
    visuals.widgets.active.weak_bg_fill = accent.gamma_multiply(0.4);
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, palette.glass_stroke);
    visuals.slider_trailing_fill = true;

    ctx.set_visuals(visuals);
    ctx.all_styles_mut(|style| {
        style.spacing.item_spacing = Vec2::new(10.0, 8.0);
        style.spacing.button_padding = Vec2::new(12.0, 6.0);
        style.spacing.interact_size.y = 30.0;
        style.spacing.slider_width = 220.0;
        style.spacing.scroll.bar_width = 8.0;
        style.spacing.scroll.floating = true;
        style.text_styles = [
            (TextStyle::Small, FontId::proportional(12.0)),
            (TextStyle::Body, FontId::proportional(15.0)),
            (TextStyle::Button, FontId::proportional(15.0)),
            (TextStyle::Heading, FontId::new(26.0, bold())),
            (TextStyle::Monospace, FontId::monospace(13.0)),
        ]
        .into();
    });
}

pub fn bold() -> FontFamily {
    FontFamily::Name("bold".into())
}

pub fn heavy() -> FontFamily {
    FontFamily::Name("heavy".into())
}

// a glyph on its own, centred by the icon font's own box rather than a text line
pub fn icon_font(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("glyphs".into()))
}

pub fn font(size: f32) -> FontId {
    FontId::proportional(size)
}

pub fn bold_font(size: f32) -> FontId {
    FontId::new(size, bold())
}

pub fn heavy_font(size: f32) -> FontId {
    FontId::new(size, heavy())
}

// segoe ui for text, yu gothic for japanese, phosphor for icons
// chinese puts its own font before the japanese one, both share many characters
pub fn install_fonts(ctx: &Context, chinese_first: bool) {
    let mut fonts = FontDefinitions::default();
    egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
    // our own icons sit in front of phosphor, see icons.rs
    // drawn without hinting, which would nudge the outlines off centre
    let icons = FontData::from_static(include_bytes!("../assets/yomikaze-icons.ttf")).tweak(egui::FontTweak { hinting: Some(false), ..Default::default() });
    fonts.font_data.insert("icons".to_string(), Arc::new(icons));

    let windows_fonts = std::path::PathBuf::from(std::env::var("WINDIR").unwrap_or_else(|_| "C:\\Windows".into())).join("Fonts");
    let mut load = |name: &str, files: &[&str]| -> bool {
        for file in files {
            if let Ok(bytes) = std::fs::read(windows_fonts.join(file)) {
                let mut data = FontData::from_owned(bytes);
                // yu gothic and yahei sit higher than segoe ui, this puts them on the same line
                let shift = if file.starts_with("YuGoth") { 0.3 } else if file.starts_with("msyh") { 0.04 } else { 0.0 };
                if shift != 0.0 {
                    data = data.tweak(egui::FontTweak { y_offset_factor: shift, ..Default::default() });
                }
                fonts.font_data.insert(name.to_string(), Arc::new(data));
                return true;
            }
        }
        false
    };

    let has_regular = load("ui", &["segoeui.ttf"]);
    let has_semibold = load("ui-semibold", &["seguisb.ttf", "segoeuib.ttf"]);
    let has_bold = load("ui-bold", &["segoeuib.ttf", "seguisb.ttf"]);
    let has_cjk = load("cjk", &["YuGothM.ttc", "meiryo.ttc", "msgothic.ttc", "msyh.ttc"]);
    let has_cjk_bold = load("cjk-bold", &["YuGothB.ttc", "meiryob.ttc", "msyhbd.ttc"]);
    let has_sc = load("cjk-sc", &["msyh.ttc", "simsun.ttc"]);
    let has_sc_bold = load("cjk-sc-bold", &["msyhbd.ttc", "msyh.ttc", "simsun.ttc"]);
    let has_emoji = load("emoji", &["seguiemj.ttf"]);

    let family = |primary: Option<&str>, cjk: Option<&str>| -> Vec<String> {
        let mut list = Vec::new();
        if let Some(p) = primary {
            list.push(p.to_string());
        }
        list.push("icons".to_string());
        list.push("phosphor".to_string());
        let bold = cjk == Some("cjk-bold");
        let sc = ((bold && has_sc_bold) || (!bold && has_sc)).then_some(if bold { "cjk-sc-bold" } else { "cjk-sc" });
        let order = if chinese_first { [sc, cjk] } else { [cjk, sc] };
        for name in order.into_iter().flatten() {
            list.push(name.to_string());
        }
        if has_emoji {
            list.push("emoji".to_string());
        }
        list
    };

    let regular = family(has_regular.then_some("ui"), has_cjk.then_some("cjk"));
    let semibold = family(has_semibold.then_some("ui-semibold"), has_cjk_bold.then_some("cjk-bold"));
    let heavy_list = family(has_bold.then_some("ui-bold"), has_cjk_bold.then_some("cjk-bold"));

    let defaults = fonts.families.get(&FontFamily::Proportional).cloned().unwrap_or_default();
    let with_defaults = |mut list: Vec<String>| {
        for name in &defaults {
            if !list.contains(name) {
                list.push(name.clone());
            }
        }
        list
    };

    fonts.families.insert(FontFamily::Proportional, with_defaults(regular));
    fonts.families.insert(bold(), with_defaults(semibold));
    fonts.families.insert(heavy(), with_defaults(heavy_list));
    fonts.families.insert(FontFamily::Name("glyphs".into()), vec!["icons".to_string(), "phosphor".to_string()]);
    ctx.set_fonts(fonts);
}

pub fn vertical_gradient(painter: &Painter, rect: Rect, top: Color32, bottom: Color32) {
    let mut mesh = Mesh::default();
    mesh.colored_vertex(rect.left_top(), top);
    mesh.colored_vertex(rect.right_top(), top);
    mesh.colored_vertex(rect.left_bottom(), bottom);
    mesh.colored_vertex(rect.right_bottom(), bottom);
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(1, 3, 2);
    painter.add(mesh);
}

pub fn horizontal_gradient(painter: &Painter, rect: Rect, left: Color32, right: Color32) {
    let mut mesh = Mesh::default();
    mesh.colored_vertex(rect.left_top(), left);
    mesh.colored_vertex(rect.right_top(), right);
    mesh.colored_vertex(rect.left_bottom(), left);
    mesh.colored_vertex(rect.right_bottom(), right);
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(1, 3, 2);
    painter.add(mesh);
}

// diagonal gradient from top-left to bottom-right, with a one pixel soft edge like egui's own shapes
pub fn rounded_gradient(painter: &Painter, rect: Rect, radius: f32, from: Color32, to: Color32) {
    let radius = radius.min(rect.width() / 2.0).min(rect.height() / 2.0);
    let feather = 1.0 / painter.pixels_per_point();
    let corners = [
        (pos2(rect.right() - radius, rect.bottom() - radius), 0.0_f32),
        (pos2(rect.left() + radius, rect.bottom() - radius), 0.5),
        (pos2(rect.left() + radius, rect.top() + radius), 1.0),
        (pos2(rect.right() - radius, rect.top() + radius), 1.5),
    ];
    let color_at = |p: Pos2| {
        let t = ((p.x - rect.left()) / rect.width() + (p.y - rect.top()) / rect.height()) / 2.0;
        mix(from, to, t)
    };
    // the outline, each point with the direction pointing out of the shape
    let steps = 12;
    let mut outline = Vec::new();
    for (corner, start) in corners {
        for step in 0..=steps {
            let angle = (start + 0.5 * step as f32 / steps as f32) * std::f32::consts::PI;
            let normal = Vec2::angled(angle);
            outline.push((corner + normal * radius, normal));
        }
    }
    let mut mesh = Mesh::default();
    let center = rect.center();
    mesh.colored_vertex(center, color_at(center));
    let count = outline.len() as u32;
    // inner ring at full colour, outer ring fades to nothing
    for (point, normal) in &outline {
        let inner = *point - *normal * (feather / 2.0);
        mesh.colored_vertex(inner, color_at(inner));
    }
    for (point, normal) in &outline {
        let outer = *point + *normal * (feather / 2.0);
        mesh.colored_vertex(outer, Color32::TRANSPARENT);
    }
    for i in 0..count {
        let next = (i + 1) % count;
        let (a, b) = (1 + i, 1 + next);
        let (c, d) = (1 + count + i, 1 + count + next);
        mesh.add_triangle(0, a, b);
        mesh.add_triangle(a, c, b);
        mesh.add_triangle(b, c, d);
    }
    painter.add(mesh);
}

pub fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let lerp = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    // Color32 is premultiplied, so mixing the stored values is the correct blend
    Color32::from_rgba_premultiplied(lerp(a.r(), b.r()), lerp(a.g(), b.g()), lerp(a.b(), b.b()), lerp(a.a(), b.a()))
}

// radial gradient approximated with a triangle fan
pub fn radial_glow(painter: &Painter, center: Pos2, radius: f32, color: Color32) {
    let mut mesh = Mesh::default();
    mesh.colored_vertex(center, color);
    let steps = 48;
    for i in 0..=steps {
        let angle = i as f32 / steps as f32 * std::f32::consts::TAU;
        mesh.colored_vertex(center + Vec2::angled(angle) * radius, Color32::TRANSPARENT);
    }
    for i in 1..=steps as u32 {
        mesh.add_triangle(0, i, i + 1);
    }
    painter.add(mesh);
}

pub fn glass_frame(palette: &Palette) -> egui::Frame {
    egui::Frame::new()
        .fill(palette.glass)
        .stroke(Stroke::new(1.0, palette.glass_stroke))
        .corner_radius(CornerRadius::same(22))
        .inner_margin(Margin::same(18))
}

pub fn card_frame(palette: &Palette) -> egui::Frame {
    egui::Frame::new()
        .fill(palette.glass_strong.gamma_multiply(if palette.dark { 0.75 } else { 0.9 }))
        .stroke(Stroke::new(1.0, palette.glass_stroke))
        .corner_radius(CornerRadius::same(24))
        .inner_margin(Margin::same(18))
}

// solid so nothing behind it shows through the text
pub fn modal_frame(palette: &Palette) -> egui::Frame {
    let fill = if palette.dark { Color32::from_rgb(28, 26, 40) } else { Color32::from_rgb(252, 252, 255) };
    egui::Frame::new()
        .fill(fill)
        .stroke(Stroke::new(1.0, palette.glass_stroke))
        .corner_radius(CornerRadius::same(24))
        .inner_margin(Margin::same(22))
        .shadow(Shadow { offset: [0, 16], blur: 48, spread: 0, color: Color32::from_black_alpha(110) })
}

pub fn readable_on(color: Color32) -> Color32 {
    let luma = 0.299 * color.r() as f32 + 0.587 * color.g() as f32 + 0.114 * color.b() as f32;
    if luma > 170.0 { Color32::from_rgb(20, 20, 28) } else { Color32::WHITE }
}

pub fn ease_out_cubic(t: f32) -> f32 {
    1.0 - (1.0 - t.clamp(0.0, 1.0)).powi(3)
}

// exponential ease towards target, frame-rate independent and can change direction
pub fn spring(ctx: &Context, id: egui::Id, target: f32, speed: f32) -> f32 {
    let dt = ctx.input(|i| i.stable_dt).min(0.05);
    let current = ctx.data(|d| d.get_temp::<f32>(id)).unwrap_or(target);
    let k = 1.0 - (-dt * speed).exp();
    let mut next = current + (target - current) * k;
    if (next - target).abs() < 0.25 {
        next = target;
    } else {
        ctx.request_repaint();
    }
    ctx.data_mut(|d| d.insert_temp(id, next));
    next
}

pub fn soft_shadow(painter: &Painter, rect: Rect, radius: u8, offset_y: f32, blur: f32, color: Color32) {
    let shadow = Shadow {
        offset: [0, offset_y.round().clamp(-127.0, 127.0) as i8],
        blur: blur.round().clamp(0.0, 255.0) as u8,
        spread: 0,
        color,
    };
    painter.add(shadow.as_shape(rect, CornerRadius::same(radius)));
}

pub fn backdrop(painter: &Painter, rect: Rect, palette: &Palette) {
    let alpha = if palette.dark { 0.22 } else { 0.14 };
    let top = Rect::from_min_size(rect.min, Vec2::new(rect.width(), 420.0));
    vertical_gradient(painter, top, palette.accent.gamma_multiply(alpha), Color32::TRANSPARENT);
    radial_glow(painter, pos2(rect.right() - 160.0, rect.top() + 30.0), 460.0, palette.accent.gamma_multiply(alpha * 0.5));
}

// used for the title bar so it blends in
pub fn top_color(palette: &Palette) -> Color32 {
    mix(palette.background, palette.accent, if palette.dark { 0.22 } else { 0.14 })
}
