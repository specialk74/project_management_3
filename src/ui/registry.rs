//! Finestra «Worker, dev e categorie» (Aggiungi ▸ Worker, dev e categorie…):
//! per ogni elemento il nome (modificabile), quanto è usato, la spunta
//! «Nascosto» e il cestino. Nascondere = archiviare: i dati restano, l'elemento
//! sparisce solo dalle scelte per il lavoro nuovo. Si elimina solo ciò che non
//! è usato da nessuna parte; altrimenti il cestino è spento e il tooltip dice
//! dove è usato.

use super::*;

/// Larghezze fisse delle colonne (px): l'intestazione sta fuori dallo scroll
/// (resta ferma in alto) e le righe devono restarle allineate.
const REG_NAME_W: f32 = 340.0;
const REG_USE_W: f32 = 170.0;
const REG_HIDDEN_W: f32 = 72.0;
/// Colonna «Nei totali», solo nella scheda Dev.
const REG_TOTALS_W: f32 = 72.0;
const REG_DEL_W: f32 = 32.0;
/// Colonna «Dev» (quote per dev), solo nella scheda Worker.
const REG_SHARES_W: f32 = 360.0;
const REG_GAP: f32 = 10.0;
/// Altezza di una riga.
const REG_ROW_H: f32 = 24.0;

/// Una cella larga `w`: spazio esatto, contenuto allineato a sinistra e
/// centrato in verticale.
fn reg_cell<R>(ui: &mut egui::Ui, w: f32, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    ui.allocate_ui_with_layout(
        egui::vec2(w, REG_ROW_H),
        egui::Layout::left_to_right(egui::Align::Center),
        |ui| {
            ui.set_min_size(egui::vec2(w, REG_ROW_H));
            ui.set_max_width(w);
            add(ui)
        },
    )
    .inner
}

/// Riga di celle a larghezza fissa, separate da `REG_GAP`.
fn reg_row<R>(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = REG_GAP;
        add(ui)
    })
    .inner
}

/// Una riga della finestra: id (come `usize`, per la chiave del nome in
/// modifica), nome, uso a parole (`None` = non usato), nascosto.
struct RegistryRow {
    id: usize,
    name: String,
    usage: Option<String>,
    hidden: bool,
    /// Solo per i dev: compare nei totali in basso a sinistra del footer
    /// (`hide_in_totals` spento). `None` nelle altre schede.
    in_totals: Option<bool>,
    /// Solo per i worker: quote per dev e il loro riassunto. `None` nelle
    /// altre schede.
    shares: Option<(Vec<(DevId, u32)>, String)>,
}

/// Riassunto delle quote: «Frontend 70% · Test 30%», «—» se non dichiarate.
pub(crate) fn shares_summary(app: &App, shares: &[(DevId, u32)]) -> String {
    if shares.is_empty() {
        return "—".into();
    }
    shares.iter().map(|(d, p)| format!("{} {p}%", dev_name(app, *d))).collect::<Vec<_>>().join(" · ")
}

/// Le righe della scheda `tab`, filtrate dalla ricerca `q` (minuscolo).
fn registry_rows(app: &App, tab: RegistryTab, q: &str) -> Vec<RegistryRow> {
    let rows: Vec<RegistryRow> = match tab {
        RegistryTab::Workers => {
            let usage = app.workers_usage();
            app.workers
                .list()
                .into_iter()
                .map(|(id, name)| RegistryRow {
                    id: id.0,
                    usage: usage.get(&id).map(|&(cells, projects)| {
                        format!(
                            "{cells} {} in {projects} {}",
                            if cells == 1 { "cella" } else { "celle" },
                            if projects == 1 { "progetto" } else { "progetti" }
                        )
                    }),
                    hidden: app.workers.is_archived(id),
                    in_totals: None,
                    shares: Some({
                        let v = app.workers.dev_shares(id).to_vec();
                        let s = shares_summary(app, &v);
                        (v, s)
                    }),
                    name,
                })
                .collect()
        }
        RegistryTab::Devs => app
            .devs
            .list()
            .into_iter()
            .map(|(id, name)| RegistryRow {
                id: id.0,
                usage: projects_usage(app.dev_usage(id)),
                hidden: app.devs.is_hidden(id),
                in_totals: Some(!app.devs.is_hidden_in_totals(id)),
                shares: None,
                name,
            })
            .collect(),
        RegistryTab::Categories => app
            .categories
            .list()
            .into_iter()
            .map(|(id, name)| RegistryRow {
                id: id.0,
                usage: projects_usage(app.category_usage(id)),
                hidden: app.categories.is_hidden(id),
                in_totals: None,
                shares: None,
                name,
            })
            .collect(),
        RegistryTab::Priorities => app
            .priorities
            .list()
            .into_iter()
            .map(|(id, name)| RegistryRow {
                id: id.0,
                usage: projects_usage(app.priority_usage(id)),
                hidden: app.priorities.is_hidden(id),
                in_totals: None,
                shares: None,
                name,
            })
            .collect(),
    };
    rows.into_iter().filter(|r| search_matches(q, &r.name)).collect()
}

