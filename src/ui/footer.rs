//! (estratto da `ui.rs` — vedi miglioramento #16)

use super::*;

/// Worker visibili nel footer: esclude `hide_in_footer` e (se attivo) quelli fuori filtro.
pub(crate) fn footer_workers(
    app: &App,
    filter: &Filter,
) -> Vec<(crate::workers_utils::worker::WorkerId, String)> {
    app.workers
        .list()
        .into_iter()
        .filter(|(id, _)| !app.workers.is_hidden_in_footer(*id))
        .filter(|(_, name)| worker_shown(filter, name))
        .collect()
}

/// Maniglia sottile e cliccabile, larga quanto il footer, con un triangolino
/// giallo centrato. Quando il footer è visibile il triangolo punta in basso
/// (click = nascondi); quando è nascosto punta in alto (click = mostra).
pub(crate) fn footer_handle(ui: &mut egui::Ui, state: &mut UiState) {
    const HANDLE_H: f32 = 14.0;
    let hidden = state.footer_hidden;
    let w = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, HANDLE_H), Sense::click());
    if resp.clicked() {
        state.footer_hidden = !hidden;
    }
    // Sfondo, con leggero risalto al passaggio del mouse.
    let bg = if resp.hovered() {
        g(Color32::from_rgb(0x2e, 0x2e, 0x2e))
    } else {
        bg()
    };
    ui.painter().rect_filled(rect, 0.0, bg);
    // Triangolino centrato orizzontalmente.
    let cx = rect.center().x;
    let cy = rect.center().y;
    const HW: f32 = 7.0; // metà base
    const HH: f32 = 4.0; // metà altezza
    let pts = if hidden {
        // punta verso l'alto → riapre il footer
        vec![
            egui::pos2(cx, cy - HH),
            egui::pos2(cx - HW, cy + HH),
            egui::pos2(cx + HW, cy + HH),
        ]
    } else {
        // punta verso il basso → nasconde il footer
        vec![
            egui::pos2(cx - HW, cy - HH),
            egui::pos2(cx + HW, cy - HH),
            egui::pos2(cx, cy + HH),
        ]
    };
    ui.painter().add(egui::Shape::convex_polygon(
        pts,
        g(START_STOP),
        Stroke::NONE,
    ));
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp.on_hover_text(if hidden {
        "Mostra il footer"
    } else {
        "Nascondi il footer"
    });
}

pub(crate) fn footer(ui: &mut egui::Ui, app: &App, state: &mut UiState, actions: &mut Vec<Action>) {
    let filter = state.worker_filter.clone();
    let workers = footer_workers(app, &filter);
    let level = if state.compact_mode {
        0
    } else {
        state.zoom_level
    };
    let cols = columns_vec(app, level);
    ui.spacing_mut().item_spacing = Vec2::ZERO;

    // striscia gialla in testa al footer
    let full_w = ui.available_width();
    alloc_strip(ui, full_w, DEV_BORDER, g(START_STOP));

    // 1 riga header + una riga per worker o per dev (il più alto dei due)
    let n_devs = app.devs.list().len();
    // +2 righe dev: una riga vuota di separazione + la riga "Totale" sotto l'elenco dev.
    let footer_h = (workers.len().max(n_devs + 2) as f32 + 1.0) * ROW_H;

    // Footer sinistro: stessa riserva da 300px della griglia (SidePanel).
    egui::SidePanel::left("ftr_left")
        .exact_width(LEFT_W)
        .resizable(false)
        .frame(egui::Frame::NONE.fill(bg()))
        .show_inside(ui, |ui| {
            ui.spacing_mut().item_spacing = Vec2::ZERO;
            let (lrect, _) = ui.allocate_exact_size(Vec2::new(LEFT_W, footer_h), Sense::hover());
            draw_left_footer(ui, lrect, app, state, &workers);
        });

    // Footer destro — scroll orizzontale sincronizzato con header/griglia.
    egui::ScrollArea::horizontal()
        .id_salt("footer_scroll")
        .scroll_offset(Vec2::new(state.scroll_x, 0.0))
        .enable_scrolling(false)
        .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing = Vec2::ZERO;
            let content_w = cols_width(&cols, COL_W);
            let (rrect, _) = ui.allocate_exact_size(Vec2::new(content_w, footer_h), Sense::hover());
            draw_right_footer(ui, rrect, app, state, &workers, &cols, actions);
        });
}

