//! (estratto da `ui.rs` — vedi miglioramento #16)

use super::*;

pub(crate) fn body(ui: &mut egui::Ui, app: &App, state: &mut UiState, actions: &mut Vec<Action>) {
    let top_down = egui::Layout::top_down(egui::Align::Min);
    // I filtri sono clonati una volta per frame per evitare conflitti di borrow.
    let filter = state.worker_filter.clone();
    let dev_filter = state.dev_filter.clone();

    // Salto rapido a un progetto: risolvo qui (prima delle aree di scroll) la
    // posizione Y del progetto scelto, usando lo stesso layout della griglia.
    if let Some(target) = state.jump_to_project.take() {
        let compact = state.compact_mode;
        let merged = !compact && state.zoom_level > 0;
        let layout = project_layout(
            ui,
            app,
            &filter,
            state.worker_filter_current_week,
            &dev_filter,
            state.project_view,
            compact,
            merged,
        );
        let mut y = DEV_BORDER;
        let mut found = false;
        for p in &layout {
            if p.proj == target {
                found = true;
                break;
            }
            y += p.proj_h + DEV_BORDER;
        }
        if found {
            state.pending_scroll_y = Some((y - DEV_BORDER).max(0.0));
        }
    }

    // Colonna sinistra: larghezza fissa, scroll verticale che segue la griglia.
    egui::SidePanel::left("leftcol")
        .exact_width(LEFT_W)
        .resizable(false)
        .frame(egui::Frame::NONE.fill(bg()))
        .show_inside(ui, |ui| {
            let sy = state.scroll_y;
            egui::ScrollArea::vertical()
                .id_salt("left_scroll")
                .vertical_scroll_offset(sy)
                .enable_scrolling(false)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.with_layout(top_down, |ui| {
                        left_column(ui, app, state, actions, &filter, &dev_filter)
                    });
                });
        });

    // Griglia: prende lo spazio restante, è la "master" dello scroll.
    egui::CentralPanel::default()
        .frame(egui::Frame::NONE.fill(bg()))
        .show_inside(ui, |ui| {
            let mut sa = egui::ScrollArea::both()
                .id_salt("grid_scroll")
                .auto_shrink([false, false]);
            // Scroll pilotato: X per la settimana corrente (init), Y per il salto
            // a un progetto. L'asse non pilotato mantiene la posizione attuale.
            if state.pending_scroll_x.is_some() || state.pending_scroll_y.is_some() {
                let x = state.pending_scroll_x.unwrap_or(state.scroll_x);
                let y = state.pending_scroll_y.unwrap_or(state.scroll_y);
                sa = sa.scroll_offset(Vec2::new(x, y));
            }
            let out = sa.show(ui, |ui| {
                ui.with_layout(top_down, |ui| {
                    grid(ui, app, state, actions, &filter, &dev_filter)
                });
            });
            state.scroll_x = out.state.offset.x;
            state.scroll_y = out.state.offset.y;
            state.pending_scroll_x = None;
            state.pending_scroll_y = None;
        });
}

pub(crate) fn alloc_strip(ui: &mut egui::Ui, w: f32, h: f32, color: Color32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(w, h), Sense::hover());
    ui.painter().rect_filled(rect, 0.0, color);
}

// ── Footer (worker + sovra) ─────────────────────────────────────────────────

// ── Note editor (finestra) ──────────────────────────────────────────────────

/// Altezza effettiva del blocco nome: il `TextEdit` multiriga cresce con testi
/// lunghi, quindi misuriamo il galley wrappato a `LEFT_INFO_W` (mai meno di
/// `NAME_ROWS` righe). Senza questo, l'altezza riservata al progetto sarebbe
/// troppo piccola e inizio/fine sforerebbero nel progetto successivo.
pub(crate) fn name_block_height(ui: &egui::Ui, name: &str) -> f32 {
    let galley = ui
        .painter()
        .layout(name.to_string(), cell_font(), text(), LEFT_INFO_W);
    galley.size().y.max(NAME_ROWS as f32 * ROW_H)
}

pub(crate) fn project_layout(
    ui: &egui::Ui,
    app: &App,
    filter: &Filter,
    current_week_only: bool,
    dev_filter: &DevFilter,
    view: ProjectViewMode,
    compact: bool,
    merged: bool,
) -> Vec<ProjLayout> {
    // Righe info oltre al nome: compatta = solo tripletta; normale = tripletta +
    // categoria + inizio + fine + avanzamento.
    let extra_rows = if compact { 1.0 } else { 5.0 };
    let mut out = Vec::new();
    for (proj_id, name) in app.projects.list() {
        // Visibilità (filtro «Progetti…») + modalità Vista (Solo aperti / Solo
        // chiusi / Tutti). Il filtro `enable` conta solo per gli aperti: i chiusi
        // sono sempre `enable = false` (vedi `project_in_body`).
        if !project_in_body(app, view, proj_id) {
            continue;
        }
        // Modalità "settimana corrente" (Ctrl+G): col filtro worker attivo mostra
        // solo i progetti in cui un worker selezionato ha effort nella settimana
        // corrente. Dentro, la resa resta identica a Ctrl+F (nessun'altra modifica).
        if current_week_only {
            if let Some(set) = filter.as_ref() {
                if !project_worker_in_current_week(app, proj_id, set) {
                    continue;
                }
            }
        }
        // Filtro dev (Ctrl+D): tiene solo i dev selezionati con effort > 0; si
        // combina in AND col filtro worker (che agisce sulle righe/celle).
        let devs: Vec<(DevId, usize)> = app
            .projects
            .list_devs(proj_id)
            .into_iter()
            .filter(|d| dev_shown(app, proj_id, *d, dev_filter))
            .filter_map(|d| filtered_dev_max_rows(app, proj_id, d, filter).map(|m| (d, m)))
            .collect();
        // con un filtro attivo, nascondi i progetti senza dev corrispondenti
        if (filter.is_some() || dev_filter.is_some()) && devs.is_empty() {
            continue;
        }
        let sum_devs: f32 = devs
            .iter()
            .map(|(_, m)| dev_block_height(*m, compact, merged))
            .sum();
        // Con un filtro (worker o dev) attivo l'info mostra solo la tripletta (1
        // riga): così non aggiunge spessore oltre alle righe dev filtrate.
        // Senza filtro riserva lo spazio reale del nome (può crescere su più righe).
        let info_h = if filter.is_some() || dev_filter.is_some() {
            ROW_H
        } else {
            extra_rows * ROW_H + name_block_height(ui, &name)
        };
        let proj_h = sum_devs.max(info_h);
        out.push(ProjLayout {
            proj: proj_id,
            name,
            devs,
            proj_h,
        });
    }
    out
}

/// True se il progetto ha almeno un dev in cui un worker con nome in `set` è
/// assegnato nella settimana corrente, **anche con effort 0** (conta
/// l'assegnazione, non le ore). Usato dal filtro Ctrl+G.
fn project_worker_in_current_week(app: &App, proj: ProjectId, set: &HashSet<String>) -> bool {
    let wk = current_week_id();
    app.projects.list_devs(proj).into_iter().any(|dev| {
        app.projects
            .get_single_dev(proj, dev)
            .and_then(|sd| sd.get_all(wk))
            .is_some_and(|sew| {
                sew.worker_id.iter().any(|(wid, _)| {
                    *wid != WORKER_ID_ZERO && set.contains(app.workers.get_name_by_id(*wid))
                })
            })
    })
}

pub(crate) fn total_content_h(layout: &[ProjLayout]) -> f32 {
    DEV_BORDER + layout.iter().map(|p| p.proj_h + DEV_BORDER).sum::<f32>()
}

/// Titolo "Effort residuo" su due righe, centrato nella cella della colonna di
/// confine (testo scuro su sfondo canarino).
pub(crate) fn draw_boundary_title(ui: &egui::Ui, cell: Rect) {
    let font = mono(7.0);
    let cx = cell.center().x;
    // Sfondo canarino sempre giallo → testo sempre nero (indipendente dal tema).
    ui.painter().text(
        egui::pos2(cx, cell.top() + cell.height() * 0.30),
        Align2::CENTER_CENTER,
        "Effort",
        font.clone(),
        Color32::BLACK,
    );
    ui.painter().text(
        egui::pos2(cx, cell.top() + cell.height() * 0.72),
        Align2::CENTER_CENTER,
        "residuo",
        font,
        Color32::BLACK,
    );
}

/// Striscia orizzontale piena (bordo dev / separatore progetto), disegnata a y assoluta.
pub(crate) fn paint_hstrip(ui: &egui::Ui, left: f32, w: f32, y: f32, color: Color32) {
    ui.painter().rect_filled(
        Rect::from_min_size(egui::pos2(left, y), Vec2::new(w, DEV_BORDER)),
        0.0,
        color,
    );
}

/// Come `paint_hstrip` ma limitata all'intervallo X `[x0, x1)`: usata per le
/// bande colorate dei dev, che non devono comparire fuori dal periodo attivo
/// del progetto. Se l'intervallo è vuoto non disegna nulla.
pub(crate) fn paint_hstrip_range(ui: &egui::Ui, x0: f32, x1: f32, y: f32, color: Color32) {
    if x1 <= x0 {
        return;
    }
    ui.painter().rect_filled(
        Rect::from_min_size(egui::pos2(x0, y), Vec2::new(x1 - x0, DEV_BORDER)),
        0.0,
        color,
    );
}

