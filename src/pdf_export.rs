//! Esportazione PDF: una pagina per progetto (solo progetti abilitati, non
//! chiusi, con inizio e fine). Ogni pagina è un diagramma di Gantt:
//! - titolo (tripletta) + descrizione del progetto;
//! - asse dei mesi con etichette anno agli estremi;
//! - milestone come bandierine in alto;
//! - marker "Today" (rosso) con barra di avanzamento sull'asse;
//! - una riga per ogni dev con effort, barra colorata (colore del dev) che va
//!   dalla prima all'ultima settimana con effort, con etichetta date a fine barra.

use chrono::{Datelike, NaiveDate};
use printpdf::*;

use crate::app::App;
use crate::date_utils::dates::{days_to_local, local_to_days};

// Font incorporati (DejaVu Sans, licenza ridistribuibile): resa corretta degli
// accenti/Unicode, che i font builtin PDF (WinAnsi) non garantiscono.
const FONT_REGULAR: &[u8] = include_bytes!("../assets/fonts/DejaVuSans.ttf");
const FONT_BOLD: &[u8] = include_bytes!("../assets/fonts/DejaVuSans-Bold.ttf");

/// Handle dei font incorporati, registrati nel documento.
struct Fonts {
    regular: FontId,
    bold: FontId,
}

impl Fonts {
    fn handle(&self, bold: bool) -> PdfFontHandle {
        PdfFontHandle::External(if bold { self.bold.clone() } else { self.regular.clone() })
    }
}

// Pagina A4 orizzontale (mm), origine in basso a sinistra.
const PAGE_W: f32 = 297.0;
const PAGE_H: f32 = 210.0;

// Colonna sinistra (etichette righe) e area del grafico (timeline).
const X_LABEL: f32 = 8.0;
const CHART_X0: f32 = 64.0;
const CHART_X1: f32 = 279.0;

// Asse dei mesi (banda orizzontale). Tenuto basso per lasciare ampio spazio
// alle bandierine sopra (usando lo spazio libero sotto le righe dev).
const AXIS_TOP: f32 = 130.0;
const AXIS_BOT: f32 = 122.0;

// Zona righe dev (tra l'asse e il footer).
const ROWS_TOP: f32 = 119.0;
const ROWS_BOT: f32 = 26.0;

// Footer (banda grigia + data creazione).
const FOOTER_TOP: f32 = 22.0;
const FOOTER_BOT: f32 = 6.0;

const BLACK: (f32, f32, f32) = (0.0, 0.0, 0.0);
const WHITE: (f32, f32, f32) = (1.0, 1.0, 1.0);
const GRAY_DK: (f32, f32, f32) = (0.62, 0.62, 0.62);
const GRAY_LT: (f32, f32, f32) = (0.72, 0.72, 0.72);
const GRAY_FOOTER: (f32, f32, f32) = (0.90, 0.90, 0.90);
const GRAY_GUIDE: (f32, f32, f32) = (0.82, 0.82, 0.82);
const TEXT_GRAY: (f32, f32, f32) = (0.35, 0.35, 0.35);
const ORANGE: (f32, f32, f32) = (0.90, 0.45, 0.10);
const RED: (f32, f32, f32) = (0.86, 0.15, 0.15);

const MONTHS_IT: [&str; 12] = [
    "Gen", "Feb", "Mar", "Apr", "Mag", "Giu", "Lug", "Ago", "Set", "Ott", "Nov", "Dic",
];

fn rgb(c: (f32, f32, f32)) -> Color {
    Color::Rgb(Rgb { r: c.0, g: c.1, b: c.2, icc_profile: None })
}

fn u32_rgb(c: u32) -> (f32, f32, f32) {
    (
        ((c >> 16) & 0xFF) as f32 / 255.0,
        ((c >> 8) & 0xFF) as f32 / 255.0,
        (c & 0xFF) as f32 / 255.0,
    )
}

// --- Helper date ------------------------------------------------------------

