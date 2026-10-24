//! (estratto da `ui.rs` — vedi miglioramento #16)

use super::*;

/// Una colonna del cruscotto: anno di riferimento (riga sopra le date),
/// etichetta breve senza anno (`mm-dd` del lunedì, o `mm` per mese), etichetta
/// completa per il tooltip e settimane coperte.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct SatCol {
    pub year: i32,
    pub label: String,
    pub full: String,
    pub weeks: Vec<i32>,
}

/// Colonne del cruscotto: per settimana → una colonna per settimana; per mese →
/// le settimane raggruppate per mese (anno + mese del lunedì).
pub(crate) fn saturation_columns(app: &App, monthly: bool) -> Vec<SatCol> {
    let mut out: Vec<SatCol> = Vec::new();
    for w in weeks_vec(app) {
        let d = primo_giorno_settimana_corrente(&days_to_local(w));
        let year = d.year();
        if !monthly {
            out.push(SatCol {
                year,
                label: d.format("%m-%d").to_string(),
                full: week_label(w),
                weeks: vec![w],
            });
            continue;
        }
        let label = format!("{:02}", d.month());
        match out.last_mut() {
            Some(last) if last.year == year && last.label == label => last.weeks.push(w),
            _ => out.push(SatCol {
                year,
                full: format!("{:02}-{label}", year % 100),
                label,
                weeks: vec![w],
            }),
        }
    }
    out
}

/// Gruppi consecutivi di colonne con lo stesso anno: `(anno, prima, ultima)`
/// (indici inclusi), per la riga dell'anno sopra le date.
pub(crate) fn year_spans(cols: &[SatCol]) -> Vec<(i32, usize, usize)> {
    let mut out: Vec<(i32, usize, usize)> = Vec::new();
    for (i, c) in cols.iter().enumerate() {
        match out.last_mut() {
            Some(last) if last.0 == c.year => last.2 = i,
            _ => out.push((c.year, i, i)),
        }
    }
    out
}

/// Metriche di una cella: ore allocate e capacità (somma su tutte le settimane
/// del periodo). La capacità è il max ore effettivo della settimana: ferie e
/// malattia NON azzerano la capacità (si possono fare 2 giorni di ferie e
/// lavorare gli altri) — il conteggio orario resta quello reale.
pub(crate) fn saturation_cell(app: &App, wid: WorkerId, weeks: &[i32]) -> (i32, i32) {
    let mut alloc = 0;
    let mut cap = 0;
    for &w in weeks {
        alloc += app
            .sovra
            .get(&(WeekId(w as usize), wid))
            .map_or(0, |e| e.0 as i32);
        cap += app.workers.get_effective_max_hours(wid, w as usize) as i32;
    }
    (alloc, cap)
}

pub(crate) fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let m = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t) as u8;
    Color32::from_rgb(m(a.r(), b.r()), m(a.g(), b.g()), m(a.b(), b.b()))
}

/// Colori (sfondo, testo) di una cella heatmap in base alla saturazione.
pub(crate) fn heat_colors(alloc: i32, cap: i32) -> (Color32, Color32) {
    if alloc == 0 {
        return (bg(), text_dim()); // scarico
    }
    if alloc > cap {
        return (g(Color32::from_rgb(0xcc, 0x30, 0x30)), Color32::WHITE); // sovra
    }
    let r = if cap > 0 {
        alloc as f32 / cap as f32
    } else {
        1.0
    };
    // verde (poco carico) → giallo (quasi pieno)
    let c = lerp_color(
        Color32::from_rgb(0x2e, 0x7d, 0x32),
        Color32::from_rgb(0xc9, 0xa0, 0x00),
        r,
    );
    (g(c), Color32::WHITE)
}

