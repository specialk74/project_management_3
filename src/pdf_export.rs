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
use crate::date_utils::dates::{days_to_local, local_to_days, primo_giorno_settimana_corrente};
use crate::dev_utils::dev::DevId;
use crate::milestones::{MilestoneId, MilestoneScope};
use crate::project_utils::project::ProjectId;
use crate::single_dev_utils::single_dev::{DeclaredPoint, WeekId};

// Flag di rendering "mostra percentuali di avanzamento" nell'export, scelto
// dall'utente nelle dialog. Thread-local come i flag tema/B-N di `ui_style`:
// impostato prima di ogni `build_*`, letto da `page_shapes`. Default: off.
thread_local! {
    static SHOW_PCT: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    // Ambito milestone dell'export corrente (vedi `MilestoneScope`): decide
    // quali bandierine finiscono nella pagina. Default: Internal (= tutte),
    // cioè il comportamento storico.
    static MS_SCOPE: std::cell::Cell<MilestoneScope> =
        const { std::cell::Cell::new(MilestoneScope::Internal) };
    // Milestone ammesse nell'export corrente, scelte una per una nella dialog
    // del singolo progetto. `None` = nessun filtro (tutte quelle collocate nel
    // progetto), che è il default e il caso dell'export multi-progetto.
    static MS_ALLOW: std::cell::RefCell<Option<std::collections::HashSet<MilestoneId>>> =
        const { std::cell::RefCell::new(None) };
    // Stampa dei worker "ghost": `true` (default, comportamento storico) marca
    // in rosso nome dev e settimane col ghost — comprese quelle in cui è
    // assegnato a **effort 0** — e fa comparire anche il dev che ha solo un
    // ghost senza ore; `false` toglie del tutto la marcatura.
    static SHOW_GHOST: std::cell::Cell<bool> = const { std::cell::Cell::new(true) };
    // Smussatura degli angoli dei rettangoli "pieni" (barre effort e celle dei
    // mesi), in percentuale: 0 = spigolo vivo, 100 = raggio massimo (metà del
    // lato corto → estremi semicircolari). Viene dal file .ron (`App.corner_pct`)
    // e la imposta `project_shapes` a ogni pagina.
    static CORNER_PCT: std::cell::Cell<i32> = const { std::cell::Cell::new(CORNER_PCT_DEFAULT) };
}

/// Smussatura predefinita degli angoli, in percentuale del raggio massimo.
/// È il valore scritto nel `.ron` quando il campo manca.
pub const CORNER_PCT_DEFAULT: i32 = 40;

/// Imposta la smussatura degli angoli (0–100, valori fuori intervallo vengono
/// riportati nei limiti — `App::validate` li segnala all'utente).
pub fn set_corner_pct(v: i32) {
    CORNER_PCT.with(|c| c.set(v.clamp(0, 100)));
}

fn corner_pct() -> i32 {
    CORNER_PCT.with(|c| c.get())
}

/// Imposta se le percentuali (presunta/dichiarata) vanno disegnate nell'export.
pub fn set_show_pct(v: bool) {
    SHOW_PCT.with(|c| c.set(v));
}

fn show_pct() -> bool {
    SHOW_PCT.with(|c| c.get())
}

/// Imposta l'ambito milestone del prossimo `build_*`: `Internal` disegna tutte
/// le milestone (Internal + External), `External` solo quelle External.
pub fn set_milestone_scope(v: MilestoneScope) {
    MS_SCOPE.with(|c| c.set(v));
}

fn milestone_scope() -> MilestoneScope {
    MS_SCOPE.with(|c| c.get())
}

/// Limita l'export alle milestone indicate (`None` = tutte). Si combina in
/// **AND** con l'ambito: una milestone è disegnata se è nell'elenco ammesso
/// **e** rientra nell'ambito del file.
pub fn set_milestone_allow(ids: Option<Vec<MilestoneId>>) {
    MS_ALLOW.with(|c| *c.borrow_mut() = ids.map(|v| v.into_iter().collect()));
}

/// Imposta se l'export deve stampare i worker "ghost" (default: sì).
pub fn set_show_ghost(v: bool) {
    SHOW_GHOST.with(|c| c.set(v));
}

fn show_ghost() -> bool {
    SHOW_GHOST.with(|c| c.get())
}

