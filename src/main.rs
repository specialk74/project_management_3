mod app;
mod builders;
mod callbacks;
mod date_utils;
mod dev_utils;
mod live_models;
mod project_utils;
mod single_dev_utils;
mod single_effort_utils;
mod ui_sync;
mod workers_utils;

slint::include_modules!();

use chrono::Utc;
use slint::{ModelRc, SharedString};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::app::{App, SAVE_PATH};
use crate::callbacks::SharedState;
use crate::date_utils::dates::{local_to_days, primo_giorno_settimana_corrente};
use crate::live_models::LiveModels;

fn main() {
    let file_path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| SAVE_PATH.to_string());
    let app = Rc::new(RefCell::new(
        App::load(&file_path).unwrap_or_else(|_| App::new()),
    ));
    let row_counts: Rc<RefCell<HashMap<(i32, i32), i32>>> = Rc::default();
    let visibility: Rc<RefCell<HashMap<(i32, i32), bool>>> = Rc::default();
    let live = Rc::new(LiveModels::new());

    let ui = AppWindow::new().unwrap();
    ui.set_app_project(live.make_efforts_data());
    ui.set_weeks(ModelRc::from(live.weeks.clone()));

    ui_sync::refresh(
        &ui,
        &mut app.borrow_mut(),
        &live,
        &row_counts.borrow(),
        &visibility.borrow(),
    );

    {
        let pcb = PjmCallback::get(&ui);
        pcb.set_current_file(SharedString::from(file_path.as_str()));
        pcb.set_changed(false);
        let today = Utc::now().date_naive();
        let this_week_date = primo_giorno_settimana_corrente(&today);
        let this_week = local_to_days(&this_week_date);
        pcb.set_this_week(this_week);
        pcb.set_current_week_text(SharedString::from(
            this_week_date.format("%y-%m-%d").to_string(),
        ));

        let start_week = app.borrow().start_week.0 as i32;
        let col_index = (this_week - start_week) / 7;
        if col_index > 0 {
            const COL_WIDTH: f32 = 90.0;
            const LEFT_COL_WIDTH: f32 = 300.0;
            const INITIAL_WINDOW_WIDTH: f32 = 1024.0;
            let visible_width = INITIAL_WINDOW_WIDTH - LEFT_COL_WIDTH;
            let col_center = col_index as f32 * COL_WIDTH + COL_WIDTH / 2.0;
            let scroll_x = (col_center - visible_width / 2.0).max(0.0);
            ui.set_viewport_x(-scroll_x);
        }
    }

    let state = SharedState {
        app,
        row_counts,
        visibility,
        live,
    };
    callbacks::setup_all(&ui, &state);

    ui.run().unwrap();
}
