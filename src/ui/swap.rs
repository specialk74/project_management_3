//! Finestre sui worker del menù Modifica, con le stesse colonne:
//! - «Scambia worker» (Modifica ▸ Scambia worker…): a sinistra il worker da
//!   sostituire, al centro quello che prende il suo posto (uno solo per
//!   parte: scelto uno, gli altri si disattivano), a destra i progetti in cui
//!   il primo ha effort e, sotto ognuno, i suoi dev (con Select All), in basso
//!   da quando. «OK» manda `Action::SwapWorker`, che sposta le celle con le
//!   stesse ore (`swap_worker`);
//! - «Cancella worker» (Modifica ▸ Cancella worker dai progetti…): a sinistra
//!   il worker, a destra progetti e dev in cui è presente nel periodo, in
//!   basso il periodo (da oggi in poi, o dal … al …). «Cancella…» chiede
//!   conferma e manda `Action::RemoveWorkerEffort` (`remove_worker_cells`).

use super::*;

/// Larghezza delle colonne dei worker e di quella dei progetti (px).
const SWAP_WORKER_W: f32 = 220.0;
const SWAP_PROJECT_W: f32 = 300.0;
/// Altezza massima degli elenchi (px), poi si scorre.
const SWAP_LIST_H: f32 = 320.0;
/// Spazio fra le colonne (px). Non un separatore verticale: in una finestra
/// che si adatta al contenuto prenderebbe tutta l'altezza disponibile e la
/// farebbe crescere a ogni frame.
const SWAP_GAP: f32 = 16.0;

/// Celle in cui `to` ha già una cella dove `from` ne ha una (nelle coppie
/// progetto × dev `targets`, dalla settimana `start`): lì le ore si sommano.
pub(crate) fn swap_conflicts(
    app: &App,
    from: WorkerId,
    to: WorkerId,
    targets: &[(ProjectId, DevId)],
    start: WeekId,
) -> usize {
    targets
        .iter()
        .map(|&(proj, dev)| {
            app.projects.get_single_dev(proj, dev).map_or(0, |sd| {
                sd.get_weeks()
                    .into_iter()
                    .filter(|w| *w >= start)
                    .filter(|w| {
                        cell_effort(app, proj, dev, *w, from).is_some() && cell_effort(app, proj, dev, *w, to).is_some()
                    })
                    .count()
            })
        })
        .sum()
}

/// Spunta (`on`) o toglie tutte le coppie `items`.
fn set_ticks(ticked: &mut HashSet<(ProjectId, DevId)>, items: &[(ProjectId, DevId)], on: bool) {
    for t in items {
        if on {
            ticked.insert(*t);
        } else {
            ticked.remove(t);
        }
    }
}

/// Colonna di worker a scelta singola: ricerca e una casella per worker;
/// scelto uno, gli altri si disattivano (togliendo la spunta si riattivano).
/// `blocked` = worker non selezionabile (quello scelto nell'altra colonna).
#[allow(clippy::too_many_arguments)]
fn single_worker_column(
    ui: &mut egui::Ui,
    salt: &str,
    title: &str,
    workers: &[(WorkerId, String)],
    search: &mut String,
    chosen: &mut Option<WorkerId>,
    blocked: Option<WorkerId>,
    enabled: bool,
) {
    ui.vertical(|ui| {
        ui.set_width(SWAP_WORKER_W);
        ui.strong(title);
        ui.add_enabled(
            enabled,
            egui::TextEdit::singleline(search).hint_text("Cerca worker…").desired_width(SWAP_WORKER_W - 8.0),
        );
        let q = search.trim().to_lowercase();
        egui::ScrollArea::vertical()
            .id_salt(salt)
            .scroll_source(drag_scroll())
            .max_height(SWAP_LIST_H)
            .auto_shrink([false, true])
            .show(ui, |ui| {
                let listed: Vec<&(WorkerId, String)> = workers.iter().filter(|(_, n)| search_matches(&q, n)).collect();
                if listed.is_empty() {
                    ui.label("(nessun worker)");
                }
                for (id, name) in listed {
                    let mut on = *chosen == Some(*id);
                    let free = chosen.is_none() || on;
                    let ok = enabled && free && blocked != Some(*id);
                    if ui.add_enabled(ok, egui::Checkbox::new(&mut on, name)).changed() {
                        *chosen = on.then_some(*id);
                    }
                }
            });
    });
}