/// Larghezza di una colonna della heatmap e passo tra colonne: fissi, così
/// l'intestazione (disegnata a parte, fuori dallo scroll verticale) resta
/// allineata colonna per colonna con le celle.
pub(crate) const SAT_CW: f32 = 32.0;
pub(crate) const SAT_GAP: f32 = 2.0;
/// Altezza di riga, uguale per colonna fissa e heatmap (righe allineate). Supera
/// con margine sia le celle (16px) sia il testo, così `min_row_height` fissa
/// ogni riga esattamente a `SAT_RH` in entrambe le griglie.
pub(crate) const SAT_RH: f32 = 22.0;
/// Spazio in più a destra dell'intestazione (vedi `draw_saturation_header`).
const SAT_HEAD_PAD: f32 = 24.0;
/// Intestazione: riga dell'anno + riga delle date.
const SAT_HEAD_H: f32 = SAT_RH * 2.0 + SAT_GAP;

/// Intestazione della heatmap (anno sopra, date sotto), larga quanto le
/// colonne. Disegnata in coordinate: `x` di colonna = `i · (SAT_CW + SAT_GAP)`,
/// lo stesso passo della griglia delle celle.
fn draw_saturation_header(
    ui: &mut egui::Ui,
    cols: &[SatCol],
    this_week: i32,
    hover_col: Option<usize>,
) {
    let pitch = SAT_CW + SAT_GAP;
    // Più lo spazio della barra verticale del corpo: senza, a fine scorrimento
    // l'intestazione (che non ha barra) arriverebbe prima del corpo e le date
    // non sarebbero più sopra le loro colonne.
    let width = (cols.len() as f32 * pitch - SAT_GAP).max(0.0) + SAT_HEAD_PAD;
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, SAT_HEAD_H), Sense::hover());
    let rect = rect.with_max_x(rect.right() - SAT_HEAD_PAD);
    let p = ui.painter();
    let small = egui::FontId::monospace(9.0);
    for (k, (year, from, to)) in year_spans(cols).into_iter().enumerate() {
        let r = Rect::from_min_max(
            egui::pos2(rect.left() + from as f32 * pitch, rect.top()),
            egui::pos2(rect.left() + to as f32 * pitch + SAT_CW, rect.top() + SAT_RH),
        );
        // anni alterni su due toni, così il passaggio d'anno si vede
        let fill = if k % 2 == 0 { row_even() } else { row_alt() };
        p.rect_filled(r, 2.0, fill);
        p.rect_stroke(r, 2.0, Stroke::new(0.5_f32, text_faint()), egui::StrokeKind::Inside);
        // l'anno resta leggibile anche quando il gruppo è in parte fuori vista:
        // centrato sulla parte visibile del gruppo
        let vis = r.intersect(ui.clip_rect());
        let at = if vis.width() > 0.0 { vis.center() } else { r.center() };
        p.text(
            at,
            Align2::CENTER_CENTER,
            year.to_string(),
            egui::FontId::monospace(11.0),
            text(),
        );
    }
    for (i, c) in cols.iter().enumerate() {
        let r = Rect::from_min_size(
            egui::pos2(rect.left() + i as f32 * pitch, rect.top() + SAT_RH + SAT_GAP),
            egui::vec2(SAT_CW, SAT_RH),
        );
        // Colonna della cella puntata: giallo grassetto su scuro, come nella
        // griglia; prevale sul chip verde della settimana corrente.
        let (col, passes): (Color32, &[f32]) = if hover_col == Some(i) {
            p.rect_filled(r, 2.0, hi_bg());
            (hi_text(), &[0.0, HI_BOLD_DX])
        } else if c.weeks.contains(&this_week) {
            p.rect_filled(r, 2.0, g(Color32::from_rgb(0x00, 0xe0, 0x4b)));
            (Color32::BLACK, &[0.0])
        } else {
            (text(), &[0.0])
        };
        for dx in passes {
            p.text(
                r.center() + egui::vec2(*dx, 0.0),
                Align2::CENTER_CENTER,
                &c.label,
                small.clone(),
                col,
            );
        }
    }
}