fn day_to_date(days: i32) -> NaiveDate {
    days_to_local(days)
}

/// Primo giorno del mese di `d`.
fn month_start(d: NaiveDate) -> NaiveDate {
    NaiveDate::from_ymd_opt(d.year(), d.month(), 1).unwrap()
}

/// `d` con `n` mesi aggiunti (gestisce l'overflow di anno; clampa il giorno).
fn add_months(d: NaiveDate, n: i32) -> NaiveDate {
    let total = (d.year() * 12 + d.month() as i32 - 1) + n;
    let year = total.div_euclid(12);
    let month = total.rem_euclid(12) as u32 + 1;
    // Giorno clampato all'ultimo valido del mese di destinazione.
    let mut day = d.day();
    loop {
        if let Some(nd) = NaiveDate::from_ymd_opt(year, month, day) {
            return nd;
        }
        day -= 1;
    }
}

/// Etichetta data breve: "22 Set".
fn short_date(days: i32) -> String {
    let d = day_to_date(days);
    format!("{} {}", d.day(), MONTHS_IT[(d.month() - 1) as usize])
}

// --- Helper testo/disegno ---------------------------------------------------

/// Larghezza approssimata di un testo in mm (media 0.5·size per carattere).
fn text_w_mm(text: &str, size_pt: f32) -> f32 {
    text.chars().count() as f32 * size_pt * 0.5 * 0.352_778
}

/// Tronca `s` con ellissi finché non sta in `max_w` mm.
fn truncate_to_w(s: &str, size: f32, max_w: f32) -> String {
    if text_w_mm(s, size) <= max_w {
        return s.to_string();
    }
    let mut out = String::new();
    for ch in s.chars() {
        if text_w_mm(&format!("{out}{ch}…"), size) > max_w {
            break;
        }
        out.push(ch);
    }
    format!("{out}…")
}

/// Ops per una stringa ancorata a sinistra a (x, y) in mm.
fn text_left(
    fonts: &Fonts,
    x: f32,
    y: f32,
    s: &str,
    size: f32,
    bold: bool,
    color: (f32, f32, f32),
) -> Vec<Op> {
    vec![
        Op::StartTextSection,
        Op::SetTextCursor { pos: Point::new(Mm(x), Mm(y)) },
        Op::SetFont { font: fonts.handle(bold), size: Pt(size) },
        Op::SetLineHeight { lh: Pt(size) },
        Op::SetFillColor { col: rgb(color) },
        Op::ShowText { items: vec![TextItem::Text(s.to_string())] },
        Op::EndTextSection,
    ]
}

/// Come `text_left` ma centrato orizzontalmente su `x`.
fn text_center(
    fonts: &Fonts,
    x: f32,
    y: f32,
    s: &str,
    size: f32,
    bold: bool,
    color: (f32, f32, f32),
) -> Vec<Op> {
    let start = (x - text_w_mm(s, size) / 2.0).clamp(2.0, PAGE_W - 2.0);
    text_left(fonts, start, y, s, size, bold, color)
}

/// Come `text_left` ma ancorato a destra: il testo termina a `x`.
fn text_right(
    fonts: &Fonts,
    x: f32,
    y: f32,
    s: &str,
    size: f32,
    bold: bool,
    color: (f32, f32, f32),
) -> Vec<Op> {
    text_left(fonts, x - text_w_mm(s, size), y, s, size, bold, color)
}

/// Rettangolo pieno tra gli angoli (x0,y0)-(x1,y1) in mm.
fn rect_fill(x0: f32, y0: f32, x1: f32, y1: f32, color: (f32, f32, f32)) -> Vec<Op> {
    let pts = [(x0, y0), (x1, y0), (x1, y1), (x0, y1)];
    vec![
        Op::SetFillColor { col: rgb(color) },
        Op::DrawPolygon {
            polygon: Polygon {
                rings: vec![PolygonRing {
                    points: pts
                        .iter()
                        .map(|(px, py)| LinePoint {
                            p: Point::new(Mm(*px), Mm(*py)),
                            bezier: false,
                        })
                        .collect(),
                }],
                mode: PaintMode::Fill,
                winding_order: WindingOrder::NonZero,
            },
        },
    ]
}