/// Posiziona un `ComboBox` compatto a coordinate assolute dentro il footer
/// (il footer è disegnato a mano col painter, quindi serve un'ui figlia).
pub(crate) fn footer_combo(
    ui: &mut egui::Ui,
    rect: Rect,
    id_salt: &str,
    selected_text: String,
    width: f32,
    add_contents: impl FnOnce(&mut egui::Ui),
) {
    ui.allocate_new_ui(egui::UiBuilder::new().max_rect(rect), |ui| {
        ui.spacing_mut().button_padding = Vec2::new(3.0, 0.0);
        ui.spacing_mut().interact_size.y = ROW_H;
        egui::ComboBox::from_id_salt(id_salt)
            .selected_text(selected_text)
            .width(width)
            .show_ui(ui, add_contents);
    });
}

/// Ore rimanenti per il worker fino alla fine dell'anno `year`.
///
/// Il periodo segue il selettore Anno del footer: se `year` è l'anno solare
/// corrente (quello di `this_week`) si parte dalla settimana corrente, altrimenti
/// dalla prima settimana di `year`. Per ogni settimana del periodo si somma il
/// monte ore effettivo del worker (`get_effective_max_hours`), così gli override
/// con orari diversi dal default riducono (o aumentano) il totale rispetto a
/// `default × n_settimane`.
///
/// Con il filtro "Nulli" attivo (`only_null`) si contano solo le settimane in cui
/// il worker non ha ancora effort assegnato (sovra = 0): il valore diventa così la
/// somma delle ore che potrebbero ancora essergli destinate.
pub(crate) fn worker_remaining_hours(
    app: &App,
    wid: WorkerId,
    year: i32,
    this_week: i32,
    only_null: bool,
) -> i32 {
    let start = if year == days_to_local(this_week).year() {
        this_week
    } else {
        i32::MIN // nessun limite inferiore: tutte le settimane dell'anno
    };
    weeks_vec(app)
        .into_iter()
        .filter(|w| *w >= start && days_to_local(*w).year() == year)
        .filter(|w| {
            !only_null
                || app
                    .sovra
                    .get(&(WeekId(*w as usize), wid))
                    .map_or(0, |e| e.0)
                    == 0
        })
        .map(|w| app.workers.get_effective_max_hours(wid, w as usize) as i32)
        .sum()
}