/// La milestone è ammessa dall'elenco scelto dall'utente?
fn milestone_allowed(id: MilestoneId) -> bool {
    MS_ALLOW.with(|c| {
        c.borrow()
            .as_ref()
            .map(|set| set.contains(&id))
            .unwrap_or(true)
    })
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

// Zona righe dev (tra l'asse e il footer). Abbassata rispetto all'asse per
// lasciare una fascia [ROWS_TOP, AXIS_BOT] in cui sta l'etichetta "Today"
// (triangolo + testo) senza mai coprire le barre dei dev.
const ROWS_TOP: f32 = 108.0;
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

/// Punti (in senso antiorario) di un rettangolo con gli **angoli arrotondati**,
/// approssimati con `SEG` segmenti per angolo: il raggio è
/// `corner_pct()%` di metà lato corto, quindi al 100% gli estremi diventano
/// semicerchi e allo 0% si torna allo spigolo vivo.
fn round_rect_pts(x0: f32, y0: f32, x1: f32, y1: f32) -> Vec<(f32, f32)> {
    const SEG: usize = 6;
    let (x0, x1) = (x0.min(x1), x0.max(x1));
    let (y0, y1) = (y0.min(y1), y0.max(y1));
    let r = (corner_pct() as f32 / 100.0) * (x1 - x0).min(y1 - y0) / 2.0;
    let mut pts = Vec::with_capacity(4 * (SEG + 1));
    // (centro dell'arco, angolo iniziale) per i quattro angoli, in senso orario
    // partendo da quello in basso a sinistra.
    let corners = [
        ((x0 + r, y0 + r), std::f32::consts::PI),
        ((x1 - r, y0 + r), 1.5 * std::f32::consts::PI),
        ((x1 - r, y1 - r), 0.0),
        ((x0 + r, y1 - r), 0.5 * std::f32::consts::PI),
    ];
    for ((cx, cy), a0) in corners {
        for i in 0..=SEG {
            let a = a0 + (i as f32 / SEG as f32) * 0.5 * std::f32::consts::PI;
            pts.push((cx + r * a.cos(), cy + r * a.sin()));
        }
    }
    pts
}

/// Rettangolo pieno con gli angoli **smussati** secondo l'impostazione del file
/// (`App.corner_pct`): usato per le barre dell'effort e le celle dei mesi. Con
/// smussatura 0 resta un rettangolo vero e proprio.
fn rect_round(x0: f32, y0: f32, x1: f32, y1: f32, color: (f32, f32, f32)) -> Vec<Shape> {
    if corner_pct() <= 0 {
        return rect_fill(x0, y0, x1, y1, color);
    }
    poly_fill(&round_rect_pts(x0, y0, x1, y1), color)
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
    /// Giorni (settimane) in cui un worker "ghost" ha effort > 0: quei tratti
    /// della barra vengono ridisegnati in rosso, anche se in mezzo alla barra.
    ghost_weeks: Vec<i32>,
}

/// Una milestone (bandierina in alto).
struct Flag {
    day: i32,
    title: String,
    date: String,
    color: (f32, f32, f32),
    /// Milestone di tipo *trigger* (evento): in cima all'asta, al posto del
    /// pennant triangolare, si disegna un mini fulmine.
    trigger: bool,
}

/// Punta dell'asta di una milestone, disegnata in cima al palo con l'angolo in
/// basso a sinistra in `(x, y)`: pennant triangolare per i traguardi, mini
/// fulmine per i trigger di evento. Entrambi occupano la stessa fascia (a
/// destra dell'asta, `PENNANT_H` mm di altezza) così la disposizione ad arco e
/// le etichette non cambiano.
const PENNANT_W: f32 = 7.0;
const PENNANT_H: f32 = 4.0;

fn pole_tip(x: f32, y: f32, trigger: bool, color: (f32, f32, f32)) -> Vec<Shape> {
    if !trigger {
        // Traguardo: triangolo con la punta verso l'asse (comportamento storico).
        return poly_fill(
            &[
                (x, y),
                (x + PENNANT_W, y - PENNANT_H / 2.0),
                (x, y - PENNANT_H),
            ],
            color,
        );
    }
    // Trigger: fulmine a zig-zag, poligono semplice in coordinate normalizzate
    // (0,0) = angolo in basso a sinistra, (1,1) = in alto a destra della fascia.
    const BOLT: [(f32, f32); 6] = [
        (0.60, 1.00), // punta in alto
        (0.10, 0.42),
        (0.42, 0.42),
        (0.28, 0.00), // punta in basso
        (0.90, 0.58),
        (0.55, 0.58),
    ];
    // Un filo più alto del pennant: il fulmine è stretto e va letto a colpo d'occhio.
    let w = PENNANT_W * 0.8;
    let h = PENNANT_H * 1.3;
    let pts: Vec<(f32, f32)> = BOLT
        .iter()
        .map(|(ux, uy)| (x + ux * w, y - h + uy * h))
        .collect();
    poly_fill(&pts, color)
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
    progress_pct: Option<u8>,
    presumed_pct: Option<u32>,
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
        shapes.extend(rect_round(cell_x0, AXIS_BOT, cell_x1, AXIS_TOP, fill));
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
        // Angoli smussati come le barre dell'effort (stesso `corner_pct`).
        shapes.extend(rect_round(
            x_of(proj_start),
            AXIS_TOP - 3.0,
            xt,
            AXIS_TOP,
            RED,
        ));
    }
    if today_in_axis {
        let xt = x_of(today);
        // Linea rossa verticale attraverso le righe, fino al bordo inferiore
        // dell'asse (dove si aggancia il triangolo "Today").
        shapes.extend(line(xt, ROWS_BOT, xt, AXIS_BOT, 0.7, RED));
        // Triangolo "Today" SOTTO l'asse, con la punta rivolta verso l'ALTO
        // (verso la barra dei mesi).
        shapes.extend(poly_fill(
            &[
                (xt - 2.5, AXIS_BOT - 4.0),
                (xt + 2.5, AXIS_BOT - 4.0),
                (xt, AXIS_BOT),
            ],
            RED,
        ));
        // Etichetta "Today" (grassetto) sotto la barra dei mesi, allineata a
        // SINISTRA appena a destra della riga rossa verticale (così la linea non
        // ci passa in mezzo); sotto, solo col toggle attivo, l'avanzamento
        // "(presunta%/attuale%)" al font 7.0. Sta nella fascia tra l'asse e le
        // righe dev, così non copre le barre.
        let lx = xt + 2.0;
        match (show_pct(), presumed_pct, progress_pct) {
            (true, Some(pres), Some(act)) => {
                shapes.extend(text_left(lx, AXIS_BOT - 8.5, "Today", 8.0, true, BLACK));
                shapes.extend(text_left(
                    lx,
                    AXIS_BOT - 12.0,
                    &format!("({pres}%/{act}%)"),
                    7.0,
                    false,
                    BLACK,
                ));
            }
            _ => shapes.extend(text_left(lx, AXIS_BOT - 8.5, "Today", 8.0, true, BLACK)),
        }
    }

    // --- Milestone come bandierine -----------------------------------------
    // Righe dell'etichetta (dal basso: data, poi nome una parola per riga).
    // Font di 1pt più piccolo del passato: scritte più compatte e strette, così
    // le bandierine vicine si sovrappongono meno (anche in orizzontale).
    const FLAG_LINE_H: f32 = 3.2; // interlinea tra le righe dell'etichetta
    let flag_lines = |f: &Flag| -> Vec<(String, f32, bool)> {
        let mut v = vec![(f.date.clone(), 6.0, false)];
        for w in f.title.split_whitespace().rev() {
            v.push((w.to_string(), 7.0, true));
        }
        v
    };

    flags.sort_by_key(|f| f.day);

    // Posizionamento "a ventaglio": tutte le bandierine vanno verso l'ALTO, ma
    // invece di allungarne l'asta per evitare le collisioni si SPARPAGLIANO le
    // etichette in orizzontale. La base di ogni asta resta ancorata alla data
    // vera sull'asse (breve tratto verticale) e poi l'asta si inclina fino al
    // centro dell'etichetta, spostato quel tanto che basta perché le etichette
    // vicine non si sovrappongano.
    let n = flags.len();
    let mut anchor = Vec::with_capacity(n); // x sull'asse (data vera)
    let mut half_w = Vec::with_capacity(n); // semilarghezza etichetta
    for f in &flags {
        anchor.push(x_of(f.day));
        let lines = flag_lines(f);
        half_w.push(
            lines
                .iter()
                .map(|(t, s, _)| text_w_mm(t, *s))
                .fold(0.0f32, f32::max)
                / 2.0
                + 1.5,
        );
    }

    // Centri delle etichette lx[i]: il più vicino possibile all'anchor ma con una
    // distanza minima tra centri consecutivi (somma delle semilarghezze + margine)
    // così non si sovrappongono. È una regressione isotona risolta con "pool
    // adjacent violators": produce uno sparpagliamento centrato, senza incroci tra
    // le aste (l'ordine per data è preservato).
    const FLAG_GAP: f32 = 1.5;
    let mut lx = anchor.clone();
    if n >= 2 {
        let sep: Vec<f32> = (0..n - 1)
            .map(|i| half_w[i] + half_w[i + 1] + FLAG_GAP)
            .collect();
        // Prefissi delle separazioni: S[i] = Σ sep[0..i].
        let mut s_pref = vec![0.0f32; n];
        for i in 1..n {
            s_pref[i] = s_pref[i - 1] + sep[i - 1];
        }
        // PAVA su t[i] = anchor[i] - S[i] a pesi unitari → sequenza non decrescente.
        let mut blocks: Vec<(f32, usize)> = Vec::with_capacity(n); // (somma, conteggio)
        for i in 0..n {
            blocks.push((anchor[i] - s_pref[i], 1));
            while blocks.len() >= 2 {
                let (sum2, cnt2) = blocks[blocks.len() - 1];
                let (sum1, cnt1) = blocks[blocks.len() - 2];
                if sum1 / cnt1 as f32 > sum2 / cnt2 as f32 {
                    blocks.truncate(blocks.len() - 2);
                    blocks.push((sum1 + sum2, cnt1 + cnt2));
                } else {
                    break;
                }
            }
        }
        // Riespande i blocchi: lx[i] = media del blocco + S[i].
        let mut i = 0usize;
        for &(sum, cnt) in &blocks {
            let mean = sum / cnt as f32;
            for _ in 0..cnt {
                lx[i] = mean + s_pref[i];
                i += 1;
            }
        }
    }

    // Quota del pennant: tratto verticale corto ancorato alla data + tratto
    // inclinato + tratto verticale corto finale su cui si aggancia la bandierina.
    // Le bandierine sono disposte a PARABOLA con la CONCA VERSO IL BASSO (arco a
    // duomo): la centrale è la più alta, quelle ai lati le più basse. La salita
    // del tratto inclinato segue 1 − t², con t da −1 a +1 lungo le bandierine
    // ordinate per data. Un tetto evita che le etichette invadano il titolo.
    // (Per tornare al ventaglio con la conca verso l'alto basta rimettere la
    // salita ∝ |lx − anchor|.)
    const FLAG_BASE_V: f32 = 5.0; // tratto verticale in basso (ancoraggio data)
    const FLAG_TOP_V: f32 = 6.0; // tratto verticale in alto (aggancio pennant)
    const FLAG_ARCH_MIN: f32 = 10.0; // salita del tratto inclinato ai lati
    const FLAG_ARCH_H: f32 = 25.0; // salita aggiuntiva al centro dell'arco
    const FLAG_TOP_MAX: f32 = 176.0; // quota massima del pennant
    let pole_top_y: Vec<f32> = (0..n)
        .map(|i| {
            // t ∈ [-1, 1] lungo le bandierine; 1 − t² = 1 al centro, 0 ai bordi.
            let t = if n > 1 {
                i as f32 / (n as f32 - 1.0) * 2.0 - 1.0
            } else {
                0.0
            };
            (AXIS_TOP + FLAG_BASE_V + FLAG_TOP_V + FLAG_ARCH_MIN + FLAG_ARCH_H * (1.0 - t * t))
                .min(FLAG_TOP_MAX)
        })
        .collect();

    // Prima passata: le aste (verticale in basso → inclinata → verticale in alto),
    // in secondo piano così non coprono le etichette delle bandierine vicine.
    for (i, f) in flags.iter().enumerate() {
        let a = anchor[i];
        let knee_bot = AXIS_TOP + FLAG_BASE_V;
        let knee_top = pole_top_y[i] - FLAG_TOP_V;
        shapes.extend(line(a, AXIS_TOP, a, knee_bot, 1.0, f.color));
        shapes.extend(line(a, knee_bot, lx[i], knee_top, 1.0, f.color));
        shapes.extend(line(lx[i], knee_top, lx[i], pole_top_y[i], 1.0, f.color));
    }
    // Seconda passata: pennant ed etichette, in primo piano, al centro spostato.
    for (i, f) in flags.iter().enumerate() {
        let cx = lx[i];
        let pole_top = pole_top_y[i];
        // Punta dell'asta a destra del palo: pennant per i traguardi, mini
        // fulmine per le milestone trigger.
        shapes.extend(pole_tip(cx, pole_top, f.trigger, f.color));
        // Etichetta sopra la bandierina, centrata sul centro spostato; la data
        // resta la riga più vicina al pennant.
        for (li, (txt, size, bold)) in flag_lines(f).iter().enumerate() {
            let ly = pole_top + 2.0 + li as f32 * FLAG_LINE_H;
            let col = if *bold { BLACK } else { TEXT_GRAY };
            shapes.extend(text_center(cx, ly, txt, *size, *bold, col));
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
        // Nome del dev in rosso se contiene almeno un worker "ghost".
        let label_col = if r.ghost_weeks.is_empty() { BLACK } else { RED };
        shapes.extend(text_left(X_LABEL, yc - 1.3, &label, 8.0, false, label_col));
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
            // Settimane con un worker "ghost" (qui necessariamente a effort 0):
            // tratto rosso sulla riga sottile, così il ghost si vede anche
            // quando il dev non ha ore. Anche qui le settimane consecutive
            // formano un tratto unico.
            let days: Vec<(i32, u32)> = r.ghost_weeks.iter().map(|d| (*d, 0)).collect();
            for (gs, ge) in contiguous_runs(&days) {
                let rx0 = x_of(gs);
                let rx1 = x_of(ge + 7);
                shapes.extend(rect_fill(
                    rx0,
                    yc - thin_hh,
                    rx1.max(rx0 + 1.0),
                    yc + thin_hh,
                    RED,
                ));
            }
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
                shapes.extend(rect_round(
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
                    shapes.extend(rect_round(
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
                    shapes.extend(rect_round(
                        rx0,
                        yc - hh,
                        rx1.max(rx0 + 1.0),
                        yc + hh,
                        r.color,
                    ));
                }
            }
        }
        // Overlay rosso sulle settimane con worker "ghost": quei rettangoli sono
        // sempre rossi, anche in mezzo alla barra del colore del dev.
        if !r.ghost_weeks.is_empty() {
            if matches!(fmt, BarFormat::Proportional) {
                // Formato proporzionale: l'altezza segue le ore della singola
                // settimana, quindi ogni settimana resta un rettangolo a sé.
                let denom = proportional_ref(r.max_week) as f32;
                for &day in &r.ghost_weeks {
                    let rx0 = x_of(day);
                    let rx1 = x_of(day + 7);
                    let hours = r
                        .weeks
                        .iter()
                        .find(|(d, _)| *d == day)
                        .map_or(0, |(_, h)| *h);
                    let hh = (bar_hh * hours as f32 / denom).max(0.15);
                    shapes.extend(rect_round(rx0, yc - hh, rx1.max(rx0 + 1.0), yc + hh, RED));
                }
            } else {
                // Altezza costante: le settimane ghost **consecutive** diventano
                // un rettangolo unico, come la barra sotto — altrimenti con gli
                // angoli smussati la marcatura si spezzerebbe in tessere.
                let days: Vec<(i32, u32)> = r.ghost_weeks.iter().map(|d| (*d, 0)).collect();
                for (gs, ge) in contiguous_runs(&days) {
                    let rx0 = x_of(gs);
                    let rx1 = x_of(ge + 7);
                    shapes.extend(rect_round(
                        rx0,
                        yc - bar_hh,
                        rx1.max(rx0 + 1.0),
                        yc + bar_hh,
                        RED,
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
    // Smussatura degli angoli: impostazione del file .ron, letta a ogni pagina
    // (così vale anche per chi chiama `project_shapes` direttamente, test inclusi).
    set_corner_pct(app.corner_pct);
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
    // Settimane (giorni) del dev in cui un worker "ghost" è assegnato, **anche
    // con effort 0**. Con la stampa dei ghost disattivata l'elenco è vuoto e
    // nulla viene marcato di rosso.
    let ghost_weeks = |dev_id: DevId| -> Vec<i32> {
        if !show_ghost() {
            return Vec::new();
        }
        let Some(sd) = app.projects.get_single_dev(proj, dev_id) else {
            return Vec::new();
        };
        sd.weeks_with_worker_assigned(|wid| app.workers.is_ghost(wid))
            .into_iter()
            .map(|w| w.0 as i32)
            .collect()
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
                let (label, color) = color_of(&dev_id);
                let gw = ghost_weeks(dev_id);
                let Some((first, last)) = sd.effort_span() else {
                    // Niente ore: di norma il dev non si stampa, ma se ci è
                    // assegnato un ghost (a effort 0) e la stampa dei ghost è
                    // attiva, la riga esce comunque — è l'anomalia da vedere.
                    if !gw.is_empty() {
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
                            ghost_weeks: gw,
                        });
                    }
                    continue;
                };
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
                    ghost_weeks: gw,
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
                            ghost_weeks: ghost_weeks(dev_id),
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
                            // Anche senza ore il ghost va segnalato: nome in
                            // rosso e tratti rossi sulla riga sottile.
                            ghost_weeks: ghost_weeks(dev_id),
                        })
                    }
                }
            }
        }
    }

    let mut flags = Vec::new();
    let scope = milestone_scope();
    for (mid, week) in app.projects.list_project_milestones(proj) {
        // Fuori ambito (es. milestone solo Internal in una stampa External) o
        // non spuntata nella dialog: la bandierina non viene disegnata.
        if !milestone_allowed(mid) || !app.milestones.in_scope(mid, scope) {
            continue;
        }
        let mname = app.milestones.get_name(mid).unwrap_or("?").to_string();
        let mcol = app.milestones.get_color(mid).map(u32_rgb).unwrap_or(BLACK);
        flags.push(Flag {
            day: week.0 as i32,
            title: mname,
            date: short_date(week.0 as i32),
            color: mcol,
            trigger: app.milestones.is_trigger(mid),
        });
    }

    let tripletta = app.projects.get_tripletta(proj);
    let progress_pct = app.projects.project_progress_pct(proj);
    let presumed_pct = app
        .projects
        .project_presumed_progress_pct(proj, WeekId(today as usize));
    Some(page_shapes(
        &tripletta,
        name,
        proj_start,
        proj_end,
        &rows,
        flags,
        today,
        created,
        chart_only,
        fmt,
        progress_pct,
        presumed_pct,
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

// ── PDF andamento nel tempo (presunta/dichiarata per dev e progetto) ─────────

// Area del grafico (percentuale nel tempo): sotto il titolo, sopra la legenda.
const TREND_PLOT_TOP: f32 = 186.0;
const TREND_PLOT_BOT: f32 = 46.0;

/// Segmento tratteggiato di direzione qualsiasi (i tratti seguono la retta).
fn dashed_seg(
    x0: f32,
    y0: f32,
    x1: f32,
    y1: f32,
    thick: f32,
    color: (f32, f32, f32),
) -> Vec<Shape> {
    const DASH: f32 = 1.6;
    const GAP: f32 = 1.3;
    let (dx, dy) = (x1 - x0, y1 - y0);
    let len = (dx * dx + dy * dy).sqrt();
    if len < 1e-4 {
        return Vec::new();
    }
    let (ux, uy) = (dx / len, dy / len);
    let mut out = Vec::new();
    let mut d = 0.0;
    while d < len {
        let e = (d + DASH).min(len);
        out.extend(line(
            x0 + ux * d,
            y0 + uy * d,
            x0 + ux * e,
            y0 + uy * e,
            thick,
            color,
        ));
        d += DASH + GAP;
    }
    out
}

/// Pallino pieno (cerchio approssimato con un ottagono) centrato in (x,y).
fn dot(x: f32, y: f32, r: f32, color: (f32, f32, f32)) -> Vec<Shape> {
    let n = 8;
    let pts: Vec<(f32, f32)> = (0..n)
        .map(|i| {
            let a = std::f32::consts::TAU * i as f32 / n as f32;
            (x + r * a.cos(), y + r * a.sin())
        })
        .collect();
    vec![Shape::Poly { pts, color }]
}

/// Disegna i pallini su ogni vertice di una polilinea.
fn dots(pts: &[(f32, f32)], r: f32, color: (f32, f32, f32)) -> Vec<Shape> {
    pts.iter().flat_map(|&(x, y)| dot(x, y, r, color)).collect()
}

/// Polilinea continua (punti già in mm).
fn polyline(pts: &[(f32, f32)], thick: f32, color: (f32, f32, f32)) -> Vec<Shape> {
    let mut out = Vec::new();
    for w in pts.windows(2) {
        out.extend(line(w[0].0, w[0].1, w[1].0, w[1].1, thick, color));
    }
    out
}

/// Polilinea tratteggiata (punti già in mm).
fn polyline_dashed(pts: &[(f32, f32)], thick: f32, color: (f32, f32, f32)) -> Vec<Shape> {
    let mut out = Vec::new();
    for w in pts.windows(2) {
        out.extend(dashed_seg(w[0].0, w[0].1, w[1].0, w[1].1, thick, color));
    }
    out
}

/// Valore dichiarato "tenuto" alla settimana `w` (ultima voce con settimana ≤ w).
fn declared_at(hist: &[DeclaredPoint], w: i32) -> u8 {
    hist.iter()
        .rev()
        .find(|p| p.week.0 as i32 <= w)
        .map_or(0, |p| p.pct)
}

/// Una pagina del PDF andamento per un progetto: linee presunta (continua) e
/// dichiarata (tratteggiata) per ogni dev (colore GUI) + una coppia aggregata di
/// progetto (nera). `None` se il progetto non ha dev con effort pianificato.
fn trend_page_shapes(
    app: &App,
    proj: ProjectId,
    name: &str,
    dev_info: &DevInfo,
    today: i32,
    today_week: i32,
    created: &str,
) -> Option<Vec<Shape>> {
    // Dev con pianificato > 0 (gli unici con presunta/dichiarata definite).
    let mut devs: Vec<(
        &crate::single_dev_utils::single_dev::SingleDev,
        u64,
        (f32, f32, f32),
        String,
    )> = Vec::new();
    for id in app.projects.get_dev_ids(proj) {
        if let Some(sd) = app.projects.get_single_dev(proj, id) {
            let planned = sd.planned_effort().0;
            if planned == 0 {
                continue;
            }
            let (dname, color) = dev_info
                .get(&id)
                .cloned()
                .unwrap_or_else(|| ("?".to_string(), BLACK));
            devs.push((sd, planned as u64, color, dname));
        }
    }
    if devs.is_empty() {
        return None;
    }
    let total_planned: u64 = devs.iter().map(|d| d.1).sum();

    // --- Asse dei tempi (X): copre dati + oggi + inizio/fine progetto ---------
    // L'asse copre solo i dati (effort + dichiarazioni) e oggi: parte dal primo
    // effort/dichiarazione in assoluto (non dall'inizio progetto) e non arriva alla
    // fine progetto — la presunta va all'ultima settimana con effort (anche oltre
    // oggi), la dichiarata a oggi.
    let mut days: Vec<i32> = vec![today_week];
    for (sd, _, _, _) in &devs {
        for w in sd.effort_weeks() {
            days.push(w.0 as i32);
        }
        for p in sd.declared_history() {
            days.push(p.week.0 as i32);
        }
    }
    let min_day = *days.iter().min().unwrap();
    let max_day = *days.iter().max().unwrap();
    let axis_start = local_to_days(&month_start(day_to_date(min_day)));
    let axis_end = local_to_days(&add_months(month_start(day_to_date(max_day)), 1));
    let span = (axis_end - axis_start).max(1) as f32;
    let x_of = |d: i32| -> f32 {
        (CHART_X0 + (d - axis_start) as f32 / span * (CHART_X1 - CHART_X0))
            .clamp(CHART_X0, CHART_X1)
    };

    // --- Asse Y (percentuale, auto 0..max arrotondato a multipli di 20) -------
    let mut ymax = 100.0f32;
    for (sd, planned, _, _) in &devs {
        if let Some(last) = sd.effort_weeks().last() {
            ymax = ymax.max(sd.effort_up_to(*last).0 as f32 * 100.0 / *planned as f32);
        }
    }
    let agg_used_max: u64 = devs
        .iter()
        .map(|(sd, _, _, _)| sd.effort_up_to(WeekId(max_day as usize)).0 as u64)
        .sum();
    ymax = ymax.max(agg_used_max as f32 * 100.0 / total_planned as f32);
    let mut y_top = (ymax / 20.0).ceil() * 20.0;
    if (y_top - ymax).abs() < 0.01 {
        y_top += 20.0; // margine sopra il massimo (la linea non tocca il bordo)
    }
    let y_of =
        |pct: f32| -> f32 { TREND_PLOT_BOT + (pct / y_top) * (TREND_PLOT_TOP - TREND_PLOT_BOT) };

    // Punti presunta/dichiarata di un dev. La presunta parte dal PRIMO effort del
    // dev stesso (non dall'inizio progetto né dal primo effort di altri dev): un
    // punto per ogni settimana con effort, valore = effort cumulato / pianificato.
    let dev_presumed =
        |sd: &crate::single_dev_utils::single_dev::SingleDev, planned: u64| -> Vec<(f32, f32)> {
            sd.effort_weeks()
                .iter()
                .map(|w| {
                    let pct = sd.effort_up_to(*w).0 as f32 * 100.0 / planned as f32;
                    (x_of(w.0 as i32), y_of(pct))
                })
                .collect()
        };
    let dev_declared = |sd: &crate::single_dev_utils::single_dev::SingleDev| -> Vec<(f32, f32)> {
        let hist = sd.declared_history();
        let mut pts: Vec<(f32, f32)> = hist
            .iter()
            .map(|p| (x_of(p.week.0 as i32), y_of(p.pct as f32)))
            .collect();
        // La dichiarata si ferma a oggi: estende piatto l'ultimo valore.
        if let Some(last) = hist.last() {
            if (last.week.0 as i32) < today_week {
                pts.push((x_of(today_week), y_of(last.pct as f32)));
            }
        }
        pts
    };

    let mut shapes: Vec<Shape> = Vec::new();

    // --- Titolo (tripletta + nome) --------------------------------------------
    let tripletta = app.projects.get_tripletta(proj);
    if !tripletta.is_empty() {
        shapes.extend(text_left(X_LABEL, 200.0, &tripletta, 16.0, true, BLACK));
    }
    if !name.is_empty() {
        let ty = if tripletta.is_empty() { 200.0 } else { 193.0 };
        for l in wrap_multiline(name, 90).into_iter().take(2) {
            shapes.extend(text_left(X_LABEL, ty, &l, 9.0, false, TEXT_GRAY));
        }
    }

    // Legenda stili in alto a destra: continua = presunta, tratteggiata = dichiarata.
    let kx = CHART_X1 - 46.0;
    shapes.extend(line(kx, 200.5, kx + 8.0, 200.5, 0.9, BLACK));
    shapes.extend(text_left(
        kx + 10.0,
        199.5,
        "presunta/rendicontata",
        7.0,
        false,
        TEXT_GRAY,
    ));
    shapes.extend(dashed_seg(kx, 196.5, kx + 8.0, 196.5, 0.9, BLACK));
    shapes.extend(text_left(
        kx + 10.0,
        195.5,
        "dichiarata",
        7.0,
        false,
        TEXT_GRAY,
    ));

    // --- Griglia Y (percentuali) ----------------------------------------------
    let mut pct = 0.0;
    while pct <= y_top + 0.1 {
        let yy = y_of(pct);
        // Il 100% (budget/lavoro completo) è evidenziato con una riga rossa.
        let is_full = (pct - 100.0).abs() < 0.01;
        let (col, thick) = if is_full {
            (RED, 0.5)
        } else {
            (GRAY_GUIDE, 0.2)
        };
        shapes.extend(line(CHART_X0, yy, CHART_X1, yy, thick, col));
        let lbl_col = if is_full { RED } else { TEXT_GRAY };
        shapes.extend(text_right(
            CHART_X0 - 1.5,
            yy - 1.0,
            &format!("{}%", pct as i32),
            6.5,
            false,
            lbl_col,
        ));
        pct += 20.0;
    }

    // --- Griglia X (mesi) + etichette -----------------------------------------
    let mut m = month_start(day_to_date(axis_start));
    let mut first = true;
    loop {
        let mx = x_of(local_to_days(&m));
        shapes.extend(line(
            mx,
            TREND_PLOT_BOT,
            mx,
            TREND_PLOT_TOP,
            0.2,
            GRAY_GUIDE,
        ));
        let mname = MONTHS_IT[(m.month() - 1) as usize];
        let lbl = if first || m.month() == 1 {
            format!("{} {:02}", mname, m.year() % 100)
        } else {
            mname.to_string()
        };
        shapes.extend(text_left(
            mx + 0.5,
            TREND_PLOT_BOT - 4.0,
            &lbl,
            6.5,
            false,
            TEXT_GRAY,
        ));
        first = false;
        let next = add_months(m, 1);
        if local_to_days(&next) >= axis_end {
            break;
        }
        m = next;
    }

    // Bordi assi.
    shapes.extend(line(
        CHART_X0,
        TREND_PLOT_BOT,
        CHART_X0,
        TREND_PLOT_TOP,
        0.4,
        TEXT_GRAY,
    ));
    shapes.extend(line(
        CHART_X0,
        TREND_PLOT_BOT,
        CHART_X1,
        TREND_PLOT_BOT,
        0.4,
        TEXT_GRAY,
    ));

    // Linea verticale "oggi".
    if today >= axis_start && today <= axis_end {
        let xt = x_of(today);
        shapes.extend(line(xt, TREND_PLOT_BOT, xt, TREND_PLOT_TOP, 0.5, RED));
        shapes.extend(text_center(
            xt,
            TREND_PLOT_TOP + 1.0,
            "oggi",
            6.5,
            false,
            RED,
        ));
    }

    // --- Linee dei dev (presunta continua, dichiarata tratteggiata) -----------
    // Con un pallino su ogni vertice, per vedere i punti che costruiscono il grafico.
    // Il nome del dev è scritto alla fine della sua linea colorata (presunta): i
    // colori sono simili, così l'associazione linea/dev è data dalla posizione.
    for (sd, planned, color, name) in &devs {
        let pres = dev_presumed(sd, *planned);
        // Settimane con worker "ghost" (effort > 0): il segmento in ingresso al
        // vertice di quella settimana — e il suo pallino — sono rossi, anche in
        // mezzo alla linea del colore del dev.
        let eweek_days: Vec<i32> = sd.effort_weeks().iter().map(|w| w.0 as i32).collect();
        let ghost: std::collections::HashSet<i32> = sd
            .weeks_with_worker(|wid| app.workers.is_ghost(wid))
            .into_iter()
            .map(|w| w.0 as i32)
            .collect();
        for i in 1..pres.len() {
            // Il segmento i-1 → i "appartiene" alla settimana i (l'incremento di
            // quella settimana): rosso e più spesso se la settimana i ha un ghost,
            // così l'anomalia risalta rispetto alla linea normale.
            let is_ghost_seg = ghost.contains(&eweek_days[i]);
            let (seg_col, thick) = if is_ghost_seg {
                (RED, 1.8)
            } else {
                (*color, 0.8)
            };
            shapes.extend(line(
                pres[i - 1].0,
                pres[i - 1].1,
                pres[i].0,
                pres[i].1,
                thick,
                seg_col,
            ));
        }
        for (i, &(x, y)) in pres.iter().enumerate() {
            let (dot_col, r) = if ghost.contains(&eweek_days[i]) {
                (RED, 1.0)
            } else {
                (*color, 0.6)
            };
            shapes.extend(dot(x, y, r, dot_col));
        }
        let decl = dev_declared(sd);
        shapes.extend(polyline_dashed(&decl, 0.8, *color));
        shapes.extend(dots(&decl, 0.6, *color));
        if let Some(&(lx, ly)) = pres.last() {
            shapes.extend(text_left(
                (lx + 1.5).min(CHART_X1 - text_w_mm(name, 7.0)),
                ly - 1.0,
                name,
                7.0,
                false,
                *color,
            ));
        }
    }

    // --- Linee aggregate di progetto (nere, più spesse) -----------------------
    let mut eweeks: Vec<i32> = devs
        .iter()
        .flat_map(|(sd, _, _, _)| sd.effort_weeks().into_iter().map(|w| w.0 as i32))
        .collect();
    eweeks.sort();
    eweeks.dedup();
    if !eweeks.is_empty() {
        // La presunta di progetto parte dal primo effort in assoluto (eweeks[0]).
        let pts: Vec<(f32, f32)> = eweeks
            .iter()
            .map(|w| {
                let used: u64 = devs
                    .iter()
                    .map(|(sd, _, _, _)| sd.effort_up_to(WeekId(*w as usize)).0 as u64)
                    .sum();
                (x_of(*w), y_of(used as f32 * 100.0 / total_planned as f32))
            })
            .collect();
        shapes.extend(polyline(&pts, 1.4, BLACK));
        shapes.extend(dots(&pts, 0.85, BLACK));
        if let Some(&(lx, ly)) = pts.last() {
            shapes.extend(text_left(
                (lx + 1.5).min(CHART_X1 - text_w_mm("Progetto", 7.0)),
                ly - 1.0,
                "Progetto",
                7.0,
                true,
                BLACK,
            ));
        }
    }
    let mut hweeks: Vec<i32> = devs
        .iter()
        .flat_map(|(sd, _, _, _)| sd.declared_history().iter().map(|p| p.week.0 as i32))
        .collect();
    hweeks.sort();
    hweeks.dedup();
    if let Some(&last_w) = hweeks.last() {
        let agg_decl = |w: i32| -> f32 {
            let num: u64 = devs
                .iter()
                .map(|(sd, planned, _, _)| *planned * declared_at(sd.declared_history(), w) as u64)
                .sum();
            num as f32 / total_planned as f32
        };
        let mut pts: Vec<(f32, f32)> = hweeks
            .iter()
            .map(|w| (x_of(*w), y_of(agg_decl(*w))))
            .collect();
        if last_w < today_week {
            pts.push((x_of(today_week), y_of(agg_decl(last_w))));
        }
        shapes.extend(polyline_dashed(&pts, 1.4, BLACK));
        shapes.extend(dots(&pts, 0.85, BLACK));
    }

    // Il nome del dev è ora scritto alla fine della sua linea (vedi sopra): niente
    // legenda colori, perché con colori simili non si distingueva l'associazione.

    // --- Footer (banda grigia + data, come gli altri export) ------------------
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

    Some(shapes)
}

/// PDF dell'andamento nel tempo: una pagina per progetto (con dati), linee
/// presunta/dichiarata per dev e progetto. `None` se nessun progetto ha dati.
pub fn build_trend_pdf(app: &App, projects: &[ProjectId]) -> Option<Vec<u8>> {
    let mut doc = PdfDocument::new("Andamento");
    let regular = ParsedFont::from_bytes(FONT_REGULAR, 0, &mut Vec::new())?;
    let bold = ParsedFont::from_bytes(FONT_BOLD, 0, &mut Vec::new())?;
    let fonts = Fonts {
        regular: doc.add_font(&regular),
        bold: doc.add_font(&bold),
    };
    let created = chrono::Local::now().format("%Y-%m-%d").to_string();
    let now = chrono::Local::now().date_naive();
    let today = local_to_days(&now);
    let today_week = local_to_days(&primo_giorno_settimana_corrente(&now));
    let dev_info = dev_info_map(app);

    let mut pages = Vec::new();
    for &id in projects {
        let name = project_name(app, id);
        if let Some(shapes) =
            trend_page_shapes(app, id, &name, &dev_info, today, today_week, &created)
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
    Some(
        doc.with_pages(pages)
            .save(&PdfSaveOptions::default(), &mut Vec::new()),
    )
}

// --- Report di progetto (File ▸ Report…) ------------------------------------

/// Numeri di un dev nel report: ore stimate (pianificato), usate fino a oggi,
/// allocate in griglia (tutte le settimane, anche future) e mancanti
/// (`stimato − usato`, negativo in caso di sforamento).
///
/// `year_split` c'è solo per i progetti a cavallo della fine dell'anno corrente
/// (vedi `report_year_end`) e scompone il mancante: `(fino_al_31_12,
/// dall_1_1)` = ore allocate in griglia da oggi (escluso) al 31/12, e il resto
/// del mancante (`missing − fino_al_31_12`, negativo se l'allocazione fino a
/// fine anno supera già lo stimato).
#[derive(Debug, Clone, PartialEq)]
struct ReportDev {
    name: String,
    color: (f32, f32, f32),
    planned: i64,
    used: i64,
    allocated: i64,
    missing: i64,
    year_split: Option<(i64, i64)>,
}

impl ReportDev {
    /// Percentuale di `value` rispetto allo stimato del dev, arrotondata.
    /// `None` (⇒ "—") se lo stimato è 0.
    fn pct(&self, value: i64) -> Option<i64> {
        (self.planned > 0).then(|| (value as f64 * 100.0 / self.planned as f64).round() as i64)
    }
}

/// Fine dell'anno corrente (anno di `today`) se il progetto la attraversa:
/// `Some((anno, giorno del 31/12))` quando inizio e fine sono entrambi definiti,
/// l'inizio cade entro quell'anno e la fine in un anno successivo (stesso
/// criterio della colonna gialla di fine anno in griglia). `None` altrimenti.
fn report_year_end(app: &App, proj: ProjectId, today: i32) -> Option<(i32, i32)> {
    let year = day_to_date(today).year();
    let start_y = day_to_date(app.projects.get_project_start_week(proj)?.0 as i32).year();
    let end_y = day_to_date(app.projects.get_project_end_week(proj)?.0 as i32).year();
    if !(start_y <= year && end_y > year) {
        return None;
    }
    let dec31 = NaiveDate::from_ymd_opt(year, 12, 31)?;
    Some((year, local_to_days(&dec31)))
}

/// Tutti i dev del progetto (nell'ordine della griglia) con i numeri del report.
fn report_devs(app: &App, proj: ProjectId, dev_info: &DevInfo, today: i32) -> Vec<ReportDev> {
    let year_end = report_year_end(app, proj, today);
    app.projects
        .list_devs(proj)
        .into_iter()
        .filter_map(|id| {
            let sd = app.projects.get_single_dev(proj, id)?;
            let (name, color) = dev_info
                .get(&id)
                .cloned()
                .unwrap_or_else(|| ("?".to_string(), BLACK));
            let planned = sd.planned_effort().0 as i64;
            let used = sd.effort_up_to(WeekId(today as usize)).0 as i64;
            let allocated = sd
                .get_weeks()
                .into_iter()
                .last()
                .map_or(0, |w| sd.effort_up_to(w).0 as i64);
            let missing = planned - used;
            // Le settimane sono chiavi per giorno d'inizio: quella che parte
            // entro il 31/12 conta nell'anno, come in griglia.
            let year_split = year_end.map(|(_, dec31)| {
                let to_year_end = sd.effort_up_to(WeekId(dec31 as usize)).0 as i64 - used;
                (to_year_end, missing - to_year_end)
            });
            Some(ReportDev {
                name,
                color,
                planned,
                used,
                allocated,
                missing,
                year_split,
            })
        })
        .collect()
}

/// Data di una settimana nel report (`gg/mm/aaaa`), "—" se assente.
fn report_date(w: Option<WeekId>) -> String {
    w.map_or_else(
        || "—".to_string(),
        |w| day_to_date(w.0 as i32).format("%d/%m/%Y").to_string(),
    )
}

/// Percentuale formattata ("—" se non definita).
fn report_pct(p: Option<i64>) -> String {
    p.map_or_else(|| "—".to_string(), |p| format!("{p}%"))
}

// Geometria della tabella dei dev (mm): colonna nome + 4 gruppi (ore, %), più
// 2 gruppi di fine anno per i progetti a cavallo (colonna nome più stretta).
const REPORT_X0: f32 = X_LABEL;
const REPORT_X1: f32 = PAGE_W - X_LABEL;
const REPORT_NAME_W: f32 = 70.0;
const REPORT_NAME_W_YEAR: f32 = 50.0;
const REPORT_ROW_H: f32 = 6.5;
const REPORT_GROUPS: [&str; 4] = [
    "Stimato",
    "Usato fino a oggi",
    "Allocato in griglia",
    "Mancante",
];

/// Pagine (primitive) del report di un progetto: intestazione con tripletta,
/// categoria, info, date e avanzamento complessivo, poi la tabella dei dev. Se
/// i dev non entrano in una pagina la tabella continua sulle successive (con
/// l'intestazione della tabella ripetuta).
fn report_pages(
    app: &App,
    proj: ProjectId,
    dev_info: &DevInfo,
    today: i32,
    created: &str,
) -> Vec<Vec<Shape>> {
    let tripletta = app.projects.get_tripletta(proj);
    let title = if tripletta.trim().is_empty() {
        "(senza tripletta)".to_string()
    } else {
        tripletta
    };
    let category = app
        .projects
        .get_category(proj)
        .and_then(|c| app.categories.get_name(c))
        .unwrap_or("—")
        .to_string();
    let info = project_name(app, proj);
    let start = report_date(app.projects.get_project_start_week(proj));
    let end = report_date(app.projects.get_project_end_week(proj));
    let presumed = app
        .projects
        .project_presumed_progress_pct(proj, WeekId(today as usize));
    let actual = app.projects.project_progress_pct(proj);
    let devs = report_devs(app, proj, dev_info, today);
    // Gruppi della tabella: i 4 fissi più, se il progetto attraversa la fine
    // dell'anno corrente, la scomposizione del mancante attorno al 31/12.
    let year_end = report_year_end(app, proj, today);
    let mut groups: Vec<String> = REPORT_GROUPS.iter().map(|g| g.to_string()).collect();
    if let Some((year, _)) = year_end {
        groups.push(format!("Fino al 31/12/{year}"));
        groups.push(format!("Dal 1/1/{} a fine", year + 1));
    }
    let name_w = if year_end.is_some() {
        REPORT_NAME_W_YEAR
    } else {
        REPORT_NAME_W
    };

    let footer = |shapes: &mut Vec<Shape>| {
        shapes.extend(rect_fill(
            REPORT_X0,
            FOOTER_BOT,
            REPORT_X1,
            FOOTER_TOP,
            GRAY_FOOTER,
        ));
        shapes.extend(text_center(
            (REPORT_X0 + REPORT_X1) / 2.0,
            (FOOTER_BOT + FOOTER_TOP) / 2.0 - 1.4,
            created,
            8.0,
            false,
            TEXT_GRAY,
        ));
    };

    // --- Intestazione del progetto (solo prima pagina) ------------------------
    let mut shapes = Vec::new();
    let mut y = PAGE_H - 16.0;
    shapes.extend(text_left(REPORT_X0, y, &title, 18.0, true, BLACK));
    y -= 9.0;
    let label_value = |shapes: &mut Vec<Shape>, y: f32, label: &str, value: &str| {
        shapes.extend(text_left(REPORT_X0, y, label, 10.0, true, BLACK));
        shapes.extend(text_left(REPORT_X0 + 34.0, y, value, 10.0, false, BLACK));
    };
    label_value(&mut shapes, y, "Categoria:", &category);
    y -= 6.0;
    // Info su più righe (a capo espliciti preservati), con un tetto per non
    // mangiarsi la tabella: le righe in eccesso finiscono in "…".
    const INFO_MAX_LINES: usize = 8;
    let mut info_lines = wrap_multiline(&info, 120);
    if info_lines.is_empty() {
        info_lines.push("—".to_string());
    }
    if info_lines.len() > INFO_MAX_LINES {
        info_lines.truncate(INFO_MAX_LINES);
        info_lines[INFO_MAX_LINES - 1].push_str(" …");
    }
    shapes.extend(text_left(REPORT_X0, y, "Info:", 10.0, true, BLACK));
    for l in &info_lines {
        shapes.extend(text_left(REPORT_X0 + 34.0, y, l, 9.0, false, BLACK));
        y -= 4.6;
    }
    y -= 1.4;
    label_value(&mut shapes, y, "Inizio:", &start);
    shapes.extend(text_left(REPORT_X0 + 80.0, y, "Fine:", 10.0, true, BLACK));
    shapes.extend(text_left(REPORT_X0 + 94.0, y, &end, 10.0, false, BLACK));
    y -= 6.0;
    label_value(
        &mut shapes,
        y,
        "Avanzamento:",
        &format!(
            "presunto {}  ·  effettivo {}",
            report_pct(presumed.map(i64::from)),
            report_pct(actual.map(i64::from)),
        ),
    );
    y -= 10.0;

    // --- Tabella dei dev -------------------------------------------------------
    let group_w = (REPORT_X1 - REPORT_X0 - name_w) / groups.len() as f32;
    let group_x = |g: usize| REPORT_X0 + name_w + g as f32 * group_w;
    // Intestazione su due righe: nome del gruppo sopra, "ore"/"%" sotto.
    let table_header = |shapes: &mut Vec<Shape>, y: f32| -> f32 {
        shapes.extend(rect_fill(
            REPORT_X0,
            y - 2.0 * REPORT_ROW_H,
            REPORT_X1,
            y,
            GRAY_FOOTER,
        ));
        shapes.extend(text_left(
            REPORT_X0 + 2.0,
            y - 2.0 * REPORT_ROW_H + 2.2,
            "Dev",
            9.0,
            true,
            BLACK,
        ));
        for (g, name) in groups.iter().enumerate() {
            let gx = group_x(g);
            // Titolo rimpicciolito se non entra nel gruppo.
            let size = (9.0 * (group_w - 2.0) / text_w_mm(name, 9.0)).min(9.0);
            shapes.extend(text_center(
                gx + group_w / 2.0,
                y - REPORT_ROW_H + 2.2,
                name,
                size,
                true,
                BLACK,
            ));
            shapes.extend(text_right(
                gx + group_w * 0.55,
                y - 2.0 * REPORT_ROW_H + 2.2,
                "ore",
                8.0,
                false,
                TEXT_GRAY,
            ));
            shapes.extend(text_right(
                gx + group_w - 5.0,
                y - 2.0 * REPORT_ROW_H + 2.2,
                "%",
                8.0,
                false,
                TEXT_GRAY,
            ));
            shapes.extend(line(gx, y, gx, y - 2.0 * REPORT_ROW_H, 0.2, GRAY_LT));
        }
        y - 2.0 * REPORT_ROW_H
    };

    let mut pages = Vec::new();
    y = table_header(&mut shapes, y);
    if devs.is_empty() {
        shapes.extend(text_left(
            REPORT_X0 + 2.0,
            y - REPORT_ROW_H + 2.2,
            "Nessun dev nel progetto.",
            9.0,
            false,
            TEXT_GRAY,
        ));
    }
    for (i, d) in devs.iter().enumerate() {
        // Pagina piena: chiudi e riparti con l'intestazione della tabella.
        if y - REPORT_ROW_H < FOOTER_TOP + 4.0 {
            footer(&mut shapes);
            pages.push(std::mem::take(&mut shapes));
            y = PAGE_H - 16.0;
            shapes.extend(text_left(
                REPORT_X0,
                y,
                &format!("{title} (continua)"),
                14.0,
                true,
                BLACK,
            ));
            y = table_header(&mut shapes, y - 8.0);
        }
        let top = y;
        y -= REPORT_ROW_H;
        if i % 2 == 1 {
            shapes.extend(rect_fill(REPORT_X0, y, REPORT_X1, top, (0.96, 0.96, 0.96)));
        }
        let ty = y + 2.2;
        // Quadratino col colore del dev + nome (troncato alla colonna).
        shapes.extend(rect_fill(
            REPORT_X0 + 2.0,
            y + 1.6,
            REPORT_X0 + 5.2,
            y + 4.8,
            d.color,
        ));
        shapes.extend(text_left(
            REPORT_X0 + 7.0,
            ty,
            &truncate_to_w(&d.name, 9.0, name_w - 9.0),
            9.0,
            false,
            BLACK,
        ));
        let mut values = vec![d.planned, d.used, d.allocated, d.missing];
        if year_end.is_some() {
            let (to_ye, after) = d.year_split.unwrap_or((0, 0));
            values.extend([to_ye, after]);
        }
        for (g, v) in values.into_iter().enumerate() {
            let gx = group_x(g);
            // Mancante (e sua quota dopo il 31/12) negativo = sforamento → rosso.
            let color = if (g == 3 || g == 5) && v < 0 { RED } else { BLACK };
            shapes.extend(text_right(
                gx + group_w * 0.55,
                ty,
                &format!("{v} h"),
                9.0,
                false,
                color,
            ));
            shapes.extend(text_right(
                gx + group_w - 5.0,
                ty,
                &report_pct(d.pct(v)),
                9.0,
                false,
                color,
            ));
            shapes.extend(line(gx, top, gx, y, 0.2, GRAY_LT));
        }
        shapes.extend(line(REPORT_X0, y, REPORT_X1, y, 0.2, GRAY_LT));
    }
    footer(&mut shapes);
    pages.push(shapes);
    pages
}

/// PDF del report: per ogni progetto dato (nell'ordine dato) una o più pagine
/// con tripletta, categoria, info, date, avanzamento complessivo (presunto ed
/// effettivo) e la tabella di tutti i dev (stimato / usato fino a oggi /
/// allocato in griglia / mancante, in ore e in % dello stimato del dev; per i
/// progetti a cavallo della fine dell'anno corrente anche il mancante scomposto
/// in "fino al 31/12" e "dal 1/1 a fine progetto").
/// `None` se la lista è vuota.
pub fn build_report_pdf(app: &App, projects: &[ProjectId]) -> Option<Vec<u8>> {
    if projects.is_empty() {
        return None;
    }
    let mut doc = PdfDocument::new("Report");
    let regular = ParsedFont::from_bytes(FONT_REGULAR, 0, &mut Vec::new())?;
    let bold = ParsedFont::from_bytes(FONT_BOLD, 0, &mut Vec::new())?;
    let fonts = Fonts {
        regular: doc.add_font(&regular),
        bold: doc.add_font(&bold),
    };
    let created = chrono::Local::now().format("%Y-%m-%d").to_string();
    let today = local_to_days(&chrono::Local::now().date_naive());
    let dev_info = dev_info_map(app);

    let pages: Vec<PdfPage> = projects
        .iter()
        .flat_map(|&id| report_pages(app, id, &dev_info, today, &created))
        .map(|shapes| PdfPage::new(Mm(PAGE_W), Mm(PAGE_H), render_pdf(&fonts, &shapes)))
        .collect();
    Some(
        doc.with_pages(pages)
            .save(&PdfSaveOptions::default(), &mut Vec::new()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::milestones::MilestoneKind;
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

    /// Report: stimato/usato fino a oggi/allocato/mancante per dev, con le %
    /// rispetto allo stimato; il mancante va in negativo quando si sfora.
    #[test]
    fn report_devs_numbers_and_overrun() {
        let mut app = App::new();
        let pid = app.projects.add("Info", Some("REP"), Some(WeekId(20000)));
        let a = app.devs.add("Backend");
        let b = app.devs.add("Frontend");
        let c = app.devs.add("QA");
        let w = WorkerId(0);
        app.projects.add_dev_effort(pid, a, Effort(40));
        app.projects.add_effort(pid, a, WeekId(20000), w, Effort(10));
        app.projects.add_effort(pid, a, WeekId(20007), w, Effort(10));
        app.projects.add_effort(pid, a, WeekId(20014), w, Effort(10)); // futuro
        app.projects.add_dev_effort(pid, b, Effort(10));
        app.projects.add_effort(pid, b, WeekId(20000), w, Effort(15)); // sforamento
        app.projects.add_dev(pid, c); // nessuno stimato

        let today = 20010; // dopo le prime due settimane, prima della terza
        let devs = report_devs(&app, pid, &dev_info_map(&app), today);
        assert_eq!(devs.len(), 3, "tutti i dev, anche senza stimato");
        let by = |n: &str| devs.iter().find(|d| d.name == n).unwrap().clone();

        let da = by("Backend");
        assert_eq!((da.planned, da.used, da.allocated, da.missing), (40, 20, 30, 20));
        assert_eq!((da.pct(da.used), da.pct(da.allocated), da.pct(da.missing)), (Some(50), Some(75), Some(50)));

        let db = by("Frontend");
        assert_eq!(db.missing, -5);
        assert_eq!(db.pct(db.missing), Some(-50));

        let dc = by("QA");
        assert_eq!((dc.planned, dc.pct(dc.planned)), (0, None));
    }

    /// Progetto a cavallo della fine dell'anno corrente: il mancante si scompone
    /// in ore allocate da oggi al 31/12 e resto dal 1/1; progetto tutto dentro
    /// l'anno: nessuna scomposizione.
    #[test]
    fn report_year_split_only_for_projects_crossing_year_end() {
        let day = |y, m, d| local_to_days(&NaiveDate::from_ymd_opt(y, m, d).unwrap());
        let wk = |y, m, d| WeekId(day(y, m, d) as usize);
        // Settimane (lunedì) del 2026 attorno a oggi e a fine anno.
        let (w_past, w_oct, w_dec, w_jan) = (
            wk(2026, 9, 14),
            wk(2026, 10, 5),
            wk(2026, 12, 28), // parte il 28/12 → conta nel 2026
            wk(2027, 1, 4),
        );
        let today = day(2026, 9, 25);
        let w = WorkerId(0);

        let mut app = App::new();
        let dev = app.devs.add("Backend");
        let cross = app.projects.add("A cavallo", Some("CRS"), Some(w_past));
        app.projects.set_project_end_week(cross, Some(wk(2027, 3, 1)));
        app.projects.add_dev_effort(cross, dev, Effort(200));
        app.projects.add_effort(cross, dev, w_past, w, Effort(80)); // usato
        app.projects.add_effort(cross, dev, w_oct, w, Effort(30));
        app.projects.add_effort(cross, dev, w_dec, w, Effort(20));
        app.projects.add_effort(cross, dev, w_jan, w, Effort(40));

        assert_eq!(report_year_end(&app, cross, today), Some((2026, day(2026, 12, 31))));
        let d = &report_devs(&app, cross, &dev_info_map(&app), today)[0];
        assert_eq!(d.missing, 120);
        // 30 + 20 allocate entro il 31/12; il resto del mancante è per il 2027.
        assert_eq!(d.year_split, Some((50, 70)));

        // Tutto nel 2026 → nessuna colonna di fine anno.
        let inside = app.projects.add("Dentro", Some("INS"), Some(w_past));
        app.projects.set_project_end_week(inside, Some(w_dec));
        app.projects.add_dev_effort(inside, dev, Effort(10));
        assert_eq!(report_year_end(&app, inside, today), None);
        assert_eq!(report_devs(&app, inside, &dev_info_map(&app), today)[0].year_split, None);

        // Senza data di fine non si può dire → nessuna colonna.
        let open_end = app.projects.add("Senza fine", Some("OPN"), Some(w_past));
        assert_eq!(report_year_end(&app, open_end, today), None);

        // Le pagine si generano con i gruppi extra.
        assert!(!report_pages(&app, cross, &dev_info_map(&app), today, "oggi").is_empty());
    }

    /// Il report produce un PDF valido e, con molti dev, la tabella continua su
    /// più pagine; senza progetti non produce nulla.
    #[test]
    fn report_pdf_is_valid_and_paginates() {
        let mut app = App::new();
        assert!(build_report_pdf(&app, &[]).is_none());
        let pid = app.projects.add("Info\nsu due righe", Some("REP"), Some(WeekId(20000)));
        for i in 0..40 {
            let d = app.devs.add(&format!("Dev {i}"));
            app.projects.add_dev_effort(pid, d, Effort(8));
        }
        let pages = report_pages(&app, pid, &dev_info_map(&app), 20000, "oggi");
        assert!(pages.len() > 1, "40 dev non stanno in una pagina");
        let bytes = build_report_pdf(&app, &[pid]).expect("un progetto → Some");
        assert!(bytes.starts_with(b"%PDF"));
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

    /// Numero di vertici di ogni `<polygon>` presente nell'SVG.
    fn polygon_vertex_counts(svg: &str) -> Vec<usize> {
        svg.split("<polygon")
            .skip(1)
            .filter_map(|p| p.split_once("points=\""))
            .filter_map(|(_, rest)| rest.split_once('"'))
            .map(|(pts, _)| pts.split_whitespace().count())
            .collect()
    }

    #[test]
    fn trigger_milestone_is_drawn_as_a_bolt_instead_of_a_pennant() {
        let mut app = App::new();
        let pid = app.projects.add("Prog", Some("ABC"), Some(WeekId(20000)));
        app.projects.set_project_end_week(pid, Some(WeekId(20070)));
        let dev = app.devs.add("Frontend");
        app.projects.add_dev(pid, dev);
        app.projects
            .add_effort(pid, dev, WeekId(20007), WorkerId(0), Effort(8));
        let mid = app.milestones.add("Consegna");
        app.projects.add_project_milestone(pid, mid, WeekId(20035));

        // Conta i poligoni per numero di vertici: il pennant del traguardo ne ha
        // 3, il fulmine 6. (Nel grafico ci sono altri triangoli — il marker
        // "Today" — quindi si confrontano i conteggi, non la sola presenza.)
        let tri = |svg: &str| polygon_vertex_counts(svg).iter().filter(|n| **n == 3).count();
        let bolt = |svg: &str| polygon_vertex_counts(svg).iter().filter(|n| **n == 6).count();

        // Traguardo: un pennant triangolare, nessun fulmine.
        let goal_svg = build_svg_project(&app, pid, &[dev], BarFormat::Continuous).unwrap();
        assert_eq!(bolt(&goal_svg), 0, "senza trigger niente fulmine");

        // Stessa milestone marcata come trigger: il triangolo lascia il posto
        // al fulmine, uno per uno.
        app.milestones.set_kind(mid, MilestoneKind::Trigger);
        let svg = build_svg_project(&app, pid, &[dev], BarFormat::Continuous).unwrap();
        assert_eq!(bolt(&svg), 1, "il trigger disegna un fulmine a 6 vertici");
        assert_eq!(
            tri(&svg),
            tri(&goal_svg) - 1,
            "col trigger sparisce esattamente un pennant triangolare"
        );

        // L'asta e l'etichetta restano: cambia solo la punta.
        assert!(svg.contains("Consegna"), "il nome resta sotto l'asta");
    }

    #[test]
    fn milestone_scope_filters_the_flags_in_the_export() {
        use crate::milestones::{MilestoneCategory, MilestoneScope};

        let mut app = App::new();
        let pid = app.projects.add("Prog", Some("ABC"), Some(WeekId(20000)));
        app.projects.set_project_end_week(pid, Some(WeekId(20070)));
        let dev = app.devs.add("Frontend");
        app.projects.add_dev(pid, dev);
        app.projects
            .add_effort(pid, dev, WeekId(20007), WorkerId(0), Effort(8));

        // Una milestone senza categorie (= Internal) e una marcata External.
        let interna = app.milestones.add("SoloInterna");
        let esterna = app.milestones.add("AncheEsterna");
        app.milestones
            .set_category(esterna, MilestoneCategory::External, true);
        app.projects.add_project_milestone(pid, interna, WeekId(20021));
        app.projects.add_project_milestone(pid, esterna, WeekId(20035));

        // Internal: tutte e due le bandierine.
        set_milestone_scope(MilestoneScope::Internal);
        let svg = build_svg_project(&app, pid, &[dev], BarFormat::Continuous).unwrap();
        assert!(svg.contains("SoloInterna"));
        assert!(svg.contains("AncheEsterna"));

        // External: solo quella External.
        set_milestone_scope(MilestoneScope::External);
        let svg = build_svg_project(&app, pid, &[dev], BarFormat::Continuous).unwrap();
        assert!(
            !svg.contains("SoloInterna"),
            "la milestone Internal non va nella stampa External"
        );
        assert!(svg.contains("AncheEsterna"));

        set_milestone_scope(MilestoneScope::Internal);
    }

    /// Testi disegnati nella pagina (etichette dev, date, titoli…).
    fn texts(shapes: &[Shape]) -> Vec<String> {
        shapes
            .iter()
            .filter_map(|sh| match sh {
                Shape::Text { s, .. } => Some(s.clone()),
                _ => None,
            })
            .collect()
    }

    /// Numero di forme piene rosse nella pagina (rettangoli **e** poligoni: con
    /// gli angoli smussati le marcature diventano poligoni). Ne esiste anche
    /// qualcuna non legata ai ghost (il marker "Today"), quindi nei test si
    /// confrontano i conteggi con e senza spunta, non il valore assoluto.
    fn red_rects(shapes: &[Shape]) -> usize {
        shapes
            .iter()
            .filter(|sh| {
                matches!(sh, Shape::Rect { color, .. } | Shape::Poly { color, .. } if *color == RED)
            })
            .count()
    }

    /// Progetto con due dev: uno con effort normale e un ghost assegnato a
    /// **effort 0**, l'altro con **solo** il ghost a 0 (nessuna ora).
    fn app_with_zero_effort_ghost() -> (App, ProjectId, DevId, DevId) {
        let mut app = App::new();
        let pid = app.projects.add("Prog", Some("ABC"), Some(WeekId(20000)));
        app.projects.set_project_end_week(pid, Some(WeekId(20070)));
        let normale = app.devs.add("Frontend");
        let solo_ghost = app.devs.add("Backend");
        app.projects.add_dev(pid, normale);
        app.projects.add_dev(pid, solo_ghost);
        let worker = app.workers.add("Mario");
        let ghost = app.workers.add("Fantasma");
        app.workers.set_ghost(ghost, true);
        // Dev normale: ore vere + il ghost assegnato senza ore.
        app.projects
            .add_effort(pid, normale, WeekId(20007), worker, Effort(8));
        app.projects
            .add_effort(pid, normale, WeekId(20014), ghost, Effort(0));
        // Dev con il solo ghost a zero: nessuna ora in tutto il progetto.
        app.projects
            .add_effort(pid, solo_ghost, WeekId(20021), ghost, Effort(0));
        (app, pid, normale, solo_ghost)
    }

    fn shapes_all_devs(app: &App, pid: ProjectId) -> Vec<Shape> {
        project_shapes(
            app,
            pid,
            &project_name(app, pid),
            &dev_info_map(app),
            20030,
            "",
            None, // come l'export multi-progetto: solo i dev con effort
            false,
            BarFormat::Continuous,
        )
        .unwrap()
    }

    /// Con la spunta «Includi i worker ghost» il ghost si stampa anche dove ha
    /// effort 0, e il dev che ha solo lui compare lo stesso; senza spunta la
    /// pagina torna a ignorarli.
    #[test]
    fn zero_effort_ghost_is_printed_only_when_enabled() {
        let (app, pid, ..) = app_with_zero_effort_ghost();

        set_show_ghost(true);
        let on = shapes_all_devs(&app, pid);
        assert!(
            texts(&on).iter().any(|t| t == "Backend"),
            "il dev con il solo ghost a effort 0 deve comparire"
        );
        set_show_ghost(false);
        let off = shapes_all_devs(&app, pid);
        assert!(
            !texts(&off).iter().any(|t| t == "Backend"),
            "senza spunta il dev senza ore non si stampa"
        );
        assert!(
            texts(&off).iter().any(|t| t == "Frontend"),
            "il dev con effort resta stampato"
        );
        // Due settimane col ghost (una sul dev con ore, una su quello senza).
        assert_eq!(
            red_rects(&on) - red_rects(&off),
            2,
            "una marcatura rossa per ogni settimana col ghost"
        );

        set_show_ghost(true);
    }

    /// Nella pagina del singolo progetto (dev scelti a mano) il ghost a effort 0
    /// marca la riga sottile del dev senza ore.
    #[test]
    fn zero_effort_ghost_marks_the_thin_row_of_a_dev_without_effort() {
        let (app, pid, _, solo_ghost) = app_with_zero_effort_ghost();
        let page = |ghost_on: bool| -> Vec<Shape> {
            set_show_ghost(ghost_on);
            project_shapes(
                &app,
                pid,
                &project_name(&app, pid),
                &dev_info_map(&app),
                20030,
                "",
                Some(&[solo_ghost]), // ordine scelto dall'utente
                true,
                BarFormat::Continuous,
            )
            .unwrap()
        };

        assert_eq!(
            red_rects(&page(true)) - red_rects(&page(false)),
            1,
            "la settimana del ghost è marcata sulla riga sottile"
        );
        set_show_ghost(true);
    }

    /// Le settimane ghost **consecutive** formano un'unica marcatura: con gli
    /// angoli smussati, un rettangolo per settimana si vedrebbe come una fila di
    /// tessere staccate invece che come un tratto continuo.
    #[test]
    fn consecutive_ghost_weeks_are_merged_into_one_mark() {
        let mut app = App::new();
        let pid = app.projects.add("Prog", Some("ABC"), Some(WeekId(20000)));
        app.projects.set_project_end_week(pid, Some(WeekId(20120)));
        let dev = app.devs.add("Frontend");
        app.projects.add_dev(pid, dev);
        let ghost = app.workers.add("Fantasma");
        app.workers.set_ghost(ghost, true);
        // Tre settimane consecutive col ghost, poi un buco, poi un'altra.
        for w in [20007, 20014, 20021, 20035] {
            app.projects
                .add_effort(pid, dev, WeekId(w), ghost, Effort(8));
        }

        let marks = |fmt| {
            set_show_ghost(true);
            let on = project_shapes(
                &app,
                pid,
                &project_name(&app, pid),
                &dev_info_map(&app),
                20030,
                "",
                Some(&[dev]),
                true,
                fmt,
            )
            .unwrap();
            set_show_ghost(false);
            let off = project_shapes(
                &app,
                pid,
                &project_name(&app, pid),
                &dev_info_map(&app),
                20030,
                "",
                Some(&[dev]),
                true,
                fmt,
            )
            .unwrap();
            set_show_ghost(true);
            red_rects(&on) - red_rects(&off)
        };

        // Barra continua e segmentata: 2 marcature (il tratto 20007-20021 unito
        // + la settimana isolata), non 4.
        assert_eq!(marks(BarFormat::Continuous), 2);
        assert_eq!(marks(BarFormat::Segmented), 2);
        // Formato proporzionale: l'altezza cambia settimana per settimana,
        // quindi restano 4 rettangoli distinti.
        assert_eq!(marks(BarFormat::Proportional), 4);
    }

    /// La smussatura letta dal file decide la forma dei rettangoli: a 0 restano
    /// rettangoli, sopra 0 le barre dell'effort e le celle dei mesi diventano
    /// poligoni con gli angoli arrotondati (dentro il rettangolo di partenza).
    #[test]
    fn corner_pct_from_the_file_rounds_bars_and_month_cells() {
        let mut app = App::new();
        let pid = app.projects.add("Prog", Some("ABC"), Some(WeekId(20000)));
        app.projects.set_project_end_week(pid, Some(WeekId(20070)));
        let dev = app.devs.add("Frontend");
        app.projects.add_dev(pid, dev);
        app.projects
            .add_effort(pid, dev, WeekId(20007), WorkerId(0), Effort(8));
        let dev_color = dev_info_map(&app).get(&dev).unwrap().1;

        let shapes_of = |app: &App| shapes_all_devs(app, pid);
        let colored = |shapes: &[Shape], want: (f32, f32, f32)| -> (usize, usize) {
            let rects = shapes
                .iter()
                .filter(|sh| matches!(sh, Shape::Rect { color, .. } if *color == want))
                .count();
            let polys = shapes
                .iter()
                .filter(|sh| matches!(sh, Shape::Poly { color, .. } if *color == want))
                .count();
            (rects, polys)
        };

        // Spigolo vivo: la barra del dev è un rettangolo.
        app.corner_pct = 0;
        let sharp = shapes_of(&app);
        assert_eq!(colored(&sharp, dev_color), (1, 0));
        let (sharp_rects, _) = colored(&sharp, GRAY_DK); // celle dei mesi pari
        assert!(sharp_rects > 0, "le celle dei mesi sono rettangoli");

        // Smussata: stessa barra, ora poligono; idem le celle dei mesi.
        app.corner_pct = 40;
        let round = shapes_of(&app);
        assert_eq!(colored(&round, dev_color), (0, 1));
        assert_eq!(colored(&round, GRAY_DK), (0, sharp_rects));
    }

    /// Geometria del rettangolo smussato: 4 angoli × (SEG+1) punti, tutti dentro
    /// il rettangolo di partenza, e raggio proporzionale alla percentuale.
    #[test]
    fn round_rect_points_stay_inside_the_rectangle() {
        set_corner_pct(100);
        let pts = round_rect_pts(10.0, 20.0, 30.0, 24.0);
        assert_eq!(pts.len(), 28);
        for (x, y) in &pts {
            assert!((9.99..=30.01).contains(x), "x fuori: {x}");
            assert!((19.99..=24.01).contains(y), "y fuori: {y}");
        }
        // Al 100% il raggio è metà del lato corto (2mm): il punto più a sinistra
        // del bordo superiore è rientrato di 2mm rispetto allo spigolo.
        let top_left_x = pts
            .iter()
            .filter(|(_, y)| (*y - 24.0).abs() < 0.01)
            .map(|(x, _)| *x)
            .fold(f32::MAX, f32::min);
        assert!((top_left_x - 12.0).abs() < 0.05, "rientro atteso 2mm: {top_left_x}");

        // Metà smussatura = metà raggio.
        set_corner_pct(50);
        let pts = round_rect_pts(10.0, 20.0, 30.0, 24.0);
        let top_left_x = pts
            .iter()
            .filter(|(_, y)| (*y - 24.0).abs() < 0.01)
            .map(|(x, _)| *x)
            .fold(f32::MAX, f32::min);
        assert!((top_left_x - 11.0).abs() < 0.05, "rientro atteso 1mm: {top_left_x}");

        set_corner_pct(CORNER_PCT_DEFAULT);
    }

    #[test]
    fn only_the_selected_milestones_are_drawn() {
        use crate::milestones::{MilestoneCategory, MilestoneScope};

        let mut app = App::new();
        let pid = app.projects.add("Prog", Some("ABC"), Some(WeekId(20000)));
        app.projects.set_project_end_week(pid, Some(WeekId(20070)));
        let dev = app.devs.add("Frontend");
        app.projects.add_dev(pid, dev);
        app.projects
            .add_effort(pid, dev, WeekId(20007), WorkerId(0), Effort(8));
        let tenuta = app.milestones.add("Tenuta");
        let scartata = app.milestones.add("Scartata");
        app.projects.add_project_milestone(pid, tenuta, WeekId(20021));
        app.projects
            .add_project_milestone(pid, scartata, WeekId(20035));

        // Solo la prima è spuntata nella dialog: l'altra non viene disegnata.
        set_milestone_allow(Some(vec![tenuta]));
        let svg = build_svg_project(&app, pid, &[dev], BarFormat::Continuous).unwrap();
        assert!(svg.contains("Tenuta"));
        assert!(!svg.contains("Scartata"));

        // Il filtro si combina in AND con l'ambito: in External resta fuori
        // anche quella spuntata, se non è marcata External.
        set_milestone_scope(MilestoneScope::External);
        let svg = build_svg_project(&app, pid, &[dev], BarFormat::Continuous).unwrap();
        assert!(!svg.contains("Tenuta"));
        app.milestones
            .set_category(tenuta, MilestoneCategory::External, true);
        let svg = build_svg_project(&app, pid, &[dev], BarFormat::Continuous).unwrap();
        assert!(svg.contains("Tenuta"));

        // Nessuna selezione = nessuna bandierina.
        set_milestone_scope(MilestoneScope::Internal);
        set_milestone_allow(Some(Vec::new()));
        let svg = build_svg_project(&app, pid, &[dev], BarFormat::Continuous).unwrap();
        assert!(!svg.contains("Tenuta"));
        assert!(!svg.contains("Scartata"));

        // Senza filtro (export multi-progetto) tornano tutte.
        set_milestone_allow(None);
        let svg = build_svg_project(&app, pid, &[dev], BarFormat::Continuous).unwrap();
        assert!(svg.contains("Tenuta"));
        assert!(svg.contains("Scartata"));
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
        app.projects.set_dev_declared_pct(pid, dev, WeekId(0), 60);
        // Dev SENZA effort ma con % dichiarata: non deve mostrare percentuali.
        let dev_empty = app.devs.add("Backend");
        app.projects.add_dev(pid, dev_empty);
        app.projects
            .set_dev_declared_pct(pid, dev_empty, WeekId(0), 33);

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
    fn show_pct_adds_project_progress_next_to_today() {
        // Progetto che contiene "oggi" nell'asse, così il marker Today è disegnato.
        let base = local_to_days(&chrono::Local::now().date_naive());
        let mut app = App::new();
        let pid = app
            .projects
            .add("Prog", Some("ABC"), Some(WeekId((base - 28) as usize)));
        app.projects
            .set_project_end_week(pid, Some(WeekId((base + 28) as usize)));
        let dev = app.devs.add("Frontend");
        app.projects.add_dev(pid, dev);
        app.projects.add_dev_effort(pid, dev, Effort(100)); // pianificato
        app.projects.add_effort(
            pid,
            dev,
            WeekId((base - 7) as usize),
            WorkerId(0),
            Effort(50),
        );
        app.projects.set_dev_declared_pct(pid, dev, WeekId(0), 70);
        // presunta = usato 50 / pianificato 100 = 50%; attuale (dichiarata) = 70%.

        set_show_pct(false);
        let svg = build_svg_project(&app, pid, &[dev], BarFormat::Continuous).unwrap();
        assert!(
            svg.contains("Today"),
            "oggi è nell'asse: Today deve comparire"
        );
        assert!(
            !svg.contains("(50%/70%)"),
            "senza flag niente % sotto Today"
        );

        set_show_pct(true);
        let svg = build_svg_project(&app, pid, &[dev], BarFormat::Continuous).unwrap();
        assert!(svg.contains("Today"), "Today resta presente");
        assert!(
            svg.contains("(50%/70%)"),
            "col flag presunta/attuale tra parentesi sotto Today"
        );
        set_show_pct(false);
    }

    #[test]
    fn build_trend_pdf_produces_valid_pdf_and_none_without_planned() {
        let mut app = App::new();
        let pid = app.projects.add("Prog", Some("ABC"), Some(WeekId(20000)));
        app.projects.set_project_end_week(pid, Some(WeekId(20070)));
        let a = app.devs.add("Frontend");
        let b = app.devs.add("Backend");
        app.projects.add_dev_effort(pid, a, Effort(100));
        app.projects.add_dev_effort(pid, b, Effort(50));
        app.projects
            .add_effort(pid, a, WeekId(20007), WorkerId(0), Effort(30));
        app.projects
            .add_effort(pid, a, WeekId(20014), WorkerId(0), Effort(20));
        app.projects
            .add_effort(pid, b, WeekId(20007), WorkerId(0), Effort(25));
        app.projects.set_dev_declared_pct(pid, a, WeekId(20007), 20);
        app.projects.set_dev_declared_pct(pid, a, WeekId(20014), 45);
        app.projects.set_dev_declared_pct(pid, b, WeekId(20007), 30);

        let bytes = build_trend_pdf(&app, &[pid]).expect("progetto con dati → Some");
        assert!(bytes.starts_with(b"%PDF"));

        // Progetto senza dev con pianificato → nessuna pagina → None.
        let empty = app.projects.add("Vuoto", Some("ZZZ"), Some(WeekId(20000)));
        assert!(build_trend_pdf(&app, &[empty]).is_none());
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

        // Conteggio delle forme piene nell'SVG (rettangoli + poligoni: con gli
        // angoli smussati le barre sono poligoni): tutto è identico tra i
        // formati tranne le barre del dev, quindi il totale isola il numero di
        // barre. 3 settimane con effort, 2 tratti contigui (20007-20014 e 20035):
        //   Continua = 1 barra, Segmentata = 2, Proporzionale = 3 (una a settimana).
        let fills = |fmt| {
            let svg = build_svg_project(&app, pid, &[dev], fmt).unwrap();
            svg.matches("<rect").count() + svg.matches("<polygon").count()
        };
        let (cont, seg, prop) = (
            fills(BarFormat::Continuous),
            fills(BarFormat::Segmented),
            fills(BarFormat::Proportional),
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