/// Progetti × dev proposti in un frame, con le spunte già ripulite.
struct Targets {
    projects: Vec<SwapProject>,
    /// Tutte le coppie in elenco, nell'ordine mostrato.
    all: Vec<(ProjectId, DevId)>,
    /// Quelle spuntate, nello stesso ordine.
    chosen: Vec<(ProjectId, DevId)>,
    /// Progetti con almeno un dev spuntato, celle e ore spuntate.
    n_projects: usize,
    cells: usize,
    hours: usize,
}

impl Targets {
    /// Le spunte restano solo sulle coppie in elenco.
    fn new(projects: Vec<SwapProject>, ticked: &mut HashSet<(ProjectId, DevId)>) -> Self {
        let all: Vec<(ProjectId, DevId)> =
            projects.iter().flat_map(|p| p.devs.iter().map(move |d| (p.proj, d.dev))).collect();
        ticked.retain(|t| all.contains(t));
        let chosen: Vec<(ProjectId, DevId)> = all.iter().copied().filter(|t| ticked.contains(t)).collect();
        let mut ps: Vec<ProjectId> = chosen.iter().map(|(p, _)| *p).collect();
        ps.dedup();
        let ticked = &*ticked;
        let (cells, hours) = projects
            .iter()
            .flat_map(|p| p.devs.iter().filter(move |d| ticked.contains(&(p.proj, d.dev))))
            .fold((0, 0), |(c, h), d| (c + d.cells, h + d.hours));
        Self { projects, all, chosen, n_projects: ps.len(), cells, hours }
    }

    /// «N celle (Hh) in M progetti (K dev)».
    fn summary(&self) -> String {
        format!(
            "{} celle ({}h) in {} progetti ({} dev)",
            self.cells,
            self.hours,
            self.n_projects,
            self.chosen.len()
        )
    }
}

/// Colonna «Progetti e dev»: Select All, poi ogni progetto (la sua spunta
/// prende o toglie tutti i suoi dev; solo alcuni ⇒ stato intermedio) con i
/// suoi dev rientrati sotto, ognuno con le ore. `none` = testo senza
/// worker scelto, `empty` = testo se non ci sono progetti.
fn project_dev_column(
    ui: &mut egui::Ui,
    app: &App,
    salt: &str,
    t: &Targets,
    ticked: &mut HashSet<(ProjectId, DevId)>,
    none: Option<&str>,
    empty: &str,
) {
    ui.vertical(|ui| {
        ui.set_width(SWAP_PROJECT_W);
        ui.strong("Progetti e dev");
        if let Some(none) = none {
            ui.label(egui::RichText::new(none).color(text_dim()));
            return;
        }
        if t.projects.is_empty() {
            ui.label(egui::RichText::new(empty).color(text_dim()));
            return;
        }
        let currently_all = t.all.iter().all(|x| ticked.contains(x));
        if let Some(on) = select_all_checkbox(ui, currently_all) {
            set_ticks(ticked, &t.all, on);
        }
        egui::ScrollArea::vertical()
            .id_salt(salt)
            .scroll_source(drag_scroll())
            .max_height(SWAP_LIST_H)
            .auto_shrink([false, true])
            .show(ui, |ui| {
                for sp in &t.projects {
                    let p = sp.proj;
                    let mine: Vec<(ProjectId, DevId)> = sp.devs.iter().map(|d| (p, d.dev)).collect();
                    let n_on = mine.iter().filter(|x| ticked.contains(x)).count();
                    ui.horizontal(|ui| {
                        let mut on = n_on == mine.len();
                        let partial = n_on > 0 && n_on < mine.len();
                        let cb = egui::Checkbox::new(&mut on, project_label(app, p)).indeterminate(partial);
                        if ui.add(cb).changed() {
                            set_ticks(ticked, &mine, on);
                        }
                        let status = app.projects.status(p);
                        let extra = if status == ProjectStatus::Active {
                            format!("{}h", sp.hours())
                        } else {
                            format!("{}h · {}", sp.hours(), status.label())
                        };
                        ui.label(egui::RichText::new(extra).small().color(text_dim()));
                    });
                    ui.indent((salt, p.0), |ui| {
                        for d in &sp.devs {
                            ui.horizontal(|ui| {
                                let x = (p, d.dev);
                                let mut on = ticked.contains(&x);
                                if ui.checkbox(&mut on, dev_name(app, d.dev)).changed() {
                                    set_ticks(ticked, &[x], on);
                                }
                                ui.label(egui::RichText::new(format!("{}h", d.hours)).small().color(text_dim()));
                            });
                        }
                    });
                }
            });
    });
}