/// Larghezza minima (px) di una banda milestone: sotto questa soglia le bande
/// diventano illeggibili, quindi se ne disegnano solo quante ce ne stanno.
pub(crate) const MS_BAND_MIN_W: f32 = 4.0;

/// Quante milestone della settimana entrano in una colonna larga `w` mantenendo
/// bande leggibili: almeno una, mai più di quelle presenti.
pub(crate) fn milestone_bands_shown(w: f32, n: usize) -> usize {
    let max = (w / MS_BAND_MIN_W).floor().max(1.0) as usize;
    n.min(max)
}

/// Dipinge lo sfondo della colonna con le milestone collocate in quella
/// settimana: una **banda verticale per milestone**, di uguale larghezza e in
/// ordine di id (con una sola milestone equivale a tingere l'intera colonna,
/// il comportamento storico). Se le milestone sono troppe perché le bande
/// restino leggibili (`MS_BAND_MIN_W`), se ne disegnano solo le prime: il
/// tooltip della riga in alto elenca comunque tutti i nomi.
pub(crate) fn paint_milestone_bands(ui: &egui::Ui, app: &App, col_rect: Rect, ms: &[MilestoneId]) {
    let colors: Vec<Color32> = ms
        .iter()
        .filter_map(|m| app.milestones.get_color(*m))
        .map(from_hex)
        .collect();
    if colors.is_empty() {
        return;
    }
    let n = milestone_bands_shown(col_rect.width(), colors.len());
    let bw = col_rect.width() / n as f32;
    for (i, c) in colors.iter().take(n).enumerate() {
        let x0 = col_rect.left() + i as f32 * bw;
        // L'ultima banda arriva esattamente al bordo destro: evita la fessura
        // di un pixel lasciata dagli arrotondamenti.
        let x1 = if i + 1 == n {
            col_rect.right()
        } else {
            x0 + bw
        };
        ui.painter().rect_filled(
            Rect::from_min_max(
                egui::pos2(x0, col_rect.top()),
                egui::pos2(x1, col_rect.bottom()),
            ),
            0.0,
            *c,
        );
    }
}

/// Intervallo X `[x0, x1)` delle colonne-settimana "attive" del progetto, cioè
/// comprese tra inizio e fine (estremi inclusi). `proj_start`/`deadline` a -1
/// significano "nessun limite" su quel lato. Se nessuna settimana visibile è
/// attiva restituisce un intervallo vuoto (`x0 == x1`).
pub(crate) fn active_hstrip_range(
    cols: &[Col],
    cw: f32,
    left: f32,
    content_w: f32,
    proj_start: i32,
    deadline: i32,
) -> (f32, f32) {
    // Nessun limite su entrambi i lati: banda a piena larghezza (comportamento storico).
    if proj_start < 0 && deadline < 0 {
        return (left, left + content_w);
    }
    let is_active = |c: &Col| {
        matches!(c, Col::Weeks(ws)
            if ws.iter().any(|w| (proj_start < 0 || *w >= proj_start) && (deadline < 0 || *w <= deadline)))
    };
    match (
        cols.iter().position(is_active),
        cols.iter().rposition(is_active),
    ) {
        (Some(first), Some(last)) => (
            left + col_x_offset(cols, first, cw),
            left + col_x_offset(cols, last + 1, cw),
        ),
        _ => (left, left), // nessuna settimana attiva in vista: niente banda
    }
}

/// Etichetta data (sempre visibile) sopra la colonna inizio/fine di un progetto
/// in vista compatta. Sfondo colorato come la colonna, testo centrato sulla colonna.
pub(crate) fn draw_compact_date_marker(
    ui: &egui::Ui,
    left: f32,
    cw: f32,
    cols: &[Col],
    week: i32,
    top_y: f32,
    bg: Color32,
) {
    if week < 0 {
        return;
    }
    let Some(ci) = cols.iter().position(|c| c.contains_week(week)) else {
        return;
    };
    let cx = left + col_x_offset(cols, ci, cw) + cw / 2.0;
    let date = primo_giorno_settimana_corrente(&days_to_local(week))
        .format("%y-%m-%d")
        .to_string();
    let galley = ui.painter().layout_no_wrap(date, mono(9.0), text());
    let pos = egui::pos2(cx - galley.size().x / 2.0, top_y + 1.0);
    let bgrect = Rect::from_min_size(pos, galley.size()).expand(1.5);
    ui.painter().rect_filled(bgrect, 2.0, bg);
    ui.painter().galley(pos, galley, text());
}

