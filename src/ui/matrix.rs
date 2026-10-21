//! Matrice progetti × worker (Vista ▸ Matrice progetti/worker…): prima una
//! dialog per scegliere i worker (la scelta resta per la sessione), poi la
//! finestra con l'intervallo di date e la matrice. I dati vengono da
//! `crate::worker_matrix`, condivisi con l'export PDF.

use super::*;
use crate::worker_matrix::{MatrixData, category_color, closed_colors, hatch_segments};

/// Giorno odierno (locale), nello stesso formato di `WeekId` (giorni).
fn today_days() -> i32 {
    local_to_days(&chrono::Local::now().date_naive())
}

pub(crate) fn days_to_jiff(days: i32) -> jiff::civil::Date {
    let d = days_to_local(days);
    jiff::civil::Date::new(d.year() as i16, d.month() as i8, d.day() as i8)
        .unwrap_or_else(|_| today_jiff())
}

pub(crate) fn jiff_to_days(d: jiff::civil::Date) -> Option<i32> {
    chrono::NaiveDate::from_ymd_opt(d.year() as i32, d.month() as u32, d.day() as u32)
        .map(|n| local_to_days(&n))
}

/// Intervallo `[1 gennaio dell'anno più vecchio, 31 dicembre del più recente]`
/// (giorni) degli anni scelti nella combo; `None` se nessun anno è scelto.
fn years_range(years: &std::collections::BTreeSet<i32>) -> Option<(i32, i32)> {
    let (first, last) = (years.first()?, years.last()?);
    let from = chrono::NaiveDate::from_ymd_opt(*first, 1, 1)?;
    let to = chrono::NaiveDate::from_ymd_opt(*last, 12, 31)?;
    Some((local_to_days(&from), local_to_days(&to)))
}

/// Combo degli anni (anni di inizio/fine dei progetti, come il filtro Anni):
/// scegliendone uno o più, Da/A diventano 1/1 del più vecchio → 31/12 del più
/// recente. Ritorna `true` se la selezione è cambiata.
fn years_combo(ui: &mut egui::Ui, app: &App, years: &mut std::collections::BTreeSet<i32>) -> bool {
    let mut changed = false;
    let text = if years.is_empty() {
        "Anni: —".to_string()
    } else {
        let list: Vec<String> = years.iter().map(i32::to_string).collect();
        format!("Anni: {}", list.join(", "))
    };
    egui::ComboBox::from_id_salt("matrix_years")
        .selected_text(text)
        .width(MX_YEARS_W)
        .show_ui(ui, |ui| {
            let all = available_years(app);
            if all.is_empty() {
                ui.label("Nessun progetto con date di inizio/fine.");
            }
            for y in all {
                let mut on = years.contains(&y);
                if ui.checkbox(&mut on, y.to_string()).changed() {
                    if on {
                        years.insert(y);
                    } else {
                        years.remove(&y);
                    }
                    changed = true;
                }
            }
            if !years.is_empty() {
                ui.separator();
                if ui.button("Deseleziona tutti").clicked() {
                    years.clear();
                    changed = true;
                }
            }
        })
        .response
        .on_hover_text(
            "Imposta Da/A dal 1° gennaio dell'anno più vecchio scelto al 31 dicembre \
             del più recente.",
        );
    changed
}

/// Combo delle categorie (prima «Senza categoria», poi le categorie per id):
/// con almeno una scelta la matrice mostra solo i progetti di quelle categorie.
fn categories_combo(
    ui: &mut egui::Ui,
    app: &App,
    cats: &mut std::collections::BTreeSet<Option<CategoryId>>,
) {
    let all: Vec<(Option<CategoryId>, String)> = std::iter::once((None, category_name(app, None)))
        .chain(
            app.categories
                .list()
                .into_iter()
                .map(|(id, n)| (Some(id), n)),
        )
        .collect();
    let text = if cats.is_empty() {
        "Categorie: tutte".to_string()
    } else {
        let names: Vec<&str> = all
            .iter()
            .filter(|(c, _)| cats.contains(c))
            .map(|(_, n)| n.as_str())
            .collect();
        format!("Categorie: {}", names.join(", "))
    };
    egui::ComboBox::from_id_salt("matrix_categories")
        .selected_text(text)
        .width(MX_YEARS_W)
        .show_ui(ui, |ui| {
            for (c, name) in &all {
                let mut on = cats.contains(c);
                if ui
                    .checkbox(
                        &mut on,
                        egui::RichText::new(name)
                            .background_color(category_fill(*c, false))
                            .color(Color32::BLACK),
                    )
                    .changed()
                {
                    if on {
                        cats.insert(*c);
                    } else {
                        cats.remove(c);
                    }
                }
            }
            if !cats.is_empty() {
                ui.separator();
                if ui.button("Tutte").clicked() {
                    cats.clear();
                }
            }
        })
        .response
        .on_hover_text("Nessuna scelta = tutte le categorie.");
}