/// I worker delle due finestre: solo quelli visibili («Elenchi» accesa).
fn visible_workers(app: &App) -> Vec<(WorkerId, String)> {
    app.workers.list().into_iter().filter(|(id, _)| app.workers.is_shown_in_find(*id)).collect()
}

/// Un calendario della finestra, legato a `day` (giorni).
fn date_field(ui: &mut egui::Ui, salt: &str, day: &mut i32, enabled: bool) {
    let mut date = days_to_jiff(*day);
    let r = ui.add_enabled(enabled, egui_extras::DatePickerButton::new(&mut date).id_salt(salt).format("%Y-%m-%d"));
    if r.changed()
        && let Some(d) = jiff_to_days(date)
    {
        *day = d;
    }
}

pub(crate) fn swap_worker_window(ctx: &egui::Context, app: &App, state: &mut UiState, actions: &mut Vec<Action>) {
    let Some(sw) = state.swap_worker.as_mut() else {
        return;
    };
    let listed = visible_workers(app);
    // Una scelta tolta dagli elenchi nel frattempo non vale più.
    if sw.from.is_some_and(|f| !listed.iter().any(|(id, _)| *id == f)) {
        sw.from = None;
    }
    if sw.to.is_some() && sw.to == sw.from {
        sw.to = None;
    }
    let start = sw.start();
    let projects = sw.from.map(|f| worker_projects(app, f, start, None)).unwrap_or_default();
    let t = Targets::new(projects, &mut sw.ticked);

    let mut open = true;
    let mut confirm = false;
    let mut cancel = false;
    egui::Window::new("Scambia worker")
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
        .open(&mut open)
        .show(ctx, |ui| {
            ui.label(
                egui::RichText::new(
                    "Il worker a sinistra viene sostituito da quello al centro nei progetti scelti: \
                     ogni sua cella passa all'altro con le stesse ore.",
                )
                .color(text_dim()),
            );
            ui.separator();
            ui.horizontal_top(|ui| {
                single_worker_column(
                    ui,
                    "swap_from",
                    "Worker da sostituire",
                    &listed,
                    &mut sw.from_search,
                    &mut sw.from,
                    None,
                    true,
                );
                ui.add_space(SWAP_GAP);
                let to_enabled = sw.from.is_some();
                single_worker_column(
                    ui,
                    "swap_to",
                    "Sostituito da",
                    &listed,
                    &mut sw.to_search,
                    &mut sw.to,
                    sw.from,
                    to_enabled,
                );
                ui.add_space(SWAP_GAP);
                let empty = sw.from.map_or(String::new(), |f| {
                    format!("{} non ha effort dal {} in poi.", app.workers.get_name_by_id(f), week_label(start.0 as i32))
                });
                let none = sw.from.is_none().then_some("Scegli prima il worker da sostituire.");
                project_dev_column(ui, app, "swap_projects", &t, &mut sw.ticked, none, &empty);
            });
            ui.separator();
            ui.radio_value(&mut sw.from_date, false, "Da oggi in avanti");
            ui.horizontal(|ui| {
                ui.radio_value(&mut sw.from_date, true, "Da una data in avanti:");
                date_field(ui, "swap_worker_date", &mut sw.date, sw.from_date);
            });
            // Riepilogo.
            if let (Some(from), Some(to)) = (sw.from, sw.to)
                && !t.chosen.is_empty()
            {
                ui.label(format!(
                    "{} → {}: {}, dalla settimana {}.",
                    app.workers.get_name_by_id(from),
                    app.workers.get_name_by_id(to),
                    t.summary(),
                    week_label(start.0 as i32)
                ));
                let n = swap_conflicts(app, from, to, &t.chosen, start);
                if n > 0 {
                    ui.label(
                        egui::RichText::new(format!(
                            "{n} celle hanno già {}: le ore si sommano.",
                            app.workers.get_name_by_id(to)
                        ))
                        .color(info_blue()),
                    );
                }
            }
            ui.separator();
            let valid = sw.from.is_some() && sw.to.is_some() && !t.chosen.is_empty();
            ui.horizontal(|ui| {
                if ui.add_enabled(valid, egui::Button::new("OK")).clicked() {
                    confirm = true;
                }
                if ui.button("Annulla").clicked() {
                    cancel = true;
                }
            });
            if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                cancel = true;
            }
        });

    if confirm && let (Some(from), Some(to)) = (sw.from, sw.to) {
        actions.push(Action::SwapWorker { from, to, targets: t.chosen, start });
    }
    if confirm || cancel || !open {
        state.swap_worker = None;
    }
}