/// Cella colorata della heatmap. `current` evidenzia la settimana corrente con un
/// bordo verde; `selected` (riga worker selezionata) con un bordo arancione che
/// prevale, così tutta la riga risulta evidenziata; le altre hanno un bordo tenue.
pub(crate) fn heat_cell(
    ui: &mut egui::Ui,
    label: &str,
    bgc: Color32,
    fg: Color32,
    current: bool,
    selected: bool,
    tip: &str,
) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(SAT_CW, 16.0), Sense::click());
    ui.painter().rect_filled(rect, 2.0, bgc);
    // Riga selezionata → bordo arancione su tutte le celle (prevale). Altrimenti
    // settimana corrente → bordo verde; le altre un bordo tenue.
    let stroke = if selected {
        Stroke::new(2.0_f32, g(EFFORT_ORANGE))
    } else if current {
        Stroke::new(2.5_f32, g(Color32::from_rgb(0x00, 0xe0, 0x4b)))
    } else {
        Stroke::new(0.5_f32, text_faint())
    };
    ui.painter()
        .rect_stroke(rect, 2.0, stroke, egui::StrokeKind::Inside);
    if !label.is_empty() {
        ui.painter()
            .text(rect.center(), Align2::CENTER_CENTER, label, mono(8.5), fg);
    }
    resp.on_hover_text(tip)
        .on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Finestra "Saturazione worker": heatmap worker × (settimana|mese), colorata per
