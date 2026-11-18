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
        // griglia. La settimana corrente ha il bordo verde, dopo il ciclo.
        let (col, passes): (Color32, &[f32]) = if hover_col == Some(i) {
            p.rect_filled(r, 2.0, hi_bg());
            (hi_text(), &[0.0, HI_BOLD_DX])
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
    // Settimana corrente: bordo verde come nella griglia, chiuso in alto sulla
    // riga delle date; nel corpo continua fino all'ultimo worker.
    if let Some(i) = cols.iter().position(|c| c.weeks.contains(&this_week)) {
        let col = sat_col_rect(rect.left(), i, rect.top() + SAT_RH + SAT_GAP, rect.bottom());
        paint_this_week_border_w(ui, col, true, false, THIS_WEEK_BORDER_THIN_W);
    }
}

/// Colonna `i` della heatmap fra `top` e `bottom` (asse che parte da `left`),
/// allargata di metà spazio per lato: il bordo della settimana corrente
/// abbraccia le celle invece di coprirne il contenuto.
fn sat_col_rect(left: f32, i: usize, top: f32, bottom: f32) -> Rect {
    let x = left + i as f32 * (SAT_CW + SAT_GAP);
    Rect::from_x_y_ranges(x - SAT_GAP / 2.0..=x + SAT_CW + SAT_GAP / 2.0, top..=bottom)
}

