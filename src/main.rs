mod devs;
mod projects;
mod single_dev;
mod sinlge_effort;
mod workers;

slint::include_modules!();

use serde::{Deserialize, Serialize};
use slint::{Model, ModelRc, SharedString, VecModel};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::devs::Devs;
use crate::projects::Projects;
use crate::sinlge_effort::Effort;
use crate::single_dev::WeekId;
use crate::workers::{WorkerId, Workers};
use std::fs;

const SAVE_PATH: &str = "workers.ron";

fn packed_to_color(packed: usize) -> slint::Color {
    slint::Color::from_rgb_u8(
        ((packed >> 16) & 0xFF) as u8,
        ((packed >> 8) & 0xFF) as u8,
        (packed & 0xFF) as u8,
    )
}

// ── App data model ────────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize)]
struct App {
    start_week: usize,
    workers: Workers,
    devs: Devs,
    projects: Projects,
}

impl App {
    fn new() -> Self {
        Self {
            start_week: 1,
            workers: Workers::new(),
            devs: Devs::new(),
            projects: Projects::new(),
        }
    }

    fn save(&self, path: &str) {
        let content = ron::ser::to_string_pretty(self, ron::ser::PrettyConfig::default()).unwrap();
        let _ = fs::write(path, content);
    }

    fn load(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string(path)?;
        Ok(ron::from_str(&content)?)
    }
}

// ── Modelli stabili (fix reattività Slint) ────────────────────────────────────
//
// Slint non propaga il cambio di `in-out property<Struct>` ai `for` che
// guardano sotto-campi del tipo `root.efforts.projects`. Mantenendo stabili
// i VecModel (stessa istanza Rc) e aggiornando il contenuto con set_vec(),
// le binding si aggiornano correttamente.

struct LiveModels {
    projects: Rc<VecModel<EffortByPrjData>>,
    worker_names: Rc<VecModel<SharedString>>,
    sovra: Rc<VecModel<SovraData>>,
    weeks: Rc<VecModel<DayData>>,
    devs: Rc<VecModel<DevInfo>>,
}

impl LiveModels {
    fn new() -> Self {
        Self {
            projects: Rc::new(VecModel::default()),
            worker_names: Rc::new(VecModel::default()),
            sovra: Rc::new(VecModel::default()),
            weeks: Rc::new(VecModel::default()),
            devs: Rc::new(VecModel::default()),
        }
    }