/// «in N progetti», `None` se 0.
fn projects_usage(n: usize) -> Option<String> {
    (n > 0).then(|| format!("in {n} {}", if n == 1 { "progetto" } else { "progetti" }))
}

/// Azioni della riga `id` della scheda `tab`.
fn rename_action(tab: RegistryTab, id: usize, name: String) -> Action {
    match tab {
        RegistryTab::Workers => Action::RenameWorker { worker: WorkerId(id), name },
        RegistryTab::Devs => Action::RenameDev { dev: DevId(id), name },
        RegistryTab::Categories => Action::RenameCategory { cat: CategoryId(id), name },
        RegistryTab::Priorities => Action::RenamePriority { pri: PriorityId(id), name },
    }
}

fn hide_action(tab: RegistryTab, id: usize, hidden: bool) -> Action {
    match tab {
        RegistryTab::Workers => Action::SetWorkerArchived {
            worker: WorkerId(id),
            archived: hidden,
        },
        RegistryTab::Devs => Action::SetDevHidden { dev: DevId(id), hidden },
        RegistryTab::Categories => Action::SetCategoryHidden {
            cat: CategoryId(id),
            hidden,
        },
        RegistryTab::Priorities => Action::SetPriorityHidden {
            pri: PriorityId(id),
            hidden,
        },
    }
}

fn delete_action(tab: RegistryTab, id: usize) -> Action {
    match tab {
        RegistryTab::Workers => Action::DeleteWorker(WorkerId(id)),
        RegistryTab::Devs => Action::DeleteDev(DevId(id)),
        RegistryTab::Categories => Action::DeleteCategory(CategoryId(id)),
        RegistryTab::Priorities => Action::DeletePriority(PriorityId(id)),
    }
}

/// Altezza massima dell'elenco dei dev in «Dev di …», poi si scorre.
const WORKER_DEVS_LIST_H: f32 = 420.0;

/// Spiegazione della colonna «Dev» (scheda Worker).
const SHARES_HELP: &str = "Dev che il worker sa fare, con la quota del suo tempo: servono \
     alla capacità per dev di Vista ▸ Domanda e capacità… (es. Frontend 70%, Test 30%). \
     Clic per modificarle.";

/// Spiegazione di «Nei totali» (scheda Dev).
const TOTALS_HELP: &str = "Nei totali: il dev compare nel riquadro in basso a sinistra del \
     footer (riga e somma «Totale»). Spenta, il dev sparisce solo da lì e resta \
     ovunque altrove: è la stessa spunta di Filtri ▸ Dev nei totali…, \
     indipendente da «Nascosto».";

/// Spiegazione di «Nascosto» per la scheda.
fn hidden_help(tab: RegistryTab) -> &'static str {
    match tab {
        RegistryTab::Workers => {
            "Nascosto: il worker non compare più nel footer né negli elenchi \
             (autocompletamento delle celle, filtri, matrice, torte, saturazione, \
             inserimento multiplo). Le sue celle restano nella griglia e nei totali. \
             Sono le spunte Footer ed Elenchi di Filtri ▸ Worker, spente insieme."
        }
        RegistryTab::Devs => {
            "Nascosto: il dev resta nei progetti in cui c'è (griglia, totali, \
             export), ma non è più proposto per aggiungerlo ai progetti, nel \
             filtro Dev e nella combo Dev delle torte."
        }
        RegistryTab::Categories => {
            "Nascosta: resta sui progetti che la hanno, ma non è più proposta \
             nella scelta della categoria del progetto, nei filtri, nelle combo \
             di matrice e torte e nel selettore del footer."
        }
        RegistryTab::Priorities => {
            "Nascosta: resta sui progetti che la hanno, ma non è più proposta \
             nella scelta della priorità del progetto e nella combo delle torte."
        }
    }
}

