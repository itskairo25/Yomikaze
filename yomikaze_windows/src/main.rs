// no console window
#![windows_subsystem = "windows"]

mod anilist;
mod app;
mod backup;
mod downloads;
mod hello;
mod i18n;
mod icons;
mod images;
mod model;
mod net;
mod source;
mod store;
mod wallpapers;
mod theme;
mod ui;

// YOMIKAZE_SIZE=WxH overrides it for testing
fn window_size() -> [f32; 2] {
    std::env::var("YOMIKAZE_SIZE")
        .ok()
        .and_then(|v| {
            let (w, h) = v.split_once('x')?;
            Some([w.parse().ok()?, h.parse().ok()?])
        })
        .unwrap_or([1240.0, 700.0])
}

fn main() -> eframe::Result {
    // an anilist login link opened by the browser: hand it to the running app if there is one
    let running = hello::another_instance_running();
    if let Some(link) = std::env::args().find(|a| a.starts_with("yomikaze://")) {
        anilist::receive_link(&link);
        if running {
            return Ok(());
        }
    }
    hello::register_app_id();
    let icon = image::load_from_memory(include_bytes!("../assets/icon.png")).ok().map(|image| {
        let image = image.resize(256, 256, image::imageops::FilterType::Lanczos3).to_rgba8();
        let (width, height) = image.dimensions();
        egui::IconData { rgba: image.into_raw(), width, height }
    });

    let mut viewport = egui::ViewportBuilder::default()
        .with_title("Yomikaze")
        .with_app_id("dev.kairo.yomikaze")
        // fits a 1366x768 laptop at 100% and 1920x1080 at 150%
        .with_inner_size(window_size())
        .with_min_inner_size([860.0, 560.0]);
    if let Some(icon) = icon {
        viewport = viewport.with_icon(icon);
    }

    let options = eframe::NativeOptions { viewport, ..Default::default() };
    eframe::run_native("Yomikaze", options, Box::new(|cc| Ok(Box::new(app::YomikazeApp::new(cc)))))
}