/// Poligono pieno da una lista di punti (mm).
fn poly_fill(pts: &[(f32, f32)], color: (f32, f32, f32)) -> Vec<Op> {
    vec![
        Op::SetFillColor { col: rgb(color) },
        Op::DrawPolygon {
            polygon: Polygon {
                rings: vec![PolygonRing {
                    points: pts
                        .iter()
                        .map(|(px, py)| LinePoint {
                            p: Point::new(Mm(*px), Mm(*py)),
                            bezier: false,
                        })
                        .collect(),
                }],
                mode: PaintMode::Fill,
                winding_order: WindingOrder::NonZero,
            },
        },
    ]
}

/// Linea da (x0,y0) a (x1,y1) in mm.
fn line(x0: f32, y0: f32, x1: f32, y1: f32, thick: f32, color: (f32, f32, f32)) -> Vec<Op> {
    vec![
        Op::SetOutlineColor { col: rgb(color) },
        Op::SetOutlineThickness { pt: Pt(thick) },
        Op::DrawLine {
            line: Line {
                points: vec![
                    LinePoint { p: Point::new(Mm(x0), Mm(y0)), bezier: false },
                    LinePoint { p: Point::new(Mm(x1), Mm(y1)), bezier: false },
                ],
                is_closed: false,
            },
        },
    ]
}

/// Linea orizzontale tratteggiata da (x0,y) a (x1,y): resa come segmenti brevi
/// (indipendente dal supporto dash della libreria).
fn dashed_hline(x0: f32, x1: f32, y: f32, thick: f32, color: (f32, f32, f32)) -> Vec<Op> {
    const DASH: f32 = 1.4;
    const GAP: f32 = 1.2;
    let mut ops = vec![
        Op::SetOutlineColor { col: rgb(color) },
        Op::SetOutlineThickness { pt: Pt(thick) },
    ];
    let mut x = x0;
    while x < x1 {
        let xe = (x + DASH).min(x1);
        ops.push(Op::DrawLine {
            line: Line {
                points: vec![
                    LinePoint { p: Point::new(Mm(x), Mm(y)), bezier: false },
                    LinePoint { p: Point::new(Mm(xe), Mm(y)), bezier: false },
                ],
                is_closed: false,
            },
        });
        x += DASH + GAP;
    }
    ops
}

/// Word-wrap grezzo: spezza `text` in righe di al più `max_chars` caratteri.
fn wrap(text: &str, max_chars: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut cur = String::new();
    for word in text.split_whitespace() {
        if cur.is_empty() {
            cur = word.to_string();
        } else if cur.chars().count() + 1 + word.chars().count() <= max_chars {
            cur.push(' ');
            cur.push_str(word);
        } else {
            lines.push(std::mem::take(&mut cur));
            cur = word.to_string();
        }
    }
    if !cur.is_empty() {
        lines.push(cur);
    }
    lines
}

/// Come `wrap` ma preserva gli a-capo espliciti (`\n`).
fn wrap_multiline(text: &str, max_chars: usize) -> Vec<String> {
    let mut out = Vec::new();
    for para in text.replace('\r', "").split('\n') {
        if para.trim().is_empty() {
            out.push(String::new());
        } else {
            out.extend(wrap(para, max_chars));
        }
    }
    out
}

// --- Dati della pagina ------------------------------------------------------

/// Una riga del Gantt (un dev con effort).
struct Row {
    label: String,
    color: (f32, f32, f32),
    start_day: i32,
    end_day: i32,
}

/// Una milestone (bandierina in alto).
struct Flag {
    day: i32,
    title: String,
    date: String,
    color: (f32, f32, f32),
}