/// Cella colorata della heatmap. `selected` (riga worker selezionata) ha un
/// bordo arancione, così tutta la riga risulta evidenziata; le altre hanno un
/// bordo tenue. La settimana corrente è il bordo verde della colonna, disegnato
/// a parte (`paint_this_week_border`).
pub(crate) fn heat_cell(
    ui: &mut egui::Ui,
    label: &str,
    bgc: Color32,
    fg: Color32,
    selected: bool,
    tip: &str,
) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(SAT_CW, 16.0), Sense::click());
    ui.painter().rect_filled(rect, 2.0, bgc);
    // Riga selezionata → bordo arancione su tutte le celle; le altre un bordo
    // tenue.
    let stroke = if selected {
        Stroke::new(2.0_f32, g(EFFORT_ORANGE))
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
/// Worker × colonna × (allocato, capacità) del cruscotto, come lo mostra la
/// finestra: worker con `show_in_find` attivo OPPURE non nascosti nel footer
/// (non dipende dal filtro Ctrl+F), colonne per settimana o mese, solo dalla
/// settimana corrente se `future_only`. Condiviso con l'export CSV.
#[allow(clippy::type_complexity)]
pub(crate) fn saturation_view(
    app: &App,
    monthly: bool,
    future_only: bool,
    this_week: i32,
) -> (Vec<(WorkerId, String)>, Vec<SatCol>, Vec<Vec<(i32, i32)>>) {
    let workers: Vec<(WorkerId, String)> = app
        .workers
        .list()
        .into_iter()
        .filter(|(id, _)| {
            app.workers.is_shown_in_find(*id) || !app.workers.is_hidden_in_footer(*id)
        })
        .collect();
    let mut columns = saturation_columns(app, monthly);
    if future_only {
        // Tieni la settimana corrente e quelle successive.
        columns.retain(|c| c.weeks.iter().any(|&w| w >= this_week));
    }
    let cells = workers
        .iter()
        .map(|(wid, _)| {
            columns
                .iter()
                .map(|c| saturation_cell(app, *wid, &c.weeks))
                .collect()
        })
        .collect();
    (workers, columns, cells)
}

/// CSV del cruscotto (vedi `view_csv::saturation_csv`) e numero di righe.
pub(crate) fn saturation_export_csv(
    app: &App,
    monthly: bool,
    future_only: bool,
    this_week: i32,
) -> (String, usize) {
    let (workers, columns, cells) = saturation_view(app, monthly, future_only, this_week);
    let mut out = Vec::new();
    for ((_, name), row) in workers.iter().zip(&cells) {
        for (c, &(alloc, cap)) in columns.iter().zip(row) {
            out.push(crate::view_csv::SatCell {
                worker: name,
                first_week: c.weeks.first().copied().unwrap_or_default(),
                monthly,
                alloc,
                cap,
            });
        }
    }
    crate::view_csv::saturation_csv(&out)
}

pub(crate) fn saturation_window(
    ctx: &egui::Context,
    app: &App,
    state: &mut UiState,
    actions: &mut Vec<Action>,
) {
    if !state.show_saturation {
        return;
    }
    let this_week = state.this_week;
    let (workers, columns, cells) = saturation_view(
        app,
        state.saturation_monthly,
        state.saturation_future_only,
        this_week,
    );

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

    let mut jump_week: Option<i32> = None;
    // Evidenziazione worker + settimana della cella puntata (frame precedente).
    let hover = state.sat_hover;
    let mut hover_next: Option<(WorkerId, usize)> = None;

    // Finestra del sistema, ridimensionabile: le aree di scorrimento
    // riempiono lo spazio disponibile.
    let open = os_window(ctx, "saturation_window", "Saturazione worker", egui::vec2(900.0, 560.0), |ui| {
            ui.horizontal(|ui| {
                ui.label("Vista:");
                ui.selectable_value(&mut state.saturation_monthly, false, "Settimana");
                ui.selectable_value(&mut state.saturation_monthly, true, "Mese");
                ui.separator();
                ui.checkbox(
                    &mut state.saturation_future_only,
                    "Solo da settimana corrente",
                );
                ui.separator();
                if ui
                    .add_enabled(!workers.is_empty() && !columns.is_empty(), egui::Button::new("Esporta CSV…"))
                    .on_hover_text("Una riga per worker e colonna (settimana o mese, come la vista): ore allocate, capacità, % e ore oltre.")
                    .clicked()
                {
                    actions.push(Action::ExportSaturationCsv);
                }
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
                            let grid = egui::Grid::new("sat_grid")
                                .min_row_height(SAT_RH)
                                .min_col_width(SAT_CW)
                                .spacing(egui::vec2(SAT_GAP, SAT_GAP))
                                .show(ui, |ui| {
                                    for ((wid, wname), row) in workers.iter().zip(&cells) {
                                        let sel = state.sat_selected.contains(wid);
                                        for (ci, c) in columns.iter().enumerate() {
                                            let (alloc, cap) = row[ci];
                                            let (bgc, fg) = heat_colors(alloc, cap);
                                            let txt = if alloc == 0 {
                                                String::new()
                                            } else {
                                                alloc.to_string()
                                            };
                                            let tip =
                                                format!("{wname} · {}\n{alloc} / {cap} h", c.full);
                                            let cell =
                                                heat_cell(ui, &txt, bgc, fg, sel, &tip);
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
                            // Settimana corrente: bordo verde su tutta la
                            // colonna, chiuso in fondo (in alto è chiuso
                            // nell'intestazione).
                            let r = grid.response.rect;
                            if let Some(i) =
                                columns.iter().position(|c| c.weeks.contains(&this_week))
                            {
                                let col = sat_col_rect(r.left(), i, r.top(), r.bottom());
                                paint_this_week_border_w(ui, col, false, true, THIS_WEEK_BORDER_THIN_W);
                            }
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
            ctx.run_ui_test(input, |ui| saturation_window(ui.ctx(), &app, state, &mut Vec::new()))
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

    /// Settimana corrente: bordo verde come nella griglia, sulla stessa
    /// colonna in intestazione (chiuso in alto) e nel corpo (chiuso in fondo);
    /// le celle non hanno più un bordo verde proprio.
    #[test]
    fn current_week_has_the_green_border_in_header_and_body() {
        let mut app = App::new();
        app.workers.add("Anna");
        app.workers.add("Bruno");
        let cols = saturation_columns(&app, false);
        let ctx = egui::Context::default();
        let mut state = UiState {
            show_saturation: true,
            saturation_future_only: false,
            this_week: cols[2].weeks[0],
            ..Default::default()
        };
        let input = egui::RawInput {
            screen_rect: Some(screen(1200.0, 700.0)),
            ..Default::default()
        };
        let mut out = None;
        for _ in 0..3 {
            out = Some(ctx.run_ui_test(input.clone(), |ui| saturation_window(ui.ctx(), &app, &mut state, &mut Vec::new())));
        }
        let out = out.unwrap();
        // La finestra appare in dissolvenza: si confronta l'RGB non premoltiplicato.
        let rgb = |c: Color32| {
            let [r, g, b, a] = c.to_srgba_unmultiplied();
            (a > 0).then_some([r, g, b])
        };
        let near = |c: Color32, want: Color32| {
            rgb(c).is_some_and(|x| {
                let w = [want.r(), want.g(), want.b()];
                x.iter().zip(w).all(|(a, b)| a.abs_diff(b) <= 3)
            })
        };
        let green: Vec<Rect> = out
            .shapes
            .iter()
            .filter_map(|cs| match &cs.shape {
                egui::epaint::Shape::Rect(r) if near(r.fill, this_week_border()) => Some(r.rect),
                _ => None,
            })
            .collect();
        assert_eq!(green.len(), 6, "2 lati + chiusura, in testa e nel corpo: {green:?}");
        let lefts: Vec<f32> = green.iter().map(|r| r.left()).collect();
        let min = lefts.iter().cloned().fold(f32::MAX, f32::min);
        let max = green.iter().map(|r| r.right()).fold(f32::MIN, f32::max);
        assert!((max - min - (SAT_CW + SAT_GAP)).abs() < 0.5, "una colonna: {}", max - min);
        let old_green = Color32::from_rgb(0x00, 0xe0, 0x4b);
        assert!(!out.shapes.iter().any(|cs| match &cs.shape {
            egui::epaint::Shape::Rect(r) => near(r.stroke.color, old_green),
            _ => false,
        }), "niente più bordo verde sulle singole celle");
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
            ctx.run_ui_test(input, |ui| saturation_window(ui.ctx(), &app, state, &mut Vec::new()))
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

    #[test]
    fn csv_button_asks_for_the_csv_and_the_csv_follows_the_view() {
        let mut app = App::new();
        let mario = app.workers.add("Mario");
        let cols = saturation_columns(&app, false);
        let w0 = cols[0].weeks[0];
        app.sovra.insert((WeekId(w0 as usize), mario), Effort(13));
        let mut state = UiState {
            show_saturation: true,
            this_week: -1,
            ..Default::default()
        };
        let ctx = egui::Context::default();
        let input = egui::RawInput {
            screen_rect: Some(screen(1200.0, 700.0)),
            ..Default::default()
        };
        let mut actions = Vec::new();
        let mut button = None;
        for _ in 0..2 {
            let out = ctx.run_ui_test(input.clone(), |ui| {
                saturation_window(ui.ctx(), &app, &mut state, &mut actions)
            });
            button = text_pos(&out, "Esporta CSV…");
        }
        for ev in click_events(button.expect("Esporta CSV…"), egui::PointerButton::Primary) {
            let mut i = input.clone();
            i.events = vec![ev];
            let _ = ctx.run_ui_test(i, |ui| {
                saturation_window(ui.ctx(), &app, &mut state, &mut actions)
            });
        }
        assert!(matches!(actions.as_slice(), [Action::ExportSaturationCsv]), "{}", actions.len());
        // Una riga per worker × colonna; per mese le colonne sono meno.
        let (csv, n) = saturation_export_csv(&app, false, false, -1);
        assert_eq!(n, cols.len());
        assert!(csv.contains(";13;"), "ore allocate della prima settimana");
        let (_, monthly) = saturation_export_csv(&app, true, false, -1);
        assert_eq!(monthly, saturation_columns(&app, true).len());
    }
}