/// Combo degli stati dei progetti (Attivi / On-Hold / Closed): con almeno una
/// scelta la matrice mostra solo i progetti in quegli stati, e il riepilogo
/// conta solo quelli (vuoto = tutti).
fn statuses_combo(ui: &mut egui::Ui, statuses: &mut std::collections::BTreeSet<ProjectStatus>) {
    let text = if statuses.is_empty() || statuses.len() == ProjectStatus::ALL.len() {
        "Stato: tutti".to_string()
    } else {
        let names: Vec<&str> = ProjectStatus::ALL
            .iter()
            .enumerate()
            .filter(|(_, s)| statuses.contains(s))
            .map(|(i, _)| STATUS_LABELS[i])
            .collect();
        format!("Stato: {}", names.join(", "))
    };
    egui::ComboBox::from_id_salt("matrix_statuses")
        .selected_text(text)
        .width(MX_STATUS_W)
        .show_ui(ui, |ui| {
            for (i, s) in ProjectStatus::ALL.iter().enumerate() {
                let mut on = statuses.contains(s);
                if ui.checkbox(&mut on, STATUS_LABELS[i]).changed() {
                    if on {
                        statuses.insert(*s);
                    } else {
                        statuses.remove(s);
                    }
                }
            }
            // Tutti spuntati = nessun filtro.
            if statuses.len() == ProjectStatus::ALL.len() {
                statuses.clear();
            }
            if !statuses.is_empty() {
                ui.separator();
                if ui.button("Tutti").clicked() {
                    statuses.clear();
                }
            }
        })
        .response
        .on_hover_text("Nessuna scelta = tutti gli stati.");
}

/// Estremo dell'intervallo: «Oggi» oppure una data scelta col calendario.
/// `value`: `None` = oggi. Ritorna `true` se l'utente l'ha cambiato.
fn range_end_picker(ui: &mut egui::Ui, label: &str, id: &str, value: &mut Option<i32>) -> bool {
    let before = *value;
    ui.label(egui::RichText::new(label).strong());
    if ui.radio(value.is_none(), "Oggi").clicked() {
        *value = None;
    }
    if ui.radio(value.is_some(), "Data:").clicked() && value.is_none() {
        *value = Some(today_days());
    }
    let mut date = days_to_jiff(value.unwrap_or_else(today_days));
    ui.add_enabled_ui(value.is_some(), |ui| {
        let r = ui.add(
            egui_extras::DatePickerButton::new(&mut date)
                .id_salt(id)
                .format("%Y-%m-%d"),
        );
        if r.changed() {
            if let Some(d) = jiff_to_days(date) {
                *value = Some(d);
            }
        }
    });
    *value != before
}

/// Worker proponibili nella dialog di selezione: quelli con `show_in_find`
/// (come la colonna Workers dei filtri).
fn matrix_candidates(app: &App) -> Vec<(WorkerId, String)> {
    app.workers
        .list()
        .into_iter()
        .filter(|(id, _)| *id != WORKER_ID_ZERO && app.workers.is_shown_in_find(*id))
        .collect()
}

/// Dialog di scelta dei worker da mostrare nella matrice. La selezione vive in
/// `UiState.matrix_workers` e resta valida finché l'app è aperta.
pub(crate) fn matrix_select_window(ctx: &egui::Context, app: &App, state: &mut UiState) {
    if !state.matrix_select_open {
        return;
    }
    let candidates = matrix_candidates(app);
    let q = state.matrix_worker_search.trim().to_lowercase();
    let listed: Vec<&(WorkerId, String)> = candidates
        .iter()
        .filter(|(_, n)| q.is_empty() || n.to_lowercase().contains(&q))
        .collect();

    let mut open = true;
    let mut show = false;
    let mut cancel = false;
    egui::Window::new("Matrice progetti × worker — scegli i worker")
        .collapsible(false)
        .resizable(false)
        .default_pos(egui::pos2(120.0, 80.0))
        .open(&mut open)
        .show(ctx, |ui| {
            ui.add(
                egui::TextEdit::singleline(&mut state.matrix_worker_search)
                    .hint_text("Cerca worker…")
                    .desired_width(240.0),
            );
            let all = !listed.is_empty()
                && listed
                    .iter()
                    .all(|(id, _)| state.matrix_workers.contains(id));
            if let Some(on) = select_all_checkbox(ui, all) {
                for (id, _) in &listed {
                    if on {
                        state.matrix_workers.insert(*id);
                    } else {
                        state.matrix_workers.remove(id);
                    }
                }
            }
            ui.separator();
            egui::ScrollArea::vertical()
                .max_height(360.0)
                .show(ui, |ui| {
                    for (id, name) in &listed {
                        let mut on = state.matrix_workers.contains(id);
                        if ui.checkbox(&mut on, name).changed() {
                            if on {
                                state.matrix_workers.insert(*id);
                            } else {
                                state.matrix_workers.remove(id);
                            }
                        }
                    }
                });
            ui.separator();
            let n = candidates
                .iter()
                .filter(|(id, _)| state.matrix_workers.contains(id))
                .count();
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(n > 0, egui::Button::new(format!("Mostra matrice ({n})")))
                    .clicked()
                {
                    show = true;
                }
                if ui.button("Annulla").clicked() {
                    cancel = true;
                }
            });
        });
    if show {
        state.show_matrix = true;
    }
    if show || cancel || !open {
        state.matrix_select_open = false;
    }
}

// Geometria della matrice a schermo (px).
const MX_LABEL_W: f32 = 144.0;
const MX_COL_W: f32 = 84.0;
const MX_HEAD_H: f32 = 24.0;
const MX_GROUP_H: f32 = 20.0;
const MX_LINE_H: f32 = 15.0;
const MX_PAD: f32 = 4.0;
const MX_SUM_GAP: f32 = 4.0;
const MX_SUM_H: f32 = 20.0;
/// Altezza delle sotto-righe per stato (Attivi / On-Hold / Closed).
const MX_SUB_H: f32 = 16.0;
const MX_YEARS_W: f32 = 180.0;
const MX_STATUS_W: f32 = 150.0;

