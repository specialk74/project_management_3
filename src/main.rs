mod app;
mod categories;
mod date_utils;
mod dev_utils;
mod git_autosync;
mod milestones;
mod pdf_export;
mod project_diff;
mod project_utils;
mod single_dev_utils;
mod single_effort_utils;
mod sync_merge;
mod ui;
mod ui_style;
mod worker_effort;
mod worker_matrix;
mod worker_pies;
mod workers_utils;

use app::{App, SAVE_PATH};
use ui::PjmApp;

fn main() -> eframe::Result<()> {
    let file_path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| SAVE_PATH.to_string());
    // Se il file esiste ma non si carica, avvisa l'utente (evita di ripartire in
    // silenzio da vuoto e sovrascrivere per sbaglio un file esistente).
    let (mut app, missing, startup_error) = match App::load_reporting_missing(&file_path) {
        Ok((a, missing)) => (a, missing, None),
        Err(e) => {
            let msg = std::path::Path::new(&file_path).exists().then(|| {
                format!(
                    "Impossibile leggere «{file_path}»:\n{e}\n\nSi parte da un file vuoto: \
                     salvando sovrascriverai il file esistente."
                )
            });
            (App::new(), Vec::new(), msg)
        }
    };
    app.compute_sovra();

    let title = format!(
        "Project Management Effort Tracker v{}",
        env!("CARGO_PKG_VERSION")
    );
    #[cfg_attr(not(windows), allow(unused_mut))]
    let mut native_options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1024.0, 768.0])
            .with_min_inner_size([800.0, 600.0])
            .with_title(&title),
        ..Default::default()
    };
    #[cfg(windows)]
    prefer_low_power_gpu(&mut native_options, std::env::var_os("WGPU_POWER_PREF").is_some());

    eframe::run_native(
        &title,
        native_options,
        Box::new(move |cc| Ok(Box::new(PjmApp::new(app, file_path, missing, startup_error, cc)))),
    )
}

/// Su Windows la GPU predefinita è quella a basso consumo (`LowPower`, di
/// solito l'integrata) invece di quella ad alte prestazioni scelta da wgpu: sui
/// portatili con due schede (es. NVIDIA RTX 2000 Ada + integrata) la NVIDIA
/// disegna e l'integrata mostra, e il passaggio dell'immagine fra le due fa
/// scattare lo scroll col trascinamento (verificato dall'utente con
/// `WGPU_POWER_PREF=low`). Se la variabile `WGPU_POWER_PREF` è impostata
/// (`env_set`) vince lei: `WGPU_POWER_PREF=high` torna alla scheda dedicata.
#[cfg_attr(not(windows), allow(dead_code))]
fn prefer_low_power_gpu(options: &mut eframe::NativeOptions, env_set: bool) {
    if env_set {
        return;
    }
    if let eframe::egui_wgpu::WgpuSetup::CreateNew(setup) = &mut options.wgpu_options.wgpu_setup {
        setup.power_preference = eframe::wgpu::PowerPreference::LowPower;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn power(o: &eframe::NativeOptions) -> eframe::wgpu::PowerPreference {
        match &o.wgpu_options.wgpu_setup {
            eframe::egui_wgpu::WgpuSetup::CreateNew(s) => s.power_preference,
            _ => panic!("atteso WgpuSetup::CreateNew"),
        }
    }

    #[test]
    fn low_power_gpu_unless_the_env_var_decides() {
        let mut o = eframe::NativeOptions::default();
        prefer_low_power_gpu(&mut o, false);
        assert_eq!(power(&o), eframe::wgpu::PowerPreference::LowPower);

        // Con `WGPU_POWER_PREF` impostata non si tocca la scelta di wgpu.
        let mut o = eframe::NativeOptions::default();
        let before = power(&o);
        prefer_low_power_gpu(&mut o, true);
        assert_eq!(power(&o), before);
    }
}

