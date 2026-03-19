use slint::{ModelRc, SharedString, VecModel};
use std::rc::Rc;

use crate::{AppProject, DayData, DevInfo, EffortByPrjData, SovraData};

// Slint non propaga il cambio di `in-out property<Struct>` ai `for` che
// guardano sotto-campi del tipo `root.efforts.projects`. Mantenendo stabili
// i VecModel (stessa istanza Rc) e aggiornando il contenuto con set_vec(),
// le binding si aggiornano correttamente.
pub struct LiveModels {
    pub projects: Rc<VecModel<EffortByPrjData>>,
    pub worker_names: Rc<VecModel<SharedString>>,
    pub sovra: Rc<VecModel<SovraData>>,
    pub weeks: Rc<VecModel<DayData>>,
    pub devs: Rc<VecModel<DevInfo>>,
}

impl LiveModels {
    pub fn new() -> Self {
        Self {
            projects: Rc::new(VecModel::default()),
            worker_names: Rc::new(VecModel::default()),
            sovra: Rc::new(VecModel::default()),
            weeks: Rc::new(VecModel::default()),
            devs: Rc::new(VecModel::default()),
        }
    }

    /// Costruisce l'AppProject iniziale con riferimenti ai modelli stabili.
    pub fn make_efforts_data(&self) -> AppProject {
        AppProject {
            projects: ModelRc::from(self.projects.clone()),
            worker_names: ModelRc::from(self.worker_names.clone()),
            sovra: ModelRc::from(self.sovra.clone()),
            week_off: ModelRc::new(VecModel::from(vec![])),
            devs: ModelRc::from(self.devs.clone()),
        }
    }
}