fn mx_row_h(lines: usize) -> f32 {
    lines.max(1) as f32 * MX_LINE_H + 2.0 * MX_PAD
}

/// Tronca `s` (con ellissi) finché non entra in `max_w` px col font dato.
fn fit_text(ui: &egui::Ui, s: &str, font: &egui::FontId, max_w: f32) -> String {
    let w = |t: &str| {
        ui.painter()
            .layout_no_wrap(t.to_string(), font.clone(), Color32::BLACK)
            .size()
            .x
    };
    if w(s) <= max_w {
        return s.to_string();
    }
    let chars: Vec<char> = s.chars().collect();
    let mut n = chars.len();
    while n > 0 {
        let t: String = chars[..n].iter().collect::<String>() + "…";
        if w(&t) <= max_w {
            return t;
        }
        n -= 1;
    }
    "…".to_string()
}

/// `0xRRGGBB` → colore egui (passato per il filtro B/N).
fn rgb_color(c: u32) -> Color32 {
    g(Color32::from_rgb((c >> 16) as u8, (c >> 8) as u8, c as u8))
}

fn category_name(app: &App, cat: Option<CategoryId>) -> String {
    cat.and_then(|c| app.categories.get_name(c))
        .map(str::to_string)
        .unwrap_or_else(|| "Senza categoria".to_string())
}

/// Tinta di sfondo di una categoria (piena per l'intestazione del gruppo,
/// schiarita per le righe dei progetti).
fn category_fill(cat: Option<CategoryId>, light: bool) -> Color32 {
    let c = category_color(cat);
    let (r, gr, b) = ((c >> 16) as u8, (c >> 8) as u8, c as u8);
    let mix = |v: u8| {
        if light {
            (255.0 - (255.0 - v as f32) * 0.45) as u8
        } else {
            v
        }
    };
    g(Color32::from_rgb(mix(r), mix(gr), mix(b)))
}

/// Cosa c'è sotto il puntatore nella matrice: `row` = indice in `rows` (righe
/// progetto e di riepilogo, `(top, altezza)`; le intestazioni di categoria non
/// contano), `col` = indice del worker. Sopra la riga dei worker (`head_bottom`,
/// fissa in cima) c'è solo la colonna; sulla colonna etichette solo la riga.
#[derive(Debug, Default, PartialEq)]
struct MxHit {
    row: Option<usize>,
    col: Option<usize>,
}

fn mx_hit(pos: egui::Pos2, x0: f32, n: usize, head_bottom: f32, rows: &[(f32, f32)]) -> MxHit {
    let rel = pos.x - x0 - MX_LABEL_W;
    let col = (rel >= 0.0 && rel < n as f32 * MX_COL_W).then(|| (rel / MX_COL_W) as usize);
    if pos.y < head_bottom {
        return MxHit { row: None, col };
    }
    let row = rows.iter().position(|(top, h)| pos.y >= *top && pos.y < top + h);
    match row {
        // Fuori da ogni riga (intestazione di categoria, spazio fra matrice e
        // riepilogo): niente.
        None => MxHit::default(),
        Some(_) => MxHit { row, col },
    }
}

/// Velo dell'evidenziazione al passaggio del mouse (cella, etichetta della
/// riga e colonna del worker). Sulla cella si somma a quello della colonna,
/// quindi risulta più marcata. Scuro nel tema chiaro, chiaro nel tema scuro;
/// va **sotto** i testi (vedi gli slot in `draw_matrix`), così non li spegne.
fn mx_veil() -> Color32 {
    if is_dark() {
        Color32::from_white_alpha(40)
    } else {
        Color32::from_black_alpha(36)
    }
}

/// Etichetta (tripletta o intestazione del riepilogo) e valore del riepilogo
/// evidenziati: scritta gialla su fondo scuro, uguale nei due temi (le righe
/// dei progetti hanno lo sfondo pastello anche col tema scuro, e il giallo su
/// fondo chiaro non si leggerebbe).
fn mx_hi_bg() -> Color32 {
    g(Color32::from_gray(40))
}

fn mx_hi_text() -> Color32 {
    g(Color32::from_rgb(255, 221, 0))
}

/// Scarto della seconda passata del grassetto simulato dei testi evidenziati.
const MX_BOLD_DX: f32 = 0.7;