/// saturazione (allocato / capacità). Mostra solo i worker visibili nel footer.
pub(crate) fn saturation_window(ctx: &egui::Context, app: &App, state: &mut UiState) {
    if !state.show_saturation {
        return;
    }
    let this_week = state.this_week;
    // Worker mostrati nel cruscotto: quelli con `show_in_find` attivo OPPURE non
    // nascosti nel footer. Non dipende dal filtro worker (Ctrl+F).
    let workers: Vec<(WorkerId, String)> = app
        .workers
        .list()
        .into_iter()
        .filter(|(id, _)| {
            app.workers.is_shown_in_find(*id) || !app.workers.is_hidden_in_footer(*id)
        })
        .collect();

    let mut columns = saturation_columns(app, state.saturation_monthly);
    if state.saturation_future_only {
        // Tieni la settimana corrente e quelle successive.
        columns.retain(|c| c.weeks.iter().any(|&w| w >= this_week));
    }

    // (allocato, capacità) per worker × colonna, calcolati una volta per frame:
    // li leggono riepilogo, colonna Σ e celle.
    let cells: Vec<Vec<(i32, i32)>> = workers
        .iter()
        .map(|(wid, _)| {
            columns
                .iter()
                .map(|c| saturation_cell(app, *wid, &c.weeks))
                .collect()
        })
        .collect();

    // Riepilogo (sovra-allocazioni e ore in eccesso) sui dati mostrati.
    let mut over_n = 0;
    let mut total_excess = 0;
    for row in &cells {
        for &(a, cap) in row {
            if a > cap && a > 0 {
                over_n += 1;
                total_excess += a - cap;
            }
        }
    }

    let mut open = true;
    let mut jump_week: Option<i32> = None;
    // Evidenziazione worker + settimana della cella puntata (frame precedente).
    let hover = state.sat_hover;
    let mut hover_next: Option<(WorkerId, usize)> = None;

    egui::Window::new("Saturazione worker")
        .collapsible(true)
        // Ridimensionabile in larghezza e altezza, fino a tutto lo schermo: le
        // aree di scorrimento riempiono lo spazio disponibile.
        .resizable(true)
        .default_width(900.0)
        .default_height(560.0)
        .default_pos(egui::pos2(80.0, 60.0))
        .open(&mut open)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("Vista:");
                ui.selectable_value(&mut state.saturation_monthly, false, "Settimana");
                ui.selectable_value(&mut state.saturation_monthly, true, "Mese");
                ui.separator();
                ui.checkbox(
                    &mut state.saturation_future_only,
                    "Solo da settimana corrente",
                );
            });
            ui.label(format!(
                "Sovra-allocazioni: {over_n}  ·  Ore in eccesso totali: {total_excess}"
            ));
            ui.separator();
            ui.label(
                "Heatmap (allocato / capacità): verde=libero, giallo=pieno, rosso=oltre; \
                 bordo verde = settimana corrente",
            );

            // Larghezza della colonna dei nomi, misurata sul nome più lungo: la
            // stessa per l'intestazione fissa e per le righe, così «Σ» resta
            // incolonnato.
            let mono = egui::TextStyle::Monospace.resolve(ui.style());
            let pad = ui.spacing().button_padding.x;
            let name_w = workers
                .iter()
                .map(|(_, n)| {
                    ui.painter()
                        .layout_no_wrap(n.clone(), mono.clone(), text())
                        .size()
                        .x
                })
                .fold(0.0_f32, f32::max)
                + 2.0 * pad
                + 2.0;
            const LEFT_GAP: f32 = 8.0;
            // Corpo (sotto le intestazioni) alto quanto resta nella finestra.
            let body_h = (ui.available_height() - SAT_HEAD_H - ui.spacing().item_spacing.y)
                .max(SAT_RH * 3.0);

            ui.horizontal_top(|ui| {
                // ── Colonna fissa: nome worker + Σ (all/cap). Non scorre in
                //    orizzontale; scorre in verticale insieme alla heatmap (offset
                //    condiviso, barra nascosta). Click sul nome = seleziona/deseleziona
                //    la riga effort (multi-selezione).
                ui.vertical(|ui| {
                    // intestazione fissa: riga anno vuota + «Σ all/cap»
                    let (hr, _) = ui.allocate_exact_size(
                        egui::vec2(name_w + LEFT_GAP + 120.0, SAT_HEAD_H),
                        Sense::hover(),
                    );
                    ui.painter().text(
                        egui::pos2(hr.left() + name_w + LEFT_GAP, hr.bottom() - SAT_RH / 2.0),
                        Align2::LEFT_CENTER,
                        "Σ  all/cap",
                        egui::FontId::new(mono.size, egui::FontFamily::Proportional),
                        ui.visuals().strong_text_color(),
                    );
                    egui::ScrollArea::vertical()
                        .id_salt("sat_left")
                        .max_height(body_h)
                        .auto_shrink([true, false])
                        .scroll_bar_visibility(
                            egui::scroll_area::ScrollBarVisibility::AlwaysHidden,
                        )
                        .vertical_scroll_offset(state.sat_scroll_y)
                        .show(ui, |ui| {
                            egui::Grid::new("sat_left_grid")
                                .min_row_height(SAT_RH)
                                .min_col_width(name_w)
                                .spacing(egui::vec2(LEFT_GAP, SAT_GAP))
                                .show(ui, |ui| {
                                    // Nomi cliccabili senza padding verticale extra:
                                    // così la loro altezza non supera `SAT_RH`.
                                    ui.spacing_mut().button_padding.y = 0.0;
                                    for ((wid, wname), row) in workers.iter().zip(&cells) {
                                        let mut tot_a = 0;
                                        let mut tot_c = 0;
                                        let mut over_w = 0;
                                        for &(a, cap) in row {
                                            tot_a += a;
                                            tot_c += cap;
                                            if a > cap {
                                                over_w += 1;
                                            }
                                        }
                                        // Nome cliccabile: toggle della selezione riga.
                                        let sel = state.sat_selected.contains(wid);
                                        let name_resp = ui
                                            .selectable_label(
                                                sel,
                                                egui::RichText::new(wname).monospace(),
                                            )
                                            .on_hover_text("Click: seleziona/deseleziona la riga");
                                        // Worker della cella puntata: nome in giallo
                                        // grassetto su scuro, ridisegnato sopra.
                                        if hover.is_some_and(|(w, _)| w == *wid) {
                                            let r = name_resp.rect;
                                            ui.painter().rect_filled(r, 2.0, hi_bg());
                                            for dx in [0.0, HI_BOLD_DX] {
                                                ui.painter().text(
                                                    r.left_center() + egui::vec2(pad + dx, 0.0),
                                                    Align2::LEFT_CENTER,
                                                    wname,
                                                    mono.clone(),
                                                    hi_text(),
                                                );
                                            }
                                        }
                                        if name_resp.clicked() {
                                            if sel {
                                                state.sat_selected.remove(wid);
                                            } else {
                                                state.sat_selected.insert(*wid);
                                            }
                                        }
                                        let tcol = if over_w > 0 {
                                            g(Color32::from_rgb(0xcc, 0x30, 0x30))
                                        } else {
                                            text()
                                        };
                                        // Anche la cella Σ risalta quando la riga è
                                        // selezionata (stesso sfondo del nome).
                                        let mut sigma_rt = egui::RichText::new(format!(
                                            "{tot_a}/{tot_c}  ({over_w}⚠)"
                                        ))
                                        .color(tcol)
                                        .monospace();
                                        if state.sat_selected.contains(wid) {
                                            sigma_rt = sigma_rt.background_color(
                                                ui.visuals().selection.bg_fill,
                                            );
                                        }
                                        ui.label(sigma_rt);
                                        ui.end_row();
                                    }
                                });
                        });
                });

                // ── Heatmap: l'intestazione (anno + date) è fuori dallo scroll
                //    verticale, quindi resta sempre visibile; segue lo scroll
                //    orizzontale del corpo. Il corpo guida entrambi gli offset.
                ui.vertical(|ui| {
                    egui::ScrollArea::horizontal()
                        .id_salt("sat_head")
                        .auto_shrink([false, true])
                        .scroll_bar_visibility(
                            egui::scroll_area::ScrollBarVisibility::AlwaysHidden,
                        )
                        .horizontal_scroll_offset(state.sat_scroll_x)
                        .show(ui, |ui| {
                            draw_saturation_header(ui, &columns, this_week, hover.map(|h| h.1))
                        });
                    let out = egui::ScrollArea::both()
                        .scroll_source(drag_scroll())
                        .id_salt("sat_heat")
                        .max_height(body_h)
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            egui::Grid::new("sat_grid")
                                .min_row_height(SAT_RH)
                                .min_col_width(SAT_CW)
                                .spacing(egui::vec2(SAT_GAP, SAT_GAP))
                                .show(ui, |ui| {
                                    for ((wid, wname), row) in workers.iter().zip(&cells) {
                                        let sel = state.sat_selected.contains(wid);
                                        for (ci, c) in columns.iter().enumerate() {
                                            let (alloc, cap) = row[ci];
                                            let (bgc, fg) = heat_colors(alloc, cap);
                                            let is_cur = c.weeks.contains(&this_week);
                                            let txt = if alloc == 0 {
                                                String::new()
                                            } else {
                                                alloc.to_string()
                                            };
                                            let tip =
                                                format!("{wname} · {}\n{alloc} / {cap} h", c.full);
                                            let cell =
                                                heat_cell(ui, &txt, bgc, fg, is_cur, sel, &tip);
                                            if cell.hovered() {
                                                hover_next = Some((*wid, ci));
                                            }
                                            if cell.clicked() {
                                                jump_week = Some(c.weeks[0]);
                                            }
                                        }
                                        ui.end_row();
                                    }
                                });
                        });
                    // Il corpo comanda: colonna fissa e intestazione seguono.
                    let offset = out.state.clamped_offset();
                    state.sat_scroll_y = offset.y;
                    state.sat_scroll_x = offset.x;
                });
            });
        });

    // Click su una cella → scrolla la griglia a quella settimana.
    if let Some(w) = jump_week {
        let level = if state.compact_mode {
            0
        } else {
            state.zoom_level
        };
        let cols = columns_vec(app, level);
        let cw = col_w(state.compact_mode);
        if let Some(idx) = cols.iter().position(|c| c.contains_week(w)) {
            state.pending_scroll_x = Some((col_x_offset(&cols, idx, cw) - 80.0).max(0.0));
        }
    }
    if hover_next != state.sat_hover {
        state.sat_hover = hover_next;
        ctx.request_repaint();
    }
    if !open {
        state.show_saturation = false;
        state.sat_hover = None;
    }
}

