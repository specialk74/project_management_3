//! Dialog unica dei filtri: Workers + Progetti + Dev in tre colonne affiancate.
//!
//! È l'unica finestra aperta da `Cmd/Ctrl+F` (workers), `Cmd/Ctrl+G` (workers
//! sulla settimana corrente), `Cmd/Ctrl+P` (progetti) e `Cmd/Ctrl+D` (dev):
//! la scorciatoia porta il focus sul campo di ricerca della propria colonna e,
//! a finestra già aperta, fa il toggle del "Select All" di quella colonna.

use super::*;

/// Per ogni nome worker: (numero di progetti APERTI, numero di progetti CHIUSI)
/// distinti in cui il worker ha effort > 0. Con `current_week_only` (modalità
/// Ctrl+G) conta solo la presenza nella **settimana corrente**; altrimenti (Ctrl+F)
/// su **tutte** le settimane. Ignora filtri e vista. Chiave per nome (come il
/// filtro worker).
fn worker_project_counts(app: &App, current_week_only: bool) -> HashMap<String, (u32, u32)> {
    let cur = current_week_id();
    let mut counts: HashMap<String, (u32, u32)> = HashMap::new();
    for (proj, _) in app.projects.list() {
        let closed = app.projects.is_closed(proj);
        // worker distinti presenti (effort > 0) in questo progetto
        let mut seen: HashSet<WorkerId> = HashSet::new();
        for dev in app.projects.list_devs(proj) {
            if let Some(sd) = app.projects.get_single_dev(proj, dev) {
                let weeks = if current_week_only {
                    vec![cur]
                } else {
                    sd.get_weeks()
                };
                for wk in weeks {
                    if let Some(sew) = sd.get_all(wk) {
                        for (wid, se) in sew.worker_id.iter() {
                            if *wid != WORKER_ID_ZERO && se.get_effort().0 > 0 {
                                seen.insert(*wid);
                            }
                        }
                    }
                }
            }
        }
        for wid in seen {
            let name = app.workers.get_name_by_id(wid).to_string();
            let e = counts.entry(name).or_default();
            if closed {
                e.1 += 1;
            } else {
                e.0 += 1;
            }
        }
    }
    counts
}

/// Campo di ricerca di una colonna: restituisce la risposta per l'auto-focus.
fn search_field(ui: &mut egui::Ui, hint: &str, text: &mut String) -> egui::Response {
    ui.add(
        egui::TextEdit::singleline(text)
            .hint_text(hint)
            .desired_width(f32::INFINITY),
    )
}

/// Vero se `q` (già in minuscolo, già trimmato) è vuoto o contenuto in `hay`.
fn matches(q: &str, hay: &str) -> bool {
    q.is_empty() || hay.to_lowercase().contains(q)
}

