mod app;
mod categories;
mod date_utils;
mod dev_utils;
mod milestones;
mod pdf_export;
mod project_utils;
mod single_dev_utils;
mod single_effort_utils;
mod sync_merge;
mod ui;
mod ui_style;
mod workers_utils;

use app::{App, SAVE_PATH};
use ui::PjmApp;

fn main() -> eframe::Result<()> {
    let file_path = std::env::args().nth(1).unwrap_or_else(|| SAVE_PATH.to_string());
    // Se il file esiste ma non si carica, avvisa l'utente (evita di ripartire in
    // silenzio da vuoto e sovrascrivere per sbaglio un file esistente).
    let (mut app, startup_error) = match App::load(&file_path) {
        Ok(a) => (a, None),
        Err(e) => {
            let msg = std::path::Path::new(&file_path).exists().then(|| {
                format!(
                    "Impossibile leggere «{file_path}»:\n{e}\n\nSi parte da un file vuoto: \
                     salvando sovrascriverai il file esistente."
                )
            });
            (App::new(), msg)
        }
    };
    app.compute_sovra();

    let title = format!(
        "Project Management Effort Tracker v{}",
        env!("CARGO_PKG_VERSION")
    );
    let native_options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1024.0, 768.0])
            .with_min_inner_size([800.0, 600.0])
            .with_title(&title),
        ..Default::default()
    };

    eframe::run_native(
        &title,
        native_options,
        Box::new(move |cc| Ok(Box::new(PjmApp::new(app, file_path, startup_error, cc)))),
    )
}