/// Genera gli `Op` di una pagina Gantt per un progetto.
fn page_ops(
    fonts: &Fonts,
    tripletta: &str,
    descr: &str,
    proj_start: i32,
    proj_end: i32,
    rows: &[Row],
    mut flags: Vec<Flag>,
    today: i32,
    created: &str,
) -> Vec<Op> {
    let mut ops = Vec::new();

    // --- Intervallo dell'asse ----------------------------------------------
    // Copre tutti gli elementi (inizio/fine progetto, barre dev, milestone,
    // today) ed estende di 2 mesi oltre la fine del progetto.
    let mut min_day = proj_start.min(proj_end).min(today);
    let mut max_day = proj_start.max(proj_end).max(today);
    for r in rows {
        min_day = min_day.min(r.start_day);
        max_day = max_day.max(r.end_day);
    }
    for f in &flags {
        min_day = min_day.min(f.day);
        max_day = max_day.max(f.day);
    }
    let end_plus_2m = local_to_days(&add_months(day_to_date(proj_end), 2));
    max_day = max_day.max(end_plus_2m);

    // Snap ai confini di mese: inizio = primo del mese di min; fine (esclusiva)
    // = primo del mese successivo a max, così l'ultimo mese è mostrato intero.
    let axis_start = local_to_days(&month_start(day_to_date(min_day)));
    let axis_end = local_to_days(&add_months(month_start(day_to_date(max_day)), 1));
    let span = (axis_end - axis_start).max(1) as f32;

    let x_of = |days: i32| -> f32 {
        let t = (days - axis_start) as f32 / span;
        (CHART_X0 + t * (CHART_X1 - CHART_X0)).clamp(CHART_X0, CHART_X1)
    };

    // --- Titolo + descrizione ----------------------------------------------
    if !tripletta.is_empty() {
        ops.extend(text_left(fonts, X_LABEL, 200.0, tripletta, 16.0, true, BLACK));
    }
    // Descrizione completa: ogni riga logica (a-capo espliciti) mandata a capo
    // automaticamente. Nessun limite di righe, così il testo non viene troncato.
    let mut y = if tripletta.is_empty() { 200.0 } else { 193.0 };
    for l in wrap_multiline(descr, 120) {
        ops.extend(text_left(fonts, X_LABEL, y, &l, 9.0, false, TEXT_GRAY));
        y -= 5.0;
    }

    // --- Asse dei mesi ------------------------------------------------------
    let mut m = month_start(day_to_date(axis_start));
    let mut idx = 0usize;
    loop {
        let cell_x0 = x_of(local_to_days(&m));
        let next = add_months(m, 1);
        let cell_x1 = x_of(local_to_days(&next));
        let fill = if idx % 2 == 0 { GRAY_DK } else { GRAY_LT };
        ops.extend(rect_fill(cell_x0, AXIS_BOT, cell_x1, AXIS_TOP, fill));
        // Separatore bianco a destra della cella.
        ops.extend(line(cell_x1, AXIS_BOT, cell_x1, AXIS_TOP, 0.6, WHITE));
        // Riga verticale del mese, attraverso la zona delle righe dev.
        ops.extend(line(cell_x0, ROWS_BOT, cell_x0, AXIS_BOT, 0.4, GRAY_GUIDE));
        // Etichetta mese (bianca, in basso a sinistra della cella) se ci sta.
        let name = MONTHS_IT[(m.month() - 1) as usize];
        if cell_x1 - cell_x0 > text_w_mm(name, 8.0) + 1.5 {
            ops.extend(text_left(fonts, cell_x0 + 1.5, AXIS_BOT + 1.6, name, 8.0, false, WHITE));
        }
        if local_to_days(&next) >= axis_end {
            break;
        }
        m = next;
        idx += 1;
    }
    // Riga verticale di chiusura a destra dell'asse.
    ops.extend(line(CHART_X1, ROWS_BOT, CHART_X1, AXIS_BOT, 0.4, GRAY_GUIDE));

    // Etichette anno agli estremi (arancione, in grassetto).
    let year_l = month_start(day_to_date(axis_start)).year();
    let year_r = add_months(month_start(day_to_date(axis_end)), -1).year();
    let axis_mid = (AXIS_BOT + AXIS_TOP) / 2.0 - 1.8;
    ops.extend(text_right(fonts, CHART_X0 - 9.0, axis_mid, &year_l.to_string(), 13.0, true, ORANGE));
    ops.extend(text_left(fonts, CHART_X1 + 2.0, axis_mid, &year_r.to_string(), 13.0, true, ORANGE));

    // --- Barra di avanzamento "Today" sull'asse ----------------------------
    let today_in_axis = today >= axis_start && today <= axis_end;
    if today_in_axis && today > proj_start {
        let xt = x_of(today);
        ops.extend(rect_fill(x_of(proj_start), AXIS_TOP - 3.0, xt, AXIS_TOP, RED));
    }
    if today_in_axis {
        let xt = x_of(today);
        // Linea rossa verticale attraverso le righe.
        ops.extend(line(xt, ROWS_BOT, xt, AXIS_BOT, 0.7, RED));
        // Triangolo + etichetta "Today" sopra l'asse.
        ops.extend(poly_fill(
            &[(xt - 2.0, AXIS_TOP + 4.0), (xt + 2.0, AXIS_TOP + 4.0), (xt, AXIS_TOP)],
            RED,
        ));
        ops.extend(text_center(fonts, xt, AXIS_TOP + 5.0, "Today", 8.0, false, BLACK));
    }

    // --- Milestone come bandierine -----------------------------------------
    // Righe dell'etichetta (dal basso: data, poi nome una parola per riga).
    let flag_lines = |f: &Flag| -> Vec<(String, f32, bool)> {
        let mut v = vec![(f.date.clone(), 7.0, false)];
        for w in f.title.split_whitespace().rev() {
            v.push((w.to_string(), 8.0, true));
        }
        v
    };

    flags.sort_by_key(|f| f.day);

    // Posizionamento anti-collisione: ogni etichetta viene alzata quel tanto che
    // basta a non sovrapporsi (né in orizzontale né in verticale) a quelle già
    // posizionate. Le bandierine isolate restano basse; quelle con etichette
    // vicine si alzano quanto serve, in base all'altezza reale di ciascuna.
    let base = AXIS_TOP + 6.0;
    let mut placed: Vec<(f32, f32, f32, f32)> = Vec::new(); // (left, right, bottom, top)
    let mut pole_tops = Vec::with_capacity(flags.len());
    for f in &flags {
        let x = x_of(f.day);
        let lines = flag_lines(f);
        let half_w = lines
            .iter()
            .map(|(t, s, _)| text_w_mm(t, *s))
            .fold(0.0f32, f32::max)
            / 2.0
            + 1.5;
        let block = lines.len() as f32 * 3.6 + 2.0;
        let (bl, br) = (x - half_w, x + half_w);
        let mut pole = base;
        loop {
            let top = pole + block;
            let mut hit = f32::MIN;
            for &(pl, pr, pb, pt) in &placed {
                if bl < pr && pl < br && pole < pt && pb < top {
                    hit = hit.max(pt);
                }
            }
            if hit == f32::MIN {
                break;
            }
            pole = hit + 1.5; // sopra la scritta che collide
        }
        placed.push((bl, br, pole, pole + block));
        pole_tops.push(pole);
    }

    // Prima passata: le aste, in secondo piano, così non coprono le etichette
    // delle bandierine vicine.
    for (i, f) in flags.iter().enumerate() {
        let x = x_of(f.day);
        ops.extend(line(x, AXIS_TOP, x, pole_tops[i], 1.0, f.color));
    }
    // Seconda passata: pennant ed etichette, in primo piano.
    for (i, f) in flags.iter().enumerate() {
        let x = x_of(f.day);
        let pole_top = pole_tops[i];
        // Pennant: triangolo a destra dell'asta.
        ops.extend(poly_fill(
            &[(x, pole_top), (x + 7.0, pole_top - 2.0), (x, pole_top - 4.0)],
            f.color,
        ));
        // Etichetta sopra la bandierina, centrata sull'asta.
        for (li, (txt, size, bold)) in flag_lines(f).iter().enumerate() {
            let ly = pole_top + 2.0 + li as f32 * 3.6;
            let col = if *bold { BLACK } else { TEXT_GRAY };
            ops.extend(text_center(fonts, x, ly, txt, *size, *bold, col));
        }
    }

    // --- Righe dev ----------------------------------------------------------
    let n = rows.len().max(1);
    let row_h = ((ROWS_TOP - ROWS_BOT) / n as f32).min(6.5);
    let bar_hh = (row_h * 0.22).min(1.7);
    let label_w = CHART_X0 - X_LABEL - 2.0;
    for (i, r) in rows.iter().enumerate() {
        let yc = ROWS_TOP - (i as f32 + 0.5) * row_h;
        // Etichetta a sinistra (nome dev), troncata se troppo lunga.
        let label = truncate_to_w(&r.label, 8.0, label_w);
        ops.extend(text_left(fonts, X_LABEL, yc - 1.3, &label, 8.0, false, BLACK));
        // Barra: dalla prima all'ultima settimana con effort. Estesa di una
        // settimana per dare larghezza minima visibile.
        let bx0 = x_of(r.start_day);
        let bx1 = x_of(r.end_day + 7);
        // Linea orizzontale dal nome del dev fino all'inizio della barra,
        // alternata: righe pari solida, righe dispari tratteggiata.
        let lead_x0 = X_LABEL + text_w_mm(&label, 8.0) + 2.0;
        if bx0 - 1.0 > lead_x0 {
            if i % 2 == 0 {
                ops.extend(line(lead_x0, yc, bx0 - 1.0, yc, 0.3, GRAY_DK));
            } else {
                ops.extend(dashed_hline(lead_x0, bx0 - 1.0, yc, 0.3, GRAY_DK));
            }
        }
        ops.extend(rect_fill(bx0, yc - bar_hh, bx1.max(bx0 + 1.0), yc + bar_hh, r.color));
        // Etichetta date a fine barra (o prima, se non ci sta a destra).
        let lbl = format!("{} - {}", short_date(r.start_day), short_date(r.end_day));
        let w = text_w_mm(&lbl, 7.0);
        if bx1 + 2.0 + w <= CHART_X1 {
            ops.extend(text_left(fonts, bx1 + 2.0, yc - 1.2, &lbl, 7.0, false, TEXT_GRAY));
        } else {
            ops.extend(text_right(fonts, bx0 - 2.0, yc - 1.2, &lbl, 7.0, false, TEXT_GRAY));
        }
    }

    // --- Footer -------------------------------------------------------------
    ops.extend(rect_fill(X_LABEL, FOOTER_BOT, CHART_X1, FOOTER_TOP, GRAY_FOOTER));
    ops.extend(text_center(
        fonts,
        (X_LABEL + CHART_X1) / 2.0,
        (FOOTER_BOT + FOOTER_TOP) / 2.0 - 1.4,
        created,
        8.0,
        false,
        TEXT_GRAY,
    ));

    ops
}