pub(crate) fn filters_window(
    ctx: &egui::Context,
    app: &App,
    state: &mut UiState,
    actions: &mut Vec<Action>,
) {
    if !state.show_filters {
        return;
    }

    // ── Dati delle tre colonne (già filtrati dalla rispettiva ricerca) ───────
    let qw = state.worker_search.trim().to_lowercase();
    let workers: Vec<String> = app
        .workers
        .list()
        .into_iter()
        .filter(|(id, _)| app.workers.is_shown_in_find(*id))
        .map(|(_, n)| n)
        .filter(|n| matches(&qw, n))
        .collect();
    let counts = worker_project_counts(app, state.worker_filter_current_week);

    // Progetti: in elenco si mostra SOLO la tripletta (fallback al nome), ma la
    // ricerca combacia sia con tripletta sia con nome.
    let qp = state.project_search.trim().to_lowercase();
    let mut projects: Vec<(ProjectId, Enable, String)> = app
        .projects
        .list_full()
        .into_iter()
        .filter(|(id, _, _)| !app.projects.is_closed(*id))
        .filter_map(|(id, name, en)| {
            let trip = app.projects.get_tripletta(id);
            if !(qp.is_empty()
                || trip.to_lowercase().contains(&qp)
                || name.to_lowercase().contains(&qp))
            {
                return None;
            }
            let label = if trip.is_empty() { name } else { trip };
            Some((id, en, label))
        })
        .collect();
    projects.sort_by(|a, b| a.2.to_lowercase().cmp(&b.2.to_lowercase()));

    let qd = state.dev_search.trim().to_lowercase();
    let devs: Vec<(DevId, String)> = app
        .devs
        .list()
        .into_iter()
        .filter(|(_, n)| matches(&qd, n))
        .collect();

    // ── Toggle "Select All" richiesti dalle scorciatoie ──────────────────────
    // Agiscono sugli elementi ATTUALMENTE elencati (quindi rispettano la ricerca).
    let mut wfilter = state.worker_filter.clone();
    let mut dfilter = state.dev_filter.clone();

    if std::mem::take(&mut state.worker_filter_toggle_all) {
        let currently_all = match &wfilter {
            None => !workers.is_empty(),
            Some(s) => !workers.is_empty() && workers.iter().all(|n| s.contains(n)),
        };
        set_worker_selection(&mut wfilter, app, &workers, !currently_all);
    }
    if std::mem::take(&mut state.dev_filter_toggle_all) {
        let currently_all = match &dfilter {
            None => !devs.is_empty(),
            Some(s) => !devs.is_empty() && devs.iter().all(|(id, _)| s.contains(id)),
        };
        set_dev_selection(&mut dfilter, app, &devs, !currently_all);
    }
    if std::mem::take(&mut state.project_filter_toggle_all) {
        let currently_all = !projects.is_empty() && projects.iter().all(|(_, en, _)| en.0);
        for (id, _, _) in &projects {
            actions.push(Action::SetProjectEnabled {
                proj: *id,
                enabled: !currently_all,
            });
        }
    }

    // Colonna su cui portare il focus: solo se una scorciatoia/voce di menù
    // l'ha appena richiesto (`filters_focus` resta come "colonna corrente").
    let focus = std::mem::take(&mut state.filters_focus_dirty)
        .then_some(state.filters_focus)
        .flatten();
    let just_opened = state.filters_just_opened;
    state.filters_just_opened = false;

    let mut open = true;
    let mut jump: Option<ProjectId> = None;
    let mut esc = false;

    let resp = egui::Window::new("Filtri")
        .id(egui::Id::new("filters_window"))
        .collapsible(false)
        .resizable(false)
        .default_pos(egui::pos2(90.0, 40.0))
        .open(&mut open)
        .show(ctx, |ui| {
            if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                esc = true;
            }
            ui.horizontal_top(|ui| {
                // ── Workers ──────────────────────────────────────────────────
                ui.vertical(|ui| {
                    ui.set_min_width(200.0);
                    ui.set_max_width(200.0);
                    ui.strong("Workers");
                    let te = search_field(ui, "Cerca worker…", &mut state.worker_search);
                    if focus == Some(FilterPane::Workers) {
                        te.request_focus();
                    }
                    // Modalità Ctrl+G: solo i progetti in cui il worker lavora
                    // nella settimana corrente.
                    ui.checkbox(
                        &mut state.worker_filter_current_week,
                        "Solo settimana corrente",
                    )
                    .on_hover_text(
                        "Mostra solo i progetti in cui un worker selezionato ha \
                         effort nella settimana corrente (⌘/Ctrl+G)",
                    );
                    ui.separator();
                    egui::ScrollArea::vertical()
                        .id_salt("filters_workers")
                        .max_height(400.0)
                        .auto_shrink([false, true])
                        .show(ui, |ui| {
                            let currently_all = match &wfilter {
                                None => !workers.is_empty(),
                                Some(s) => {
                                    !workers.is_empty() && workers.iter().all(|n| s.contains(n))
                                }
                            };
                            if let Some(v) = select_all_checkbox(ui, currently_all) {
                                set_worker_selection(&mut wfilter, app, &workers, v);
                            }
                            for name in &workers {
                                let mut sel = match &wfilter {
                                    None => true,
                                    Some(s) => s.contains(name),
                                };
                                // Etichetta = nome + conteggio progetti aperti/chiusi.
                                let (op, cl) = counts.get(name).copied().unwrap_or((0, 0));
                                let label = format!("{name}    {op}/Aperti - {cl}/Chiuso");
                                if ui.checkbox(&mut sel, label).changed() {
                                    let set = wfilter
                                        .get_or_insert_with(|| all_worker_names(app).collect());
                                    if sel {
                                        set.insert(name.clone());
                                    } else {
                                        set.remove(name);
                                    }
                                }
                            }
                        });
                });
                ui.separator();

                // ── Progetti ─────────────────────────────────────────────────
                ui.vertical(|ui| {
                    ui.set_min_width(220.0);
                    ui.set_max_width(220.0);
                    ui.strong("Progetti");
                    let te = search_field(ui, "Cerca per tripletta…", &mut state.project_search);
                    if focus == Some(FilterPane::Projects) {
                        te.request_focus();
                    }
                    let enter = te.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                    ui.separator();
                    egui::ScrollArea::vertical()
                        .id_salt("filters_projects")
                        .max_height(400.0)
                        .auto_shrink([false, true])
                        .show(ui, |ui| {
                            let currently_all =
                                !projects.is_empty() && projects.iter().all(|(_, en, _)| en.0);
                            if let Some(v) = select_all_checkbox(ui, currently_all) {
                                for (id, _, _) in &projects {
                                    actions.push(Action::SetProjectEnabled {
                                        proj: *id,
                                        enabled: v,
                                    });
                                }
                            }
                            for (id, en, label) in &projects {
                                ui.horizontal(|ui| {
                                    let mut on = en.0;
                                    if ui.checkbox(&mut on, "").changed() {
                                        actions.push(Action::SetProjectEnabled {
                                            proj: *id,
                                            enabled: on,
                                        });
                                    }
                                    // Tripletta cliccabile → salta al progetto.
                                    if ui
                                        .selectable_label(false, label)
                                        .on_hover_text("Vai al progetto")
                                        .clicked()
                                    {
                                        jump = Some(*id);
                                    }
                                });
                            }
                            // Invio → salta al primo risultato.
                            if enter {
                                if let Some((id, _, _)) = projects.first() {
                                    jump = Some(*id);
                                }
                            }
                        });
                });
                ui.separator();

                // ── Dev ──────────────────────────────────────────────────────
                ui.vertical(|ui| {
                    ui.set_min_width(160.0);
                    ui.set_max_width(160.0);
                    ui.strong("Dev");
                    let te = search_field(ui, "Cerca dev…", &mut state.dev_search);
                    if focus == Some(FilterPane::Devs) {
                        te.request_focus();
                    }
                    ui.separator();
                    egui::ScrollArea::vertical()
                        .id_salt("filters_devs")
                        .max_height(400.0)
                        .auto_shrink([false, true])
                        .show(ui, |ui| {
                            let currently_all = match &dfilter {
                                None => !devs.is_empty(),
                                Some(s) => {
                                    !devs.is_empty() && devs.iter().all(|(id, _)| s.contains(id))
                                }
                            };
                            if let Some(v) = select_all_checkbox(ui, currently_all) {
                                set_dev_selection(&mut dfilter, app, &devs, v);
                            }
                            for (id, name) in &devs {
                                let mut sel = match &dfilter {
                                    None => true,
                                    Some(s) => s.contains(id),
                                };
                                if ui.checkbox(&mut sel, name).changed() {
                                    let set = dfilter.get_or_insert_with(|| {
                                        app.devs.list().into_iter().map(|(i, _)| i).collect()
                                    });
                                    if sel {
                                        set.insert(*id);
                                    } else {
                                        set.remove(id);
                                    }
                                }
                            }
                        });
                });
            });
        });

    // Se tutto è selezionato il filtro è considerato spento (`None`): così la
    // griglia non paga il costo del filtro e i menù non mostrano la spunta.
    normalize_worker_filter(&mut wfilter, app);
    normalize_dev_filter(&mut dfilter, app);
    state.worker_filter = wfilter;
    state.dev_filter = dfilter;

    if let Some(id) = jump {
        state.jump_to_project = Some(id);
        state.show_filters = false;
        state.project_search.clear();
        return;
    }

    // click fuori dalla finestra → chiudi (ma non nello stesso frame dell'apertura)
    let clicked_outside = resp
        .map(|r| r.response.clicked_elsewhere())
        .unwrap_or(false);
    if !open || esc || (!just_opened && clicked_outside) {
        state.show_filters = false;
    }
}