pub(crate) fn draw_left_footer(
    ui: &mut egui::Ui,
    rect: Rect,
    app: &App,
    state: &mut UiState,
    workers: &[(crate::workers_utils::worker::WorkerId, String)],
) {
    let total_x = rect.left() + DEV_NAME_W; // 90
    let dev_section_w = DEV_NAME_W + DEV_TOTAL_W; // 150
    let wk_x = rect.left() + dev_section_w + 5.0;
    let year = state.selected_year;
    let cat = state.selected_category;

    // ── Riga header (row 0) ──
    // Selettore categoria (scope dei totali-anno) sopra la colonna dei nomi dev.
    let cat_rect = Rect::from_min_size(rect.left_top(), Vec2::new(DEV_NAME_W, ROW_H));
    let cat_label = match cat {
        None => "Tutte".to_string(),
        Some(c) => app.categories.get_name(c).unwrap_or("?").to_string(),
    };
    footer_combo(
        ui,
        cat_rect,
        "footer_cat_combo",
        cat_label,
        DEV_NAME_W - 6.0,
        |ui| {
            ui.selectable_value(&mut state.selected_category, None, "Tutte");
            for (id, name) in app.categories.list() {
                ui.selectable_value(&mut state.selected_category, Some(id), name);
            }
        },
    );

    // Selettore anno (totali-anno per dev) sopra la colonna dei totali.
    let th = Rect::from_min_size(
        egui::pos2(total_x, rect.top()),
        Vec2::new(DEV_TOTAL_W, ROW_H),
    );
    let year_label = if year == 0 {
        "Tot".to_string()
    } else {
        year.to_string()
    };
    footer_combo(ui, th, "footer_year_combo", year_label, 54.0, |ui| {
        ui.selectable_value(&mut state.selected_year, 0, "—");
        for y in available_years(app) {
            ui.selectable_value(&mut state.selected_year, y, y.to_string());
        }
    });

    // filtro effort sopra la sezione worker
    let labels = ["Tutti", "Nulli", "≥40"];
    let fbw = 46.0;
    for (i, lab) in labels.iter().enumerate() {
        let x = wk_x + i as f32 * (fbw + 2.0);
        let br = Rect::from_min_size(egui::pos2(x, rect.top()), Vec2::new(fbw, ROW_H));
        let active = state.effort_filter_mode == i as i32;
        if ui
            .put(
                br,
                egui::SelectableLabel::new(active, egui::RichText::new(*lab).color(text())),
            )
            .clicked()
        {
            state.effort_filter_mode = i as i32;
        }
    }

    // ── Sezione dev: nome + totale-anno ──
    // `grand_total` si accumula qui per evitare un secondo passaggio nella riga "Totale".
    // `projects` è costruita una volta sola e condivisa da tutte le chiamate a `dev_year_total`.
    let devs = app.devs.list();
    let projects = app.projects.list();
    let mut grand_total = 0i32;
    for (di, (dev, dname)) in devs.iter().enumerate() {
        let y = rect.top() + (di as f32 + 1.0) * ROW_H;
        let color = dev_color(app, *dev);
        let tcol = dev_text_color(app, *dev);
        let nrect = Rect::from_min_size(egui::pos2(rect.left(), y), Vec2::new(DEV_NAME_W, ROW_H));
        ui.painter().rect_filled(nrect, 0.0, color);
        ui.painter().text(
            nrect.center(),
            Align2::CENTER_CENTER,
            dname,
            cell_font(),
            tcol,
        );
        let dev_total = dev_year_total(app, &projects, *dev, year, cat);
        grand_total += dev_total;
        let trect = Rect::from_min_size(egui::pos2(total_x, y), Vec2::new(DEV_TOTAL_W, ROW_H));
        let ttxt = if year == 0 {
            "—".to_string()
        } else {
            dev_total.to_string()
        };
        ui.painter().text(
            trect.center(),
            Align2::CENTER_CENTER,
            ttxt,
            cell_font(),
            text(),
        );
    }

    // ── Riga "Totale": somma dei totali-anno di tutti i dev ──
    // Rispetta i selettori anno e categoria attivi nel footer.
    // Una riga vuota (+1) la separa dall'elenco dev.
    let ty = rect.top() + (devs.len() as f32 + 2.0) * ROW_H;
    let nrect = Rect::from_min_size(egui::pos2(rect.left(), ty), Vec2::new(DEV_NAME_W, ROW_H));
    ui.painter().rect_filled(nrect, 0.0, g(START_STOP));
    ui.painter().text(
        nrect.center(),
        Align2::CENTER_CENTER,
        "Totale",
        cell_font(),
        Color32::BLACK,
    );
    let trect = Rect::from_min_size(egui::pos2(total_x, ty), Vec2::new(DEV_TOTAL_W, ROW_H));
    let ttxt = if year == 0 {
        "—".to_string()
    } else {
        grand_total.to_string()
    };
    ui.painter().text(
        trect.center(),
        Align2::CENTER_CENTER,
        ttxt,
        cell_font(),
        text(),
    );

    // ── Sezione worker: nomi (allineati col sovra a destra) ──
    // Click sul nome → popup "Ore max" (limite settimanale globale del worker).
    for (idx, (wid, name)) in workers.iter().enumerate() {
        let y = rect.top() + (idx as f32 + 1.0) * ROW_H;
        let cell = Rect::from_min_size(egui::pos2(wk_x, y), Vec2::new(COL_W, ROW_H));
        if idx % 2 == 0 {
            ui.painter().rect_filled(cell, 0.0, BETWEEN_PROJECTS);
        }
        if app.workers.get_max_hours(*wid) < DEFAULT_MAX_HOURS {
            draw_corner_triangle_left(ui, cell);
        }
        ui.painter().text(
            cell.center(),
            Align2::CENTER_CENTER,
            name,
            cell_font(),
            text(),
        );

        // Ore rimanenti dell'anno selezionato, nello spazio a destra del nome.
        // Su "Tot" (year == 0) la cella resta vuota.
        if year != 0 {
            let rem_left = wk_x + COL_W;
            let rem_rect = Rect::from_min_size(
                egui::pos2(rem_left, y),
                Vec2::new((rect.right() - rem_left).max(0.0), ROW_H),
            );
            let remaining = worker_remaining_hours(
                app,
                *wid,
                year,
                state.this_week,
                state.effort_filter_mode == 1,
            );
            ui.painter().text(
                rem_rect.right_center() - Vec2::new(12.0, 0.0),
                Align2::RIGHT_CENTER,
                remaining.to_string(),
                cell_font(),
                g(ok_green()),
            );
        }

        let resp = ui
            .interact(cell, ui.id().with(("wmax", wid.0)), Sense::click())
            .on_hover_cursor(egui::CursorIcon::PointingHand);
        if resp.clicked() {
            let cur = app.workers.get_max_hours(*wid);
            state.popup = Some(Popup::WorkerMax {
                worker: *wid,
                name: name.clone(),
                text: cur.to_string(),
            });
        }
    }
}

