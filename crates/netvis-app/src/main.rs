// Hide the console window in Windows release builds.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod app;
mod canvas;
#[cfg(debug_assertions)]
mod debug_capture;
mod file_dialog;
mod icons;
mod panels;
mod state;
mod theme;
mod widgets;

fn main() -> eframe::Result {
    let open = std::env::args_os().nth(1).map(std::path::PathBuf::from);

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(theme::APP_NAME)
            .with_app_id("netvis")
            .with_inner_size([1440.0, 900.0])
            .with_min_inner_size([960.0, 600.0])
            .with_icon(eframe::icon_data::from_png_bytes(include_bytes!("../../../assets/netvis.png")).unwrap_or_default()),
        ..Default::default()
    };
    eframe::run_native(theme::APP_NAME, options, Box::new(|cc| Ok(Box::new(app::NetVisApp::new(cc, open)))))
}