/// Costruisce il PDF con una pagina per ogni progetto visibile (abilitato e non
/// chiuso) dotato di data di inizio E fine. `None` se nessun progetto è idoneo.
pub fn build_pdf(app: &App) -> Option<Vec<u8>> {
    let mut doc = PdfDocument::new("Progetti");
    let regular = ParsedFont::from_bytes(FONT_REGULAR, 0, &mut Vec::new())?;
    let bold = ParsedFont::from_bytes(FONT_BOLD, 0, &mut Vec::new())?;
    let fonts = Fonts {
        regular: doc.add_font(&regular),
        bold: doc.add_font(&bold),
    };

    // Data di creazione (locale), uguale su tutte le pagine di questo PDF.
    let created = chrono::Local::now().format("%Y-%m-%d").to_string();
    let today = local_to_days(&chrono::Local::now().date_naive());

    // Lookup dev → (nome, colore).
    let dev_info: std::collections::HashMap<_, _> = app
        .devs
        .list_full()
        .into_iter()
        .map(|(id, name, bg, _)| (id, (name, u32_rgb(bg as u32))))
        .collect();

    let mut pages = Vec::new();

    for (id, name, enable) in app.projects.list_full() {
        if !enable.0 || app.projects.is_closed(id) {
            continue;
        }
        let (Some(start_w), Some(end_w)) = (
            app.projects.get_project_start_week(id),
            app.projects.get_project_end_week(id),
        ) else {
            continue;
        };
        let proj_start = start_w.0 as i32;
        let proj_end = end_w.0 as i32;

        // Righe dev: una per ogni dev con effort registrato, ordinata per data
        // di inizio.
        let mut rows = Vec::new();
        for dev_id in app.projects.get_dev_ids(id) {
            let Some(sd) = app.projects.get_single_dev(id, dev_id) else {
                continue;
            };
            let Some((first, last)) = sd.effort_span() else {
                continue;
            };
            let (label, color) = dev_info
                .get(&dev_id)
                .cloned()
                .unwrap_or_else(|| ("?".to_string(), BLACK));
            rows.push(Row {
                label,
                color,
                start_day: first.0 as i32,
                end_day: last.0 as i32,
            });
        }
        rows.sort_by_key(|r| (r.start_day, r.end_day));

        // Milestone del progetto.
        let mut flags = Vec::new();
        for (mid, week) in app.projects.list_project_milestones(id) {
            let mname = app.milestones.get_name(mid).unwrap_or("?").to_string();
            let mcol = app.milestones.get_color(mid).map(u32_rgb).unwrap_or(BLACK);
            flags.push(Flag {
                day: week.0 as i32,
                title: mname,
                date: short_date(week.0 as i32),
                color: mcol,
            });
        }

        let tripletta = app.projects.get_tripletta(id);
        let ops = page_ops(
            &fonts, &tripletta, &name, proj_start, proj_end, &rows, flags, today, &created,
        );
        pages.push(PdfPage::new(Mm(PAGE_W), Mm(PAGE_H), ops));
    }

    if pages.is_empty() {
        return None;
    }

    let bytes = doc.with_pages(pages).save(&PdfSaveOptions::default(), &mut Vec::new());
    Some(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::single_dev_utils::single_dev::WeekId;
    use crate::single_effort_utils::sinlge_effort::Effort;
    use crate::workers_utils::worker::WorkerId;

    #[test]
    fn no_eligible_projects_returns_none() {
        let app = App::new();
        assert!(build_pdf(&app).is_none());
    }

    #[test]
    fn project_with_start_and_end_produces_valid_pdf() {
        let mut app = App::new();
        let start = WeekId(20000);
        let pid = app.projects.add("Descrizione progetto", Some("ABC"), Some(start));
        app.projects.set_project_end_week(pid, Some(WeekId(20070)));
        // Un dev con effort su alcune settimane → una riga del Gantt.
        let dev = app.devs.add("Frontend");
        let worker = WorkerId(0);
        app.projects.add_dev(pid, dev);
        app.projects.add_effort(pid, dev, WeekId(20007), worker, Effort(8));
        app.projects.add_effort(pid, dev, WeekId(20035), worker, Effort(8));
        let mid = app.milestones.add("Beta");
        app.projects.add_project_milestone(pid, mid, WeekId(20035));

        let bytes = build_pdf(&app).expect("un progetto idoneo → Some");
        assert!(bytes.starts_with(b"%PDF"), "l'output deve essere un PDF valido");
    }

    #[test]
    fn project_without_end_is_skipped() {
        let mut app = App::new();
        app.projects.add("Senza fine", Some("XYZ"), Some(WeekId(20000)));
        assert!(build_pdf(&app).is_none());
    }
}