    /// Costruisce l'EffortsData iniziale con riferimenti ai modelli stabili.
    fn make_efforts_data(&self) -> AppProject {
        AppProject {
            projects: ModelRc::from(self.projects.clone()),
            worker_names: ModelRc::from(self.worker_names.clone()),
            sovra: ModelRc::from(self.sovra.clone()),
            week_off: ModelRc::new(VecModel::from(vec![])),
            devs: ModelRc::from(self.devs.clone()),
        }
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn mk<T: Clone + 'static>(v: Vec<T>) -> ModelRc<T> {
    ModelRc::new(VecModel::from(v))
}

fn current_week() -> usize {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let days = secs / 86400;
    ((days + 3) / 7 % 52 + 1) as usize
}

// ── Calcolo range settimane ───────────────────────────────────────────────────

fn find_end_week(app: &App) -> usize {
    let start = app.start_week;
    let mut end = start + 51;
    for (proj_id, _) in app.projects.list() {
        for dev_id in app.projects.get_dev_ids(proj_id) {
            if let Some(sd) = app.projects.get_single_dev(proj_id, dev_id) {
                for w in sd.get_weeks() {
                    if w.0 > end {
                        end = w.0;
                    }
                }
            }
        }
    }
    end
}

fn build_weeks(start: usize, end: usize) -> Vec<DayData> {
    (start..=end)
        .map(|w| DayData {
            week: w as i32,
            text: SharedString::from(format!("W{:02}", (w.saturating_sub(1)) % 52 + 1)),
        })
        .collect()
}

// ── Costruzione dati Slint ────────────────────────────────────────────────────

fn build_project_data(
    app: &App,
    row_counts: &HashMap<(i32, i32), i32>,
    visibility: &HashMap<(i32, i32), bool>,
) -> Vec<EffortByPrjData> {
    let workers = app.workers.list();
    let projects = app.projects.list();
    let start_w = app.start_week;
    let end_w = find_end_week(app);
    let n_weeks = end_w - start_w + 1;

    projects
        .iter()
        .enumerate()
        .map(|(pi, (proj_id, proj_name))| {
            let dev_data: Vec<EffortByDevData> = app
                .projects
                .list_devs(*proj_id)
                .iter()
                .map(|dev_id| {
                    let extra = *row_counts.get(&(pi as i32, dev_id.0 as i32)).unwrap_or(&0);
                    let max = extra;
                    let enable = *visibility
                        .get(&(pi as i32, dev_id.0 as i32))
                        .unwrap_or(&true);
                    if let Some(sd) = app.projects.get_single_dev(*proj_id, *dev_id) {
                        build_dev(
                            pi as i32,
                            dev_id.0 as i32,
                            sd,
                            &workers,
                            start_w,
                            end_w,
                            max,
                            enable,
                        )
                    } else {
                        empty_dev(pi as i32, dev_id.0 as i32, n_weeks, start_w, max)
                    }
                })
                .collect();

            EffortByPrjData {
                project_id: pi as i32,
                text: SharedString::from(proj_name.as_str()),
                start_week: start_w as i32,
                end_week: end_w as i32,
                visible: true,
                enable: true,
                devs_of_the_project: mk(dev_data),
            }
        })
        .collect()
}

fn build_dev(
    proj_idx: i32,
    dev_idx: i32,
    sd: &single_dev::SingleDev,
    workers: &[(WorkerId, String)],
    start_w: usize,
    end_w: usize,
    max: i32,
    enable: bool,
) -> EffortByDevData {
    let planned = sd.planned_effort().0 as i32;
    let total = sd.get_effort_tot().0 as i32;
    let extra_rows = (max - workers.len() as i32).max(0) as usize;
    println!("extra_rows: {}", extra_rows);

    let mut cumulative = 0i32;
    let week_data: Vec<EffortByDateData> = (start_w..=end_w)
        .map(|w| {
            let mut persons: Vec<SharedString> = workers
                .iter()
                .map(|(wid, _)| {
                    let e = sd.get_effort(WeekId(w), *wid).0;
                    SharedString::from(if e == 0 { String::new() } else { e.to_string() })
                })
                .collect();
            persons.extend(std::iter::repeat_n(SharedString::default(), extra_rows));
            let week_total: i32 = workers
                .iter()
                .map(|(wid, _)| sd.get_effort(WeekId(w), *wid).0 as i32)
                .sum();
            cumulative += week_total;

            EffortByDateData {
                total: week_total,
                remains: planned - cumulative,
                dev: dev_idx,
                project: proj_idx,
                effort: planned,
                week: w as i32,
                persons: mk(persons),
            }
        })
        .collect();

    EffortByDevData {
        project: proj_idx,
        dev: dev_idx,
        total,
        effort: planned,
        remains: planned - total,
        visible: true,
        enable,
        max: (max - 1).max(0),
        datas: mk(week_data),
    }
}

fn empty_dev(
    proj_idx: i32,
    dev_idx: i32,
    n_weeks: usize,
    start_w: usize,
    max: i32,
) -> EffortByDevData {
    let n_persons = max.max(0) as usize;
    let week_data: Vec<EffortByDateData> = (0..n_weeks)
        .map(|i| EffortByDateData {
            total: 0,
            remains: 0,
            dev: dev_idx,
            project: proj_idx,
            effort: 0,
            week: (start_w + i) as i32,
            persons: mk(vec![SharedString::default(); n_persons]),
        })
        .collect();

    EffortByDevData {
        project: proj_idx,
        dev: dev_idx,
        total: 0,
        effort: 0,
        remains: 0,
        visible: true,
        enable: false,
        max: (max - 1).max(0),
        datas: mk(week_data),
    }
}

fn build_sovra_data(app: &App) -> Vec<SovraData> {
    let workers = app.workers.list();
    let projects = app.projects.list();
    let devs = app.devs.list();
    let start_w = app.start_week;
    let end_w = find_end_week(app);

    (start_w..=end_w)
        .map(|w| {
            let values: Vec<i32> = workers
                .iter()
                .map(|(wid, _)| {
                    let total_h: usize = projects
                        .iter()
                        .flat_map(|(pid, _)| {
                            devs.iter().map(move |(did, _)| {
                                app.projects
                                    .get_single_dev(*pid, *did)
                                    .map_or(0, |sd| sd.get_effort(WeekId(w), *wid).0)
                            })
                        })
                        .sum();
                    ((total_h * 100) / 40) as i32
                })
                .collect();
            SovraData {
                week: w as i32,
                value: mk(values),
            }
        })
        .collect()
}

// ── Costruisce la lista DevInfo dai dev correnti ──────────────────────────────

fn build_dev_infos(app: &App) -> Vec<DevInfo> {
    app.devs
        .list_full()
        .into_iter()
        .map(|(_, name, bg, font)| DevInfo {
            title: SharedString::from(name.as_str()),
            col: packed_to_color(bg),
            text_color: packed_to_color(font),
        })
        .collect()
}

// ── Sync testi progetto da Slint → Rust (prima di ogni rebuild) ───────────────

fn sync_project_texts(ui: &AppWindow, app: &mut App) {
    let efforts = ui.get_app_project();
    let projects = app.projects.list();
    for (idx, (proj_id, _)) in projects.iter().enumerate() {
        if let Some(item) = efforts.projects.row_data(idx) {
            app.projects.set_project_info(*proj_id, &item.text);
        }
    }
}

// ── Refresh completo dei modelli stabili ──────────────────────────────────────

fn refresh(
    _ui: &AppWindow,
    app: &App,
    live: &LiveModels,
    row_counts: &HashMap<(i32, i32), i32>,
    visibility: &HashMap<(i32, i32), bool>,
) {
    let end_w = find_end_week(app);

    live.projects
        .set_vec(build_project_data(app, row_counts, visibility));
    live.worker_names.set_vec(
        app.workers
            .list()
            .into_iter()
            .map(|(_, n)| SharedString::from(n.as_str()))
            .collect::<Vec<_>>(),
    );
    live.sovra.set_vec(build_sovra_data(app));
    live.weeks.set_vec(build_weeks(app.start_week, end_w));
    live.devs.set_vec(build_dev_infos(app));
}

// ── Entry point ───────────────────────────────────────────────────────────────

fn main() {
    let app = Rc::new(RefCell::new(
        App::load(SAVE_PATH).unwrap_or_else(|_| App::new()),
    ));

    let row_counts: Rc<RefCell<HashMap<(i32, i32), i32>>> = Rc::default();
    let visibility: Rc<RefCell<HashMap<(i32, i32), bool>>> = Rc::default();

    let live = Rc::new(LiveModels::new());

    let ui = AppWindow::new().unwrap();

    // Prima impostazione: collega i modelli stabili all'UI
    ui.set_app_project(live.make_efforts_data());
    ui.set_weeks(ModelRc::from(live.weeks.clone()));

    // Populate iniziale
    refresh(
        &ui,
        &app.borrow(),
        &live,
        &row_counts.borrow(),
        &visibility.borrow(),
    );
    {
        let pcb = PjmCallback::get(&ui);
        pcb.set_current_file(SharedString::from(SAVE_PATH));
        pcb.set_this_week(current_week() as i32);
        pcb.set_changed(false);
    }

    // ── changed_effort ────────────────────────────────────────────────────────
    {
        let app = app.clone();
        let ui_w = ui.as_weak();
        PjmCallback::get(&ui).on_changed_effort(move |effort| {
            let mut a = app.borrow_mut();
            let projects = a.projects.list();
            let devs = a.devs.list();
            let workers = a.workers.list();
            let Some(&(proj_id, _)) = projects.get(effort.project as usize) else {
                return;
            };
            let Some(&(dev_id, _)) = devs.get(effort.dev as usize) else {
                return;
            };
            let week = effort.week as usize;
            for (i, person) in effort.persons.iter().enumerate() {
                if let Some(&(worker_id, _)) = workers.get(i) {
                    let e = person.trim().parse::<usize>().unwrap_or(0);
                    a.projects.add_effort(proj_id, dev_id, WeekId(week), worker_id, Effort(e));
                }
            }
            if let Some(ui) = ui_w.upgrade() {
                PjmCallback::get(&ui).set_changed(true);
            }
        });
    }

    // ── set_dev_effort ────────────────────────────────────────────────────────
    {
        let app = app.clone();
        let live = live.clone();
        let row_counts = row_counts.clone();
        let visibility = visibility.clone();
        let ui_w = ui.as_weak();
        PjmCallback::get(&ui).on_set_dev_effort(move |proj_idx, dev_idx, effort| {
            if let Some(ui) = ui_w.upgrade() {
                let mut a = app.borrow_mut();
                let projects = a.projects.list();
                let devs = a.devs.list();
                let Some(&(proj_id, _)) = projects.get(proj_idx as usize) else {
                    return;
                };
                let Some(&(dev_id, _)) = devs.get(dev_idx as usize) else {
                    return;
                };
                a.projects.add_dev_effort(proj_id, dev_id, Effort(effort as usize));
                refresh(&ui, &a, &live, &row_counts.borrow(), &visibility.borrow());
                PjmCallback::get(&ui).set_changed(true);
            }
        });
    }

    // ── save_file ─────────────────────────────────────────────────────────────
    {
        let app = app.clone();
        let ui_w = ui.as_weak();
        PjmCallback::get(&ui).on_save_file(move || {
            if let Some(ui) = ui_w.upgrade() {
                let mut a = app.borrow_mut();
                sync_project_texts(&ui, &mut a);
                let path = PjmCallback::get(&ui).get_current_file().to_string();
                a.save(&path);
                PjmCallback::get(&ui).set_changed(false);
            }
        });
    }

    // ── open_file ─────────────────────────────────────────────────────────────
    {
        let app = app.clone();
        let live = live.clone();
        let row_counts = row_counts.clone();
        let visibility = visibility.clone();
        let ui_w = ui.as_weak();
        PjmCallback::get(&ui).on_open_file(move || {
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

    // ── new_project ───────────────────────────────────────────────────────────
    {
        let app = app.clone();
        let live = live.clone();
        let row_counts = row_counts.clone();
        let visibility = visibility.clone();
        let ui_w = ui.as_weak();
        PjmCallback::get(&ui).on_new_project(move || {
            if let Some(ui) = ui_w.upgrade() {
                let mut a = app.borrow_mut();
                sync_project_texts(&ui, &mut a);
                a.projects.add("Nuovo Progetto");
                refresh(&ui, &a, &live, &row_counts.borrow(), &visibility.borrow());
                PjmCallback::get(&ui).set_changed(true);
            }
        });
    }

    // ── add_worker ────────────────────────────────────────────────────────────
    {
        let app = app.clone();
        let live = live.clone();
        let row_counts = row_counts.clone();
        let visibility = visibility.clone();
        let ui_w = ui.as_weak();
        PjmCallback::get(&ui).on_add_worker(move |name| {
            if name.is_empty() {
                return;
            }
            if let Some(ui) = ui_w.upgrade() {
                let mut a = app.borrow_mut();
                sync_project_texts(&ui, &mut a);
                a.workers.add(&name);
                refresh(&ui, &a, &live, &row_counts.borrow(), &visibility.borrow());
                PjmCallback::get(&ui).set_changed(true);
            }
        });
    }

    // ── add_dev ───────────────────────────────────────────────────────────────
    {
        let app = app.clone();
        let live = live.clone();
        let row_counts = row_counts.clone();
        let visibility = visibility.clone();
        let ui_w = ui.as_weak();
        PjmCallback::get(&ui).on_add_dev(move |name| {
            if name.is_empty() {
                return;
            }
            if let Some(ui) = ui_w.upgrade() {
                let mut a = app.borrow_mut();
                sync_project_texts(&ui, &mut a);
                a.devs.add(&name);
                refresh(&ui, &a, &live, &row_counts.borrow(), &visibility.borrow());
                PjmCallback::get(&ui).set_changed(true);
            }
        });
    }

    // ── add_dev_to_project ────────────────────────────────────────────────────
    {
        let app = app.clone();
        let live = live.clone();
        let row_counts = row_counts.clone();
        let visibility = visibility.clone();
        let ui_w = ui.as_weak();
        PjmCallback::get(&ui).on_add_dev_to_project(move |proj_idx, dev_idx, add| {
            if let Some(ui) = ui_w.upgrade() {
                let mut a = app.borrow_mut();
                let projects = a.projects.list();
                let devs = a.devs.list();
                let Some(&(proj_id, _)) = projects.get(proj_idx as usize) else {
                    return;
                };
                let Some(&(dev_id, _)) = devs.get(dev_idx as usize) else {
                    return;
                };
                sync_project_texts(&ui, &mut a);
                if add {
                    a.projects.add_dev(proj_id, dev_id);
                } else {
                    a.projects.del_dev(proj_id, dev_id);
                }
                refresh(&ui, &a, &live, &row_counts.borrow(), &visibility.borrow());
                PjmCallback::get(&ui).set_changed(true);
            }
        });
    }

    // ── set_project_name ──────────────────────────────────────────────────────
    {
        let app = app.clone();
        let ui_w = ui.as_weak();
        PjmCallback::get(&ui).on_set_project_name(move |proj_idx, name| {
            if let Some(_ui) = ui_w.upgrade() {
                let mut a = app.borrow_mut();
                let projects = a.projects.list();
                let Some(&(proj_id, _)) = projects.get(proj_idx as usize) else {
                    return;
                };
                a.projects.set_project_info(proj_id, name.as_str());
                PjmCallback::get(&_ui).set_changed(true);
            }
        });
    }

    // ── add_row ───────────────────────────────────────────────────────────────
    {
        let app = app.clone();
        let live = live.clone();
        let row_counts = row_counts.clone();
        let visibility = visibility.clone();
        let ui_w = ui.as_weak();
        PjmCallback::get(&ui).on_add_row(move |proj_idx, dev_idx| {
            *row_counts
                .borrow_mut()
                .entry((proj_idx, dev_idx))
                .or_insert(0) += 1;
            if let Some(ui) = ui_w.upgrade() {
                let mut a = app.borrow_mut();
                sync_project_texts(&ui, &mut a);
                refresh(&ui, &a, &live, &row_counts.borrow(), &visibility.borrow());
            }
        });
    }

    // ── del_row ───────────────────────────────────────────────────────────────
    {
        let app = app.clone();
        let live = live.clone();
        let row_counts = row_counts.clone();
        let visibility = visibility.clone();
        let ui_w = ui.as_weak();
        PjmCallback::get(&ui).on_del_row(move |proj_idx, dev_idx| {
            {
                let mut rc = row_counts.borrow_mut();
                let entry = rc.entry((proj_idx, dev_idx)).or_insert(0);
                if *entry > 0 {
                    *entry -= 1;
                }
            }
            if let Some(ui) = ui_w.upgrade() {
                let mut a = app.borrow_mut();
                sync_project_texts(&ui, &mut a);
                refresh(&ui, &a, &live, &row_counts.borrow(), &visibility.borrow());
            }
        });
    }

    // ── hide_dev ──────────────────────────────────────────────────────────────
    {
        let app = app.clone();
        let live = live.clone();
        let row_counts = row_counts.clone();
        let visibility = visibility.clone();
        let ui_w = ui.as_weak();
        PjmCallback::get(&ui).on_hide_dev(move |proj_idx, dev_idx, new_enable| {
            visibility
                .borrow_mut()
                .insert((proj_idx, dev_idx), new_enable);
            if let Some(ui) = ui_w.upgrade() {
                let mut a = app.borrow_mut();
                sync_project_texts(&ui, &mut a);
                refresh(&ui, &a, &live, &row_counts.borrow(), &visibility.borrow());
            }
        });
    }

    // ── move_effort ───────────────────────────────────────────────────────────
    {
        let app = app.clone();
        let live = live.clone();
        let row_counts = row_counts.clone();
        let visibility = visibility.clone();
        let ui_w = ui.as_weak();
        PjmCallback::get(&ui).on_move_effort(
            move |proj_idx, dev_idx, start_week, end_week, start_row, end_row, offset| {
                if offset == 0 {
                    return;
                }
                let mut a = app.borrow_mut();
                let projects = a.projects.list();
                let devs = a.devs.list();
                let workers = a.workers.list();
                let Some(&(proj_id, _)) = projects.get(proj_idx as usize) else {
                    return;
                };
                let Some(&(dev_id, _)) = devs.get(dev_idx as usize) else {
                    return;
                };

                let max_row = (end_row as usize).min(workers.len().saturating_sub(1));

                // Raccogli i valori sorgente
                let mut moves: Vec<(usize, WorkerId, usize)> = Vec::new();
                for w in (start_week as usize)..=(end_week as usize) {
                    for r in (start_row as usize)..=max_row {
                        if let Some(&(worker_id, _)) = workers.get(r) {
                            let e = a
                                .projects
                                .get_single_dev(proj_id, dev_id)
                                .map_or(0, |sd| sd.get_effort(WeekId(w), worker_id).0);
                            moves.push((w, worker_id, e));
                        }
                    }
                }

                // Azzera sorgente
                for w in (start_week as usize)..=(end_week as usize) {
                    for r in (start_row as usize)..=max_row {
                        if let Some(&(worker_id, _)) = workers.get(r) {
                            a.projects.add_effort(proj_id, dev_id, WeekId(w), worker_id, Effort(0));
                        }
                    }
                }

                // Scrivi destinazione
                for (w, worker_id, effort) in moves {
                    let dst_w = (w as i32 + offset) as usize;
                    a.projects
                        .add_effort(proj_id, dev_id, WeekId(dst_w), worker_id, Effort(effort));
                }

                if let Some(ui) = ui_w.upgrade() {
                    sync_project_texts(&ui, &mut a);
                    refresh(&ui, &a, &live, &row_counts.borrow(), &visibility.borrow());
                    PjmCallback::get(&ui).set_changed(true);
                }
            },
        );
    }

    // ── search ────────────────────────────────────────────────────────────────
    // last-search è sufficiente: Cell-RW.selected lo confronta per evidenziare.
    PjmCallback::get(&ui).on_search(|_| {});

    ui.run().unwrap();
}
