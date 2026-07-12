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
use crate::dev_utils::dev::DevId;
use crate::project_utils::project::ProjectId;
use crate::single_dev_utils::single_dev::WeekId;

// Flag di rendering "mostra percentuali di avanzamento" nell'export, scelto
// dall'utente nelle dialog. Thread-local come i flag tema/B-N di `ui_style`:
// impostato prima di ogni `build_*`, letto da `page_shapes`. Default: off.
thread_local! {
    static SHOW_PCT: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Imposta se le percentuali (presunta/dichiarata) vanno disegnate nell'export.
pub fn set_show_pct(v: bool) {
    SHOW_PCT.with(|c| c.set(v));
}

fn show_pct() -> bool {
    SHOW_PCT.with(|c| c.get())
}

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
        PdfFontHandle::External(if bold {
            self.bold.clone()
        } else {
            self.regular.clone()
        })
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

/// Riferimento minimo (in ore) per l'altezza delle barre nel formato
/// `Proportional`: se il massimo settimanale del dev non supera queste ore, si
/// usa comunque questo valore come denominatore (100% = altezza piena). Così una
/// settimana piena "standard" non riempie tutta l'altezza se il dev è scarico.
const PROPORTIONAL_REF_MIN: u32 = 40;

const MONTHS_IT: [&str; 12] = [
    "Gen", "Feb", "Mar", "Apr", "Mag", "Giu", "Lug", "Ago", "Set", "Ott", "Nov", "Dic",
];

/// Primitiva di disegno indipendente dal formato: coordinate in mm, origine in
/// basso a sinistra (come nel PDF). Renderizzata sia in PDF (`render_pdf`) sia in
/// SVG (`render_svg`), così il grafico è identico nei due formati.
#[derive(Clone)]
enum Shape {
    Rect {
        x0: f32,
        y0: f32,
        x1: f32,
        y1: f32,
        color: (f32, f32, f32),
    },
    Poly {
        pts: Vec<(f32, f32)>,
        color: (f32, f32, f32),
    },
    Line {
        x0: f32,
        y0: f32,
        x1: f32,
        y1: f32,
        thick: f32,
        color: (f32, f32, f32),
    },
    Text {
        x: f32,
        y: f32,
        s: String,
        size: f32,
        bold: bool,
        color: (f32, f32, f32),
    },
}

fn rgb(c: (f32, f32, f32)) -> Color {
    Color::Rgb(Rgb {
        r: c.0,
        g: c.1,
        b: c.2,
        icc_profile: None,
    })
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

/// Testo ancorato a sinistra a (x, y) in mm (y = baseline).
fn text_left(x: f32, y: f32, s: &str, size: f32, bold: bool, color: (f32, f32, f32)) -> Vec<Shape> {
    vec![Shape::Text {
        x,
        y,
        s: s.to_string(),
        size,
        bold,
        color,
    }]
}

/// Come `text_left` ma centrato orizzontalmente su `x`.
fn text_center(
    x: f32,
    y: f32,
    s: &str,
    size: f32,
    bold: bool,
    color: (f32, f32, f32),
) -> Vec<Shape> {
    let start = (x - text_w_mm(s, size) / 2.0).clamp(2.0, PAGE_W - 2.0);
    text_left(start, y, s, size, bold, color)
}

/// Come `text_left` ma ancorato a destra: il testo termina a `x`.
fn text_right(
    x: f32,
    y: f32,
    s: &str,
    size: f32,
    bold: bool,
    color: (f32, f32, f32),
) -> Vec<Shape> {
    text_left(x - text_w_mm(s, size), y, s, size, bold, color)
}

/// Rettangolo pieno tra gli angoli (x0,y0)-(x1,y1) in mm.
fn rect_fill(x0: f32, y0: f32, x1: f32, y1: f32, color: (f32, f32, f32)) -> Vec<Shape> {
    vec![Shape::Rect {
        x0,
        y0,
        x1,
        y1,
        color,
    }]
}

/// Poligono pieno da una lista di punti (mm).
fn poly_fill(pts: &[(f32, f32)], color: (f32, f32, f32)) -> Vec<Shape> {
    vec![Shape::Poly {
        pts: pts.to_vec(),
        color,
    }]
}

/// Linea da (x0,y0) a (x1,y1) in mm.
fn line(x0: f32, y0: f32, x1: f32, y1: f32, thick: f32, color: (f32, f32, f32)) -> Vec<Shape> {
    vec![Shape::Line {
        x0,
        y0,
        x1,
        y1,
        thick,
        color,
    }]
}

/// Linea orizzontale tratteggiata da (x0,y) a (x1,y): segmenti brevi.
fn dashed_hline(x0: f32, x1: f32, y: f32, thick: f32, color: (f32, f32, f32)) -> Vec<Shape> {
    const DASH: f32 = 1.4;
    const GAP: f32 = 1.2;
    let mut out = Vec::new();
    let mut x = x0;
    while x < x1 {
        let xe = (x + DASH).min(x1);
        out.push(Shape::Line {
            x0: x,
            y0: y,
            x1: xe,
            y1: y,
            thick,
            color,
        });
        x += DASH + GAP;
    }
    out
}

/// Converte le primitive in operazioni PDF (printpdf).
fn render_pdf(fonts: &Fonts, shapes: &[Shape]) -> Vec<Op> {
    let poly_ring = |pts: &[(f32, f32)]| PolygonRing {
        points: pts
            .iter()
            .map(|(px, py)| LinePoint {
                p: Point::new(Mm(*px), Mm(*py)),
                bezier: false,
            })
            .collect(),
    };
    let mut ops = Vec::new();
    for s in shapes {
        match s {
            Shape::Rect {
                x0,
                y0,
                x1,
                y1,
                color,
            } => {
                let pts = [(*x0, *y0), (*x1, *y0), (*x1, *y1), (*x0, *y1)];
                ops.push(Op::SetFillColor { col: rgb(*color) });
                ops.push(Op::DrawPolygon {
                    polygon: Polygon {
                        rings: vec![poly_ring(&pts)],
                        mode: PaintMode::Fill,
                        winding_order: WindingOrder::NonZero,
                    },
                });
            }
            Shape::Poly { pts, color } => {
                ops.push(Op::SetFillColor { col: rgb(*color) });
                ops.push(Op::DrawPolygon {
                    polygon: Polygon {
                        rings: vec![poly_ring(pts)],
                        mode: PaintMode::Fill,
                        winding_order: WindingOrder::NonZero,
                    },
                });
            }
            Shape::Line {
                x0,
                y0,
                x1,
                y1,
                thick,
                color,
            } => {
                ops.push(Op::SetOutlineColor { col: rgb(*color) });
                ops.push(Op::SetOutlineThickness { pt: Pt(*thick) });
                ops.push(Op::DrawLine {
                    line: Line {
                        points: vec![
                            LinePoint {
                                p: Point::new(Mm(*x0), Mm(*y0)),
                                bezier: false,
                            },
                            LinePoint {
                                p: Point::new(Mm(*x1), Mm(*y1)),
                                bezier: false,
                            },
                        ],
                        is_closed: false,
                    },
                });
            }
            Shape::Text {
                x,
                y,
                s,
                size,
                bold,
                color,
            } => {
                ops.push(Op::StartTextSection);
                ops.push(Op::SetTextCursor {
                    pos: Point::new(Mm(*x), Mm(*y)),
                });
                ops.push(Op::SetFont {
                    font: fonts.handle(*bold),
                    size: Pt(*size),
                });
                ops.push(Op::SetLineHeight { lh: Pt(*size) });
                ops.push(Op::SetFillColor { col: rgb(*color) });
                ops.push(Op::ShowText {
                    items: vec![TextItem::Text(s.clone())],
                });
                ops.push(Op::EndTextSection);
            }
        }
    }
    ops
}

fn svg_color(c: (f32, f32, f32)) -> String {
    let q = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    format!("#{:02x}{:02x}{:02x}", q(c.0), q(c.1), q(c.2))
}

fn svg_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

/// Converte le primitive in un documento SVG, ritagliato al riquadro effettivo
/// del disegno (più un piccolo margine). Le coordinate PDF (origine in basso a
/// sinistra) vengono ribaltate in coordinate SVG (origine in alto a sinistra).
fn render_svg(shapes: &[Shape]) -> String {
    const PT_MM: f32 = 0.352_778; // punti → mm

    // Bounding box in coordinate PDF.
    let (mut minx, mut miny, mut maxx, mut maxy) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    let mut ext = |x: f32, y: f32| {
        minx = minx.min(x);
        maxx = maxx.max(x);
        miny = miny.min(y);
        maxy = maxy.max(y);
    };
    for s in shapes {
        match s {
            Shape::Rect { x0, y0, x1, y1, .. } | Shape::Line { x0, y0, x1, y1, .. } => {
                ext(*x0, *y0);
                ext(*x1, *y1);
            }
            Shape::Poly { pts, .. } => {
                for (px, py) in pts {
                    ext(*px, *py);
                }
            }
            Shape::Text { x, y, s, size, .. } => {
                let w = text_w_mm(s, *size);
                let h = size * PT_MM;
                ext(*x, *y - h * 0.25);
                ext(*x + w, *y + h * 0.9);
            }
        }
    }
    if minx > maxx {
        return String::from("<svg xmlns=\"http://www.w3.org/2000/svg\"/>");
    }
    let margin = 3.0;
    minx -= margin;
    miny -= margin;
    maxx += margin;
    maxy += margin;
    let (w, h) = (maxx - minx, maxy - miny);
    let tx = |x: f32| x - minx;
    let ty = |y: f32| maxy - y; // ribalta l'asse verticale

    let mut out = String::new();
    out.push_str(&format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w:.2}mm\" height=\"{h:.2}mm\" \
         viewBox=\"0 0 {w:.2} {h:.2}\" font-family=\"sans-serif\">\n"
    ));
    out.push_str(&format!(
        "<rect x=\"0\" y=\"0\" width=\"{w:.2}\" height=\"{h:.2}\" fill=\"#ffffff\"/>\n"
    ));
    for s in shapes {
        match s {
            Shape::Rect {
                x0,
                y0,
                x1,
                y1,
                color,
            } => {
                out.push_str(&format!(
                    "<rect x=\"{:.2}\" y=\"{:.2}\" width=\"{:.2}\" height=\"{:.2}\" fill=\"{}\"/>\n",
                    tx(x0.min(*x1)),
                    ty(y0.max(*y1)),
                    (x1 - x0).abs(),
                    (y1 - y0).abs(),
                    svg_color(*color)
                ));
            }
            Shape::Poly { pts, color } => {
                let p = pts
                    .iter()
                    .map(|(px, py)| format!("{:.2},{:.2}", tx(*px), ty(*py)))
                    .collect::<Vec<_>>()
                    .join(" ");
                out.push_str(&format!(
                    "<polygon points=\"{p}\" fill=\"{}\"/>\n",
                    svg_color(*color)
                ));
            }
            Shape::Line {
                x0,
                y0,
                x1,
                y1,
                thick,
                color,
            } => {
                out.push_str(&format!(
                    "<line x1=\"{:.2}\" y1=\"{:.2}\" x2=\"{:.2}\" y2=\"{:.2}\" stroke=\"{}\" \
                     stroke-width=\"{:.2}\"/>\n",
                    tx(*x0),
                    ty(*y0),
                    tx(*x1),
                    ty(*y1),
                    svg_color(*color),
                    thick * PT_MM
                ));
            }
            Shape::Text {
                x,
                y,
                s,
                size,
                bold,
                color,
            } => {
                let weight = if *bold { " font-weight=\"bold\"" } else { "" };
                out.push_str(&format!(
                    "<text x=\"{:.2}\" y=\"{:.2}\" font-size=\"{:.2}\"{} fill=\"{}\">{}</text>\n",
                    tx(*x),
                    ty(*y),
                    size * PT_MM,
                    weight,
                    svg_color(*color),
                    svg_escape(s)
                ));
            }
        }
    }
    out.push_str("</svg>\n");
    out
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

/// Formato di disegno delle barre dei dev nell'export (scelto dall'utente).
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum BarFormat {
    /// Un unico rettangolo dalla prima all'ultima settimana con effort (default,
    /// storico): i buchi interni non si vedono.
    #[default]
    Continuous,
    /// Un rettangolo per ogni tratto di settimane consecutive con effort: dove
    /// manca l'effort resta un gap. Altezza fissa.
    Segmented,
    /// Come `Segmented` ma un rettangolo per settimana, con altezza proporzionale
    /// al massimo settimanale del dev (la settimana più carica = altezza piena,
    /// pari a quella di `Continuous`/`Segmented`).
    Proportional,
}

/// Una riga del Gantt (un dev). Con effort: barra dalla prima all'ultima
/// settimana. Senza effort (`no_effort`): riga sottile fino al margine destro.
struct Row {
    label: String,
    color: (f32, f32, f32),
    start_day: i32,
    end_day: i32,
    no_effort: bool,
    /// Settimane con effort del dev, ordinate: `(giorno, ore totali della
    /// settimana)`. Vuoto se `no_effort`. Usato dai formati `Segmented` e
    /// `Proportional`.
    weeks: Vec<(i32, u32)>,
    /// Massimo delle ore settimanali del dev (denominatore per `Proportional`).
    max_week: u32,
    /// % "presunta" (usato fino a oggi / pianificato); `None` se pianificato 0.
    presumed_pct: Option<u32>,
    /// % di avanzamento dichiarata dallo sviluppatore (0 di default).
    declared_pct: u8,
}

/// Una milestone (bandierina in alto).
struct Flag {
    day: i32,
    title: String,
    date: String,
    color: (f32, f32, f32),
}

/// Riferimento (denominatore) per l'altezza delle barre `Proportional`: il
/// massimo settimanale del dev, ma mai sotto `PROPORTIONAL_REF_MIN`.
fn proportional_ref(max_week: u32) -> u32 {
    max_week.max(PROPORTIONAL_REF_MIN)
}

/// Raggruppa settimane (già ordinate per giorno) in tratti di settimane
/// consecutive — giorni adiacenti che distano esattamente 7 — restituendo il
/// `(primo_giorno, ultimo_giorno)` di ciascun tratto.
fn contiguous_runs(weeks: &[(i32, u32)]) -> Vec<(i32, i32)> {
    let mut runs = Vec::new();
    let mut it = weeks.iter();
    let Some(&(first, _)) = it.next() else {
        return runs;
    };
    let (mut run_start, mut run_end) = (first, first);
    for &(day, _) in it {
        if day - run_end == 7 {
            run_end = day;
        } else {
            runs.push((run_start, run_end));
            run_start = day;
            run_end = day;
        }
    }
    runs.push((run_start, run_end));
    runs
}

/// Genera gli `Op` di una pagina Gantt per un progetto.
fn page_shapes(
    tripletta: &str,
    descr: &str,
    proj_start: i32,
    proj_end: i32,
    rows: &[Row],
    mut flags: Vec<Flag>,
    today: i32,
    created: &str,
    chart_only: bool,
    fmt: BarFormat,
) -> Vec<Shape> {
    let mut shapes: Vec<Shape> = Vec::new();

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

    // --- Titolo + descrizione (esclusi nell'export "solo grafico") ---------
    if !chart_only {
        if !tripletta.is_empty() {
            shapes.extend(text_left(X_LABEL, 200.0, tripletta, 16.0, true, BLACK));
        }
        // Descrizione completa: ogni riga logica (a-capo espliciti) mandata a capo
        // automaticamente. Nessun limite di righe, così il testo non viene troncato.
        let mut y = if tripletta.is_empty() { 200.0 } else { 193.0 };
        for l in wrap_multiline(descr, 120) {
            shapes.extend(text_left(X_LABEL, y, &l, 9.0, false, TEXT_GRAY));
            y -= 5.0;
        }
    }

    // --- Asse dei mesi ------------------------------------------------------
    let mut m = month_start(day_to_date(axis_start));
    let mut idx = 0usize;
    loop {
        let cell_x0 = x_of(local_to_days(&m));
        let next = add_months(m, 1);
        let cell_x1 = x_of(local_to_days(&next));
        let fill = if idx % 2 == 0 { GRAY_DK } else { GRAY_LT };
        shapes.extend(rect_fill(cell_x0, AXIS_BOT, cell_x1, AXIS_TOP, fill));
        // Separatore bianco a destra della cella.
        shapes.extend(line(cell_x1, AXIS_BOT, cell_x1, AXIS_TOP, 0.6, WHITE));
        // Riga verticale del mese, attraverso la zona delle righe dev.
        shapes.extend(line(cell_x0, ROWS_BOT, cell_x0, AXIS_BOT, 0.4, GRAY_GUIDE));
        // Etichetta mese (bianca, in basso a sinistra della cella) se ci sta.
        let name = MONTHS_IT[(m.month() - 1) as usize];
        if cell_x1 - cell_x0 > text_w_mm(name, 8.0) + 1.5 {
            shapes.extend(text_left(
                cell_x0 + 1.5,
                AXIS_BOT + 1.6,
                name,
                8.0,
                false,
                WHITE,
            ));
        }
        if local_to_days(&next) >= axis_end {
            break;
        }
        m = next;
        idx += 1;
    }
    // Riga verticale di chiusura a destra dell'asse.
    shapes.extend(line(
        CHART_X1, ROWS_BOT, CHART_X1, AXIS_BOT, 0.4, GRAY_GUIDE,
    ));

    // Etichette anno agli estremi (arancione, in grassetto).
    let year_l = month_start(day_to_date(axis_start)).year();
    let year_r = add_months(month_start(day_to_date(axis_end)), -1).year();
    let axis_mid = (AXIS_BOT + AXIS_TOP) / 2.0 - 1.8;
    shapes.extend(text_right(
        CHART_X0 - 9.0,
        axis_mid,
        &year_l.to_string(),
        13.0,
        true,
        ORANGE,
    ));
    shapes.extend(text_left(
        CHART_X1 + 2.0,
        axis_mid,
        &year_r.to_string(),
        13.0,
        true,
        ORANGE,
    ));

    // --- Barra di avanzamento "Today" sull'asse ----------------------------
    let today_in_axis = today >= axis_start && today <= axis_end;
    if today_in_axis && today > proj_start {
        let xt = x_of(today);
        shapes.extend(rect_fill(
            x_of(proj_start),
            AXIS_TOP - 3.0,
            xt,
            AXIS_TOP,
            RED,
        ));
    }
    if today_in_axis {
        let xt = x_of(today);
        // Linea rossa verticale attraverso le righe.
        shapes.extend(line(xt, ROWS_BOT, xt, AXIS_BOT, 0.7, RED));
        // Triangolo + etichetta "Today" sopra l'asse.
        shapes.extend(poly_fill(
            &[
                (xt - 2.0, AXIS_TOP + 4.0),
                (xt + 2.0, AXIS_TOP + 4.0),
                (xt, AXIS_TOP),
            ],
            RED,
        ));
        shapes.extend(text_center(xt, AXIS_TOP + 5.0, "Today", 8.0, false, BLACK));
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
        shapes.extend(line(x, AXIS_TOP, x, pole_tops[i], 1.0, f.color));
    }
    // Seconda passata: pennant ed etichette, in primo piano.
    for (i, f) in flags.iter().enumerate() {
        let x = x_of(f.day);
        let pole_top = pole_tops[i];
        // Pennant: triangolo a destra dell'asta.
        shapes.extend(poly_fill(
            &[
                (x, pole_top),
                (x + 7.0, pole_top - 2.0),
                (x, pole_top - 4.0),
            ],
            f.color,
        ));
        // Etichetta sopra la bandierina, centrata sull'asta.
        for (li, (txt, size, bold)) in flag_lines(f).iter().enumerate() {
            let ly = pole_top + 2.0 + li as f32 * 3.6;
            let col = if *bold { BLACK } else { TEXT_GRAY };
            shapes.extend(text_center(x, ly, txt, *size, *bold, col));
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
        shapes.extend(text_left(X_LABEL, yc - 1.3, &label, 8.0, false, BLACK));
        let lead_x0 = X_LABEL + text_w_mm(&label, 8.0) + 2.0;

        // Dev senza effort: riga sottile del colore del dev che copre tutta la
        // larghezza del calendario (dall'inizio alla fine dell'asse dei mesi).
        if r.no_effort {
            let thin_hh = 0.4;
            shapes.extend(rect_fill(
                CHART_X0,
                yc - thin_hh,
                CHART_X1,
                yc + thin_hh,
                r.color,
            ));
            continue;
        }

        // Barra: dalla prima all'ultima settimana con effort. Estesa di una
        // settimana per dare larghezza minima visibile.
        let bx0 = x_of(r.start_day);
        let bx1 = x_of(r.end_day + 7);
        // Linea orizzontale dal nome del dev fino all'inizio della barra,
        // alternata: righe pari solida, righe dispari tratteggiata.
        if bx0 - 1.0 > lead_x0 {
            if i % 2 == 0 {
                shapes.extend(line(lead_x0, yc, bx0 - 1.0, yc, 0.3, GRAY_DK));
            } else {
                shapes.extend(dashed_hline(lead_x0, bx0 - 1.0, yc, 0.3, GRAY_DK));
            }
        }
        // Disegno della/e barra/e secondo il formato scelto.
        match fmt {
            BarFormat::Continuous => {
                // Un unico rettangolo dalla prima all'ultima settimana.
                shapes.extend(rect_fill(
                    bx0,
                    yc - bar_hh,
                    bx1.max(bx0 + 1.0),
                    yc + bar_hh,
                    r.color,
                ));
            }
            BarFormat::Segmented => {
                // Un rettangolo per ogni tratto di settimane consecutive (diff 7).
                for (s, e) in contiguous_runs(&r.weeks) {
                    let rx0 = x_of(s);
                    let rx1 = x_of(e + 7);
                    shapes.extend(rect_fill(
                        rx0,
                        yc - bar_hh,
                        rx1.max(rx0 + 1.0),
                        yc + bar_hh,
                        r.color,
                    ));
                }
            }
            BarFormat::Proportional => {
                // Un rettangolo per settimana, altezza ∝ ore/riferimento. Il
                // riferimento è il massimo settimanale del dev, ma mai sotto
                // `PROPORTIONAL_REF_MIN` ore: così una settimana da 40h è "piena"
                // solo se il dev non lavora di più altrove.
                let denom = proportional_ref(r.max_week) as f32;
                for &(day, hours) in &r.weeks {
                    let hh = (bar_hh * hours as f32 / denom).max(0.15);
                    let rx0 = x_of(day);
                    let rx1 = x_of(day + 7);
                    shapes.extend(rect_fill(
                        rx0,
                        yc - hh,
                        rx1.max(rx0 + 1.0),
                        yc + hh,
                        r.color,
                    ));
                }
            }
        }
        // Etichetta date a fine barra (o prima, se non ci sta a destra); se
        // richiesto, le percentuali (presunta/dichiarata) la seguono, tutto a
        // destra e con lo **stesso font delle date** (7.0). Le righe senza effort
        // non arrivano qui (hanno già fatto `continue`), quindi non mostrano la %.
        let mut lbl = format!("{} - {}", short_date(r.start_day), short_date(r.end_day));
        if show_pct() {
            let pct = match r.presumed_pct {
                Some(p) => format!("({p}%/{}%)", r.declared_pct),
                None => format!("(-/{}%)", r.declared_pct),
            };
            lbl = format!("{lbl}  {pct}");
        }
        let w = text_w_mm(&lbl, 7.0);
        if bx1 + 2.0 + w <= CHART_X1 {
            shapes.extend(text_left(bx1 + 2.0, yc - 1.2, &lbl, 7.0, false, TEXT_GRAY));
        } else {
            shapes.extend(text_right(bx0 - 2.0, yc - 1.2, &lbl, 7.0, false, TEXT_GRAY));
        }
    }

    // --- Footer (banda grigia + data; escluso nell'export "solo grafico") --
    if !chart_only {
        shapes.extend(rect_fill(
            X_LABEL,
            FOOTER_BOT,
            CHART_X1,
            FOOTER_TOP,
            GRAY_FOOTER,
        ));
        shapes.extend(text_center(
            (X_LABEL + CHART_X1) / 2.0,
            (FOOTER_BOT + FOOTER_TOP) / 2.0 - 1.4,
            created,
            8.0,
            false,
            TEXT_GRAY,
        ));
    }

    shapes
}

type DevInfo = std::collections::HashMap<DevId, (String, (f32, f32, f32))>;

/// Lookup dev → (nome, colore di sfondo).
fn dev_info_map(app: &App) -> DevInfo {
    app.devs
        .list_full()
        .into_iter()
        .map(|(id, name, bg, _)| (id, (name, u32_rgb(bg as u32))))
        .collect()
}

/// Nome (descrizione) di un progetto.
fn project_name(app: &App, proj: ProjectId) -> String {
    app.projects
        .list_full()
        .into_iter()
        .find(|(id, _, _)| *id == proj)
        .map(|(_, n, _)| n)
        .unwrap_or_default()
}

/// Primitive di disegno di una pagina Gantt per un progetto. `order`:
/// - `None` → dev con effort ordinati per data (usato dall'export "tutti i progetti");
/// - `Some(devs)` → dev nell'ordine dato (i senza effort → riga sottile).
///
/// `chart_only` esclude tripletta/descrizione e la banda della data in fondo.
/// `None` se il progetto non ha inizio E fine.
fn project_shapes(
    app: &App,
    proj: ProjectId,
    name: &str,
    dev_info: &DevInfo,
    today: i32,
    created: &str,
    order: Option<&[DevId]>,
    chart_only: bool,
    fmt: BarFormat,
) -> Option<Vec<Shape>> {
    let (Some(start_w), Some(end_w)) = (
        app.projects.get_project_start_week(proj),
        app.projects.get_project_end_week(proj),
    ) else {
        return None;
    };
    let proj_start = start_w.0 as i32;
    let proj_end = end_w.0 as i32;
    let color_of = |dev_id: &DevId| {
        dev_info
            .get(dev_id)
            .cloned()
            .unwrap_or_else(|| ("?".to_string(), BLACK))
    };
    // Settimane con effort del dev: `(giorno, ore totali)` ordinate, + massimo.
    let week_data = |dev_id: DevId| -> (Vec<(i32, u32)>, u32) {
        let Some(sd) = app.projects.get_single_dev(proj, dev_id) else {
            return (Vec::new(), 0);
        };
        let weeks: Vec<(i32, u32)> = sd
            .effort_weeks()
            .into_iter()
            .map(|w| (w.0 as i32, sd.get_effort_by_week(w).0 as u32))
            .collect();
        let max = weeks.iter().map(|(_, h)| *h).max().unwrap_or(0);
        (weeks, max)
    };
    // % presunta (usato fino a oggi / pianificato) e % dichiarata dal dev.
    let pct_data = |dev_id: DevId| -> (Option<u32>, u8) {
        let Some(sd) = app.projects.get_single_dev(proj, dev_id) else {
            return (None, 0);
        };
        let planned = sd.planned_effort().0;
        let presumed = (planned != 0).then(|| {
            let used = sd.effort_up_to(WeekId(today as usize)).0;
            ((used * 100 + planned / 2) / planned) as u32
        });
        (presumed, sd.declared_pct())
    };

    let mut rows = Vec::new();
    match order {
        None => {
            for dev_id in app.projects.get_dev_ids(proj) {
                let Some(sd) = app.projects.get_single_dev(proj, dev_id) else {
                    continue;
                };
                let Some((first, last)) = sd.effort_span() else {
                    continue;
                };
                let (label, color) = color_of(&dev_id);
                let (weeks, max_week) = week_data(dev_id);
                let (presumed_pct, declared_pct) = pct_data(dev_id);
                rows.push(Row {
                    label,
                    color,
                    start_day: first.0 as i32,
                    end_day: last.0 as i32,
                    no_effort: false,
                    weeks,
                    max_week,
                    presumed_pct,
                    declared_pct,
                });
            }
            rows.sort_by_key(|r| (r.start_day, r.end_day));
        }
        Some(devs) => {
            for &dev_id in devs {
                let (label, color) = color_of(&dev_id);
                match app
                    .projects
                    .get_single_dev(proj, dev_id)
                    .and_then(|sd| sd.effort_span())
                {
                    Some((first, last)) => {
                        let (weeks, max_week) = week_data(dev_id);
                        let (presumed_pct, declared_pct) = pct_data(dev_id);
                        rows.push(Row {
                            label,
                            color,
                            start_day: first.0 as i32,
                            end_day: last.0 as i32,
                            no_effort: false,
                            weeks,
                            max_week,
                            presumed_pct,
                            declared_pct,
                        })
                    }
                    None => {
                        let (presumed_pct, declared_pct) = pct_data(dev_id);
                        rows.push(Row {
                            label,
                            color,
                            start_day: proj_start,
                            end_day: proj_end,
                            no_effort: true,
                            weeks: Vec::new(),
                            max_week: 0,
                            presumed_pct,
                            declared_pct,
                        })
                    }
                }
            }
        }
    }

    let mut flags = Vec::new();
    for (mid, week) in app.projects.list_project_milestones(proj) {
        let mname = app.milestones.get_name(mid).unwrap_or("?").to_string();
        let mcol = app.milestones.get_color(mid).map(u32_rgb).unwrap_or(BLACK);
        flags.push(Flag {
            day: week.0 as i32,
            title: mname,
            date: short_date(week.0 as i32),
            color: mcol,
        });
    }

    let tripletta = app.projects.get_tripletta(proj);
    Some(page_shapes(
        &tripletta, name, proj_start, proj_end, &rows, flags, today, created, chart_only, fmt,
    ))
}

/// Costruisce il PDF con una pagina per ogni progetto visibile (abilitato e non
/// chiuso) dotato di data di inizio E fine. `None` se nessun progetto è idoneo.
pub fn build_pdf(app: &App, fmt: BarFormat) -> Option<Vec<u8>> {
    let mut doc = PdfDocument::new("Progetti");
    let regular = ParsedFont::from_bytes(FONT_REGULAR, 0, &mut Vec::new())?;
    let bold = ParsedFont::from_bytes(FONT_BOLD, 0, &mut Vec::new())?;
    let fonts = Fonts {
        regular: doc.add_font(&regular),
        bold: doc.add_font(&bold),
    };
    let created = chrono::Local::now().format("%Y-%m-%d").to_string();
    let today = local_to_days(&chrono::Local::now().date_naive());
    let dev_info = dev_info_map(app);

    let mut pages = Vec::new();
    for (id, name, enable) in app.projects.list_full() {
        if !enable.0 || app.projects.is_closed(id) {
            continue;
        }
        if let Some(shapes) =
            project_shapes(app, id, &name, &dev_info, today, &created, None, false, fmt)
        {
            pages.push(PdfPage::new(
                Mm(PAGE_W),
                Mm(PAGE_H),
                render_pdf(&fonts, &shapes),
            ));
        }
    }

    if pages.is_empty() {
        return None;
    }
    let bytes = doc
        .with_pages(pages)
        .save(&PdfSaveOptions::default(), &mut Vec::new());
    Some(bytes)
}

/// Come `build_pdf`, ma limitato ai soli progetti indicati in `selected`
/// (una pagina Gantt ciascuno, nell'ordine di visualizzazione). Restano validi
/// i vincoli per pagina: il progetto deve avere inizio E fine. `None` se nessun
/// progetto selezionato produce una pagina.
pub fn build_pdf_selected(app: &App, selected: &[ProjectId], fmt: BarFormat) -> Option<Vec<u8>> {
    let mut doc = PdfDocument::new("Progetti");
    let regular = ParsedFont::from_bytes(FONT_REGULAR, 0, &mut Vec::new())?;
    let bold = ParsedFont::from_bytes(FONT_BOLD, 0, &mut Vec::new())?;
    let fonts = Fonts {
        regular: doc.add_font(&regular),
        bold: doc.add_font(&bold),
    };
    let created = chrono::Local::now().format("%Y-%m-%d").to_string();
    let today = local_to_days(&chrono::Local::now().date_naive());
    let dev_info = dev_info_map(app);

    let mut pages = Vec::new();
    for (id, name, enable) in app.projects.list_full() {
        // I chiusi non sono esclusi a priori: la selezione a monte riflette la
        // modalità Vista (può includere progetti chiusi).
        if !selected.contains(&id) || !enable.0 {
            continue;
        }
        if let Some(shapes) =
            project_shapes(app, id, &name, &dev_info, today, &created, None, false, fmt)
        {
            pages.push(PdfPage::new(
                Mm(PAGE_W),
                Mm(PAGE_H),
                render_pdf(&fonts, &shapes),
            ));
        }
    }

    if pages.is_empty() {
        return None;
    }
    let bytes = doc
        .with_pages(pages)
        .save(&PdfSaveOptions::default(), &mut Vec::new());
    Some(bytes)
}

/// Costruisce un PDF di **un solo progetto**, con i dev nell'ordine `ordered_devs`
/// scelto dall'utente. I dev con effort producono la barra normale; quelli senza
/// effort una riga sottile del colore del dev per tutta la larghezza del calendario.
/// Con `ordered_devs` vuoto esporta comunque la pagina (asse, milestone, today…)
/// senza righe dev. `None` solo se il progetto non ha inizio E fine.
pub fn build_pdf_project(
    app: &App,
    proj: ProjectId,
    ordered_devs: &[DevId],
    fmt: BarFormat,
) -> Option<Vec<u8>> {
    let created = chrono::Local::now().format("%Y-%m-%d").to_string();
    let today = local_to_days(&chrono::Local::now().date_naive());
    let dev_info = dev_info_map(app);
    let name = project_name(app, proj);
    let shapes = project_shapes(
        app,
        proj,
        &name,
        &dev_info,
        today,
        &created,
        Some(ordered_devs),
        false,
        fmt,
    )?;

    let mut doc = PdfDocument::new("Progetto");
    let regular = ParsedFont::from_bytes(FONT_REGULAR, 0, &mut Vec::new())?;
    let bold = ParsedFont::from_bytes(FONT_BOLD, 0, &mut Vec::new())?;
    let fonts = Fonts {
        regular: doc.add_font(&regular),
        bold: doc.add_font(&bold),
    };
    let page = PdfPage::new(Mm(PAGE_W), Mm(PAGE_H), render_pdf(&fonts, &shapes));
    let bytes = doc
        .with_pages(vec![page])
        .save(&PdfSaveOptions::default(), &mut Vec::new());
    Some(bytes)
}

/// Esporta il **solo grafico** di un progetto in SVG: come il PDF a singolo
/// progetto ma senza tripletta/descrizione e senza la data in fondo. `None` se il
/// progetto non ha inizio E fine.
pub fn build_svg_project(
    app: &App,
    proj: ProjectId,
    ordered_devs: &[DevId],
    fmt: BarFormat,
) -> Option<String> {
    let today = local_to_days(&chrono::Local::now().date_naive());
    let dev_info = dev_info_map(app);
    let name = project_name(app, proj);
    let shapes = project_shapes(
        app,
        proj,
        &name,
        &dev_info,
        today,
        "",
        Some(ordered_devs),
        true, // solo grafico
        fmt,
    )?;
    Some(render_svg(&shapes))
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
        assert!(build_pdf(&app, BarFormat::Continuous).is_none());
    }

    #[test]
    fn project_with_start_and_end_produces_valid_pdf() {
        let mut app = App::new();
        let start = WeekId(20000);
        let pid = app
            .projects
            .add("Descrizione progetto", Some("ABC"), Some(start));
        app.projects.set_project_end_week(pid, Some(WeekId(20070)));
        // Un dev con effort su alcune settimane → una riga del Gantt.
        let dev = app.devs.add("Frontend");
        let worker = WorkerId(0);
        app.projects.add_dev(pid, dev);
        app.projects
            .add_effort(pid, dev, WeekId(20007), worker, Effort(8));
        app.projects
            .add_effort(pid, dev, WeekId(20035), worker, Effort(8));
        let mid = app.milestones.add("Beta");
        app.projects.add_project_milestone(pid, mid, WeekId(20035));

        let bytes = build_pdf(&app, BarFormat::Continuous).expect("un progetto idoneo → Some");
        assert!(
            bytes.starts_with(b"%PDF"),
            "l'output deve essere un PDF valido"
        );
    }

    #[test]
    fn project_without_end_is_skipped() {
        let mut app = App::new();
        app.projects
            .add("Senza fine", Some("XYZ"), Some(WeekId(20000)));
        assert!(build_pdf(&app, BarFormat::Continuous).is_none());
    }

    #[test]
    fn build_pdf_selected_only_includes_selected_projects() {
        let mut app = App::new();
        let a = app
            .projects
            .add("Progetto A", Some("AAA"), Some(WeekId(20000)));
        app.projects.set_project_end_week(a, Some(WeekId(20070)));
        let b = app
            .projects
            .add("Progetto B", Some("BBB"), Some(WeekId(20000)));
        app.projects.set_project_end_week(b, Some(WeekId(20070)));

        // Solo A selezionato → PDF valido (una pagina).
        let bytes = build_pdf_selected(&app, &[a], BarFormat::Continuous)
            .expect("progetto selezionato idoneo → Some");
        assert!(bytes.starts_with(b"%PDF"));

        // Nessun progetto selezionato → None.
        assert!(build_pdf_selected(&app, &[], BarFormat::Continuous).is_none());

        // Progetto selezionato ma senza fine → None (non idoneo).
        let c = app
            .projects
            .add("Senza fine", Some("CCC"), Some(WeekId(20000)));
        assert!(build_pdf_selected(&app, &[c], BarFormat::Continuous).is_none());
    }

    #[test]
    fn single_project_with_effortless_dev_produces_valid_pdf() {
        let mut app = App::new();
        let pid = app.projects.add("Prog", Some("ABC"), Some(WeekId(20000)));
        app.projects.set_project_end_week(pid, Some(WeekId(20070)));
        // Un dev con effort e uno senza (solo aggiunto al progetto).
        let dev_eff = app.devs.add("Frontend");
        let dev_empty = app.devs.add("Backend");
        app.projects.add_dev(pid, dev_eff);
        app.projects.add_dev(pid, dev_empty);
        app.projects
            .add_effort(pid, dev_eff, WeekId(20007), WorkerId(0), Effort(8));

        // Ordine scelto dall'utente: prima il dev senza effort.
        let bytes = build_pdf_project(&app, pid, &[dev_empty, dev_eff], BarFormat::Continuous)
            .expect("progetto con inizio/fine e dev → Some");
        assert!(bytes.starts_with(b"%PDF"));

        // Nessun dev selezionato → esporta comunque il resto (milestone, asse…).
        let bytes =
            build_pdf_project(&app, pid, &[], BarFormat::Continuous).expect("senza dev → Some");
        assert!(bytes.starts_with(b"%PDF"));
    }

    #[test]
    fn single_project_without_end_returns_none() {
        let mut app = App::new();
        let pid = app.projects.add("Prog", Some("ABC"), Some(WeekId(20000)));
        // niente fine → None anche col percorso singolo progetto
        assert!(build_pdf_project(&app, pid, &[], BarFormat::Continuous).is_none());
    }

    #[test]
    fn svg_export_is_chart_only_valid_svg() {
        let mut app = App::new();
        let pid = app.projects.add(
            "Descrizione lunga del progetto",
            Some("ABC"),
            Some(WeekId(20000)),
        );
        app.projects.set_project_end_week(pid, Some(WeekId(20070)));
        let dev = app.devs.add("Frontend");
        app.projects.add_dev(pid, dev);
        app.projects
            .add_effort(pid, dev, WeekId(20007), WorkerId(0), Effort(8));

        let svg = build_svg_project(&app, pid, &[dev], BarFormat::Continuous)
            .expect("progetto valido → Some");
        assert!(svg.trim_start().starts_with("<svg"), "deve essere un SVG");
        assert!(svg.contains("</svg>"));
        // "solo grafico": niente tripletta né descrizione del progetto.
        assert!(!svg.contains("ABC"), "la tripletta non deve comparire");
        assert!(
            !svg.contains("Descrizione lunga"),
            "la descrizione non deve comparire"
        );

        // Senza inizio/fine → None.
        let p2 = app.projects.add("NoFine", Some("ZZZ"), Some(WeekId(20000)));
        assert!(build_svg_project(&app, p2, &[], BarFormat::Continuous).is_none());
    }

    #[test]
    fn show_pct_toggle_adds_percentages_to_svg() {
        let mut app = App::new();
        let pid = app.projects.add("Prog", Some("ABC"), Some(WeekId(20000)));
        app.projects.set_project_end_week(pid, Some(WeekId(20070)));
        let dev = app.devs.add("Frontend");
        app.projects.add_dev(pid, dev);
        // Pianificato 10h, usato 8h in una settimana passata → presunta 80%.
        app.projects.add_dev_effort(pid, dev, Effort(10));
        app.projects
            .add_effort(pid, dev, WeekId(20007), WorkerId(0), Effort(8));
        app.projects.set_dev_declared_pct(pid, dev, 60);
        // Dev SENZA effort ma con % dichiarata: non deve mostrare percentuali.
        let dev_empty = app.devs.add("Backend");
        app.projects.add_dev(pid, dev_empty);
        app.projects.set_dev_declared_pct(pid, dev_empty, 33);

        // Flag off (default): niente percentuali.
        set_show_pct(false);
        let svg = build_svg_project(&app, pid, &[dev, dev_empty], BarFormat::Continuous).unwrap();
        assert!(!svg.contains("80%/60%"), "senza flag non devono comparire");

        // Flag on: compare "presunta/dichiarata" solo per il dev con effort.
        set_show_pct(true);
        let svg = build_svg_project(&app, pid, &[dev, dev_empty], BarFormat::Continuous).unwrap();
        assert!(svg.contains("80%/60%"), "con flag deve comparire 80%/60%");
        assert!(!svg.contains("33%"), "il dev senza effort non mostra la %");
        set_show_pct(false);
    }

    #[test]
    fn proportional_reference_never_below_40() {
        assert_eq!(proportional_ref(0), 40, "dev scarico → riferimento 40");
        assert_eq!(
            proportional_ref(20),
            40,
            "max 20h → riferimento comunque 40"
        );
        assert_eq!(proportional_ref(40), 40);
        assert_eq!(
            proportional_ref(56),
            56,
            "oltre 40 → il massimo reale del dev"
        );
    }

    #[test]
    fn contiguous_runs_groups_consecutive_weeks() {
        // 20007 e 20014 contigui (dist 7); 20035 isolato (gap).
        let weeks = [(20007, 8), (20014, 40), (20035, 16)];
        assert_eq!(
            contiguous_runs(&weeks),
            vec![(20007, 20014), (20035, 20035)]
        );
        assert!(contiguous_runs(&[]).is_empty());
        assert_eq!(contiguous_runs(&[(100, 5)]), vec![(100, 100)]);
    }

    #[test]
    fn segmented_and_proportional_formats_produce_valid_output() {
        let mut app = App::new();
        let pid = app.projects.add("Prog", Some("ABC"), Some(WeekId(20000)));
        app.projects.set_project_end_week(pid, Some(WeekId(20070)));
        let dev = app.devs.add("Frontend");
        app.projects.add_dev(pid, dev);
        // Due settimane contigue (una con due worker → somma), un buco, poi un'altra.
        app.projects
            .add_effort(pid, dev, WeekId(20007), WorkerId(0), Effort(8));
        app.projects
            .add_effort(pid, dev, WeekId(20007), WorkerId(1), Effort(4));
        app.projects
            .add_effort(pid, dev, WeekId(20014), WorkerId(0), Effort(40));
        app.projects
            .add_effort(pid, dev, WeekId(20035), WorkerId(0), Effort(16));

        // Conteggio dei rettangoli nell'SVG: tutto è identico tra i formati
        // tranne le barre del dev, quindi il totale isola il numero di barre.
        // 3 settimane con effort, 2 tratti contigui (20007-20014 e 20035):
        //   Continua = 1 barra, Segmentata = 2, Proporzionale = 3 (una a settimana).
        let rects = |fmt| {
            build_svg_project(&app, pid, &[dev], fmt)
                .unwrap()
                .matches("<rect")
                .count()
        };
        let (cont, seg, prop) = (
            rects(BarFormat::Continuous),
            rects(BarFormat::Segmented),
            rects(BarFormat::Proportional),
        );
        assert_eq!(
            seg - cont,
            1,
            "Segmentata deve avere 1 barra in più (2 tratti vs 1)"
        );
        assert_eq!(
            prop - cont,
            2,
            "Proporzionale deve avere 2 barre in più (3 settimane vs 1)"
        );

        for fmt in [BarFormat::Segmented, BarFormat::Proportional] {
            let bytes = build_pdf_project(&app, pid, &[dev], fmt).expect("PDF valido");
            assert!(bytes.starts_with(b"%PDF"));
            let svg = build_svg_project(&app, pid, &[dev], fmt).expect("SVG valido");
            assert!(svg.contains("</svg>"));
        }
    }
}
