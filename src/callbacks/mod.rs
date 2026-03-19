mod effort;
mod file_ops;
mod members;
mod project;
mod rows;

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::app::App;
use crate::live_models::LiveModels;
use crate::AppWindow;

pub struct SharedState {
    pub app: Rc<RefCell<App>>,
    pub row_counts: Rc<RefCell<HashMap<(i32, i32), i32>>>,
    pub visibility: Rc<RefCell<HashMap<(i32, i32), bool>>>,
    pub live: Rc<LiveModels>,
}

pub fn setup_all(ui: &AppWindow, state: &SharedState) {
    effort::register(ui, state);
    file_ops::register(ui, state);
    members::register(ui, state);
    project::register(ui, state);
    rows::register(ui, state);
}