/// «Cancella worker dai progetti…»: il worker a sinistra, progetti e dev in
/// cui è presente nel periodo a destra, il periodo in basso. «Cancella…»
/// apre la conferma; «Sì, cancella» manda `Action::RemoveWorkerEffort` e
/// chiude, «Annulla» torna alla finestra.
pub(crate) fn remove_worker_window(ctx: &egui::Context, app: &App, state: &mut UiState, actions: &mut Vec<Action>) {
    let Some(rw) = state.remove_worker.as_mut() else {
        return;
    };
    let listed = visible_workers(app);
    if rw.worker.is_some_and(|w| !listed.iter().any(|(id, _)| *id == w)) {
        rw.worker = None;
    }
    let (start, end) = (rw.start(), rw.end());
    let bad_range = end.is_some_and(|e| e < start);
    let projects = match rw.worker {
        Some(w) if !bad_range => worker_projects(app, w, start, end),
        _ => Vec::new(),
    };
    let t = Targets::new(projects, &mut rw.ticked);
    let name = rw.worker.map(|w| app.workers.get_name_by_id(w).to_string()).unwrap_or_default();
    let period = match end {
        None => format!("dal {} in poi", week_label(start.0 as i32)),
        Some(e) => format!("dal {} al {}", week_label(start.0 as i32), week_label(e.0 as i32)),
    };

    // Conferma: finché è aperta la finestra principale attende.
    if rw.asking {
        let body = format!("Cancellare gli effort di {name}: {}, {period}?", t.summary());
        match confirm_window(ctx, "Cancellare gli effort?", "Sì, cancella", |ui| {
            ui.label(body);
            ui.label(
                egui::RichText::new("Si tolgono ore, assegnazioni a 0 e note delle celle. Si può annullare con Cmd/Ctrl+Z.")
                    .color(text_dim()),
            );
        }) {
            Some(true) => {
                if let Some(worker) = rw.worker {
                    actions.push(Action::RemoveWorkerEffort { worker, targets: t.chosen, start, end });
                }
                state.remove_worker = None;
            }
            Some(false) => rw.asking = false,
            None => {}
        }
        return;
    }

    let mut open = true;
    let mut ask = false;
    let mut cancel = false;
    egui::Window::new("Cancella worker")
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
        .open(&mut open)
        .show(ctx, |ui| {
            ui.label(
                egui::RichText::new(
                    "Le celle del worker nei dev scelti, nel periodo indicato, vengono cancellate \
                     (ore, assegnazioni a 0 e note).",
                )
                .color(text_dim()),
            );
            ui.separator();
            ui.horizontal_top(|ui| {
                single_worker_column(ui, "remove_worker", "Worker", &listed, &mut rw.search, &mut rw.worker, None, true);
                ui.add_space(SWAP_GAP);
                let empty = format!("{name} non ha effort {period}.");
                let none = rw.worker.is_none().then_some("Scegli prima il worker.");
                project_dev_column(ui, app, "remove_projects", &t, &mut rw.ticked, none, &empty);
            });
            ui.separator();
            ui.radio_value(&mut rw.range, false, "Da oggi in poi");
            ui.horizontal(|ui| {
                ui.radio_value(&mut rw.range, true, "Dal");
                date_field(ui, "remove_worker_from", &mut rw.from, rw.range);
                ui.label("al");
                date_field(ui, "remove_worker_to", &mut rw.to, rw.range);
            });
            if bad_range {
                ui.colored_label(Color32::RED, "La data di fine è prima di quella di inizio.");
            } else if rw.worker.is_some() && !t.chosen.is_empty() {
                ui.label(format!("{name}: {}, {period}.", t.summary()));
            }
            ui.separator();
            let valid = rw.worker.is_some() && !t.chosen.is_empty() && !bad_range;
            ui.horizontal(|ui| {
                if ui.add_enabled(valid, egui::Button::new("Cancella…")).clicked() {
                    ask = true;
                }
                if ui.button("Annulla").clicked() {
                    cancel = true;
                }
            });
            if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                cancel = true;
            }
        });
    if ask {
        rw.asking = true;
    }
    if cancel || !open {
        state.remove_worker = None;
    }
}