/// Disegna la matrice (righe progetto raggruppate per categoria) e, sotto, il
/// riepilogo dei progetti per categoria di ogni worker.
fn draw_matrix(ui: &mut egui::Ui, app: &App, m: &MatrixData) {
    let n = m.workers.len();
    let width = MX_LABEL_W + n as f32 * MX_COL_W;
    let mut groups = 0;
    let mut prev: Option<Option<CategoryId>> = None;
    let mut body_h = 0.0;
    for r in &m.rows {
        if prev != Some(r.category) {
            groups += 1;
            prev = Some(r.category);
        }
        body_h += mx_row_h(r.cells.iter().map(Vec::len).max().unwrap_or(0));
    }
    // categorie + totale, ciascuno con le 3 sotto-righe per stato
    let subs = m.statuses.len() as f32;
    let sum_h = MX_SUM_GAP + (m.categories.len() + 1) as f32 * (MX_SUM_H + subs * MX_SUB_H);
    let height = MX_HEAD_H + groups as f32 * MX_GROUP_H + body_h + sum_h;
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, height), Sense::hover());
    let p = ui.painter().clone();
    let x0 = rect.left();
    let col_x = |i: usize| x0 + MX_LABEL_W + i as f32 * MX_COL_W;
    let border = Stroke::new(1.0_f32, strip_border());
    let bold = egui::FontId::proportional(13.0);
    let font = egui::FontId::proportional(12.5);

    let header = |y: f32, label: &str| {
        let r = Rect::from_min_size(egui::pos2(x0, y), egui::vec2(width, MX_HEAD_H));
        p.rect_filled(r, 0.0, strip_bg(false));
        p.text(
            egui::pos2(x0 + MX_PAD, y + MX_HEAD_H / 2.0),
            Align2::LEFT_CENTER,
            label,
            bold.clone(),
            text(),
        );
        for (i, (_, name)) in m.workers.iter().enumerate() {
            p.text(
                egui::pos2(col_x(i) + MX_COL_W / 2.0, y + MX_HEAD_H / 2.0),
                Align2::CENTER_CENTER,
                fit_text(ui, name, &bold, MX_COL_W - 2.0 * MX_PAD),
                bold.clone(),
                text(),
            );
        }
        p.hline(x0..=x0 + width, y + MX_HEAD_H, border);
    };
    let vlines = |y0: f32, y1: f32| {
        for x in [x0, x0 + MX_LABEL_W].into_iter().chain((1..=n).map(col_x)) {
            p.vline(x, y0..=y1, border);
        }
    };

    // Evidenziazione al passaggio del mouse: per ogni fascia orizzontale si
    // riserva uno slot nella lista delle forme **dopo** lo sfondo e **prima**
    // dei testi; a fine disegno, noto cosa è puntato, lo slot riceve il velo
    // (o il fondo scuro dell'evidenziazione). Le intestazioni di categoria
    // (`Group`) non sono puntabili ma la colonna le attraversa.
    #[derive(Clone, Copy, PartialEq)]
    enum Band {
        Group,
        Project,
        Summary,
    }
    let slots = std::cell::RefCell::new(Vec::<(egui::layers::ShapeIdx, f32, f32, Band)>::new());
    let reserve = |top: f32, h: f32, kind: Band| -> usize {
        let mut v = slots.borrow_mut();
        v.push((p.add(egui::Shape::Noop), top, h, kind));
        v.len() - 1
    };
    // Testi che cambiano colore se evidenziati: anche loro uno slot, riempito
    // a fine disegno. `col` = None per l'etichetta a sinistra della fascia.
    struct HiText {
        idx: egui::layers::ShapeIdx,
        band: usize,
        col: Option<usize>,
        pos: egui::Pos2,
        align: Align2,
        text: String,
        font: egui::FontId,
        color: Color32,
    }
    let texts = std::cell::RefCell::new(Vec::<HiText>::new());
    // Restituisce la larghezza del testo (per posizionare ciò che lo segue).
    let put = |band: usize,
               col: Option<usize>,
               pos: egui::Pos2,
               align: Align2,
               text: String,
               font: &egui::FontId,
               color: Color32|
     -> f32 {
        let w = p.layout_no_wrap(text.clone(), font.clone(), color).size().x;
        texts.borrow_mut().push(HiText {
            idx: p.add(egui::Shape::Noop),
            band,
            col,
            pos,
            align,
            text,
            font: font.clone(),
            color,
        });
        w
    };
    let mut y = rect.top();
    // L'intestazione dei worker si ridisegna in fondo, fissa in cima all'area
    // visibile; qui serve solo a riservarne lo spazio.
    y += MX_HEAD_H;

    let mut prev: Option<Option<CategoryId>> = None;
    for r in &m.rows {
        if prev != Some(r.category) {
            prev = Some(r.category);
            let gr = Rect::from_min_size(egui::pos2(x0, y), egui::vec2(width, MX_GROUP_H));
            p.rect_filled(gr, 0.0, category_fill(r.category, false));
            reserve(y, MX_GROUP_H, Band::Group);
            p.text(
                egui::pos2(x0 + MX_PAD, y + MX_GROUP_H / 2.0),
                Align2::LEFT_CENTER,
                category_name(app, r.category),
                bold.clone(),
                Color32::BLACK,
            );
            p.hline(x0..=x0 + width, y + MX_GROUP_H, border);
            y += MX_GROUP_H;
        }
        let h = mx_row_h(r.cells.iter().map(Vec::len).max().unwrap_or(0));
        let rr = Rect::from_min_size(egui::pos2(x0, y), egui::vec2(width, h));
        // Progetto chiuso: tinta della categoria spenta verso il grigio, a strisce.
        let fg = if r.closed {
            let (fill, hatch) = closed_colors(r.category);
            p.rect_filled(rr, 0.0, rgb_color(fill));
            let hatch = Stroke::new(1.0_f32, rgb_color(hatch));
            for (ax, ay, bx, by) in hatch_segments(rr.left(), rr.right(), rr.top(), rr.bottom(), 8.0)
            {
                p.line_segment([egui::pos2(ax, ay), egui::pos2(bx, by)], hatch);
            }
            Color32::from_gray(0x50)
        } else {
            p.rect_filled(rr, 0.0, category_fill(r.category, true));
            Color32::BLACK
        };
        let band = reserve(y, h, Band::Project);

        // Etichetta: solo la tripletta (il nome è nel tooltip), più «chiuso».
        let ty = y + MX_PAD + MX_LINE_H / 2.0;
        let trip_max = MX_LABEL_W - 2.0 * MX_PAD - if r.closed { 50.0 } else { 0.0 };
        let trip_w = put(
            band,
            None,
            egui::pos2(x0 + MX_PAD, ty),
            Align2::LEFT_CENTER,
            fit_text(ui, &r.tripletta, &bold, trip_max),
            &bold,
            fg,
        );
        if r.closed {
            put(
                band,
                None,
                egui::pos2(x0 + MX_PAD + trip_w + 6.0, ty),
                Align2::LEFT_CENTER,
                closed_tag(r.status).to_string(),
                &font,
                fg,
            );
        }
        let label_rect = Rect::from_min_size(egui::pos2(x0, y), egui::vec2(MX_LABEL_W, h));
        let tip = if r.closed {
            format!("{} ({})\n{}", r.tripletta, closed_tag(r.status), r.name)
        } else {
            format!("{}\n{}", r.tripletta, r.name)
        };
        ui.interact(
            label_rect,
            ui.id().with(("mx_row", r.proj.0)),
            Sense::hover(),
        )
        .on_hover_text(tip);

        for (i, devs) in r.cells.iter().enumerate() {
            for (l, dev) in devs.iter().enumerate() {
                put(
                    band,
                    Some(i),
                    egui::pos2(col_x(i) + MX_COL_W / 2.0, ty + l as f32 * MX_LINE_H),
                    Align2::CENTER_CENTER,
                    fit_text(ui, dev, &font, MX_COL_W - 2.0 * MX_PAD),
                    &font,
                    fg,
                );
            }
        }
        p.hline(x0..=x0 + width, y + h, border);
        vlines(y, y + h);
        y += h;
    }

    // Riepilogo, fuori dalla matrice: progetti per categoria di ogni worker,
    // ciascuna seguita dalle sotto-righe per stato; poi il totale, idem.
    let status_rows = |y: f32, counts: &dyn Fn(usize) -> [u32; 3]| -> f32 {
        let mut y = y;
        // Solo gli stati mostrati (filtro «Stato»).
        for &si in &m.statuses {
            let label = STATUS_LABELS[si];
            let band = reserve(y, MX_SUB_H, Band::Summary);
            put(
                band,
                None,
                egui::pos2(x0 + MX_PAD + 12.0, y + MX_SUB_H / 2.0),
                Align2::LEFT_CENTER,
                label.to_string(),
                &font,
                text(),
            );
            for wi in 0..n {
                put(
                    band,
                    Some(wi),
                    egui::pos2(col_x(wi) + MX_COL_W / 2.0, y + MX_SUB_H / 2.0),
                    Align2::CENTER_CENTER,
                    counts(wi)[si].to_string(),
                    &font,
                    text(),
                );
            }
            p.hline(x0..=x0 + width, y + MX_SUB_H, Stroke::new(0.5_f32, strip_border()));
            vlines(y, y + MX_SUB_H);
            y += MX_SUB_H;
        }
        y
    };
    y += MX_SUM_GAP;
    for (ci, cat) in m.categories.iter().enumerate() {
        let lr = Rect::from_min_size(egui::pos2(x0, y), egui::vec2(MX_LABEL_W, MX_SUM_H));
        p.rect_filled(lr, 0.0, category_fill(*cat, false));
        let band = reserve(y, MX_SUM_H, Band::Summary);
        put(
            band,
            None,
            egui::pos2(x0 + MX_PAD, y + MX_SUM_H / 2.0),
            Align2::LEFT_CENTER,
            fit_text(ui, &category_name(app, *cat), &bold, MX_LABEL_W - 2.0 * MX_PAD),
            &bold,
            Color32::BLACK,
        );
        for (wi, cnt) in m.summary[ci].iter().enumerate() {
            if *cnt > 0 {
                put(
                    band,
                    Some(wi),
                    egui::pos2(col_x(wi) + MX_COL_W / 2.0, y + MX_SUM_H / 2.0),
                    Align2::CENTER_CENTER,
                    cnt.to_string(),
                    &bold,
                    text(),
                );
            }
        }
        p.hline(x0..=x0 + width, y + MX_SUM_H, border);
        vlines(y, y + MX_SUM_H);
        y += MX_SUM_H;
        y = status_rows(y, &|wi| m.summary_status[ci][wi]);
    }
    // Totale: progetti (tutte le categorie) in cui è coinvolto il worker.
    let tr = Rect::from_min_size(egui::pos2(x0, y), egui::vec2(width, MX_SUM_H));
    p.rect_filled(tr, 0.0, strip_bg(false));
    let band = reserve(y, MX_SUM_H, Band::Summary);
    put(
        band,
        None,
        egui::pos2(x0 + MX_PAD, y + MX_SUM_H / 2.0),
        Align2::LEFT_CENTER,
        "Totale progetti".to_string(),
        &bold,
        text(),
    );
    for (wi, cnt) in m.totals.iter().enumerate() {
        put(
            band,
            Some(wi),
            egui::pos2(col_x(wi) + MX_COL_W / 2.0, y + MX_SUM_H / 2.0),
            Align2::CENTER_CENTER,
            cnt.to_string(),
            &bold,
            text(),
        );
    }
    p.hline(x0..=x0 + width, y + MX_SUM_H, border);
    vlines(y, y + MX_SUM_H);
    y += MX_SUM_H;
    status_rows(y, &|wi| m.status_totals[wi]);

    // Riga dei worker sempre visibile: in cima all'area visibile (o al suo
    // posto, se la matrice non è scorsa). Scorre in orizzontale con le colonne.
    let head_y = ui.clip_rect().top().max(rect.top());
    let head_bottom = head_y + MX_HEAD_H;

    // Evidenziazione: velo su tutta la colonna del worker; etichetta della
    // riga (tripletta o intestazione del riepilogo) e contenuto della cella
    // puntata (dev o valore) in giallo grassetto su fondo scuro.
    let slots = slots.into_inner();
    let pointable: Vec<usize> = (0..slots.len()).filter(|&i| slots[i].3 != Band::Group).collect();
    let rows: Vec<(f32, f32)> = pointable.iter().map(|&i| (slots[i].1, slots[i].2)).collect();
    let visible = ui.clip_rect().intersect(rect);
    let hit = ui
        .input(|i| i.pointer.hover_pos())
        .filter(|_| ui.rect_contains_pointer(visible))
        .map(|pos| mx_hit(pos, x0, n, head_bottom, &rows))
        .unwrap_or_default();
    let veil = mx_veil();
    let hit_band = hit.row.map(|r| pointable[r]);
    for (bi, (idx, top, h, _)) in slots.iter().enumerate() {
        let mut fills = Vec::new();
        let cell_of = |c: usize| {
            Rect::from_min_size(egui::pos2(col_x(c), *top), egui::vec2(MX_COL_W, *h))
        };
        if let Some(c) = hit.col {
            fills.push(egui::Shape::rect_filled(cell_of(c), 0.0, veil));
        }
        if hit_band == Some(bi) {
            let lr = Rect::from_min_size(egui::pos2(x0, *top), egui::vec2(MX_LABEL_W, *h));
            fills.push(egui::Shape::rect_filled(lr, 0.0, mx_hi_bg()));
            if let Some(c) = hit.col {
                // Il contenuto della cella (dev o valore) diventa giallo: serve
                // il fondo scuro.
                fills.push(egui::Shape::rect_filled(cell_of(c), 0.0, mx_hi_bg()));
            }
        }
        if !fills.is_empty() {
            p.set(*idx, egui::Shape::Vec(fills));
        }
    }
    for t in texts.into_inner() {
        let lit = hit_band == Some(t.band) && (t.col.is_none() || t.col == hit.col);
        let color = if lit { mx_hi_text() } else { t.color };
        let galley = p.layout_no_wrap(t.text, t.font, color);
        let at = t.align.anchor_size(t.pos, galley.size()).min;
        let shape = if lit {
            // Grassetto: l'app ha un solo peso di font, quindi la scritta è
            // ripetuta con un piccolo scarto orizzontale.
            egui::Shape::Vec(vec![
                egui::Shape::galley(at, galley.clone(), color),
                egui::Shape::galley(at + egui::vec2(MX_BOLD_DX, 0.0), galley, color),
            ])
        } else {
            egui::Shape::galley(at, galley, color)
        };
        p.set(t.idx, shape);
    }

    header(head_y, "Progetto");
    vlines(head_y, head_bottom);
    // Il nome del worker puntato: lo sfondo della riga è già grigio e un velo
    // leggero non si nota, quindi colori invertiti (fondo del colore del
    // testo, nome del colore dello sfondo): spicca in entrambi i temi.
    if let Some(c) = hit.col {
        let hr = Rect::from_min_size(egui::pos2(col_x(c), head_y), egui::vec2(MX_COL_W, MX_HEAD_H));
        p.rect_filled(hr, 0.0, text());
        p.text(
            hr.center(),
            Align2::CENTER_CENTER,
            fit_text(ui, &m.workers[c].1, &bold, MX_COL_W - 2.0 * MX_PAD),
            bold.clone(),
            bg(),
        );
    }
}

