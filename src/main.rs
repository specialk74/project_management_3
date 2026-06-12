mod app;
mod categories;
mod date_utils;
mod dev_utils;
mod project_utils;
mod single_dev_utils;
mod single_effort_utils;
mod ui;
mod ui_style;
mod workers_utils;

use app::{App, SAVE_PATH};
use ui::PjmApp;

fn main() -> eframe::Result<()> {
    let file_path = std::env::args().nth(1).unwrap_or_else(|| SAVE_PATH.to_string());
    let mut app = App::load(&file_path).unwrap_or_else(|_| App::new());
    app.compute_sovra();

    let native_options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1024.0, 768.0])
            .with_min_inner_size([800.0, 600.0])
            .with_title("Project Management Effort Tracker"),
        ..Default::default()
    };

    eframe::run_native(
        "Project Management Effort Tracker",
        native_options,
        Box::new(move |cc| Ok(Box::new(PjmApp::new(app, file_path, cc)))),
    )
}