pub(crate) fn registry_window(
    ctx: &egui::Context,
    app: &App,
    state: &mut UiState,
    actions: &mut Vec<Action>,
) {
    if !state.show_registry {
        return;
    }
    let mut open = true;
    let tab = state.registry_tab;
    let q = state.registry_search.trim().to_lowercase();
    let rows = registry_rows(app, tab, &q);
    let what = match tab {
        RegistryTab::Workers => "worker",
        RegistryTab::Devs => "dev",
        RegistryTab::Categories => "categorie",
        RegistryTab::Priorities => "priorità",
    };
    egui::Window::new("Worker, dev e categorie")
        .collapsible(false)
        .resizable(true)
        .default_pos(egui::pos2(160.0, 40.0))
        .open(&mut open)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                for (t, label) in [
                    (RegistryTab::Workers, "Worker"),
                    (RegistryTab::Devs, "Dev"),
                    (RegistryTab::Categories, "Categorie"),
                    (RegistryTab::Priorities, "Priorità"),
                ] {
                    ui.selectable_value(&mut state.registry_tab, t, label);
                }
            });
            ui.add(
                egui::TextEdit::singleline(&mut state.registry_search)
                    .hint_text(format!("Cerca {what}…"))
                    .desired_width(240.0),
            );
            ui.label(
                egui::RichText::new(
                    "Clic sul nome per rinominarlo (Invio conferma, Esc annulla). \
                     Il cestino elimina solo ciò che non è usato da nessuna parte.",
                )
                .small()
                .color(text_dim()),
            );
            ui.separator();
            if rows.is_empty() {
                ui.label(format!("(nessun {what})"));
                return;
            }
            // Intestazione fuori dallo scroll: resta visibile scorrendo l'elenco.
            reg_row(ui, |ui| {
                reg_cell(ui, REG_NAME_W, |ui| ui.label(egui::RichText::new("Nome").strong()));
                reg_cell(ui, REG_USE_W, |ui| ui.label(egui::RichText::new("Uso").strong()));
                reg_cell(ui, REG_HIDDEN_W, |ui| {
                    ui.label(egui::RichText::new("Nascosto").strong())
                        .on_hover_text(hidden_help(tab))
                });
                if tab == RegistryTab::Devs {
                    reg_cell(ui, REG_TOTALS_W, |ui| {
                        ui.label(egui::RichText::new("Nei totali").strong())
                            .on_hover_text(TOTALS_HELP)
                    });
                }
                if tab == RegistryTab::Workers {
                    reg_cell(ui, REG_SHARES_W, |ui| {
                        ui.label(egui::RichText::new("Dev (capacità)").strong()).on_hover_text(SHARES_HELP)
                    });
                }
                reg_cell(ui, REG_DEL_W, |_| ());
            });
            ui.separator();
            egui::ScrollArea::vertical()
                .id_salt(("registry_rows", tab))
                .scroll_source(drag_scroll())
                .max_height(420.0)
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = 2.0;
                    for (i, r) in rows.iter().enumerate() {
                        // Righe alterne su due toni, come le altre tabelle.
                        let bg = ui.painter().add(egui::Shape::Noop);
                        let row = reg_row(ui, |ui| registry_row(ui, state, actions, tab, r));
                        if i % 2 == 1 {
                            let rect = row.expand2(egui::vec2(2.0, 1.0));
                            ui.painter().set(bg, egui::Shape::rect_filled(rect, 2.0, row_alt()));
                        }
                    }
                });
        });
    if !open {
        state.show_registry = false;
        state.registry_names.clear();
    }
}

