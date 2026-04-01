mod app;
mod builders;
mod callbacks;
mod date_utils;
mod dev_utils;
mod live_models;
mod projects;
mod single_dev;
mod single_efforts;
mod ui_sync;
mod workers;

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
    let app = Rc::new(RefCell::new(
        App::load(SAVE_PATH).unwrap_or_else(|_| App::new()),
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
        pcb.set_current_file(SharedString::from(SAVE_PATH));
        //pcb.set_this_week(current_week() as i32);
        pcb.set_changed(false);
        let this_week = local_to_days(&primo_giorno_settimana_corrente(&Utc::now().date_naive()));
        //println!("this_week: {}", this_week);
        pcb.set_this_week(this_week);
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
