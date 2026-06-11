use slint::{ModelRc, SharedString, VecModel};
use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;

use crate::{AppProject, CategoryInfo, DayData, DevInfo, EffortByPrjData, SovraData};

// Slint non propaga il cambio di `in-out property<Struct>` ai `for` che
// guardano sotto-campi del tipo `root.efforts.projects`. Mantenendo stabili
// i VecModel (stessa istanza Rc) e aggiornando il contenuto con set_vec(),
// le binding si aggiornano correttamente.
pub struct LiveModels {
    pub projects: Rc<VecModel<EffortByPrjData>>,
    pub worker_names: Rc<VecModel<SharedString>>,
    pub worker_max_hours: Rc<VecModel<i32>>,
    pub worker_filter_selected: Rc<VecModel<bool>>,
    pub worker_footer_visible: Rc<VecModel<bool>>,
    pub worker_filter: Rc<RefCell<HashSet<String>>>,
    pub sovra: Rc<VecModel<SovraData>>,
    pub weeks: Rc<VecModel<DayData>>,
    pub devs: Rc<VecModel<DevInfo>>,
    pub years: Rc<VecModel<i32>>,
    pub dev_year_totals: Rc<VecModel<i32>>,
    pub categories: Rc<VecModel<CategoryInfo>>,
}

impl LiveModels {
    pub fn new() -> Self {
        Self {
            projects: Rc::new(VecModel::default()),
            worker_names: Rc::new(VecModel::default()),
            worker_max_hours: Rc::new(VecModel::default()),
            worker_filter_selected: Rc::new(VecModel::default()),
            worker_footer_visible: Rc::new(VecModel::default()),
            worker_filter: Rc::new(RefCell::new(HashSet::new())),
            sovra: Rc::new(VecModel::default()),
            weeks: Rc::new(VecModel::default()),
            devs: Rc::new(VecModel::default()),
            years: Rc::new(VecModel::default()),
            dev_year_totals: Rc::new(VecModel::default()),
            categories: Rc::new(VecModel::default()),
        }
    }

    /// Costruisce l'AppProject iniziale con riferimenti ai modelli stabili.
    pub fn make_efforts_data(&self) -> AppProject {
        AppProject {
            projects: ModelRc::from(self.projects.clone()),
            worker_names: ModelRc::from(self.worker_names.clone()),
            worker_max_hours: ModelRc::from(self.worker_max_hours.clone()),
            worker_filter_selected: ModelRc::from(self.worker_filter_selected.clone()),
            worker_footer_visible: ModelRc::from(self.worker_footer_visible.clone()),
            sovra: ModelRc::from(self.sovra.clone()),
            week_off: ModelRc::new(VecModel::from(vec![])),
            devs: ModelRc::from(self.devs.clone()),
            years: ModelRc::from(self.years.clone()),
            dev_year_totals: ModelRc::from(self.dev_year_totals.clone()),
            categories: ModelRc::from(self.categories.clone()),
        }
    }
}