/// Una riga: nome modificabile, uso, «Nascosto», cestino. Ritorna il
/// rettangolo occupato (per lo sfondo alternato).
fn registry_row(
    ui: &mut egui::Ui,
    state: &mut UiState,
    actions: &mut Vec<Action>,
    tab: RegistryTab,
    r: &RegistryRow,
) -> Rect {
    let top_left = ui.cursor().min;
    // Nome: il testo in modifica vive in `registry_names` finché il campo ha
    // il focus; uscendo (Invio o clic altrove) si conferma, Esc annulla.
    let key = (tab, r.id);
    let mut buf = state
        .registry_names
        .get(&key)
        .cloned()
        .unwrap_or_else(|| r.name.clone());
    let te = reg_cell(ui, REG_NAME_W, |ui| {
        ui.add(
            egui::TextEdit::singleline(&mut buf)
                .desired_width(REG_NAME_W - 8.0)
                .text_color_opt(r.hidden.then(text_dim)),
        )
    });
    if te.lost_focus() {
        let cancel = ui.input(|i| i.key_pressed(egui::Key::Escape));
        if !cancel && buf.trim() != r.name {
            actions.push(rename_action(tab, r.id, buf.trim().to_string()));
        }
        state.registry_names.remove(&key);
    } else if te.has_focus() {
        state.registry_names.insert(key, buf);
    }

    reg_cell(ui, REG_USE_W, |ui| match &r.usage {
        Some(u) => ui.label(u),
        None => ui.label(egui::RichText::new("non usato").color(text_dim())),
    });

    let mut hidden = r.hidden;
    let changed = reg_cell(ui, REG_HIDDEN_W, |ui| {
        ui.checkbox(&mut hidden, "").on_hover_text(hidden_help(tab)).changed()
    });
    if changed {
        actions.push(hide_action(tab, r.id, hidden));
    }

    // Solo dev: «Nei totali», indipendente da «Nascosto» (stessa spunta di
    // Filtri ▸ Dev nei totali…).
    if let Some(in_totals) = r.in_totals {
        let mut on = in_totals;
        let changed = reg_cell(ui, REG_TOTALS_W, |ui| {
            ui.checkbox(&mut on, "").on_hover_text(TOTALS_HELP).changed()
        });
        if changed {
            actions.push(Action::SetDevHiddenInTotals {
                dev: DevId(r.id),
                hidden: !on,
            });
        }
    }

    // Solo worker: le quote per dev, modificate nella finestra a parte.
    if let Some((shares, summary)) = &r.shares {
        let clicked = reg_cell(ui, REG_SHARES_W, |ui| {
            let text = egui::RichText::new(summary).color(if shares.is_empty() { text_dim() } else { text() });
            // Il pulsante può troncare: nel tooltip l'elenco completo, un dev per riga.
            let hint = if shares.is_empty() {
                SHARES_HELP.to_string()
            } else {
                format!("{}\n\n{SHARES_HELP}", summary.replace(" · ", "\n"))
            };
            ui.add(egui::Button::new(text).truncate()).on_hover_text(hint).clicked()
        });
        if clicked {
            state.worker_devs_edit = Some(WorkerDevsEdit { worker: WorkerId(r.id), shares: shares.clone(), note: None });
        }
    }

    let del = reg_cell(ui, REG_DEL_W, |ui| {
        ui.add_enabled(r.usage.is_none(), egui::Button::new("🗑"))
    });
    let del = if r.usage.is_none() {
        del.on_hover_text("Elimina")
    } else {
        del.on_disabled_hover_text(format!(
            "Non si può eliminare: è usato ({}). Puoi nasconderlo.",
            r.usage.as_deref().unwrap_or_default()
        ))
    };
    if del.clicked() {
        actions.push(delete_action(tab, r.id));
    }
    Rect::from_min_max(top_left, egui::pos2(ui.min_rect().right(), top_left.y + REG_ROW_H))
}