/// Nomi di tutti i worker selezionabili nel filtro (`show_in_find`), a
/// prescindere dalla ricerca: è l'insieme "tutti selezionati".
fn all_worker_names(app: &App) -> impl Iterator<Item = String> + '_ {
    app.workers
        .list()
        .into_iter()
        .filter(|(id, _)| app.workers.is_shown_in_find(*id))
        .map(|(_, n)| n)
}

/// Seleziona/deseleziona i soli worker `shown` (quelli elencati), lasciando
/// invariati gli altri.
fn set_worker_selection(filter: &mut Filter, app: &App, shown: &[String], on: bool) {
    let set = filter.get_or_insert_with(|| all_worker_names(app).collect());
    for n in shown {
        if on {
            set.insert(n.clone());
        } else {
            set.remove(n);
        }
    }
}

/// Come sopra per i dev elencati.
fn set_dev_selection(filter: &mut DevFilter, app: &App, shown: &[(DevId, String)], on: bool) {
    let set = filter.get_or_insert_with(|| app.devs.list().into_iter().map(|(i, _)| i).collect());
    for (id, _) in shown {
        if on {
            set.insert(*id);
        } else {
            set.remove(id);
        }
    }
}

/// `Some(set)` con tutti i worker selezionati equivale a "nessun filtro".
fn normalize_worker_filter(filter: &mut Filter, app: &App) {
    if let Some(set) = filter {
        let all: Vec<String> = all_worker_names(app).collect();
        if all.iter().all(|n| set.contains(n)) {
            *filter = None;
        }
    }
}