pub(crate) fn grid(
    ui: &mut egui::Ui,
    app: &App,
    state: &mut UiState,
    actions: &mut Vec<Action>,
    filter: &Filter,
    dev_filter: &DevFilter,
) {
    let compact = state.compact_mode;
    // zoom attivo solo in vista normale; le colonne mergiate sono sola lettura.
    let level = if compact { 0 } else { state.zoom_level };
    let merged = level > 0;
    let cw = col_w(compact);
    let cols = columns_vec(app, level);
    let content_w = cols_width(&cols, cw);
    let layout = project_layout(
        ui,
        app,
        filter,
        state.worker_filter_current_week,
        dev_filter,
        state.project_view,
        compact,
        merged,
    );
    let total_h = total_content_h(&layout);

    // Un'unica allocazione: tutto il resto è disegno a coordinate assolute.
    let (rect, _) = ui.allocate_exact_size(Vec2::new(content_w, total_h), Sense::hover());
    let left = rect.left();
    let mut y = rect.top();

    paint_hstrip(ui, left, content_w, y, g(START_STOP));
    y += DEV_BORDER;

    for p in &layout {
        // I progetti chiusi si disegnano in scala di grigi (come la modalità B/N),
        // indipendentemente dal B/N globale; ripristinato a fine progetto.
        set_bw_mode(state.bw_mode || app.projects.is_closed(p.proj));
        let proj_start = app
            .projects
            .get_project_start_week(p.proj)
            .map(|w| w.0 as i32)
            .unwrap_or(-1);
        let deadline = app
            .projects
            .get_project_end_week(p.proj)
            .map(|w| w.0 as i32)
            .unwrap_or(-1);
        // Le bande colorate dei dev sono limitate al periodo attivo del progetto:
        // niente riga colorata prima dell'inizio o dopo la fine.
        let (act_x0, act_x1) =
            active_hstrip_range(&cols, cw, left, content_w, proj_start, deadline);
        let proj_top = y;
        let mut dy = y;

        for (dev_id, max_rows) in &p.devs {
            let color = dev_color(app, *dev_id);
            // i bordi dev sono trasparenti in compatta
            let border = if compact { bg() } else { color };
            paint_hstrip_range(ui, act_x0, act_x1, dy, border);
            dy += DEV_BORDER;
            let inner_h = dev_inner_h(*max_rows, compact, merged);
            let block = Rect::from_min_size(egui::pos2(left, dy), Vec2::new(content_w, inner_h));
            draw_dev_cells(
                ui, block, app, state, actions, p.proj, *dev_id, *max_rows, &cols, proj_start,
                deadline, filter, compact, cw, merged,
            );
            dy += inner_h;
            paint_hstrip_range(ui, act_x0, act_x1, dy, border);
            dy += DEV_BORDER;
        }

        // Totale progetto in cima a ogni colonna di confine d'anno (solo progetti a
        // cavallo): somma dei residui positivi dei dev per completare il progetto.
        if !compact {
            let mut cx = left;
            for c in &cols {
                let colw = col_width(c, cw);
                if let Col::YearEnd(year_ending) = c {
                    if let Some(total) = project_missing_at_year_end(app, p.proj, *year_ending) {
                        let cell =
                            Rect::from_min_size(egui::pos2(cx, proj_top), Vec2::new(colw, ROW_H));
                        ui.painter().rect_filled(cell, 0.0, bg());
                        ui.painter().text(
                            cell.center(),
                            Align2::CENTER_CENTER,
                            format!("T:{total}"),
                            cell_font(),
                            g(EFFORT_ORANGE),
                        );
                    }
                }
                cx += colw;
            }
        }

        // in compatta: etichetta data sopra le colonne inizio (azzurra) e fine (verde)
        if compact {
            draw_compact_date_marker(ui, left, cw, &cols, proj_start, proj_top, g(START_BG));
            draw_compact_date_marker(ui, left, cw, &cols, deadline, proj_top, g(DEADLINE_BG));
        }

        y = proj_top + p.proj_h;
        paint_hstrip(ui, left, content_w, y, BETWEEN_PROJECTS);
        y += DEV_BORDER;
    }
    // Ripristina il B/N globale dopo l'eventuale grigio dei progetti chiusi.
    set_bw_mode(state.bw_mode);
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_dev_cells(
    ui: &mut egui::Ui,
    rect: Rect,
    app: &App,
    state: &mut UiState,
    actions: &mut Vec<Action>,
    proj: ProjectId,
    dev: DevId,
    max_rows: usize,
    cols: &[Col],
    proj_start: i32,
    deadline: i32,
    filter: &Filter,
    compact: bool,
    cw: f32,
    merged: bool,
) {
    let planned = app
        .projects
        .get_single_dev(proj, dev)
        .map(|sd| sd.planned_effort().0 as i32)
        .unwrap_or(0);
    let hide_effort = app
        .projects
        .get_single_dev(proj, dev)
        .map(|sd| sd.get_hide_effort())
        .unwrap_or(false);

    // range di attività del dev (per le barre della vista compatta)
    let (act_start, act_end) = app
        .projects
        .get_single_dev(proj, dev)
        .map(|sd| {
            let ws = sd.get_weeks();
            (
                ws.iter().map(|w| w.0 as i32).min().unwrap_or(-1),
                ws.iter().map(|w| w.0 as i32).max().unwrap_or(-1),
            )
        })
        .unwrap_or((-1, -1));
    let dcolor = dev_color(app, dev);

    let mut running = 0i32;
    let mut next_x = rect.left();
    for c in cols.iter() {
        let colw = col_width(c, cw);
        let x = next_x;
        next_x += colw;

        // ── Colonna di confine d'anno: sola lettura, sfondo canarino ──
        if let Col::YearEnd(year_ending) = c {
            let col_rect =
                Rect::from_min_size(egui::pos2(x, rect.top()), Vec2::new(colw, rect.height()));
            draw_year_end_cell(ui, app, col_rect, proj, dev, *year_ending, hide_effort);
            continue;
        }
        let ws = match c {
            Col::Weeks(ws) => ws,
            Col::YearEnd(_) => unreachable!(),
        };
        let w = &ws[0]; // settimana rappresentativa (etichette/id)

        // Flag aggregati sul gruppo di settimane (una sola al livello zoom 0).
        let before_start = proj_start >= 0 && ws.iter().all(|wk| *wk < proj_start);
        let after_deadline = deadline >= 0 && ws.iter().all(|wk| *wk > deadline);
        let is_deadline = deadline >= 0 && ws.contains(&deadline);
        let is_start = proj_start >= 0 && ws.contains(&proj_start);

        // colonna settimana: bg deadline/start
        let col_rect = Rect::from_min_size(egui::pos2(x, rect.top()), Vec2::new(cw, rect.height()));
        if is_deadline {
            ui.painter().rect_filled(col_rect, 0.0, g(DEADLINE_BG));
        } else if is_start {
            ui.painter().rect_filled(col_rect, 0.0, g(START_BG));
        }
        if !compact && ws.contains(&state.this_week) {
            ui.painter()
                .rect_filled(col_rect, 0.0, g(THIS_WEEK).gamma_multiply(0.18));
        }

        // ── Milestone: tinta colonna col colore della milestone + tooltip;
        //    tasto destro sulla riga in alto per aggiungere/rimuovere. ──
        let ms_here = milestones_in_group(app, proj, ws);
        paint_milestone_bands(ui, app, col_rect, &ms_here);
        draw_milestone_strip(
            ui, app, state, actions, proj, dev, *w, &ms_here, col_rect, merged,
        );

        // Effort del gruppo = somma delle settimane mergiate (una sola al livello 0).
        let week_total = app
            .projects
            .get_single_dev(proj, dev)
            .map(|sd| {
                ws.iter()
                    .map(|wk| sd.get_effort_by_week(WeekId(*wk as usize)).0 as i32)
                    .sum()
            })
            .unwrap_or(0);

        // ── Vista compatta: una barra (altezza ∝ effort) per settimana attiva ──
        if compact {
            let in_activity = act_start >= 0 && *w >= act_start && *w <= act_end;
            if !before_start && !after_deadline && in_activity {
                draw_compact_bar(ui, col_rect, proj, dev, *w, week_total, dcolor);
            }
            continue;
        }

        running += week_total;

        // riga cumulativo (sola lettura)
        if !before_start && !after_deadline {
            draw_cumulative_row(
                ui,
                app,
                col_rect,
                proj,
                dev,
                ws,
                planned,
                running,
                week_total,
                is_deadline,
                hide_effort,
            );
        }

        // righe persona (editabili)
        if before_start || after_deadline {
            continue;
        }

        // ── Colonna mergiata (zoom): una sola cella con la somma, sola lettura,
        //    niente nomi worker. Occupa la riga sotto il cumulativo. ──
        if merged {
            if !hide_effort && (week_total > 0 || is_deadline) {
                draw_merged_cell(ui, col_rect, week_total);
            }
            continue;
        }

        let slots = gather_slots(app, proj, dev, *w, max_rows, filter);
        for (row, (text, note)) in slots.iter().enumerate() {
            let y = rect.top() + (row as f32 + 1.0) * ROW_H;
            let cell = Rect::from_min_size(egui::pos2(x, y), Vec2::new(cw, ROW_H));
            let cx = CellCtx {
                proj,
                dev,
                week: *w,
                row,
            };

            let is_editing = state
                .editing
                .as_ref()
                .map(|e| e.proj == proj && e.dev == dev && e.week == *w && e.row == row)
                .unwrap_or(false);

            if is_editing {
                draw_cell_editor(ui, app, state, actions, &cx, cell, note);
            } else {
                draw_cell_static(ui, app, state, actions, &cx, cell, text, note);
            }
        }
    }
}

/// Coordinate della cella worker su cui agiscono editing, click e tasto destro.
struct CellCtx {
    proj: ProjectId,
    dev: DevId,
    /// Settimana (numero di giorno, non indice).
    week: i32,
    /// Riga worker dentro il blocco del dev.
    row: usize,
}

/// Colonna di confine d'anno: sfondo canarino e ore mancanti del dev (solo se
/// il progetto è a cavallo del confine). Sola lettura.
fn draw_year_end_cell(
    ui: &egui::Ui,
    app: &App,
    col_rect: Rect,
    proj: ProjectId,
    dev: DevId,
    year_ending: i32,
    hide_effort: bool,
) {
    ui.painter().rect_filled(col_rect, 0.0, g(CANARY));
    if let Some(missing) = dev_missing_at_year_end(app, proj, dev, year_ending) {
        if !hide_effort {
            // In basso, per non collidere col totale progetto in cima alla colonna.
            ui.painter().text(
                col_rect.center_bottom() - Vec2::new(0.0, 2.0),
                Align2::CENTER_BOTTOM,
                missing.to_string(),
                cell_font(),
                Color32::BLACK, // su sfondo canarino: sempre nero
            );
        }
    }
}

/// Unione (senza ripetizioni) delle milestone di tutte le settimane del gruppo
/// mergiato — una sola settimana al livello di zoom 0.
fn milestones_in_group(app: &App, proj: ProjectId, ws: &[i32]) -> Vec<MilestoneId> {
    let mut v = Vec::new();
    for wk in ws.iter() {
        for m in app
            .projects
            .project_milestones_at_week(proj, WeekId(*wk as usize))
        {
            if !v.contains(&m) {
                v.push(m);
            }
        }
    }
    v
}

/// Fascia alta della colonna-dev: tooltip con le milestone della settimana e,
/// sulle colonne **non** mergiate, il menù col tasto destro (aggiungi/rimuovi
/// milestone, Sposta blocco/devs). Sulle colonne mergiate resta informativa.
#[allow(clippy::too_many_arguments)]
fn draw_milestone_strip(
    ui: &mut egui::Ui,
    app: &App,
    state: &mut UiState,
    actions: &mut Vec<Action>,
    proj: ProjectId,
    dev: DevId,
    w: i32,
    ms_here: &[MilestoneId],
    col_rect: Rect,
    merged: bool,
) {
    let top = Rect::from_min_size(col_rect.min, Vec2::new(col_rect.width(), ROW_H));
    let names = || -> String {
        ms_here
            .iter()
            .filter_map(|m| app.milestones.get_name(*m).map(|s| s.to_string()))
            .collect::<Vec<_>>()
            .join(", ")
    };

    if merged {
        // Colonna mergiata: milestone solo informative (tooltip), niente menù.
        if !ms_here.is_empty() {
            ui.interact(
                top,
                egui::Id::new(("msrow_m", proj.0, dev.0, w)),
                Sense::hover(),
            )
            .on_hover_text(names());
        }
        return;
    }

    let mut resp = ui.interact(
        top,
        egui::Id::new(("msrow", proj.0, dev.0, w)),
        Sense::click(),
    );
    if !ms_here.is_empty() {
        resp = resp.on_hover_text(names());
    }
    let week_id = WeekId(w as usize);
    resp.context_menu(|ui| {
        // ── Sottomenù: Aggiungi milestone qui (+ rimozione) ──
        ui.menu_button("Aggiungi milestone qui", |ui| {
            let all = app.milestones.list();
            if all.is_empty() {
                ui.label("(nessuna — creane dalla toolbar)");
            }
            for (id, name, color) in &all {
                let here = ms_here.contains(id);
                let mark = if here { "● " } else { "" };
                // Icona del tipo prima del nome: bandierina o fulmine,
                // gli stessi simboli che finiscono nell'export.
                let icon = app.milestones.get_kind(*id).icon();
                let label =
                    egui::RichText::new(format!("{mark}{icon} {name}")).color(u32_to_color(*color));
                if ui.button(label).clicked() {
                    actions.push(Action::AddProjectMilestone {
                        proj,
                        milestone: *id,
                        week: week_id,
                    });
                    ui.close();
                }
            }
            if !ms_here.is_empty() {
                ui.separator();
                for m in ms_here {
                    if let Some(name) = app.milestones.get_name(*m) {
                        let icon = app.milestones.get_kind(*m).icon();
                        if ui.button(format!("Rimuovi: {icon} {name}")).clicked() {
                            actions.push(Action::RemoveProjectMilestone {
                                proj,
                                milestone: *m,
                            });
                            ui.close();
                        }
                    }
                }
            }
        });

        // ── Sottomenù: Sposta effort ──
        ui.menu_button("Sposta", |ui| {
            // Sposta blocco → blocco contiguo del dev attorno alla settimana.
            let block = app.projects.dev_contiguous_block(proj, dev, week_id);
            if ui
                .add_enabled(!block.is_empty(), egui::Button::new("Sposta blocco"))
                .clicked()
            {
                state.move_dialog = Some(make_move_params(app, proj, vec![(dev, block)]));
                state.move_dialog_was_open = false;
                ui.close();
            }
            // Sposta devs → elenco dei dev del progetto con effort.
            let with_effort = app.projects.devs_with_effort(proj);
            if ui
                .add_enabled(!with_effort.is_empty(), egui::Button::new("Sposta devs"))
                .clicked()
            {
                let names: HashMap<DevId, String> = app.devs.list().into_iter().collect();
                let candidates: Vec<(DevId, String)> = with_effort
                    .iter()
                    .map(|d| (*d, names.get(d).cloned().unwrap_or_default()))
                    .collect();
                // Pre-seleziona il dev da cui è stato aperto il menù.
                let mut selected = HashSet::new();
                if with_effort.contains(&dev) {
                    selected.insert(dev);
                }
                state.move_dialog = Some(MoveDialog::SelectDevs {
                    proj,
                    candidates,
                    selected,
                });
                state.move_dialog_was_open = false;
                ui.close();
            }
        });
    });
}

/// Vista compatta: barra in fondo alla colonna con altezza ∝ effort della
/// settimana (tetto a 40h), più tooltip con data e ore.
fn draw_compact_bar(
    ui: &mut egui::Ui,
    col_rect: Rect,
    proj: ProjectId,
    dev: DevId,
    w: i32,
    week_total: i32,
    dcolor: Color32,
) {
    let ratio = (week_total as f32 / 40.0).min(1.0);
    let bar_h = if week_total == 0 {
        1.0
    } else {
        (ROW_H * ratio).max(1.0)
    };
    let bar = Rect::from_min_size(
        egui::pos2(col_rect.left(), col_rect.bottom() - bar_h),
        Vec2::new(col_rect.width(), bar_h),
    );
    ui.painter().rect_filled(bar, 0.0, dcolor);
    // tooltip con data + effort della settimana
    let date = primo_giorno_settimana_corrente(&days_to_local(w))
        .format("%y-%m-%d")
        .to_string();
    ui.interact(
        col_rect,
        egui::Id::new(("cbar", proj.0, dev.0, w)),
        Sense::hover(),
    )
    .on_hover_text(format!("{}  ·  {}h", date, week_total));
}

/// Riga cumulativa (sola lettura) in cima al blocco del dev: `svolto | residuo`,
/// col colore che vira dal verde al rosso avvicinandosi al pianificato.
#[allow(clippy::too_many_arguments)]
fn draw_cumulative_row(
    ui: &egui::Ui,
    app: &App,
    col_rect: Rect,
    proj: ProjectId,
    dev: DevId,
    ws: &[i32],
    planned: i32,
    running: i32,
    week_total: i32,
    is_deadline: bool,
    hide_effort: bool,
) {
    let cum_rect = Rect::from_min_size(col_rect.min, Vec2::new(col_rect.width(), ROW_H));
    let has_workers = app
        .projects
        .get_single_dev(proj, dev)
        .map(|sd| {
            ws.iter().any(|wk| {
                sd.get_all(WeekId(*wk as usize))
                    .map(|s| s.worker_id.keys().any(|k| *k != WORKER_ID_ZERO))
                    .unwrap_or(false)
            })
        })
        .unwrap_or(false);
    if (has_workers || is_deadline) && !hide_effort {
        let remaining = planned - running;
        let txt = format!("{} | {}", running, remaining);
        let color = if is_deadline {
            text()
        } else {
            cumulative_color(week_total, planned)
        };
        ui.painter().text(
            cum_rect.center(),
            Align2::CENTER_CENTER,
            txt,
            cell_font(),
            color,
        );
    }
}

/// Colonna mergiata (zoom): una sola cella con la somma delle settimane, sola
/// lettura e senza nomi worker, subito sotto la riga cumulativa.
fn draw_merged_cell(ui: &egui::Ui, col_rect: Rect, week_total: i32) {
    let cell = Rect::from_min_size(
        col_rect.min + Vec2::new(0.0, ROW_H),
        Vec2::new(col_rect.width(), ROW_H),
    );
    ui.painter().text(
        cell.center(),
        Align2::CENTER_CENTER,
        week_total.to_string(),
        cell_font(),
        text(),
    );
}

/// Cella worker **in modifica**: tastiera (autocompletamento, Invio/Tab, Esc),
/// copia/taglia/incolla e disegno di sfondo, testo e caret. Alla conferma spinge
/// l'`Action`, altrimenti rimette l'editing nello stato.
fn draw_cell_editor(
    ui: &mut egui::Ui,
    app: &App,
    state: &mut UiState,
    actions: &mut Vec<Action>,
    cx: &CellCtx,
    cell: Rect,
    note: &str,
) {
    let mut ed = state.editing.take().unwrap();
    let id = egui::Id::new(("celledit", cx.proj.0, cx.dev.0, cx.week, cx.row));
    let resp = ui.interact(cell, id, Sense::click());
    if ed.just_opened {
        resp.request_focus();
        ed.just_opened = false;
    }
    let has_focus = resp.has_focus();

    let mut commit = false;
    let mut cancel = false;
    if has_focus {
        ed.had_focus = true;
        let events = ui.input(|i| i.events.clone());
        for ev in events {
            match ev {
                egui::Event::Text(t) => {
                    for ch in t.chars() {
                        if ch != '\n' && ch != '\t' {
                            ed.typed.push(ch);
                        }
                    }
                    recompute_completion(app, &mut ed.typed, &mut ed.buf);
                }
                egui::Event::Key {
                    key: egui::Key::Backspace,
                    pressed: true,
                    ..
                } => {
                    // Cancellazione letterale: niente re-completamento, altrimenti
                    // un nome completo verrebbe ri-completato e non si potrebbe svuotare.
                    ed.typed.pop();
                    ed.buf = ed.typed.clone();
                }
                egui::Event::Key {
                    key: egui::Key::Delete,
                    pressed: true,
                    ..
                } => {
                    ed.typed.clear();
                    ed.buf.clear();
                }
                egui::Event::Key {
                    key: egui::Key::Enter,
                    pressed: true,
                    ..
                }
                | egui::Event::Key {
                    key: egui::Key::Tab,
                    pressed: true,
                    ..
                } => {
                    commit = true;
                }
                egui::Event::Key {
                    key: egui::Key::Escape,
                    pressed: true,
                    ..
                } => {
                    cancel = true;
                }
                // Copia / Taglia / Incolla: eventi semantici di egui,
                // portabili (Cmd su macOS, Ctrl su Windows/Linux). Si scrive
                // anche nella clipboard di SISTEMA: senza, su Windows egui non
                // genera mai `Event::Paste` (lo emette solo se la clipboard di
                // sistema non è vuota — vedi egui-winit), e l'incolla non parte.
                // La nota della cella resta nella clipboard interna.
                egui::Event::Copy => {
                    state.copied_text = ed.buf.clone();
                    state.copied_note = note.to_string();
                    ui.ctx().copy_text(ed.buf.clone());
                }
                egui::Event::Cut => {
                    state.copied_text = ed.buf.clone();
                    state.copied_note = note.to_string();
                    ui.ctx().copy_text(ed.buf.clone());
                    ed.buf.clear();
                    ed.typed.clear();
                }
                egui::Event::Paste(text) => {
                    // Se l'incolla viene dalla nostra copia (stesso testo),
                    // ripristina anche la nota; altrimenti incolla il testo
                    // esterno senza nota.
                    let note = if text == state.copied_text {
                        state.copied_note.clone()
                    } else {
                        String::new()
                    };
                    ed.buf = text.clone();
                    ed.typed = text;
                    ed.paste_note = Some(note);
                }
                _ => {}
            }
        }
    } else if ed.had_focus {
        commit = true; // focus perso → conferma
    }

    // disegno cella in editing + caret
    ui.painter().rect_filled(cell, 0.0, g(SEL_BG));
    ui.painter().rect_stroke(
        cell,
        0.0,
        Stroke::new(1.0, g(FOCUS_BORDER)),
        egui::StrokeKind::Inside,
    );
    let trect = ui.painter().text(
        egui::pos2(cell.left() + 3.0, cell.center().y),
        Align2::LEFT_CENTER,
        &ed.buf,
        person_font(),
        crate::ui_style::text(),
    );
    let caret_x = (trect.right() + 1.0).min(cell.right() - 1.0);
    ui.painter().vline(
        caret_x,
        (cell.top() + 2.0)..=(cell.bottom() - 2.0),
        Stroke::new(1.0, crate::ui_style::text()),
    );

    if cancel {
        // scarta
    } else if commit {
        actions.push(commit_editing(app, &ed));
    } else {
        state.editing = Some(ed);
    }
}

/// Cella worker **a riposo** (non in modifica): hover, triangolo della nota,
/// colore del testo (ghost/nascosto/sovra-allocato), click sinistro che apre
/// l'editing e tasto destro che apre la nota (cella piena) o l'inserimento
/// multiplo (cella vuota).
#[allow(clippy::too_many_arguments)]
fn draw_cell_static(
    ui: &mut egui::Ui,
    app: &App,
    state: &mut UiState,
    actions: &mut Vec<Action>,
    cx: &CellCtx,
    cell: Rect,
    text: &str,
    note: &str,
) {
    let resp = ui.interact(
        cell,
        egui::Id::new(("cell", cx.proj.0, cx.dev.0, cx.week, cx.row)),
        Sense::click(),
    );
    let hovered = resp.hovered();
    let clicked = resp.clicked();
    let secondary = resp.secondary_clicked();

    if hovered {
        ui.painter()
            .rect_filled(cell, 0.0, g(SEL_BG).gamma_multiply(0.4));
    }
    if !note.is_empty() {
        draw_note_triangle(ui, cell);
    }
    if !text.is_empty() {
        let wname = text.split('|').next().unwrap_or("").trim();
        let wid = app.workers.get_id_by_name(wname);
        let hidden = wid.map_or(false, |id| app.workers.is_hidden_in_footer(id));
        let is_ghost = wid.map_or(false, |id| app.workers.is_ghost(id));
        let sovra = wid
            .map(|id| {
                app.sovra
                    .get(&(WeekId(cx.week as usize), id))
                    .map_or(0, |e| e.0 as i32)
            })
            .unwrap_or(0);
        let max_h = wid
            .map(|id| app.workers.get_effective_max_hours(id, cx.week as usize) as i32)
            .unwrap_or(DEFAULT_MAX_HOURS as i32);
        // worker "ghost" → sempre porpora (max effort di fatto 0),
        // anche se nascosti nel footer, così si distinguono dai
        // sovra-allocati (rossi); worker nascosti → grigi; oltre il max
        // → rossi; altrimenti colore testo normale.
        let color = if is_ghost {
            g(GHOST_PURPLE)
        } else if hidden {
            Color32::from_gray(0x80)
        } else if sovra > max_h {
            g(Color32::RED)
        } else {
            crate::ui_style::text()
        };
        paint_person_cell(ui, cell, text, color);
    }
    if !note.is_empty() {
        resp.on_hover_text(note.to_string());
    }

    if clicked {
        if let Some(old) = state.editing.take() {
            actions.push(commit_editing(app, &old));
        }
        let orig_worker = text.split('|').next().unwrap_or("").trim().to_string();
        state.editing = Some(Editing {
            proj: cx.proj,
            dev: cx.dev,
            week: cx.week,
            row: cx.row,
            buf: text.to_string(),
            typed: text.to_string(),
            just_opened: true,
            had_focus: false,
            paste_note: None,
            orig_worker,
            orig_note: note.to_string(),
        });
    }
    if secondary {
        if text.is_empty() {
            // Cella vuota → inserimento multiplo (worker + ore + settimane).
            if let Some(old) = state.editing.take() {
                actions.push(commit_editing(app, &old));
            }
            state.bulk_fill = Some(BulkFill {
                proj: cx.proj,
                dev: cx.dev,
                week: cx.week,
                search: String::new(),
                selected: HashSet::new(),
                effort_text: String::new(),
                weeks_text: "1".to_string(),
            });
            state.bulk_fill_was_open = false;
        } else {
            let wname = text.split('|').next().unwrap_or("").trim().to_string();
            state.note_editor = Some(NoteEditing {
                target: NoteTarget::Effort {
                    proj: cx.proj,
                    dev: cx.dev,
                    week: cx.week,
                    worker: wname,
                },
                text: note.to_string(),
            });
        }
    }
}

/// Tutte le voci (worker, effort, nota) della settimana, ignorando il filtro.
pub(crate) fn full_week_entries(
    app: &App,
    proj: ProjectId,
    dev: DevId,
    week: i32,
) -> Vec<(String, usize, String)> {
    app.projects
        .get_single_dev(proj, dev)
        .and_then(|sd| sd.get_all(WeekId(week as usize)))
        .map(|sew| {
            sew.worker_id
                .iter()
                .filter(|(wid, _)| **wid != WORKER_ID_ZERO)
                .map(|(wid, se)| {
                    (
                        app.workers.get_name_by_id(*wid).to_string(),
                        se.get_effort().0,
                        se.get_note(),
                    )
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Commit mirato: rimpiazza solo il worker della cella editata, preservando gli
/// altri worker della settimana (compresi quelli nascosti da un filtro attivo).
pub(crate) fn commit_editing(app: &App, ed: &Editing) -> Action {
    let mut entries = full_week_entries(app, ed.proj, ed.dev, ed.week);
    // rimuovi il worker originale della cella (sarà sostituito o svuotato)
    if !ed.orig_worker.is_empty() {
        entries.retain(|(n, _, _)| n != &ed.orig_worker);
    }
    // interpreta il nuovo contenuto "worker|effort"
    let parts: Vec<&str> = ed.buf.split('|').collect();
    if parts.len() == 2 {
        let wname = parts[0].trim().to_string();
        if !wname.is_empty() {
            let eff = parts[1].trim().parse::<usize>().unwrap_or(0);
            let note = ed.paste_note.clone().unwrap_or_else(|| {
                if wname == ed.orig_worker {
                    ed.orig_note.clone()
                } else {
                    String::new()
                }
            });
            entries.retain(|(n, _, _)| n != &wname);
            entries.push((wname, eff, note));
        }
    }
    let rows: Vec<String> = entries
        .iter()
        .map(|(n, e, _)| format!("{}|{}", n, e))
        .collect();
    let notes: Vec<String> = entries.iter().map(|(_, _, n)| n.clone()).collect();
    Action::CommitCell {
        proj: ed.proj,
        dev: ed.dev,
        week: WeekId(ed.week as usize),
        rows,
        notes,
    }
}

/// Disegna il testo "nome|effort" centrato nella cella, troncando il NOME con "…"
/// quando non entra ma mantenendo sempre visibile la parte "|effort".
pub(crate) fn paint_person_cell(ui: &egui::Ui, cell: Rect, text: &str, color: Color32) {
    let font = person_font();
    let avail = COL_W - 4.0;
    let measure = |s: &str| {
        ui.painter()
            .layout_no_wrap(s.to_string(), font.clone(), color)
            .size()
            .x
    };

    let display = if measure(text) <= avail {
        text.to_string()
    } else if let Some(p) = text.rfind('|') {
        let name = &text[..p];
        let suffix = &text[p..]; // "|effort"
        let mut keep = name.chars().count();
        loop {
            if keep == 0 {
                break format!("…{}", suffix);
            }
            let trimmed: String = name.chars().take(keep).collect();
            let cand = format!("{}…{}", trimmed, suffix);
            if measure(&cand) <= avail {
                break cand;
            }
            keep -= 1;
        }
    } else {
        let mut keep = text.chars().count();
        loop {
            if keep == 0 {
                break String::from("…");
            }
            let cand: String = text
                .chars()
                .take(keep)
                .chain(std::iter::once('…'))
                .collect();
            if measure(&cand) <= avail {
                break cand;
            }
            keep -= 1;
        }
    };

    let galley = ui.painter().layout_no_wrap(display, font, color);
    let pos = egui::pos2(
        cell.center().x - galley.size().x / 2.0,
        cell.center().y - galley.size().y / 2.0,
    );
    ui.painter().galley(pos, galley, color);
}

/// Triangolo di stato nell'angolo in alto a SINISTRA (ferie).
pub(crate) fn draw_status_triangle_top_left(ui: &egui::Ui, cell: Rect, color: Color32) {
    let x = cell.left() + STATUS_TRI_INSET;
    let y = cell.top() + STATUS_TRI_INSET;
    let pts = vec![
        egui::pos2(x, y),
        egui::pos2(x + 10.0, y),
        egui::pos2(x, y + 10.0),
    ];
    ui.painter()
        .add(egui::Shape::convex_polygon(pts, color, Stroke::NONE));
}

/// Triangolo di stato nell'angolo in basso a SINISTRA (malattia).
pub(crate) fn draw_status_triangle_bottom_left(ui: &egui::Ui, cell: Rect, color: Color32) {
    let x = cell.left() + STATUS_TRI_INSET;
    let y = cell.bottom() - STATUS_TRI_INSET;
    let pts = vec![
        egui::pos2(x, y),
        egui::pos2(x + 10.0, y),
        egui::pos2(x, y - 10.0),
    ];
    ui.painter()
        .add(egui::Shape::convex_polygon(pts, color, Stroke::NONE));
}

pub(crate) fn draw_note_triangle(ui: &egui::Ui, cell: Rect) {
    let tr = cell.right_top();
    let pts = vec![
        egui::pos2(tr.x - 10.0, tr.y),
        egui::pos2(tr.x, tr.y),
        egui::pos2(tr.x, tr.y + 10.0),
    ];
    ui.painter().add(egui::Shape::convex_polygon(
        pts,
        g(NOTE_ORANGE),
        Stroke::NONE,
    ));
}

pub(crate) fn left_column(
    ui: &mut egui::Ui,
    app: &App,
    state: &mut UiState,
    actions: &mut Vec<Action>,
    filter: &Filter,
    dev_filter: &DevFilter,
) {
    let compact = state.compact_mode;
    // stesso zoom della griglia: blocchi dev alti 2 righe quando mergiato.
    let merged = !compact && state.zoom_level > 0;
    let layout = project_layout(
        ui,
        app,
        filter,
        state.worker_filter_current_week,
        dev_filter,
        state.project_view,
        compact,
        merged,
    );
    let total_h = total_content_h(&layout);

    // Stessa altezza totale e stessa allocazione singola della griglia.
    let (rect, _) = ui.allocate_exact_size(Vec2::new(LEFT_W, total_h), Sense::hover());
    let left = rect.left();
    let mut y = rect.top();

    paint_hstrip(ui, left, LEFT_W, y, g(START_STOP));
    y += DEV_BORDER;

    for p in &layout {
        // Progetti chiusi in grigio (come la griglia), a prescindere dal B/N globale.
        set_bw_mode(state.bw_mode || app.projects.is_closed(p.proj));
        let proj_rect = Rect::from_min_size(egui::pos2(left, y), Vec2::new(LEFT_W, p.proj_h));
        draw_project_info(
            ui,
            proj_rect,
            app,
            state,
            actions,
            p.proj,
            &p.name,
            compact,
            filter.is_some() || dev_filter.is_some(),
        );
        draw_left_dev_strip(ui, proj_rect, p.proj, state);
        draw_left_devs(
            ui, proj_rect, app, state, actions, p.proj, &p.devs, compact, merged,
        );

        y += p.proj_h;
        paint_hstrip(ui, left, LEFT_W, y, BETWEEN_PROJECTS);
        y += DEV_BORDER;
    }
    // Ripristina il B/N globale dopo l'eventuale grigio dei progetti chiusi.
    set_bw_mode(state.bw_mode);
}

/// WeekId della settimana che contiene oggi, normalizzata al primo giorno della
/// settimana (come le etichette della griglia).
pub(crate) fn current_week_id() -> WeekId {
    let today = Utc::now().date_naive();
    WeekId(local_to_days(&primo_giorno_settimana_corrente(&today)) as usize)
}

/// Costruisce il popup delle note di progetto a partire dalla mappa salvata.
/// La settimana corrente è sempre in cima (con il testo esistente o vuoto), le
/// altre seguono per data decrescente (più recente prima).
pub(crate) fn make_project_notes_popup(proj: ProjectId, notes: &HashMap<WeekId, String>) -> Popup {
    let current = current_week_id();
    let mut weeks: Vec<(WeekId, String)> = notes
        .iter()
        .filter(|(w, _)| **w != current)
        .map(|(w, t)| (*w, t.clone()))
        .collect();
    weeks.sort_by(|a, b| b.0.cmp(&a.0));
    let cur_text = notes.get(&current).cloned().unwrap_or_default();
    weeks.insert(0, (current, cur_text));
    Popup::ProjectNotes {
        proj,
        weeks,
        current,
    }
}

pub(crate) fn draw_project_info(
    ui: &mut egui::Ui,
    rect: Rect,
    app: &App,
    state: &mut UiState,
    actions: &mut Vec<Action>,
    proj: ProjectId,
    proj_name: &str,
    compact: bool,
    filter_active: bool,
) {
    let x = rect.left();
    let w = LEFT_INFO_W;
    let mut y = rect.top();
    // larghezza dei due pulsanti sposta (▲▼) a destra della riga tripletta
    let btn_w = 16.0;

    // tripletta (right-click per modificare): centrata tra il bordo sinistro e i
    // pulsanti ▲▼ (quindi nello spazio largo `w - 2*btn_w`).
    let trip = app.projects.get_tripletta(proj);
    let trip_rect = Rect::from_min_size(egui::pos2(x, y), Vec2::new(w - 2.0 * btn_w, ROW_H));
    if trip.is_empty() {
        ui.painter().text(
            trip_rect.center(),
            Align2::CENTER_CENTER,
            "—",
            cell_font(),
            text_faint(),
        );
    } else {
        ui.painter().text(
            trip_rect.center(),
            Align2::CENTER_CENTER,
            &trip,
            cell_font(),
            g(EFFORT_ORANGE),
        );
    }
    let tr = ui.interact(trip_rect, egui::Id::new(("trip", proj.0)), Sense::click());
    // Tasto sinistro: modifica la tripletta.
    if tr.clicked() {
        state.popup = Some(Popup::Tripletta {
            proj,
            text: trip.clone(),
        });
    }
    // Tasto destro: note del progetto per settimana. La settimana corrente è in
    // cima, pronta da compilare; le altre seguono per data decrescente.
    if tr.secondary_clicked() {
        state.popup = Some(make_project_notes_popup(
            proj,
            &app.projects.get_notes(proj),
        ));
    }
    tr.on_hover_text("Tasto sinistro: modifica tripletta · Tasto destro: note progetto");

    // Con filtro worker attivo resta visibile solo la tripletta: niente pulsanti
    // sposta, categoria, nome, inizio/fine (così non crea spessore).
    if filter_active {
        return;
    }

    // Pulsanti sposta su/giù all'estrema destra della riga tripletta.
    let up_rect = Rect::from_min_size(egui::pos2(x + w - 2.0 * btn_w, y), Vec2::new(btn_w, ROW_H));
    let down_rect = Rect::from_min_size(egui::pos2(x + w - btn_w, y), Vec2::new(btn_w, ROW_H));
    if ui
        .put(up_rect, egui::Button::new("▲").frame(false).small())
        .on_hover_text("Sposta progetto su")
        .clicked()
    {
        actions.push(Action::MoveProjectUp { proj });
    }
    if ui
        .put(down_rect, egui::Button::new("▼").frame(false).small())
        .on_hover_text("Sposta progetto giù")
        .clicked()
    {
        actions.push(Action::MoveProjectDown { proj });
    }
    y += ROW_H;

    // categoria (click per scegliere) — nascosta in compatta
    if !compact {
        let cat = app
            .projects
            .get_category(proj)
            .and_then(|c| app.categories.get_name(c))
            .unwrap_or("—")
            .to_string();
        let cat_rect = Rect::from_min_size(egui::pos2(x, y), Vec2::new(w, ROW_H));
        let cat_col = if cat == "—" {
            text_faint()
        } else {
            g(CAT_BLUE)
        };
        ui.painter().text(
            cat_rect.center(),
            Align2::CENTER_CENTER,
            cat,
            cell_font(),
            cat_col,
        );
        let cr = ui.interact(cat_rect, egui::Id::new(("cat", proj.0)), Sense::click());
        if cr.clicked() {
            state.popup = Some(Popup::Category { proj });
        }
        y += ROW_H;
    }

    // nome progetto (editabile, multiriga). Padding sinistro per non incollarlo
    // al bordo della finestra.
    let pad = 6.0;
    let name_top = y;
    let name_h = NAME_ROWS as f32 * ROW_H;
    let name_w = w - pad;
    let name_rect = Rect::from_min_size(egui::pos2(x + pad, name_top), Vec2::new(name_w, name_h));
    let buf = state
        .name_buffers
        .entry(proj.0)
        .or_insert_with(|| proj_name.to_string());
    let resp = ui.put(
        name_rect,
        egui::TextEdit::multiline(buf)
            .font(cell_font())
            .text_color(text())
            .frame(egui::Frame::NONE)
            .margin(egui::Margin::ZERO)
            .desired_rows(NAME_ROWS)
            .desired_width(name_w),
    );
    if resp.lost_focus() {
        actions.push(Action::SetProjectName {
            proj,
            name: buf.clone(),
        });
    } else if !resp.has_focus() && *buf != proj_name {
        *buf = proj_name.to_string();
    }
    // inizio/fine vanno sotto il fondo REALE del campo (che può crescere su più righe)
    y = resp.rect.bottom().max(name_top + name_h);

    // inizio/fine nascosti in compatta
    if compact {
        return;
    }

    // inizio (right-click per modificare)
    let start_date = app
        .projects
        .get_project_start_week(proj)
        .map(|wk| {
            primo_giorno_settimana_corrente(&days_to_local(wk.0 as i32))
                .format("%y-%m-%d")
                .to_string()
        })
        .unwrap_or_default();
    let start_txt = if start_date.is_empty() {
        "Inizio: —".to_string()
    } else {
        format!("Inizio: {}", start_date)
    };
    let sr = Rect::from_min_size(egui::pos2(x, y), Vec2::new(w, ROW_H));
    ui.painter().text(
        sr.center(),
        Align2::CENTER_CENTER,
        start_txt,
        mono(FONT_SIZE - 2.0),
        text_dim(),
    );
    let srr = ui.interact(sr, egui::Id::new(("start", proj.0)), Sense::click());
    if srr.secondary_clicked() {
        state.popup = Some(Popup::Start {
            proj,
            text: start_date,
        });
    }
    srr.on_hover_text("Tasto destro: modifica data inizio");
    y += ROW_H;

    // fine (right-click per modificare)
    let end_date = app
        .projects
        .get_project_end_week(proj)
        .map(|wk| {
            primo_giorno_settimana_corrente(&days_to_local(wk.0 as i32))
                .format("%y-%m-%d")
                .to_string()
        })
        .unwrap_or_default();
    let end_txt = if end_date.is_empty() {
        "Fine: —".to_string()
    } else {
        format!("Fine: {}", end_date)
    };
    let er = Rect::from_min_size(egui::pos2(x, y), Vec2::new(w, ROW_H));
    ui.painter().text(
        er.center(),
        Align2::CENTER_CENTER,
        end_txt,
        mono(FONT_SIZE - 2.0),
        text_dim(),
    );
    let err = ui.interact(er, egui::Id::new(("end", proj.0)), Sense::click());
    if err.secondary_clicked() {
        state.popup = Some(Popup::End {
            proj,
            text: end_date,
        });
    }
    err.on_hover_text("Tasto destro: modifica deadline");
    y += ROW_H;

    // Avanzamento complessivo del progetto nel formato "presunta%/attuale%" — la
    // presunta è l'effort usato fino a oggi sul pianificato, l'attuale la media
    // delle % dichiarate pesata sul pianificato. Valore e numeri del tooltip
    // vengono dalla stessa breakdown. "—" se non c'è alcun pianificato.
    let (prog_txt, prog_tip) = match app
        .projects
        .project_progress_breakdown(proj, current_week_id())
    {
        Some((used, planned, wdecl)) => {
            let pres = (used * 100 + planned / 2) / planned; // presunta
            let act = (wdecl + planned / 2) / planned; // attuale (≤100)
            let done = (wdecl + 50) / 100; // ore "dichiarate completate" (≈)
            (
                format!("Avanz.: {pres}%/{act}%"),
                format!(
                    "Presunta {pres}% = usato {used}h / pianificato {planned}h\n\
                         Attuale {act}% = dichiarate ≈{done}h / pianificato {planned}h\n\
                         (pesate sull'effort pianificato di ogni dev)"
                ),
            )
        }
        None => (
            "Avanz.: —".to_string(),
            "Nessun effort pianificato: avanzamento non calcolabile".to_string(),
        ),
    };
    let pr = Rect::from_min_size(egui::pos2(x, y), Vec2::new(w, ROW_H));
    ui.painter().text(
        pr.center(),
        Align2::CENTER_CENTER,
        prog_txt,
        mono(FONT_SIZE - 2.0),
        text_dim(),
    );
    ui.interact(pr, egui::Id::new(("prog", proj.0)), Sense::hover())
        .on_hover_text(prog_tip);
}

pub(crate) fn draw_left_dev_strip(
    ui: &mut egui::Ui,
    rect: Rect,
    proj: ProjectId,
    state: &mut UiState,
) {
    let x = rect.left() + LEFT_INFO_W;
    let strip = Rect::from_min_size(
        egui::pos2(x, rect.top()),
        Vec2::new(DEV_STRIP_W, rect.height()),
    );
    let resp = ui.interact(strip, egui::Id::new(("devstrip", proj.0)), Sense::click());
    ui.painter()
        .rect_filled(strip, 0.0, strip_bg(resp.hovered()));
    ui.painter().rect_stroke(
        strip,
        0.0,
        Stroke::new(1.0, strip_border()),
        egui::StrokeKind::Inside,
    );
    // etichetta "Dev" verticale (lettere impilate)
    for (i, ch) in "Dev".chars().enumerate() {
        let cy = strip.top() + 10.0 + i as f32 * 11.0;
        if cy > strip.bottom() {
            break;
        }
        ui.painter().text(
            egui::pos2(strip.center().x, cy),
            Align2::CENTER_CENTER,
            ch.to_string(),
            mono(FONT_SIZE - 3.0),
            text_dim(),
        );
    }
    if resp.clicked() {
        state.dev_manage = Some(proj);
    }
    resp.on_hover_text("Aggiungi / rimuovi dev dal progetto");
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_left_devs(
    ui: &mut egui::Ui,
    rect: Rect,
    app: &App,
    state: &mut UiState,
    actions: &mut Vec<Action>,
    proj: ProjectId,
    devs: &[(DevId, usize)],
    compact: bool,
    merged: bool,
) {
    let x0 = rect.left() + LEFT_INFO_W + DEV_STRIP_W;
    let mut y = rect.top();

    for (dev, max_rows) in devs {
        let max_rows = *max_rows;
        let color = dev_color(app, *dev);
        let tcol = dev_text_color(app, *dev);
        // Se il dev contiene un worker "ghost" (con effort), il suo nome lampeggia
        // fra il colore normale e il porpora (~0.5s per fase) come segnale d'allarme.
        let has_ghost = app.projects.get_single_dev(proj, *dev).map_or(false, |sd| {
            sd.has_worker_matching(|id| app.workers.is_ghost(id))
        });
        let name_col = if has_ghost {
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(120));
            let t = ui.ctx().input(|i| i.time);
            if t.rem_euclid(1.0) < 0.5 {
                g(GHOST_PURPLE)
            } else {
                tcol
            }
        } else {
            tcol
        };
        let block_h = dev_block_height(max_rows, compact, merged);

        // bordo superiore
        let top_b = Rect::from_min_size(egui::pos2(x0, y), Vec2::new(LEFT_DEV_W, DEV_BORDER));
        ui.painter().rect_filled(top_b, 0.0, color);
        let inner_y = y + DEV_BORDER;
        let inner_h = dev_inner_h(max_rows, compact, merged);

        // cella nome dev (90px, doppio click = add row)
        let name_rect =
            Rect::from_min_size(egui::pos2(x0, inner_y), Vec2::new(DEV_NAME_W, inner_h));
        ui.painter().rect_filled(name_rect, 0.0, color);
        // % di effort usato fino a oggi sul pianificato (nascosta in compatta,
        // come gli altri campi effort; assente se il pianificato è 0).
        let used_pct = if compact {
            None
        } else {
            app.projects.get_single_dev(proj, *dev).and_then(|sd| {
                let planned = sd.planned_effort().0;
                (planned != 0).then(|| {
                    let used = sd.effort_up_to(current_week_id()).0;
                    (used * 100 + planned / 2) / planned
                })
            })
        };
        if compact {
            ui.painter().text(
                name_rect.center(),
                Align2::CENTER_CENTER,
                dev_name(app, *dev),
                cell_font(),
                name_col,
            );
        } else {
            // Nome allineato verticalmente con l'effort (riga 1) e percentuali
            // col residuo (riga 2), che stanno sempre alle prime due righe a
            // destra: presunta a sinistra, dichiarata (editabile) a destra.
            let eff_y = name_rect.top() + ROW_H * 0.5;
            let rem_y = name_rect.top() + ROW_H * 1.5;
            ui.painter().text(
                egui::pos2(name_rect.center().x, eff_y),
                Align2::CENTER_CENTER,
                dev_name(app, *dev),
                cell_font(),
                name_col,
            );
            if let Some(pct) = used_pct {
                // oltre il 100% = sforamento del pianificato → rosso.
                let pct_col = if pct > 100 { g(Color32::RED) } else { tcol };
                ui.painter().text(
                    egui::pos2(x0 + DEV_NAME_W * 0.27, rem_y),
                    Align2::CENTER_CENTER,
                    format!("{pct}%"),
                    cell_font(),
                    pct_col,
                );
            }
        }
        let dev_note = app
            .projects
            .get_single_dev(proj, *dev)
            .map(|sd| sd.get_note())
            .unwrap_or_default();
        let hide_effort = app
            .projects
            .get_single_dev(proj, *dev)
            .map(|sd| sd.get_hide_effort())
            .unwrap_or(false);
        if !dev_note.is_empty() {
            draw_note_triangle(ui, name_rect);
        }
        let nresp = ui.interact(
            name_rect,
            egui::Id::new(("devname", proj.0, dev.0)),
            Sense::click(),
        );
        if nresp.double_clicked() {
            actions.push(Action::AddRow { proj, dev: *dev });
        }
        nresp.context_menu(|ui| {
            if ui.button("Aggiungi riga").clicked() {
                actions.push(Action::AddRow { proj, dev: *dev });
                ui.close_menu();
            }
            if ui.button("Elimina riga").clicked() {
                actions.push(Action::DelRow { proj, dev: *dev });
                ui.close_menu();
            }
            if ui.button("Nota Dev…").clicked() {
                state.note_editor = Some(NoteEditing {
                    target: NoteTarget::Dev { proj, dev: *dev },
                    text: dev_note.clone(),
                });
                ui.close_menu();
            }
            ui.separator();
            let label = if hide_effort {
                "Visualizza effort"
            } else {
                "Nascondi effort"
            };
            if ui.button(label).clicked() {
                actions.push(Action::SetDevHideEffort {
                    proj,
                    dev: *dev,
                    hide: !hide_effort,
                });
                ui.close_menu();
            }
        });
        if !dev_note.is_empty() {
            nresp.on_hover_text(dev_note);
        }

        // Campo editabile "% dichiarata" dallo sviluppatore, a destra della
        // presunta e sotto il nome. Aggiunto DOPO l'interazione del nome così il
        // widget resta in primo piano (il click non innesca l'aggiunta riga).
        // Sfondo rosso se dichiarata < presunta, verde se ≥. Nascosto in compatta
        // e quando l'effort pianificato è 0 (`used_pct` è `Some` sse pianificato
        // ≠ 0): in tal caso il campo non compare e non è modificabile.
        if !compact && used_pct.is_some() {
            let declared = app
                .projects
                .get_single_dev(proj, *dev)
                .map(|sd| sd.declared_pct())
                .unwrap_or(0);
            let bg = used_pct.map(|presumed| {
                if (declared as usize) < presumed {
                    g(Color32::RED)
                } else {
                    g(ok_green())
                }
            });
            // Allineata verticalmente col residuo (riga 2), come la presunta.
            let decl_rect = Rect::from_center_size(
                egui::pos2(x0 + DEV_NAME_W * 0.72, name_rect.top() + ROW_H * 1.5),
                Vec2::new(DEV_NAME_W * 0.42, ROW_H - 2.0),
            );
            if let Some(bg) = bg {
                ui.painter().rect_filled(decl_rect, 2.0, bg);
            }
            let txt_col = if bg.is_some() { Color32::WHITE } else { tcol };
            let dbuf = state
                .declared_buffers
                .entry((proj.0, dev.0))
                .or_insert_with(|| declared.to_string());
            let resp = ui.put(
                decl_rect,
                egui::TextEdit::singleline(dbuf)
                    .font(cell_font())
                    .text_color(txt_col)
                    .frame(egui::Frame::NONE)
                    .horizontal_align(egui::Align::Center),
            );
            if resp.lost_focus() {
                if let Ok(v) = dbuf.trim().trim_end_matches('%').trim().parse::<u8>() {
                    actions.push(Action::SetDevDeclaredPct {
                        proj,
                        dev: *dev,
                        pct: v.min(100),
                    });
                }
            } else if !resp.has_focus() && *dbuf != declared.to_string() {
                *dbuf = declared.to_string();
            }
        }

        // area effort/remains (65px) — i campi sono nascosti in compatta
        let eff_x = x0 + DEV_NAME_W;
        let eff_area =
            Rect::from_min_size(egui::pos2(eff_x, inner_y), Vec2::new(DEV_EFFORT_W, inner_h));
        ui.painter().rect_filled(eff_area, 0.0, color);

        if !compact {
            // effort pianificato (editabile)
            let planned = app
                .projects
                .get_single_dev(proj, *dev)
                .map(|sd| sd.planned_effort().0)
                .unwrap_or(0);
            let eff_rect =
                Rect::from_min_size(egui::pos2(eff_x, inner_y), Vec2::new(DEV_EFFORT_W, ROW_H));
            let key = (proj.0, dev.0);
            let buf = state
                .effort_buffers
                .entry(key)
                .or_insert_with(|| planned.to_string());
            let resp = ui.put(
                eff_rect,
                egui::TextEdit::singleline(buf)
                    .font(cell_font())
                    .text_color(dev_text_color(app, *dev))
                    .frame(egui::Frame::NONE)
                    .horizontal_align(egui::Align::Center),
            );
            if resp.lost_focus() {
                if let Ok(v) = buf.trim().parse::<usize>() {
                    actions.push(Action::SetDevEffort {
                        proj,
                        dev: *dev,
                        effort: v,
                    });
                }
            } else if !resp.has_focus() && *buf != planned.to_string() {
                *buf = planned.to_string();
            }

            // remains
            let total = app
                .projects
                .get_single_dev(proj, *dev)
                .map(|sd| sd.get_effort_tot().0 as i32)
                .unwrap_or(0);
            let remains = planned as i32 - total;
            let rem_rect = Rect::from_min_size(
                egui::pos2(eff_x, inner_y + ROW_H),
                Vec2::new(DEV_EFFORT_W, ROW_H),
            );
            let rem_red = (remains == planned as i32 && planned != 0) || remains < 0;
            if rem_red {
                ui.painter().rect_filled(rem_rect, 0.0, g(Color32::RED));
            }
            // su sfondo rosso il testo è sempre bianco (a prescindere dal tema
            // di sistema), altrimenti segue il colore del dev.
            let rem_color = if rem_red {
                Color32::WHITE
            } else {
                dev_text_color(app, *dev)
            };
            ui.painter().text(
                rem_rect.center(),
                Align2::CENTER_CENTER,
                remains.to_string(),
                cell_font(),
                rem_color,
            );
        }

        // bordo inferiore
        let bot_b = Rect::from_min_size(
            egui::pos2(x0, inner_y + inner_h),
            Vec2::new(LEFT_DEV_W, DEV_BORDER),
        );
        ui.painter().rect_filled(bot_b, 0.0, color);

        y += block_h;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Filtro Ctrl+G: un progetto matcha solo se un worker selezionato ha effort
    // nella settimana corrente.
    #[test]
    fn current_week_filter_matches_only_current_week_worker() {
        let mut app = App::new();
        let alice = app.workers.add("Alice");
        let bob = app.workers.add("Bob");
        let dev = app.devs.add("Frontend");
        let wk = current_week_id();
        let other = WeekId(wk.0 + 7); // settimana successiva (WEEK_STEP)

        // P1: Alice lavora nella settimana corrente.
        let p1 = app.projects.add("P1", Some("AAA"), Some(wk));
        app.projects.add_dev(p1, dev);
        app.projects.add_effort(p1, dev, wk, alice, Effort(8));

        // P2: Bob lavora solo in un'altra settimana.
        let p2 = app.projects.add("P2", Some("BBB"), Some(wk));
        app.projects.add_dev(p2, dev);
        app.projects.add_effort(p2, dev, other, bob, Effort(8));

        let both: HashSet<String> = ["Alice".to_string(), "Bob".to_string()]
            .into_iter()
            .collect();
        assert!(project_worker_in_current_week(&app, p1, &both));
        assert!(!project_worker_in_current_week(&app, p2, &both));

        // Con solo Bob selezionato, P1 (Alice nella settimana corrente) non matcha.
        let only_bob: HashSet<String> = ["Bob".to_string()].into_iter().collect();
        assert!(!project_worker_in_current_week(&app, p1, &only_bob));

        // Anche con effort 0 il worker è assegnato nella settimana corrente → matcha.
        let p3 = app.projects.add("P3", Some("CCC"), Some(wk));
        app.projects.add_dev(p3, dev);
        app.projects.add_effort(p3, dev, wk, alice, Effort(0));
        assert!(project_worker_in_current_week(&app, p3, &both));
    }

    // Filtro Ctrl+D: è mostrato solo il dev selezionato, e solo se ha almeno un
    // worker assegnato nel progetto — anche a effort 0 (il dev aggiunto ma mai
    // compilato non fa comparire il progetto).
    #[test]
    fn dev_filter_shows_only_selected_devs_with_effort() {
        let mut app = App::new();
        let alice = app.workers.add("Alice");
        let front = app.devs.add("Frontend");
        let back = app.devs.add("Backend");
        let wk = current_week_id();

        // P1: Frontend con effort, Backend aggiunto ma vuoto.
        let p1 = app.projects.add("P1", Some("AAA"), Some(wk));
        app.projects.add_dev(p1, front);
        app.projects.add_dev(p1, back);
        app.projects.add_effort(p1, front, wk, alice, Effort(8));

        let only_front: DevFilter = Some([front].into_iter().collect());
        assert!(dev_shown(&app, p1, front, &only_front));
        assert!(!dev_shown(&app, p1, back, &only_front)); // non selezionato

        // Backend selezionato ma senza worker assegnati → nascosto (e P1 sparirebbe).
        let only_back: DevFilter = Some([back].into_iter().collect());
        assert!(!dev_shown(&app, p1, back, &only_back));
        assert!(!dev_shown(&app, p1, front, &only_back));

        // Un worker assegnato con effort 0 basta a mostrarlo.
        app.projects.add_effort(p1, back, wk, alice, Effort(0));
        assert!(dev_shown(&app, p1, back, &only_back));

        // Senza filtro nulla è nascosto.
        assert!(dev_shown(&app, p1, back, &None));
    }
}