/// Finestra «Dev di …» (dalla colonna «Dev» del registro dei worker): una
/// quota in % per **ogni** dev (i nascosti in grigio, «nascosto»), in un
/// elenco che scorre, la somma
/// (al massimo 100%), «Proponi dagli ultimi 6 mesi» (`propose_shares`) e
/// «Azzera». OK manda `Action::SetWorkerDevShares`.
pub(crate) fn worker_devs_window(ctx: &egui::Context, app: &App, state: &mut UiState, actions: &mut Vec<Action>) {
    let Some(ed) = state.worker_devs_edit.as_mut() else {
        return;
    };
    let name = app.workers.get_name_by_id(ed.worker).to_string();
    let mut open = true;
    let mut ok = false;
    let mut cancel = false;
    egui::Window::new(format!("Dev di {name}"))
        .id(egui::Id::unique("worker_devs_window"))
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
        .open(&mut open)
        .show(ctx, |ui| {
            ui.label(
                egui::RichText::new(
                    "Quota del tempo di questo worker per ogni dev che sa fare: la sua capacità per \
                     quel dev è la quota delle sue ore massime di ogni settimana.",
                )
                .color(text_dim()),
            );
            ui.separator();
            egui::ScrollArea::vertical()
                .id_salt("worker_devs_scroll")
                .scroll_source(drag_scroll())
                .max_height(WORKER_DEVS_LIST_H)
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    egui::Grid::new("worker_devs_grid").num_columns(3).spacing([16.0, 4.0]).show(ui, |ui| {
                        for (dev, dname) in app.devs.list() {
                            let cur = ed.shares.iter().find(|(d, _)| *d == dev).map_or(0, |(_, p)| *p);
                            let fill = dev_color(app, dev);
                            ui.label(egui::RichText::new(format!(" {dname} ")).background_color(fill).color(contrast_text(fill)));
                            let mut v = cur;
                            if ui.add(egui::DragValue::new(&mut v).range(0..=100).suffix("%")).changed() {
                                ed.shares.retain(|(d, _)| *d != dev);
                                if v > 0 {
                                    ed.shares.push((dev, v));
                                }
                                ed.shares.sort_by_key(|(d, _)| *d);
                            }
                            if app.devs.is_hidden(dev) {
                                ui.label(egui::RichText::new("nascosto").small().color(text_dim()));
                            } else {
                                ui.label("");
                            }
                            ui.end_row();
                        }
                    });
                });
            let sum: u32 = ed.shares.iter().map(|(_, p)| p).sum();
            if sum > 100 {
                ui.colored_label(g(error_red()), format!("Totale {sum}%: al massimo 100%."));
            } else {
                ui.label(format!("Totale {sum}%"));
                if (1..100).contains(&sum) {
                    ui.label(egui::RichText::new("Il resto del tempo non conta per nessun dev.").small().color(text_dim()));
                }
            }
            ui.horizontal(|ui| {
                if ui
                    .button("Proponi dagli ultimi 6 mesi")
                    .on_hover_text("Le quote in proporzione alle sue ore per dev nelle 26 settimane prima di questa.")
                    .clicked()
                {
                    let proposal = crate::demand::propose_shares(app, ed.worker, current_week_id());
                    ed.note = proposal.is_empty().then(|| "Nessuna ora negli ultimi 6 mesi.".to_string());
                    if !proposal.is_empty() {
                        ed.shares = proposal;
                    }
                }
                if ui.button("Azzera").clicked() {
                    ed.shares.clear();
                    ed.note = None;
                }
            });
            if let Some(n) = &ed.note {
                ui.colored_label(warn_amber(), n);
            }
            ui.separator();
            ui.horizontal(|ui| {
                if ui.add_enabled(sum <= 100, egui::Button::new("OK")).clicked() {
                    ok = true;
                }
                if ui.button("Annulla").clicked() {
                    cancel = true;
                }
            });
            if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                cancel = true;
            }
        });
    if ok {
        actions.push(Action::SetWorkerDevShares { worker: ed.worker, shares: ed.shares.clone() });
    }
    if ok || cancel || !open {
        state.worker_devs_edit = None;
    }
}

/// Dopo l'eliminazione di un worker: toglie i riferimenti rimasti nello stato
/// della sessione (filtri, selezioni di matrice/torte/saturazione, worker
/// evidenziato nel footer).
pub(crate) fn forget_worker(state: &mut UiState, worker: WorkerId, name: &str) {
    if let Some(set) = &mut state.worker_filter {
        set.remove(name);
    }
    state.matrix_workers.remove(&worker);
    state.pies_workers.remove(&worker);
    state.worker_effort_workers.remove(&worker);
    state.sat_selected.remove(&worker);
    if state.footer_selected_worker == Some(worker) {
        state.footer_selected_worker = None;
    }
    if state.matrix_sort == Some(worker) {
        state.matrix_sort = None;
    }
}

/// Idem per un dev eliminato (filtro Dev, combo Dev delle torte).
pub(crate) fn forget_dev(state: &mut UiState, dev: DevId) {
    if let Some(set) = &mut state.dev_filter {
        set.remove(&dev);
    }
    state.pies_devs.remove(&dev);
}

/// Idem per una categoria eliminata (filtro Categorie, combo di matrice e
/// torte, selettore del footer).
pub(crate) fn forget_category(state: &mut UiState, cat: CategoryId) {
    if let Some(set) = &mut state.category_filter {
        set.remove(&Some(cat));
    }
    state.matrix_categories.remove(&Some(cat));
    state.pies_categories.remove(&Some(cat));
    if state.selected_category == Some(cat) {
        state.selected_category = None;
    }
}

/// Idem per una priorità eliminata (combo di matrice e torte).
pub(crate) fn forget_priority(state: &mut UiState, pri: PriorityId) {
    state.matrix_priorities.remove(&Some(pri));
    state.pies_priorities.remove(&Some(pri));
}