/// Idem per il filtro dev.
fn normalize_dev_filter(filter: &mut DevFilter, app: &App) {
    if let Some(set) = filter {
        if app.devs.list().iter().all(|(id, _)| set.contains(id)) {
            *filter = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn worker_project_counts_splits_open_and_closed() {
        let mut app = App::new();
        let alice = app.workers.add("Alice");
        let bob = app.workers.add("Bob");
        let dev = app.devs.add("Frontend");
        let cur = current_week_id();
        let other = WeekId(20000); // settimana lontana (non corrente)

        // P1 aperto: Alice nella settimana corrente, Bob in un'altra settimana.
        let p1 = app.projects.add("P1", Some("AAA"), Some(cur));
        app.projects.add_dev(p1, dev);
        app.projects.add_effort(p1, dev, cur, alice, Effort(8));
        app.projects.add_effort(p1, dev, other, bob, Effort(8));

        // P2 chiuso: Alice nella settimana corrente.
        let p2 = app.projects.add("P2", Some("BBB"), Some(cur));
        app.projects.add_dev(p2, dev);
        app.projects.add_effort(p2, dev, cur, alice, Effort(8));
        app.projects.set_closed(p2, true);

        // P3 aperto: Alice con effort 0 nella settimana corrente → non conta.
        let p3 = app.projects.add("P3", Some("CCC"), Some(cur));
        app.projects.add_dev(p3, dev);
        app.projects.add_effort(p3, dev, cur, alice, Effort(0));

        // P4 aperto: Bob nella settimana corrente.
        let p4 = app.projects.add("P4", Some("DDD"), Some(cur));
        app.projects.add_dev(p4, dev);
        app.projects.add_effort(p4, dev, cur, bob, Effort(8));

        // Ctrl+F: tutte le settimane. Bob è in P1 (altra settimana) + P4 = 2 aperti.
        let all = worker_project_counts(&app, false);
        assert_eq!(all.get("Alice").copied(), Some((1, 1)));
        assert_eq!(all.get("Bob").copied(), Some((2, 0)));

        // Ctrl+G: solo settimana corrente. Bob non è in P1 questa settimana → solo P4.
        let cw = worker_project_counts(&app, true);
        assert_eq!(cw.get("Alice").copied(), Some((1, 1)));
        assert_eq!(cw.get("Bob").copied(), Some((1, 0)));
    }
}
