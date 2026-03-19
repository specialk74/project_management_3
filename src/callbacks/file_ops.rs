use slint::{ComponentHandle, Global};

use crate::app::App;
use crate::ui_sync::{refresh, sync_project_texts};
use crate::{AppWindow, PjmCallback};

use super::SharedState;

pub fn register(ui: &AppWindow, state: &SharedState) {
    register_save_file(ui, state);
    register_open_file(ui, state);
}

fn register_save_file(ui: &AppWindow, state: &SharedState) {
    let app = state.app.clone();
    let ui_w = ui.as_weak();
    PjmCallback::get(ui).on_save_file(move || {
        if let Some(ui) = ui_w.upgrade() {
            let mut a = app.borrow_mut();
            sync_project_texts(&ui, &mut a);
            let path = PjmCallback::get(&ui).get_current_file().to_string();
            a.save(&path);
            PjmCallback::get(&ui).set_changed(false);
        }
    });
}

fn register_open_file(ui: &AppWindow, state: &SharedState) {
    let app = state.app.clone();
    let live = state.live.clone();
    let row_counts = state.row_counts.clone();
    let visibility = state.visibility.clone();
    let ui_w = ui.as_weak();
    PjmCallback::get(ui).on_open_file(move || {
        if let Some(ui) = ui_w.upgrade() {
            let path = PjmCallback::get(&ui).get_current_file().to_string();
            match App::load(&path) {
                Ok(loaded) => {
                    *app.borrow_mut() = loaded;
                    row_counts.borrow_mut().clear();
                    visibility.borrow_mut().clear();
                    refresh(
                        &ui,
                        &app.borrow(),
                        &live,
                        &row_counts.borrow(),
                        &visibility.borrow(),
                    );
                    PjmCallback::get(&ui).set_changed(false);
                }
                Err(e) => eprintln!("Errore apertura '{path}': {e}"),
            }
        }
    });
}