// ── Corpo: colonna sinistra + griglia con scroll sincronizzato ──────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn texts(out: &egui::FullOutput) -> Vec<(String, egui::Pos2)> {
        out.shapes
            .iter()
            .filter_map(|cs| match &cs.shape {
                egui::epaint::Shape::Text(t) => {
                    Some((t.galley.text().to_string(), t.visual_bounding_rect().center()))
                }
                _ => None,
            })
            .collect()
    }

    #[test]
    fn labels_drop_the_year_which_goes_in_its_own_row() {
        let app = App::new();
        let weekly = saturation_columns(&app, false);
        let first = &weekly[0];
        let d = primo_giorno_settimana_corrente(&days_to_local(first.weeks[0]));
        assert_eq!(first.label, d.format("%m-%d").to_string());
        assert_eq!(first.full, d.format("%y-%m-%d").to_string());
        assert_eq!(first.year, d.year());
        assert!(weekly.iter().all(|c| c.label.len() == 5), "solo mm-dd");

        let monthly = saturation_columns(&app, true);
        assert!(monthly.iter().all(|c| c.label.len() == 2), "solo mm");
        // stesso mese di anni diversi = colonne diverse
        let spans = year_spans(&monthly);
        assert!(
            spans
                .windows(2)
                .all(|w| w[0].0 != w[1].0 && w[0].2 + 1 == w[1].1)
        );
        assert_eq!(spans.last().unwrap().2, monthly.len() - 1);
    }

    #[test]
    fn header_stays_visible_and_aligned_when_scrolled_down() {
        let mut app = App::new();
        let cols = saturation_columns(&app, false);
        let w0 = cols[0].weeks[0];
        let mut last = None;
        for i in 0..60 {
            let wid = app.workers.add(&format!("W{i:02}"));
            last = Some(wid);
        }
        // valore riconoscibile sulla prima colonna dell'ultimo worker (in fondo)
        app.sovra
            .insert((WeekId(w0 as usize), last.unwrap()), Effort(13));

        let ctx = egui::Context::default();
        let screen = screen(1200.0, 700.0);
        let mut state = UiState {
            show_saturation: true,
            this_week: -1,
            ..Default::default()
        };
        let frame = |state: &mut UiState, events: Vec<egui::Event>| {
            let input = egui::RawInput {
                screen_rect: Some(screen),
                events,
                ..Default::default()
            };
            ctx.run_ui_test(input, |ui| saturation_window(ui.ctx(), &app, state))
        };
        let _ = frame(&mut state, vec![]); // prima passata: dimensionamento
        let first = texts(&frame(&mut state, vec![]));
        let date_at = |t: &[(String, egui::Pos2)]| {
            t.iter().find(|(s, _)| *s == cols[0].label).map(|(_, p)| *p)
        };
        let top = date_at(&first).expect("data della prima colonna");
        assert!(first.iter().any(|(s, _)| *s == cols[0].year.to_string()), "riga dell'anno");

        // scorre in fondo con la rotella sopra la heatmap
        let over = top + egui::vec2(0.0, 200.0);
        let mut out = None;
        for _ in 0..6 {
            out = Some(frame(
                &mut state,
                vec![
                    egui::Event::PointerMoved(over),
                    egui::Event::MouseWheel {
                        unit: egui::MouseWheelUnit::Point,
                        delta: egui::vec2(0.0, -400.0),
                        phase: egui::TouchPhase::Move,
                        source: egui::MouseWheelSource::Wheel,
                        modifiers: Default::default(),
                    },
                ],
            ));
        }
        for _ in 0..30 {
            out = Some(frame(&mut state, vec![egui::Event::PointerMoved(over)]));
        }
        assert!(state.sat_scroll_y > 0.0, "il corpo deve essere scorso");
        let t = texts(&out.unwrap());
        let now = date_at(&t).expect("la data resta disegnata");
        assert!((now.y - top.y).abs() < 0.5, "la data resta ferma: {top:?} → {now:?}");
        let cell = t
            .iter()
            .find(|(s, _)| s == "13")
            .map(|(_, p)| *p)
            .expect("cella dell'ultimo worker visibile in fondo");
        assert!((cell.x - now.x).abs() < 1.0, "data {now:?} sopra la cella {cell:?}");
        let name = t
            .iter()
            .find(|(s, _)| s == "W59")
            .map(|(_, p)| *p)
            .expect("nome dell'ultimo worker");
        assert!((name.y - cell.y).abs() < 1.0, "riga del nome {name:?} ≠ cella {cell:?}");
    }

    #[test]
    fn hovering_a_cell_highlights_its_worker_and_week() {
        let mut app = App::new();
        let cols = saturation_columns(&app, false);
        let ci = 3;
        let w = cols[ci].weeks[0];
        let anna = app.workers.add("Anna");
        app.workers.add("Bruno");
        app.sovra.insert((WeekId(w as usize), anna), Effort(13));

        let ctx = egui::Context::default();
        let screen = screen(1200.0, 700.0);
        let mut state = UiState {
            show_saturation: true,
            this_week: -1,
            ..Default::default()
        };
        let frame = |state: &mut UiState, events: Vec<egui::Event>| {
            let input = egui::RawInput {
                screen_rect: Some(screen),
                events,
                ..Default::default()
            };
            ctx.run_ui_test(input, |ui| saturation_window(ui.ctx(), &app, state))
        };
        // testi disegnati in giallo evidenziazione
        let yellow = |out: &egui::FullOutput| -> Vec<String> {
            out.shapes
                .iter()
                .filter_map(|cs| match &cs.shape {
                    egui::epaint::Shape::Text(t)
                        if t.fallback_color == hi_text()
                            || t.galley.job.sections.iter().any(|s| s.format.color == hi_text()) =>
                    {
                        Some(t.galley.text().to_string())
                    }
                    _ => None,
                })
                .collect()
        };
        let _ = frame(&mut state, vec![]);
        let out = frame(&mut state, vec![]);
        assert!(yellow(&out).is_empty(), "niente evidenziato senza cursore");
        let cell = texts(&out)
            .into_iter()
            .find(|(s, _)| s == "13")
            .map(|(_, p)| p)
            .expect("cella di Anna");

        let mut out = None;
        for _ in 0..3 {
            out = Some(frame(&mut state, vec![egui::Event::PointerMoved(cell)]));
        }
        assert_eq!(state.sat_hover, Some((anna, ci)));
        let y = yellow(&out.unwrap());
        assert!(y.contains(&cols[ci].label), "data evidenziata: {y:?}");
        assert!(y.contains(&"Anna".to_string()), "nome evidenziato: {y:?}");
        assert!(!y.contains(&"Bruno".to_string()));

        // cursore fuori: l'evidenziazione sparisce
        let _ = frame(&mut state, vec![egui::Event::PointerMoved(egui::pos2(1190.0, 690.0))]);
        assert_eq!(state.sat_hover, None);
    }
}