pub(crate) fn draw_right_footer(
    ui: &egui::Ui,
    rect: Rect,
    app: &App,
    state: &mut UiState,
    workers: &[(crate::workers_utils::worker::WorkerId, String)],
    cols: &[Col],
    actions: &mut Vec<Action>,
) {
    let lightgreen = g(ok_green());
    let mut next_x = rect.left();
    for c in cols.iter() {
        let colw = col_width(c, COL_W);
        let x = next_x;
        next_x += colw;

        // colonna di confine d'anno: sfondo canarino su tutta l'altezza, niente valori
        let ws = match c {
            Col::YearEnd(_) => {
                let col =
                    Rect::from_min_size(egui::pos2(x, rect.top()), Vec2::new(colw, rect.height()));
                ui.painter().rect_filled(col, 0.0, g(CANARY));
                continue;
            }
            Col::Weeks(ws) => ws,
        };
        let Some(w) = ws.first().copied() else {
            continue;
        };
        // gruppi mergiati (zoom): valori sommati e sola lettura (niente click/note/stato)
        let merged = ws.len() > 1;

        // tinta settimana corrente su tutta la colonna
        if ws.contains(&state.this_week) {
            let col =
                Rect::from_min_size(egui::pos2(x, rect.top()), Vec2::new(COL_W, rect.height()));
            ui.painter()
                .rect_filled(col, 0.0, g(THIS_WEEK).gamma_multiply(0.18));
        }

        // header: data settimana
        let hdr = Rect::from_min_size(egui::pos2(x, rect.top()), Vec2::new(COL_W, ROW_H));
        let date = primo_giorno_settimana_corrente(&days_to_local(w))
            .format("%y-%m-%d")
            .to_string();
        ui.painter().text(
            hdr.center(),
            Align2::CENTER_CENTER,
            date,
            cell_font(),
            text(),
        );

        // celle sovra per worker
        for (idx, (wid, name)) in workers.iter().enumerate() {
            let y = rect.top() + (idx as f32 + 1.0) * ROW_H;
            let cell = Rect::from_min_size(egui::pos2(x, y), Vec2::new(COL_W, ROW_H));

            let bg = if idx % 2 == 0 { row_even() } else { row_alt() };
            ui.painter().rect_filled(cell, 0.0, bg);

            // Somma su tutte le settimane del gruppo (una sola al livello zoom 0).
            let value: i32 = ws
                .iter()
                .map(|wk| {
                    app.sovra
                        .get(&(WeekId(*wk as usize), *wid))
                        .map_or(0, |e| e.0 as i32)
                })
                .sum();
            let eff_max: i32 = ws
                .iter()
                .map(|wk| app.workers.get_effective_max_hours(*wid, *wk as usize) as i32)
                .sum();
            let global_max = app.workers.get_max_hours(*wid) as i32 * ws.len() as i32;

            let shown = state.effort_filter_mode == 0
                || (state.effort_filter_mode == 1 && value == 0 && eff_max != 0)
                || (state.effort_filter_mode == 2 && value >= 40);
            if !shown {
                continue;
            }

            // Elementi per-settimana (note, stato, click): solo per colonne non mergiate.
            let has_note = !merged && app.workers.has_week_note(*wid, w as usize);
            if has_note {
                draw_note_triangle(ui, cell.shrink(STATUS_TRI_INSET));
            }

            // triangoli di stato: ferie (verde, alto-sx) / malattia (rosso, basso-sx)
            if !merged {
                match app.workers.get_week_status(*wid, w as usize) {
                    Some(WeekStatus::Ferie) => draw_status_triangle_top_left(
                        ui,
                        cell,
                        g(Color32::from_rgb(0x33, 0x99, 0xFF)),
                    ),
                    Some(WeekStatus::Malattia) => {
                        draw_status_triangle_bottom_left(ui, cell, g(Color32::RED))
                    }
                    None => {}
                }
            }

            let color = if value > eff_max {
                g(Color32::RED)
            } else if eff_max == 0 {
                g(override_brown()) // marrone: override a zero
            } else if value == 0 {
                g(zero_yellow())
            } else {
                lightgreen
            };
            let txt = if eff_max != global_max {
                format!("{} | {}", value, eff_max)
            } else {
                value.to_string()
            };
            ui.painter().text(
                cell.center(),
                Align2::CENTER_CENTER,
                txt,
                cell_font(),
                color,
            );

            // Colonna mergiata (zoom): sola lettura, niente interazioni per-settimana.
            if merged {
                continue;
            }

            // Tasto sinistro → override ore max; tasto destro → nota worker/settimana.
            let resp = ui
                .interact(cell, ui.id().with(("wkmax", wid.0, w)), Sense::click())
                .on_hover_cursor(egui::CursorIcon::PointingHand);
            if resp.clicked() {
                state.popup = Some(Popup::WorkerWeekMax {
                    worker: *wid,
                    name: name.clone(),
                    week: w as usize,
                    text: eff_max.to_string(),
                });
            }
            let cur_status = app.workers.get_week_status(*wid, w as usize);
            resp.context_menu(|ui| {
                // "Ferie" e "Malattia" fanno da toggle: riscegliere lo stato attivo lo rimuove.
                if ui
                    .selectable_label(cur_status == Some(WeekStatus::Ferie), "Ferie")
                    .clicked()
                {
                    let status =
                        (cur_status != Some(WeekStatus::Ferie)).then_some(WeekStatus::Ferie);
                    actions.push(Action::SetWorkerWeekStatus {
                        worker: *wid,
                        week: w as usize,
                        status,
                    });
                    ui.close();
                }
                if ui
                    .selectable_label(cur_status == Some(WeekStatus::Malattia), "Malattia")
                    .clicked()
                {
                    let status =
                        (cur_status != Some(WeekStatus::Malattia)).then_some(WeekStatus::Malattia);
                    actions.push(Action::SetWorkerWeekStatus {
                        worker: *wid,
                        week: w as usize,
                        status,
                    });
                    ui.close();
                }
                if ui.button("Note").clicked() {
                    state.note_editor = Some(NoteEditing {
                        target: NoteTarget::WorkerWeek {
                            worker: *wid,
                            week: w as usize,
                            name: name.clone(),
                        },
                        text: app.workers.get_week_note(*wid, w as usize),
                    });
                    ui.close();
                }
            });

            // Passando sopra il triangolo si vede il testo della nota.
            if has_note {
                let tri = Rect::from_min_size(
                    egui::pos2(cell.right() - 12.0, cell.top()),
                    Vec2::new(12.0, 12.0),
                );
                if resp.hover_pos().is_some_and(|p| tri.contains(p)) {
                    let note = app.workers.get_week_note(*wid, w as usize);
                    // stesse impostazioni di `Response::on_hover_text` (tooltip
                    // ancorato alla cella, non al puntatore).
                    resp.show_tooltip_ui(|ui| {
                        ui.set_max_width(ui.spacing().tooltip_width);
                        ui.add(egui::widgets::Label::new(note));
                    });
                }
            }
        }
    }
}

/// Triangolo arancio nell'angolo in alto a SINISTRA (ore ridotte).
pub(crate) fn draw_corner_triangle_left(ui: &egui::Ui, cell: Rect) {
    let tl = cell.left_top();
    let pts = vec![
        egui::pos2(tl.x, tl.y),
        egui::pos2(tl.x + 10.0, tl.y),
        egui::pos2(tl.x, tl.y + 10.0),
    ];
    ui.painter().add(egui::Shape::convex_polygon(
        pts,
        g(EFFORT_ORANGE),
        Stroke::NONE,
    ));
}

// ── Griglia (colonna destra) ────────────────────────────────────────────────