/// Etichette delle sotto-righe per stato (ordine di `ProjectStatus::ALL`).
const STATUS_LABELS: [&str; 3] = ["Attivi", "On-Hold", "Closed"];

/// Etichetta accanto alla tripletta di un progetto chiuso nella matrice.
fn closed_tag(status: ProjectStatus) -> &'static str {
    match status {
        ProjectStatus::OnHold => "on-hold",
        _ => "chiuso",
    }
}

/// Finestra della matrice progetti × worker: in alto l'intervallo (a sinistra
/// l'inizio, a destra la fine, ognuno «Oggi» o una data), poi la matrice.
pub(crate) fn matrix_window(
    ctx: &egui::Context,
    app: &App,
    state: &mut UiState,
    actions: &mut Vec<Action>,
) {
    if !state.show_matrix {
        return;
    }
    let today = today_days();
    let from = state.matrix_from.unwrap_or(today);
    let to = state.matrix_to.unwrap_or(today);
    let valid = from <= to;
    let m = crate::worker_matrix::build_filtered(
        app,
        &state.matrix_workers,
        from,
        to,
        &state.matrix_categories,
        &state.matrix_statuses,
    );

    let mut open = true;
    egui::Window::new("Matrice progetti × worker")
        .collapsible(true)
        .resizable(true)
        .default_size(egui::vec2(900.0, 560.0))
        .default_pos(egui::pos2(80.0, 60.0))
        .open(&mut open)
        .show(ctx, |ui| {
            // Da/A cambiati a mano: la combo anni non descrive più l'intervallo.
            let mut manual = false;
            let mut years_changed = false;
            ui.horizontal(|ui| {
                manual |= range_end_picker(ui, "Da:", "matrix_from", &mut state.matrix_from);
                // Fine intervallo ancorata a destra: un blocco di larghezza fissa
                // (disposto da sinistra a destra) in un layout destra→sinistra.
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.allocate_ui_with_layout(
                        egui::vec2(280.0, ui.spacing().interact_size.y),
                        egui::Layout::left_to_right(egui::Align::Center),
                        |ui| {
                            manual |=
                                range_end_picker(ui, "A:", "matrix_to", &mut state.matrix_to)
                        },
                    );
                    // In mezzo: le combo di anni e categorie, centrate nello spazio
                    // rimasto (layout destra→sinistra: categorie prima, a destra).
                    let combos = 2.0 * MX_YEARS_W
                        + MX_STATUS_W
                        + 5.0 * ui.spacing().item_spacing.x
                        + 60.0;
                    let gap = ((ui.available_width() - combos) / 2.0).max(0.0);
                    ui.add_space(gap);
                    statuses_combo(ui, &mut state.matrix_statuses);
                    categories_combo(ui, app, &mut state.matrix_categories);
                    years_changed = years_combo(ui, app, &mut state.matrix_years);
                });
            });
            if years_changed {
                if let Some((f, t)) = years_range(&state.matrix_years) {
                    state.matrix_from = Some(f);
                    state.matrix_to = Some(t);
                }
            } else if manual {
                state.matrix_years.clear();
            }
            ui.horizontal(|ui| {
                if ui.button("Worker…").clicked() {
                    state.matrix_select_open = true;
                }
                let can_export = valid && !m.rows.is_empty();
                if ui
                    .add_enabled(can_export, egui::Button::new("Esporta PDF…"))
                    .clicked()
                {
                    actions.push(Action::ExportWorkerMatrix {
                        from,
                        to,
                        years: state.matrix_years.iter().copied().collect(),
                    });
                }
                ui.label(
                    egui::RichText::new(format!(
                        "{} worker · {} progetti",
                        m.workers.len(),
                        m.rows.len()
                    ))
                    .color(text_dim()),
                );
            });
            ui.separator();
            if !valid {
                ui.colored_label(
                    warn_amber(),
                    "La data iniziale è successiva a quella finale.",
                );
                return;
            }
            if m.workers.is_empty() {
                ui.label("Nessun worker selezionato: usa «Worker…».");
                return;
            }
            if m.rows.is_empty() {
                ui.label("Nessun progetto con i worker selezionati nell'intervallo.");
            }
            egui::ScrollArea::both()
                .auto_shrink([false, false])
                .show(ui, |ui| draw_matrix(ui, app, &m));
        });
    if !open {
        state.show_matrix = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    /// Anni scelti → Da = 1/1 del più vecchio, A = 31/12 del più recente (anche
    /// se in mezzo c'è un anno non scelto); nessun anno → nessun intervallo.
    #[test]
    fn years_range_spans_oldest_to_newest() {
        let d = |y, m, dd| local_to_days(&chrono::NaiveDate::from_ymd_opt(y, m, dd).unwrap());
        assert_eq!(years_range(&BTreeSet::new()), None);
        let one: BTreeSet<i32> = [2026].into();
        assert_eq!(years_range(&one), Some((d(2026, 1, 1), d(2026, 12, 31))));
        let gap: BTreeSet<i32> = [2027, 2024].into();
        assert_eq!(years_range(&gap), Some((d(2024, 1, 1), d(2027, 12, 31))));
    }

    /// Cella → riga + colonna; tripletta → solo riga; nome worker (riga fissa
    /// in cima) → solo colonna; intestazione di categoria → niente.
    #[test]
    fn hover_hit_finds_row_and_column() {
        let x0 = 10.0;
        let rows = [(50.0, 20.0), (90.0, 20.0)]; // in mezzo (70..90) un gruppo
        let cell_x = x0 + MX_LABEL_W + MX_COL_W * 1.5; // worker 1
        let at = |x, y| mx_hit(egui::pos2(x, y), x0, 3, 34.0, &rows);
        assert_eq!(at(cell_x, 95.0), MxHit { row: Some(1), col: Some(1) });
        assert_eq!(at(x0 + 5.0, 55.0), MxHit { row: Some(0), col: None });
        assert_eq!(at(cell_x, 20.0), MxHit { row: None, col: Some(1) });
        assert_eq!(at(cell_x, 75.0), MxHit::default());
        // Oltre l'ultima colonna: solo la riga.
        let past = x0 + MX_LABEL_W + MX_COL_W * 3.0 + 1.0;
        assert_eq!(at(past, 55.0), MxHit { row: Some(0), col: None });
    }

    /// Tema scuro, mouse su una cella della sotto-riga «Attivi»: etichetta e
    /// valore della cella in giallo su fondo scuro (fondo disegnato **prima**
    /// della scritta), colonna col velo, nome del worker a colori invertiti;
    /// tolto il mouse, la scritta torna del colore di sistema.
    #[test]
    fn hover_highlight_is_yellow_on_dark_in_dark_theme() {
        crate::ui_style::set_dark_theme(true);
        let mut app = App::new();
        let mario = app.workers.add("Mario");
        let fe = app.devs.add("Frontend");
        let p = app.projects.add("P", Some("T01"), None);
        app.projects.add_dev(p, fe);
        app.projects.add_effort(p, fe, WeekId(20000), mario, Effort(8));
        let sel: std::collections::HashSet<_> = [mario].into_iter().collect();
        let m = crate::worker_matrix::build_filtered(
            &app,
            &sel,
            20000,
            20006,
            &BTreeSet::new(),
            &BTreeSet::new(),
        );

        let ctx = egui::Context::default();
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0)));
        let frame = |input: egui::RawInput| {
            let cs = ctx
                .run_ui(input, |ui| {
                    crate::ui_style::set_dark_theme(true);
                    draw_matrix(ui, &app, &m);
                })
                .shapes;
            // Appiattisce le forme (gli slot sono `Shape::Vec`), in ordine di disegno.
            fn flat(s: &egui::Shape, out: &mut Vec<egui::Shape>) {
                match s {
                    egui::Shape::Vec(v) => v.iter().for_each(|s| flat(s, out)),
                    s => out.push(s.clone()),
                }
            }
            let mut out = Vec::new();
            cs.iter().for_each(|c| flat(&c.shape, &mut out));
            out
        };
        // Primo testo `t` dall'alto (indice di disegno + forma).
        let find = |v: &[egui::Shape], t: &str| {
            v.iter().enumerate().find_map(|(i, s)| match s {
                egui::Shape::Text(x) if x.galley.text() == t => Some((i, x.clone())),
                _ => None,
            })
        };
        let first = frame(input.clone());
        let (_, attivi) = find(&first, "Attivi").expect("Attivi");
        assert_eq!(attivi.fallback_color, text(), "senza mouse: colore di sistema");
        let label = attivi.visual_bounding_rect();
        // Cella del worker 0 sulla stessa riga.
        let x0 = label.left() - MX_PAD - 12.0;
        let pos = egui::pos2(x0 + MX_LABEL_W + MX_COL_W / 2.0, label.center().y);
        let mut hover = input.clone();
        hover.events.push(egui::Event::PointerMoved(pos));
        frame(hover);
        let v = frame(input.clone());
        let fill_at = |fill: Color32, at: egui::Pos2| {
            v.iter().position(|s| {
                matches!(s, egui::Shape::Rect(r) if r.fill == fill && r.rect.contains(at))
            })
        };
        let (ai, attivi) = find(&v, "Attivi").unwrap();
        assert_eq!(attivi.fallback_color, mx_hi_text(), "etichetta non gialla");
        let copies = v
            .iter()
            .filter(|s| matches!(s, egui::Shape::Text(t) if t.galley.text() == "Attivi"))
            .count();
        // Due righe «Attivi» (categoria + totale); quella puntata ha 2 passate.
        assert_eq!(copies, 3, "«Attivi» evidenziato non in grassetto");
        let bg_i = fill_at(mx_hi_bg(), label.center()).expect("fondo scuro sull'etichetta");
        assert!(bg_i < ai, "il fondo copre la scritta «Attivi»");
        // Valore della cella puntata: giallo su fondo scuro.
        let value = v.iter().enumerate().find_map(|(i, s)| match s {
            egui::Shape::Text(t) if t.visual_bounding_rect().contains(pos) => Some((i, t.clone())),
            _ => None,
        });
        let (vi, value) = value.expect("valore della cella");
        assert_eq!(value.fallback_color, mx_hi_text(), "valore non giallo");
        assert!(fill_at(mx_hi_bg(), pos).is_some_and(|i| i < vi));
        // Nome del worker: fondo `text()` e scritta `bg()`.
        let name = v
            .iter()
            .rev()
            .find_map(|s| match s {
                egui::Shape::Text(t) if t.galley.text() == "Mario" => Some(t.clone()),
                _ => None,
            })
            .unwrap();
        assert_eq!(name.fallback_color, bg());
        assert!(fill_at(text(), name.visual_bounding_rect().center()).is_some());

        // Cella di un progetto: il dev diventa giallo su fondo scuro, come la
        // tripletta della riga.
        let (_, dev) = find(&v, "Frontend").expect("dev nella cella");
        assert_eq!(dev.fallback_color, Color32::BLACK, "dev non puntato: nero");
        let mut hover = input.clone();
        hover.events.push(egui::Event::PointerMoved(dev.visual_bounding_rect().center()));
        frame(hover);
        let v = frame(input.clone());
        let (_, dev) = find(&v, "Frontend").unwrap();
        assert_eq!(dev.fallback_color, mx_hi_text(), "dev non giallo");
        let (_, trip) = find(&v, "T01").unwrap();
        assert_eq!(trip.fallback_color, mx_hi_text(), "tripletta non gialla");
    }

    /// Scorrendo la matrice in verticale, i nomi dei worker restano in cima
    /// all'area visibile.
    #[test]
    fn worker_header_stays_visible_when_scrolled() {
        let mut app = App::new();
        let mario = app.workers.add("Mario");
        let fe = app.devs.add("Frontend");
        for i in 0..60 {
            let p = app.projects.add("P", Some(&format!("T{i:02}")), None);
            app.projects.add_dev(p, fe);
            app.projects.add_effort(p, fe, WeekId(20000), mario, Effort(8));
        }
        let sel: std::collections::HashSet<_> = [mario].into_iter().collect();
        let m = crate::worker_matrix::build_filtered(
            &app,
            &sel,
            20000,
            20006,
            &BTreeSet::new(),
            &BTreeSet::new(),
        );
        assert_eq!(m.rows.len(), 60);

        let ctx = egui::Context::default();
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 400.0)));
        let mut view_top = 0.0;
        let mut name_y = None;
        for _ in 0..2 {
            let out = ctx.run_ui(input.clone(), |ui| {
                let sa = egui::ScrollArea::both()
                    .vertical_scroll_offset(500.0)
                    .auto_shrink([false, false])
                    .show(ui, |ui| draw_matrix(ui, &app, &m));
                view_top = sa.inner_rect.top();
            });
            name_y = out.shapes.iter().find_map(|cs| match &cs.shape {
                egui::epaint::Shape::Text(t) if t.galley.text() == "Mario" => {
                    Some(t.visual_bounding_rect().center().y)
                }
                _ => None,
            });
        }
        let y = name_y.expect("nome del worker non disegnato");
        assert!(
            y >= view_top && y <= view_top + MX_HEAD_H,
            "intestazione non in cima: y={y}, area da {view_top}"
        );
    }
}
