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
use crate::date_utils::dates::{
    current_monday_days, days_to_local, local_to_days, primo_giorno_settimana_corrente, today,
    today_days,
};
use crate::dev_utils::dev::DevId;
use crate::milestones::{MilestoneId, MilestoneScope};
use crate::project_utils::project::ProjectId;
use crate::single_dev_utils::single_dev::{DeclaredPoint, WeekId};

/// Scelte dell'utente per un export Gantt (PDF/SVG), fatte nelle dialog e
/// passate esplicitamente a ogni `build_*`. I default sono quelli delle dialog
/// alla prima apertura.
#[derive(Clone, Debug, PartialEq)]
pub struct ExportOptions {
    /// Percentuali `presunta%/dichiarata%` accanto ai dev e sotto «Today».
    pub show_pct: bool,
    /// Anno abbreviato nelle date dei dev: "Sep 22 '26".
    pub show_year: bool,
    /// Ambito milestone del file: quali bandierine finiscono nella pagina.
    pub scope: MilestoneScope,
    /// Milestone scelte una per una nella dialog del singolo progetto; `None` =
    /// tutte quelle collocate. Si combina in **AND** con `scope`.
    pub milestones: Option<std::collections::HashSet<MilestoneId>>,
    /// Stampa i worker "ghost" (anche a effort 0); `false` toglie la marcatura.
    pub show_ghost: bool,
    /// Colore della marcatura ghost: rosso con nome dev rosso, oppure colore
    /// del dev con nome nero.
    pub ghost_red: bool,
    /// Periodo del grafico («Da»/«A», giorni) scelto nella dialog del singolo
    /// progetto; `None` = asse automatico.
    pub range: Option<(i32, i32)>,
}

impl Default for ExportOptions {
    fn default() -> Self {
        Self {
            show_pct: false,
            show_year: true,
            scope: MilestoneScope::Mix,
            milestones: None,
            show_ghost: true,
            ghost_red: true,
            range: None,
        }
    }
}

impl ExportOptions {
    /// Finestra dell'asse `[inizio, fine)` in giorni dal periodo scelto: ogni
    /// estremo vale per l'**intera settimana** che lo contiene (lunedì della
    /// settimana di «Da» → lunedì successivo alla settimana di «A»), così le
    /// date di inizio/fine progetto mostrano la loro settimana per intero.
    fn window(&self) -> Option<(i32, i32)> {
        let monday = |d: i32| local_to_days(&primo_giorno_settimana_corrente(&day_to_date(d)));
        self.range
            .map(|(a, b)| (monday(a.min(b)), monday(a.max(b)) + 7))
    }

    /// La milestone va disegnata? Deve essere tra quelle scelte **e** rientrare
    /// nell'ambito del file.
    fn milestone_shown(&self, app: &App, id: MilestoneId) -> bool {
        self.milestones.as_ref().is_none_or(|set| set.contains(&id))
            && app.milestones.in_scope(id, self.scope)
    }
}

/// Smussatura predefinita degli angoli, in percentuale del raggio massimo.
/// È il valore scritto nel `.ron` quando il campo manca.
pub const CORNER_PCT_DEFAULT: i32 = 40;

// Font incorporati (DejaVu Sans, licenza ridistribuibile): resa corretta degli
// accenti/Unicode, che i font builtin PDF (WinAnsi) non garantiscono.
const FONT_REGULAR: &[u8] = include_bytes!("../assets/fonts/DejaVuSans.ttf");
const FONT_BOLD: &[u8] = include_bytes!("../assets/fonts/DejaVuSans-Bold.ttf");

/// Handle dei font incorporati, registrati nel documento.
struct Fonts {
    regular: FontId,
    bold: FontId,
}

/// Documento PDF `title` con una pagina (formato `PAGE_W`×`PAGE_H`) per ogni
/// elenco di forme in `pages`: registra i font, disegna e salva. Unico punto
/// in cui si crea un PDF. `None` se non c'è nessuna pagina o i font non si
/// caricano.
fn pdf_document(title: &str, pages: &[Vec<Shape>]) -> Option<Vec<u8>> {
    if pages.is_empty() {
        return None;
    }
    let mut doc = PdfDocument::new(title);
    let regular = ParsedFont::from_bytes(FONT_REGULAR, 0, &mut Vec::new())?;
    let bold = ParsedFont::from_bytes(FONT_BOLD, 0, &mut Vec::new())?;
    let fonts = Fonts {
        regular: doc.add_font(&regular),
        bold: doc.add_font(&bold),
    };
    let pages: Vec<PdfPage> = pages
        .iter()
        .map(|shapes| PdfPage::new(Mm(PAGE_W), Mm(PAGE_H), render_pdf(&fonts, shapes)))
        .collect();
    Some(
        doc.with_pages(pages)
            .save(&PdfSaveOptions::default(), &mut Vec::new()),
    )
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

/// Nomi brevi dei mesi stampati in PDF/SVG (gli export sono in inglese).
const MONTHS_EN: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

/// Formato data degli export (americano, `mm/dd/yyyy`).
const US_DATE: &str = "%m/%d/%Y";

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

/// Altezza in più (mm) di una riga dev con le date sotto i tratti: due righe di
/// testo da `SEG_DATE_FONT` pt (inizio sopra, fine sotto).
const SEG_DATES_H: f32 = 4.6;
const SEG_DATE_FONT: f32 = 5.0;
/// Passo verticale (mm) tra le due righe di date sotto un tratto.
const SEG_DATE_LINE: f32 = 2.0;

/// Le due righe di date sotto un tratto `[start, end]` (giorni): l'anno, se
/// richiesto, segue la stessa regola dell'etichetta a destra — solo sulla fine
/// se le due date sono nello stesso anno.
fn segment_dates(start: i32, end: i32, show_year: bool) -> (String, String) {
    let same_year = day_to_date(start).year() == day_to_date(end).year();
    let first = if same_year {
        short_date(start)
    } else {
        dev_date(start, show_year)
    };
    (first, dev_date(end, show_year))
}

/// Giorni da aggiungere al lunedì dell'ultima settimana di effort per la data
/// di fine nell'etichetta del dev: 4 = venerdì (fine settimana lavorativa).
const DEV_END_OFFSET: i32 = 4;

/// Etichetta data breve, all'americana: "Sep 22".
fn short_date(days: i32) -> String {
    let d = day_to_date(days);
    format!("{} {}", MONTHS_EN[(d.month() - 1) as usize], d.day())
}

/// Data del dev accanto alla barra: "Sep 22", oppure "Sep 22 '26" se l'utente
/// ha chiesto l'anno (`ExportOptions::show_year`).
fn dev_date(days: i32, show_year: bool) -> String {
    if show_year {
        let y = day_to_date(days).year().rem_euclid(100);
        format!("{} '{y:02}", short_date(days))
    } else {
        short_date(days)
    }
}

/// Etichetta «inizio - fine» del dev. Con l'anno attivo, se le due date cadono
/// nello stesso anno l'anno compare solo nella fine («Sep 21 - Oct 9 '26»),
/// altrimenti in entrambe («Dec 14 '26 - Jan 8 '27»).
fn dev_range(start: i32, end: i32, show_year: bool) -> String {
    let same_year = day_to_date(start).year() == day_to_date(end).year();
    let from = if same_year {
        short_date(start)
    } else {
        dev_date(start, show_year)
    };
    format!("{from} - {}", dev_date(end, show_year))
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
/// `corner%` (0–100) di metà lato corto, quindi al 100% gli estremi diventano
/// semicerchi e allo 0% si torna allo spigolo vivo.
fn round_rect_pts(x0: f32, y0: f32, x1: f32, y1: f32, corner: i32) -> Vec<(f32, f32)> {
    const SEG: usize = 6;
    let (x0, x1) = (x0.min(x1), x0.max(x1));
    let (y0, y1) = (y0.min(y1), y0.max(y1));
    let r = (corner.clamp(0, 100) as f32 / 100.0) * (x1 - x0).min(y1 - y0) / 2.0;
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
/// (`App.corner_pct`, passata come `corner`): usato per le barre dell'effort e
/// le celle dei mesi. Con smussatura 0 resta un rettangolo vero e proprio.
fn rect_round(
    x0: f32,
    y0: f32,
    x1: f32,
    y1: f32,
    color: (f32, f32, f32),
    corner: i32,
) -> Vec<Shape> {
    if corner <= 0 {
        return rect_fill(x0, y0, x1, y1, color);
    }
    poly_fill(&round_rect_pts(x0, y0, x1, y1, corner), color)
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
#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
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

/// Come disegnare la barra di **un** dev: la forma (`fmt`) e, per i formati
/// segmentati, se scrivere le date **sotto ogni tratto** (inizio e fine su due
/// righe) invece dell'intervallo complessivo a destra della barra.
#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub struct DevBar {
    pub fmt: BarFormat,
    pub dates_below: bool,
}

impl DevBar {
    pub const fn new(fmt: BarFormat, dates_below: bool) -> Self {
        Self { fmt, dates_below }
    }

    /// Le date sotto i tratti valgono solo per i formati segmentati: con la
    /// barra continua il flag è ignorato.
    pub fn dates_below(self) -> bool {
        self.dates_below && self.fmt != BarFormat::Continuous
    }
}

impl From<BarFormat> for DevBar {
    fn from(fmt: BarFormat) -> Self {
        Self::new(fmt, false)
    }
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
    /// Formato della barra di **questo** dev (scelto per dev nella dialog del
    /// singolo progetto; uguale per tutti nell'export multi-progetto).
    fmt: BarFormat,
    /// Date sotto ogni tratto (formati segmentati): la riga è più alta e a
    /// destra della barra restano solo le eventuali percentuali.
    dates_below: bool,
    /// Dev con effort ma **nessuno nel periodo** scelto per l'export: niente
    /// barra né linea, solo le sue date reali (e le %) al bordo che indica
    /// dove sta il suo lavoro. `None` = riga normale.
    off: Option<OffWindow>,
}

/// Dove cade il lavoro di un dev rispetto al periodo dell'export, quando nel
/// periodo non ha effort.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum OffWindow {
    /// Già finito prima del periodo: date al bordo **sinistro** del grafico.
    Ended,
    /// Inizia dopo il periodo: date al bordo **destro**.
    NotStarted,
    /// Lavora prima e dopo, il periodo cade in un suo buco: date **al centro**.
    Gap,
}

impl OffWindow {
    /// Classifica lo span `[first, last]` (lunedì delle settimane con effort)
    /// rispetto alla finestra `[start, end)` in giorni.
    fn of(window: (i32, i32), first: i32, last: i32) -> Self {
        if last + 7 <= window.0 {
            OffWindow::Ended
        } else if first >= window.1 {
            OffWindow::NotStarted
        } else {
            OffWindow::Gap
        }
    }
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

/// Percentuali «(presunta%/dichiarata%)» accanto alle date di un dev, se la
/// stampa delle percentuali è attiva.
fn pct_label(r: &Row, show_pct: bool) -> Option<String> {
    show_pct.then(|| match r.presumed_pct {
        Some(p) => format!("({p}%/{}%)", r.declared_pct),
        None => format!("(-/{}%)", r.declared_pct),
    })
}

/// Genera gli `Op` di una pagina Gantt per un progetto.
#[allow(clippy::too_many_arguments)]
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
    progress_pct: Option<u8>,
    presumed_pct: Option<u32>,
    opts: &ExportOptions,
    corner: i32,
) -> Vec<Shape> {
    let mut shapes: Vec<Shape> = Vec::new();
    let window = opts.window();

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
    // Con un periodo scelto dall'utente l'asse è **esattamente** quel periodo:
    // i mesi agli estremi restano tagliati e le barre si fermano ai bordi
    // (`x_of` limita già al riquadro del grafico).
    let (axis_start, axis_end) = window.unwrap_or_else(|| {
        (
            local_to_days(&month_start(day_to_date(min_day))),
            local_to_days(&add_months(month_start(day_to_date(max_day)), 1)),
        )
    });
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
    // Primo e ultimo mese col nome scritto: gli anni ai lati si riferiscono a
    // quelli. Con un periodo scelto a mano l'asse parte e finisce sulle
    // settimane intere, quindi può iniziare con pochi giorni del mese prima
    // (troppo stretti per l'etichetta): l'anno di quel ritaglio sarebbe
    // fuorviante («2025 · Jan Feb…» per un periodo che inizia a gennaio 2026).
    let mut labeled: Option<(NaiveDate, NaiveDate)> = None;
    loop {
        let cell_x0 = x_of(local_to_days(&m));
        let next = add_months(m, 1);
        let cell_x1 = x_of(local_to_days(&next));
        let fill = if idx.is_multiple_of(2) {
            GRAY_DK
        } else {
            GRAY_LT
        };
        shapes.extend(rect_round(cell_x0, AXIS_BOT, cell_x1, AXIS_TOP, fill, corner));
        // Separatore bianco a destra della cella.
        shapes.extend(line(cell_x1, AXIS_BOT, cell_x1, AXIS_TOP, 0.6, WHITE));
        // Riga verticale del mese, attraverso la zona delle righe dev.
        shapes.extend(line(cell_x0, ROWS_BOT, cell_x0, AXIS_BOT, 0.4, GRAY_GUIDE));
        // Etichetta mese (bianca, in basso a sinistra della cella) se ci sta.
        let name = MONTHS_EN[(m.month() - 1) as usize];
        if cell_x1 - cell_x0 > text_w_mm(name, 8.0) + 1.5 {
            shapes.extend(text_left(
                cell_x0 + 1.5,
                AXIS_BOT + 1.6,
                name,
                8.0,
                false,
                WHITE,
            ));
            labeled = Some(labeled.map_or((m, m), |(first, _)| (first, m)));
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

    // Etichette anno agli estremi (arancione, in grassetto): l'anno del primo e
    // dell'ultimo mese leggibili; senza nessun nome scritto, quelli degli estremi.
    let (year_l, year_r) = match labeled {
        Some((first, last)) => (first.year(), last.year()),
        None => (
            month_start(day_to_date(axis_start)).year(),
            add_months(month_start(day_to_date(axis_end)), -1).year(),
        ),
    };
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
            corner,
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
        match (opts.show_pct, presumed_pct, progress_pct) {
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
    // Le righe con le date sotto i tratti sono più alte di `dates_h` (due righe
    // di testo); se non ci sta tutto, righe e spazio date si riducono insieme.
    let n = rows.len().max(1);
    let n_dates = rows.iter().filter(|r| r.dates_below && !r.no_effort).count();
    let avail = ROWS_TOP - ROWS_BOT;
    let want = n as f32 * 6.5 + n_dates as f32 * SEG_DATES_H;
    let scale = (avail / want).min(1.0);
    let row_h = 6.5 * scale;
    let dates_h = SEG_DATES_H * scale;
    let bar_hh = (row_h * 0.22).min(1.7);
    let label_w = CHART_X0 - X_LABEL - 2.0;
    let mut row_top = ROWS_TOP;
    for (i, r) in rows.iter().enumerate() {
        let yc = row_top - row_h / 2.0;
        row_top -= row_h;
        if r.dates_below && !r.no_effort {
            row_top -= dates_h;
        }
        // Etichetta a sinistra (nome dev), troncata se troppo lunga.
        let label = truncate_to_w(&r.label, 8.0, label_w);
        // Marcatura ghost: rosso, oppure il colore del dev se l'utente l'ha
        // scelto nell'export (allora anche il nome resta nero).
        let ghost_col = if opts.ghost_red { RED } else { r.color };
        // Nome del dev in rosso se contiene almeno un worker "ghost".
        let label_col = if r.ghost_weeks.is_empty() || !opts.ghost_red {
            BLACK
        } else {
            RED
        };
        shapes.extend(text_left(X_LABEL, yc - 1.3, &label, 8.0, false, label_col));
        let lead_x0 = X_LABEL + text_w_mm(&label, 8.0) + 2.0;

        // Dev con effort solo fuori dal periodo dell'export: niente barra né
        // linea, solo le date reali (+ % se richieste) al bordo sinistro (già
        // finito), destro (deve iniziare) o al centro (periodo in un buco).
        if let Some(off) = r.off {
            let range = dev_range(r.start_day, r.end_day + DEV_END_OFFSET, opts.show_year);
            let lbl = match pct_label(r, opts.show_pct) {
                Some(p) => format!("{range}  {p}"),
                None => range,
            };
            let y = yc - 1.2;
            shapes.extend(match off {
                OffWindow::Ended => text_left(CHART_X0 + 1.0, y, &lbl, 7.0, false, TEXT_GRAY),
                OffWindow::NotStarted => {
                    text_right(CHART_X1 - 1.0, y, &lbl, 7.0, false, TEXT_GRAY)
                }
                OffWindow::Gap => {
                    text_center((CHART_X0 + CHART_X1) / 2.0, y, &lbl, 7.0, false, TEXT_GRAY)
                }
            });
            continue;
        }

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
                    ghost_col,
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
        // Disegno della/e barra/e secondo il formato scelto per il dev.
        match r.fmt {
            BarFormat::Continuous => {
                // Un unico rettangolo dalla prima all'ultima settimana.
                shapes.extend(rect_round(
                    bx0,
                    yc - bar_hh,
                    bx1.max(bx0 + 1.0),
                    yc + bar_hh,
                    r.color,
                    corner,
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
                        corner,
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
                        corner,
                    ));
                }
            }
        }
        // Overlay rosso sulle settimane con worker "ghost": quei rettangoli sono
        // sempre rossi, anche in mezzo alla barra del colore del dev.
        if !r.ghost_weeks.is_empty() {
            if matches!(r.fmt, BarFormat::Proportional) {
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
                    shapes.extend(rect_round(
                        rx0,
                        yc - hh,
                        rx1.max(rx0 + 1.0),
                        yc + hh,
                        ghost_col,
                        corner,
                    ));
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
                        ghost_col,
                        corner,
                    ));
                }
            }
        }
        // Etichetta date a fine barra (o prima, se non ci sta a destra); se
        // richiesto, le percentuali (presunta/dichiarata) la seguono, tutto a
        // destra e con lo **stesso font delle date** (7.0). Le righe senza effort
        // non arrivano qui (hanno già fatto `continue`), quindi non mostrano la %.
        // La fine è il **venerdì** dell'ultima settimana (lunedì + 4): un dev
        // con effort in una sola settimana mostra «lun - ven» di quella settimana.
        let pct = pct_label(r, opts.show_pct);
        let lbl = if r.dates_below {
            // Date sotto ogni tratto (inizio sopra, fine sotto, centrate sul
            // tratto); a destra della barra restano solo le percentuali.
            for (s0, e0) in contiguous_runs(&r.weeks) {
                let cx = (x_of(s0) + x_of(e0 + 7)) / 2.0;
                let (d0, d1) = segment_dates(s0, e0 + DEV_END_OFFSET, opts.show_year);
                let y0 = yc - bar_hh - 0.6 - SEG_DATE_FONT * 0.352_778;
                for (k, d) in [d0, d1].iter().enumerate() {
                    shapes.extend(text_center(
                        cx,
                        y0 - k as f32 * SEG_DATE_LINE * scale,
                        d,
                        SEG_DATE_FONT,
                        false,
                        TEXT_GRAY,
                    ));
                }
            }
            match pct {
                Some(p) => p,
                None => continue,
            }
        } else {
            let range = dev_range(r.start_day, r.end_day + DEV_END_OFFSET, opts.show_year);
            match pct {
                Some(p) => format!("{range}  {p}"),
                None => range,
            }
        };
        let w = text_w_mm(&lbl, 7.0);
        if bx1 + 2.0 + w <= CHART_X1 {
            shapes.extend(text_left(bx1 + 2.0, yc - 1.2, &lbl, 7.0, false, TEXT_GRAY));
        } else if bx0 - 2.0 - w >= lead_x0 {
            shapes.extend(text_right(bx0 - 2.0, yc - 1.2, &lbl, 7.0, false, TEXT_GRAY));
        } else {
            // Né a destra né a sinistra (barra tagliata ai due bordi da un
            // periodo più corto del dev): sopra la barra, allineata a destra.
            shapes.extend(text_right(
                CHART_X1 - 1.0,
                yc + bar_hh + 0.4,
                &lbl,
                7.0,
                false,
                TEXT_GRAY,
            ));
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
    app.projects.get_info(proj)
}

/// Primitive di disegno di una pagina Gantt per un progetto. `order`:
/// - `None` → dev con effort ordinati per data (usato dall'export "tutti i progetti");
/// - `Some(devs)` → dev nell'ordine dato (i senza effort → riga sottile).
///
/// `chart_only` esclude tripletta/descrizione e la banda della data in fondo.
/// `None` se il progetto non ha inizio E fine.
#[cfg(test)]
#[allow(clippy::too_many_arguments)]
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
    opts: &ExportOptions,
) -> Option<Vec<Shape>> {
    let bar = DevBar::from(fmt);
    project_shapes_fmt(
        app, proj, name, dev_info, today, created, order, chart_only, &|_| bar, opts,
    )
}

/// Come `project_shapes`, ma con il formato barra scelto **per dev**
/// (`fmt_of`).
#[allow(clippy::too_many_arguments)]
fn project_shapes_fmt(
    app: &App,
    proj: ProjectId,
    name: &str,
    dev_info: &DevInfo,
    today: i32,
    created: &str,
    order: Option<&[DevId]>,
    chart_only: bool,
    fmt_of: &dyn Fn(DevId) -> DevBar,
    opts: &ExportOptions,
) -> Option<Vec<Shape>> {
    let (Some(start_w), Some(end_w)) = (
        app.projects.get_project_start_week(proj),
        app.projects.get_project_end_week(proj),
    ) else {
        return None;
    };
    let proj_start = start_w.0 as i32;
    let proj_end = end_w.0 as i32;
    // Smussatura degli angoli: impostazione del file .ron (non una scelta della
    // dialog), riportata nei limiti 0–100 — `App::validate` segnala i valori fuori.
    let corner = app.corner_pct.clamp(0, 100);
    // Periodo scelto (dialog del singolo progetto): fuori restano settimane
    // di effort, settimane ghost e milestone.
    let window = opts.window();
    let in_window = |day: i32| window.is_none_or(|(s, e)| day + 7 > s && day < e);
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
        // Massimo su tutte le settimane, non solo quelle del periodo: le
        // altezze `Proportional` restano le stesse dell'export completo.
        let max = weeks.iter().map(|(_, h)| *h).max().unwrap_or(0);
        let weeks = weeks.into_iter().filter(|(d, _)| in_window(*d)).collect();
        (weeks, max)
    };
    // Settimane (giorni) del dev in cui un worker "ghost" è assegnato, **anche
    // con effort 0**. Con la stampa dei ghost disattivata l'elenco è vuoto e
    // nulla viene marcato di rosso.
    let ghost_weeks = |dev_id: DevId| -> Vec<i32> {
        if !opts.show_ghost {
            return Vec::new();
        }
        let Some(sd) = app.projects.get_single_dev(proj, dev_id) else {
            return Vec::new();
        };
        sd.weeks_with_worker_assigned(|wid| app.workers.is_ghost(wid))
            .into_iter()
            .map(|w| w.0 as i32)
            .filter(|d| in_window(*d))
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
                // Senza effort nel periodo scelto conta come «senza ore».
                let span = sd.effort_span().filter(|_| !week_data(dev_id).0.is_empty());
                let Some((first, last)) = span else {
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
                            fmt: fmt_of(dev_id).fmt,
                            dates_below: fmt_of(dev_id).dates_below(),
                            off: None,
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
                            fmt: fmt_of(dev_id).fmt,
                            dates_below: fmt_of(dev_id).dates_below(),
                            off: None,
                });
            }
            rows.sort_by_key(|r| (r.start_day, r.end_day));
        }
        Some(devs) => {
            for &dev_id in devs {
                let (label, color) = color_of(&dev_id);
                // Con effort nel periodo: barra tagliata ai bordi, etichetta con
                // le date reali. Con effort solo fuori dal periodo: niente barra,
                // le date reali al bordo dove sta il lavoro (`OffWindow`).
                // Senza effort: riga sottile.
                let span = app
                    .projects
                    .get_single_dev(proj, dev_id)
                    .and_then(|sd| sd.effort_span());
                let (weeks, max_week) = week_data(dev_id);
                let off = match (span, window) {
                    (Some((f, l)), Some(w)) if weeks.is_empty() => {
                        Some(OffWindow::of(w, f.0 as i32, l.0 as i32))
                    }
                    _ => None,
                };
                match span {
                    Some((first, last)) => {
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
                            fmt: fmt_of(dev_id).fmt,
                            // Fuori periodo non ci sono tratti sotto cui
                            // scrivere: riga di altezza normale.
                            dates_below: off.is_none() && fmt_of(dev_id).dates_below(),
                            off,
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
                            fmt: fmt_of(dev_id).fmt,
                            dates_below: fmt_of(dev_id).dates_below(),
                            off: None,
                        })
                    }
                }
            }
        }
    }

    let mut flags = Vec::new();
    for (mid, week) in app.projects.list_project_milestones(proj) {
        // Fuori ambito (es. milestone solo Internal in una stampa External) o
        // non spuntata nella dialog: la bandierina non viene disegnata.
        if !opts.milestone_shown(app, mid) || !in_window(week.0 as i32) {
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
        progress_pct,
        presumed_pct,
        opts,
        corner,
    ))
}

/// Costruisce il PDF con una pagina per ogni progetto visibile (abilitato e non
/// chiuso) dotato di data di inizio E fine. `None` se nessun progetto è idoneo.
#[cfg(test)]
pub fn build_pdf(app: &App, fmt: impl Into<DevBar>, opts: &ExportOptions) -> Option<Vec<u8>> {
    gantt_pdf(app, fmt.into(), opts, |id, enable| {
        enable && !app.projects.is_closed(id)
    })
}

/// Come `build_pdf`, ma limitato ai soli progetti indicati in `selected`
/// (una pagina Gantt ciascuno, nell'ordine di visualizzazione). Restano validi
/// i vincoli per pagina: il progetto deve avere inizio E fine. `None` se nessun
/// progetto selezionato produce una pagina.
pub fn build_pdf_selected(
    app: &App,
    selected: &[ProjectId],
    fmt: impl Into<DevBar>,
    opts: &ExportOptions,
) -> Option<Vec<u8>> {
    // I chiusi non sono esclusi a priori: la selezione a monte riflette la
    // modalità Vista (può includere progetti chiusi).
    gantt_pdf(app, fmt.into(), opts, |id, enable| {
        enable && selected.contains(&id)
    })
}

/// Una pagina Gantt per ogni progetto (in ordine di visualizzazione) che passa
/// `keep(id, abilitato)` e ha inizio e fine; tutti i dev con lo stesso `bar`.
fn gantt_pdf(
    app: &App,
    bar: DevBar,
    opts: &ExportOptions,
    keep: impl Fn(ProjectId, bool) -> bool,
) -> Option<Vec<u8>> {
    let created = today().format(US_DATE).to_string();
    let today = today_days();
    let dev_info = dev_info_map(app);
    let pages: Vec<Vec<Shape>> = app
        .projects
        .list_full()
        .into_iter()
        .filter(|(id, _, enable)| keep(*id, enable.0))
        .filter_map(|(id, name, _)| {
            project_shapes_fmt(
                app, id, &name, &dev_info, today, &created, None, false, &|_| bar, opts,
            )
        })
        .collect();
    pdf_document("Projects", &pages)
}

/// Costruisce un PDF di **un solo progetto**, con i dev nell'ordine `ordered_devs`
/// scelto dall'utente. I dev con effort producono la barra normale; quelli senza
/// effort una riga sottile del colore del dev per tutta la larghezza del calendario.
/// Con `ordered_devs` vuoto esporta comunque la pagina (asse, milestone, today…)
/// senza righe dev. `None` solo se il progetto non ha inizio E fine.
#[cfg(test)]
pub fn build_pdf_project(
    app: &App,
    proj: ProjectId,
    ordered_devs: &[DevId],
    fmt: BarFormat,
    opts: &ExportOptions,
) -> Option<Vec<u8>> {
    let devs: Vec<(DevId, DevBar)> = ordered_devs.iter().map(|&d| (d, fmt.into())).collect();
    build_pdf_project_fmt(app, proj, &devs, opts)
}

/// Lookup del formato per dev a partire dalla lista `(dev, formato)` della
/// dialog; un dev assente (non dovrebbe capitare) usa il formato predefinito.
fn fmt_lookup(devs: &[(DevId, DevBar)]) -> impl Fn(DevId) -> DevBar + '_ {
    move |d| {
        devs.iter()
            .find(|(x, _)| *x == d)
            .map(|(_, f)| *f)
            .unwrap_or_default()
    }
}

/// Come `build_pdf_project`, con il **formato barra scelto per ogni dev**
/// (`devs` = dev nell'ordine scelto, ciascuno col suo formato).
pub fn build_pdf_project_fmt(
    app: &App,
    proj: ProjectId,
    devs: &[(DevId, DevBar)],
    opts: &ExportOptions,
) -> Option<Vec<u8>> {
    let created = today().format(US_DATE).to_string();
    let today = today_days();
    let dev_info = dev_info_map(app);
    let name = project_name(app, proj);
    let order: Vec<DevId> = devs.iter().map(|(d, _)| *d).collect();
    let shapes = project_shapes_fmt(
        app,
        proj,
        &name,
        &dev_info,
        today,
        &created,
        Some(&order),
        false,
        &fmt_lookup(devs),
        opts,
    )?;
    pdf_document("Project", &[shapes])
}

/// Esporta il **solo grafico** di un progetto in SVG: come il PDF a singolo
/// progetto ma senza tripletta/descrizione e senza la data in fondo. `None` se il
/// progetto non ha inizio E fine.
#[cfg(test)]
pub fn build_svg_project(
    app: &App,
    proj: ProjectId,
    ordered_devs: &[DevId],
    fmt: BarFormat,
    opts: &ExportOptions,
) -> Option<String> {
    let devs: Vec<(DevId, DevBar)> = ordered_devs.iter().map(|&d| (d, fmt.into())).collect();
    build_svg_project_fmt(app, proj, &devs, opts)
}

/// Come `build_svg_project`, con il formato barra scelto per ogni dev.
pub fn build_svg_project_fmt(
    app: &App,
    proj: ProjectId,
    devs: &[(DevId, DevBar)],
    opts: &ExportOptions,
) -> Option<String> {
    let today = today_days();
    let dev_info = dev_info_map(app);
    let name = project_name(app, proj);
    let order: Vec<DevId> = devs.iter().map(|(d, _)| *d).collect();
    let shapes = project_shapes_fmt(
        app,
        proj,
        &name,
        &dev_info,
        today,
        "",
        Some(&order),
        true, // solo grafico
        &fmt_lookup(devs),
        opts,
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

/// Dev nel PDF andamento: dati, pianificato, colore, nome.
type TrendDev<'a> = (
    &'a crate::single_dev_utils::single_dev::SingleDev,
    u64,
    (f32, f32, f32),
    String,
);

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
    let mut devs: Vec<TrendDev> = Vec::new();
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
        if let Some(last) = hist.last()
            && (last.week.0 as i32) < today_week
        {
            pts.push((x_of(today_week), y_of(last.pct as f32)));
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
        "estimated/reported",
        7.0,
        false,
        TEXT_GRAY,
    ));
    shapes.extend(dashed_seg(kx, 196.5, kx + 8.0, 196.5, 0.9, BLACK));
    shapes.extend(text_left(
        kx + 10.0,
        195.5,
        "declared",
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
        let mname = MONTHS_EN[(m.month() - 1) as usize];
        let lbl = if first || m.month() == 1 {
            format!("{} '{:02}", mname, m.year() % 100)
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
            "today",
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
                (lx + 1.5).min(CHART_X1 - text_w_mm("Project", 7.0)),
                ly - 1.0,
                "Project",
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
    let created = today().format(US_DATE).to_string();
    let now = today();
    let today = local_to_days(&now);
    let today_week = local_to_days(&primo_giorno_settimana_corrente(&now));
    let dev_info = dev_info_map(app);

    let pages: Vec<Vec<Shape>> = projects
        .iter()
        .filter_map(|&id| {
            let name = project_name(app, id);
            trend_page_shapes(app, id, &name, &dev_info, today, today_week, &created)
        })
        .collect();
    pdf_document("Trend", &pages)
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

/// Data di una settimana nel report (`mm/dd/yyyy`, americana), "—" se assente.
fn report_date(w: Option<WeekId>) -> String {
    w.map_or_else(
        || "—".to_string(),
        |w| day_to_date(w.0 as i32).format(US_DATE).to_string(),
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
    "Estimated",
    "Used to date",
    "Allocated in grid",
    "Remaining",
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
        "(no code)".to_string()
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
        groups.push(format!("Through 12/31/{year}"));
        groups.push(format!("From 1/1/{} to end", year + 1));
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
    label_value(&mut shapes, y, "Category:", &category);
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
    label_value(&mut shapes, y, "Start:", &start);
    shapes.extend(text_left(REPORT_X0 + 80.0, y, "End:", 10.0, true, BLACK));
    shapes.extend(text_left(REPORT_X0 + 94.0, y, &end, 10.0, false, BLACK));
    y -= 6.0;
    label_value(
        &mut shapes,
        y,
        "Progress:",
        &format!(
            "estimated {}  ·  actual {}",
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
                "hours",
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
            "No devs in this project.",
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
                &format!("{title} (continued)"),
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
    let created = today().format(US_DATE).to_string();
    let today = today_days();
    let dev_info = dev_info_map(app);

    let pages: Vec<Vec<Shape>> = projects
        .iter()
        .flat_map(|&id| report_pages(app, id, &dev_info, today, &created))
        .collect();
    pdf_document("Report", &pages)
}

// --- Matrice progetti × worker -------------------------------------------------

const MATRIX_X0: f32 = X_LABEL;
const MATRIX_X1: f32 = PAGE_W - X_LABEL;
const MATRIX_TOP: f32 = PAGE_H - 30.0;
const MATRIX_BOT: f32 = FOOTER_TOP + 4.0;
const MATRIX_LABEL_W: f32 = 40.5;
const MATRIX_COL_MIN: f32 = 18.0;
const MATRIX_COL_MAX: f32 = 24.0;
const MATRIX_HEAD_H: f32 = 7.0;
const MATRIX_GROUP_H: f32 = 5.0;
const MATRIX_LINE_H: f32 = 3.4;
const MATRIX_FONT: f32 = 7.5;
const MATRIX_SUM_H: f32 = 5.0;
/// Sotto-righe per stato del riepilogo (Active / On-Hold / Closed).
const MATRIX_SUB_H: f32 = 4.0;
const MATRIX_SUB_FONT: f32 = 6.5;
/// Bordo fra un worker e l'altro (mm): marcato e nero, perché con le bande
/// colorate dei dev la divisione fra le colonne si perdeva.
const MATRIX_SEP_W: f32 = 0.6;

/// Etichetta data dell'intervallo: `10/17/2026`, con «(today)» se è oggi.
fn matrix_date(day: i32, today: i32) -> String {
    let s = day_to_date(day).format(US_DATE).to_string();
    if day == today {
        format!("{s} (today)")
    } else {
        s
    }
}

/// Nome inglese della categoria di un gruppo.
fn matrix_category_name(app: &App, cat: Option<crate::categories::CategoryId>) -> String {
    cat.and_then(|c| app.categories.get_name(c))
        .map(str::to_string)
        .unwrap_or_else(|| "No category".to_string())
}

/// Pagine della matrice: le colonne (worker) sono divise in blocchi che
/// entrano in larghezza; per ogni blocco le righe vanno su quante pagine
/// servono (titolo e intestazione ripetuti) e il riepilogo per categoria chiude
/// l'ultima pagina del blocco (o ne apre una nuova se non c'è spazio).
/// Sottotitolo della matrice: l'intervallo e, se scelti nella combo, gli anni
/// («From 01/01/2024 to 12/31/2026  ·  Years: 2024, 2026»).
fn matrix_subtitle(from: i32, to: i32, today: i32, years: &[i32], categories: &[String]) -> String {
    let mut out = format!(
        "From {} to {}",
        matrix_date(from, today),
        matrix_date(to, today)
    );
    if !years.is_empty() {
        let list: Vec<String> = years.iter().map(i32::to_string).collect();
        let label = if years.len() == 1 { "Year" } else { "Years" };
        out = format!("{out}  ·  {label}: {}", list.join(", "));
    }
    if !categories.is_empty() {
        let label = if categories.len() == 1 {
            "Category"
        } else {
            "Categories"
        };
        out = format!("{out}  ·  {label}: {}", categories.join(", "));
    }
    out
}

/// Fondo dell'intestazione di un worker senza ore nel periodo.
const MATRIX_NO_HOURS_BG: (f32, f32, f32) = (0.63, 0.63, 0.63);

/// Testo nero o bianco, quello che si legge meglio sul fondo `bg`.
fn text_on(bg: (f32, f32, f32)) -> (f32, f32, f32) {
    let lum = 0.299 * bg.0 + 0.587 * bg.1 + 0.114 * bg.2;
    if lum > 0.55 { BLACK } else { WHITE }
}

/// Larghezza **vera** di un testo in mm, dalle metriche del font incorporato
/// (DejaVu Sans, normale o grassetto). `text_w_mm` dà a ogni carattere la
/// stessa larghezza: va bene per impaginare, ma nelle celle strette della
/// matrice «MCSW 226h» sconfinava e «Test SYS 50h» veniva troncato senza
/// bisogno.
fn font_text_w_mm(s: &str, size_pt: f32, bold: bool) -> f32 {
    use std::sync::OnceLock;
    static REGULAR: OnceLock<Option<ttf_parser::Face<'static>>> = OnceLock::new();
    static BOLD: OnceLock<Option<ttf_parser::Face<'static>>> = OnceLock::new();
    let face = if bold {
        BOLD.get_or_init(|| ttf_parser::Face::parse(FONT_BOLD, 0).ok())
    } else {
        REGULAR.get_or_init(|| ttf_parser::Face::parse(FONT_REGULAR, 0).ok())
    };
    let Some(face) = face else {
        return text_w_mm(s, size_pt);
    };
    let em = face.units_per_em() as f32;
    let units: f32 = s
        .chars()
        .map(|c| {
            face.glyph_index(c)
                .and_then(|g| face.glyph_hor_advance(g))
                .map_or(em * 0.5, f32::from)
        })
        .sum();
    units / em * size_pt * 0.352_778
}

/// Come `truncate_to_w`, ma con la larghezza vera del font
/// (`font_text_w_mm`).
fn truncate_to_font_w(s: &str, size: f32, bold: bool, max_w: f32) -> String {
    if font_text_w_mm(s, size, bold) <= max_w {
        return s.to_string();
    }
    let mut out = String::new();
    for ch in s.chars() {
        if font_text_w_mm(&format!("{out}{ch}…"), size, bold) > max_w {
            break;
        }
        out.push(ch);
    }
    format!("{out}…")
}

/// Testo centrato su `x` con la larghezza vera del font.
fn text_center_font(
    x: f32,
    y: f32,
    s: &str,
    size: f32,
    bold: bool,
    color: (f32, f32, f32),
) -> Vec<Shape> {
    text_left(x - font_text_w_mm(s, size, bold) / 2.0, y, s, size, bold, color)
}

/// Caratteri del nome del dev che restano sempre in una cella della matrice
/// («MCSW», «Test…»): sotto, il dev non si riconoscerebbe più.
const MATRIX_MIN_NAME_CHARS: usize = 4;
/// Font minimo di una riga di cella rimpicciolita per starci.
const MATRIX_MIN_CELL_FONT: f32 = 5.5;

/// Nome del dev accorciato al minimo che resta sempre: i primi
/// `MATRIX_MIN_NAME_CHARS` caratteri e «…».
fn matrix_short_name(dev: &str) -> String {
    if dev.chars().count() > MATRIX_MIN_NAME_CHARS {
        dev.chars().take(MATRIX_MIN_NAME_CHARS).collect::<String>() + "…"
    } else {
        dev.to_string()
    }
}

/// Font di una riga di cella della matrice (nome del dev + `tail`, cioè ore e
/// percentuale) entro `max_w` mm: `MATRIX_FONT` se ci sta il nome accorciato
/// (`matrix_short_name`), altrimenti più piccolo, fino a
/// `MATRIX_MIN_CELL_FONT`. Una cella usa il minimo fra le sue righe, così le
/// righe hanno tutte la stessa dimensione.
fn matrix_cell_font(dev: &str, tail: &str, max_w: f32) -> f32 {
    let short = matrix_short_name(dev);
    let mut size = MATRIX_FONT;
    while size > MATRIX_MIN_CELL_FONT
        && font_text_w_mm(&format!("{short}{tail}"), size, false) > max_w
    {
        size -= 0.25;
    }
    size
}

/// Testo di una riga di cella al font `size`: il nome del dev si accorcia con
/// «…» quanto serve (ma mai sotto `matrix_short_name`), `tail` resta intero.
fn matrix_cell_text(dev: &str, tail: &str, max_w: f32, size: f32) -> String {
    let short = matrix_short_name(dev);
    let name_w = (max_w - font_text_w_mm(tail, size, false))
        .max(font_text_w_mm(&short, size, false));
    format!("{}{tail}", truncate_to_font_w(dev, size, false, name_w))
}

/// Nomi degli stati nel PDF della matrice (ordine di `ProjectStatus::ALL`).
const MATRIX_STATUS_EN: [&str; 3] = ["Active", "On-Hold", "Closed"];

#[allow(clippy::too_many_arguments)]
fn matrix_pages(
    app: &App,
    m: &crate::worker_matrix::MatrixData,
    from: i32,
    to: i32,
    years: &[i32],
    categories: &[String],
    today: i32,
    created: &str,
) -> Vec<Vec<Shape>> {
    use crate::worker_matrix::category_color;
    let avail = MATRIX_X1 - MATRIX_X0 - MATRIX_LABEL_W;
    let per_page = ((avail / MATRIX_COL_MIN).floor() as usize).max(1);
    let mut subtitle = matrix_subtitle(from, to, today, years, categories);
    // Filtro «Stato»: riportato solo se non sono mostrati tutti gli stati.
    if m.statuses.len() < MATRIX_STATUS_EN.len() {
        let names: Vec<&str> = m.statuses.iter().map(|&i| MATRIX_STATUS_EN[i]).collect();
        subtitle = format!("{subtitle}  ·  Status: {}", names.join(", "));
    }
    let row_h = |r: &crate::worker_matrix::MatrixRow, cols: &[usize]| -> f32 {
        let lines = cols
            .iter()
            .map(|&c| r.cells[c].len())
            .max()
            .unwrap_or(0)
            .max(1);
        lines as f32 * MATRIX_LINE_H + 1.6
    };

    let mut pages = Vec::new();
    let chunks: Vec<Vec<usize>> = (0..m.workers.len())
        .collect::<Vec<_>>()
        .chunks(per_page)
        .map(|c| c.to_vec())
        .collect();
    for cols in &chunks {
        let col_w = (avail / cols.len() as f32).min(MATRIX_COL_MAX);
        let x1 = MATRIX_X0 + MATRIX_LABEL_W + col_w * cols.len() as f32;
        let col_x = |i: usize| MATRIX_X0 + MATRIX_LABEL_W + i as f32 * col_w;
        let dev_rgb = |dev| {
            app.devs
                .get(dev)
                .map(|d| u32_rgb(d.bg_color() as u32))
                .unwrap_or(GRAY_FOOTER)
        };
        // Bordi verticali della fascia [y_bot, y_top]: sottile a sinistra,
        // marcato fra le etichette e i worker e fra un worker e l'altro.
        let vlines = |shapes: &mut Vec<Shape>, y_bot: f32, y_top: f32| {
            shapes.extend(line(MATRIX_X0, y_bot, MATRIX_X0, y_top, 0.2, GRAY_DK));
            for x in std::iter::once(MATRIX_X0 + MATRIX_LABEL_W).chain((1..=cols.len()).map(col_x)) {
                shapes.extend(line(x, y_bot, x, y_top, MATRIX_SEP_W, BLACK));
            }
        };

        // Titolo + sottotitolo con l'intervallo, in testa a ogni pagina.
        let title = |shapes: &mut Vec<Shape>| {
            shapes.extend(text_left(
                MATRIX_X0,
                PAGE_H - 14.0,
                "Projects × workers",
                16.0,
                true,
                BLACK,
            ));
            shapes.extend(text_left(
                MATRIX_X0,
                PAGE_H - 21.0,
                &subtitle,
                10.0,
                false,
                BLACK,
            ));
        };
        // Intestazione delle colonne (etichetta a sinistra + nomi dei worker)
        // con il bordo alto a `y`; restituisce la y sotto l'intestazione.
        let header = |shapes: &mut Vec<Shape>, y: f32, label: &str| -> f32 {
            shapes.extend(rect_fill(MATRIX_X0, y - MATRIX_HEAD_H, x1, y, GRAY_FOOTER));
            shapes.extend(text_left(MATRIX_X0 + 1.5, y - 4.6, label, 8.0, true, BLACK));
            for (i, &c) in cols.iter().enumerate() {
                // Sfondo diviso fra i dev del worker in proporzione alle ore
                // (stesse bande della finestra); grigio senza ore.
                let (cx0, cx1) = (col_x(i), col_x(i) + col_w);
                let bands = crate::worker_matrix::dev_bands(&m.dev_hours[c]);
                let mut under_name = MATRIX_NO_HOURS_BG;
                if bands.is_empty() {
                    shapes.extend(rect_fill(cx0, y - MATRIX_HEAD_H, cx1, y, under_name));
                }
                for (dev, from, to) in &bands {
                    let color = dev_rgb(*dev);
                    let bx0 = cx0 + (cx1 - cx0) * from;
                    let bx1 = cx0 + (cx1 - cx0) * to;
                    shapes.extend(rect_fill(bx0, y - MATRIX_HEAD_H, bx1, y, color));
                    if (*from..=*to).contains(&0.5) {
                        under_name = color;
                    }
                }
                let name = truncate_to_font_w(&m.workers[c].1, 8.0, true, col_w - 2.0);
                shapes.extend(text_center_font(
                    col_x(i) + col_w / 2.0,
                    y - 4.6,
                    &name,
                    8.0,
                    true,
                    text_on(under_name),
                ));
            }
            vlines(shapes, y - MATRIX_HEAD_H, y);
            y - MATRIX_HEAD_H
        };
        let new_page = |shapes: &mut Vec<Shape>| -> f32 {
            title(shapes);
            header(shapes, MATRIX_TOP, "Project")
        };
        let footer = |shapes: &mut Vec<Shape>| {
            shapes.extend(rect_fill(
                MATRIX_X0,
                FOOTER_BOT,
                MATRIX_X1,
                FOOTER_TOP,
                GRAY_FOOTER,
            ));
            shapes.extend(text_center(
                (MATRIX_X0 + MATRIX_X1) / 2.0,
                (FOOTER_BOT + FOOTER_TOP) / 2.0 - 1.4,
                created,
                8.0,
                false,
                TEXT_GRAY,
            ));
        };
        let mut shapes = Vec::new();
        let mut y = new_page(&mut shapes);
        let mut prev_cat: Option<Option<crate::categories::CategoryId>> = None;
        // Solo i progetti in cui compare almeno un worker di questo blocco.
        let rows = m
            .rows
            .iter()
            .filter(|r| cols.iter().any(|&c| !r.cells[c].is_empty()));
        for r in rows {
            let group = prev_cat != Some(r.category);
            let h = row_h(r, cols) + if group { MATRIX_GROUP_H } else { 0.0 };
            if y - h < MATRIX_BOT {
                footer(&mut shapes);
                pages.push(std::mem::take(&mut shapes));
                y = new_page(&mut shapes);
            }
            let color = u32_rgb(category_color(r.category));
            if group {
                // Intestazione del gruppo: nome della categoria su tutta la riga.
                shapes.extend(rect_fill(MATRIX_X0, y - MATRIX_GROUP_H, x1, y, color));
                shapes.extend(text_left(
                    MATRIX_X0 + 1.5,
                    y - 3.6,
                    &matrix_category_name(app, r.category),
                    8.0,
                    true,
                    BLACK,
                ));
                shapes.extend(line(
                    MATRIX_X0,
                    y - MATRIX_GROUP_H,
                    x1,
                    y - MATRIX_GROUP_H,
                    0.2,
                    GRAY_DK,
                ));
                y -= MATRIX_GROUP_H;
                prev_cat = Some(r.category);
            }
            let rh = row_h(r, cols);
            // Riga del progetto: tinta della categoria e solo la tripletta.
            let light = (
                1.0 - (1.0 - color.0) * 0.45,
                1.0 - (1.0 - color.1) * 0.45,
                1.0 - (1.0 - color.2) * 0.45,
            );
            // Progetto chiuso: tinta della categoria spenta verso il grigio, a
            // strisce, testo grigio, «closed».
            let fg = if r.closed {
                let (fill, hatch) = crate::worker_matrix::closed_colors(r.category);
                shapes.extend(rect_fill(MATRIX_X0, y - rh, x1, y, u32_rgb(fill)));
                let segs = crate::worker_matrix::hatch_segments(MATRIX_X0, x1, y, y - rh, 2.5);
                for (ax, ay, bx, by) in segs {
                    shapes.extend(line(ax, ay, bx, by, 0.2, u32_rgb(hatch)));
                }
                TEXT_GRAY
            } else {
                shapes.extend(rect_fill(MATRIX_X0, y - rh, x1, y, light));
                BLACK
            };
            let base = y - 2.9;
            let trip = truncate_to_w(&r.tripletta, MATRIX_FONT, MATRIX_LABEL_W - if r.closed { 13.0 } else { 3.0 });
            shapes.extend(text_left(MATRIX_X0 + 1.5, base, &trip, MATRIX_FONT, true, fg));
            if r.closed {
                let x = MATRIX_X0 + 1.5 + text_w_mm(&trip, MATRIX_FONT) * 1.15 + 2.0;
                let tag = match r.status {
                    crate::project_utils::project::ProjectStatus::OnHold => "on-hold",
                    _ => "closed",
                };
                shapes.extend(text_left(x, base, tag, MATRIX_FONT, false, fg));
            }
            for (i, &c) in cols.iter().enumerate() {
                // Dev con le ore: si tronca il nome, mai le ore.
                // Una sola dimensione di font per cella: la più piccola che
                // serve fra le sue righe.
                let total = crate::worker_matrix::worker_hours(&m.dev_hours[c]);
                let tails: Vec<String> = r.hours[c]
                    .iter()
                    .map(|h| crate::worker_matrix::cell_hours_label(*h, total))
                    .collect();
                let size = r.cells[c]
                    .iter()
                    .zip(&tails)
                    .map(|(dev, tail)| matrix_cell_font(dev, tail, col_w - 2.0))
                    .fold(MATRIX_FONT, f32::min);
                for (l, (dev, hours)) in r.cells[c].iter().zip(&tails).enumerate() {
                    let s = matrix_cell_text(dev, hours, col_w - 2.0, size);
                    shapes.extend(text_center_font(
                        col_x(i) + col_w / 2.0,
                        base - l as f32 * MATRIX_LINE_H,
                        &s,
                        size,
                        false,
                        fg,
                    ));
                }
            }
            shapes.extend(line(MATRIX_X0, y - rh, x1, y - rh, 0.2, GRAY_DK));
            vlines(&mut shapes, y - rh, y);
            y -= rh;
        }

        // Riepilogo: per ogni categoria, in quanti progetti è presente il worker,
        // seguita dalle sotto-righe per stato; poi il totale, idem; infine le ore
        // per dev (intestazione con le ore del worker, poi una riga per ogni dev
        // del file). Riga per riga, con salto pagina (intestazione dei worker
        // ripetuta) se non c'è spazio.
        enum SumRow {
            Cat(usize),
            CatStatus(usize, usize),
            Total,
            TotalStatus(usize),
            DevHead,
            Dev(usize),
        }
        let mut sum_rows = Vec::new();
        for ci in 0..m.categories.len() {
            sum_rows.push(SumRow::Cat(ci));
            sum_rows.extend(m.statuses.iter().map(|&si| SumRow::CatStatus(ci, si)));
        }
        sum_rows.push(SumRow::Total);
        sum_rows.extend(m.statuses.iter().map(|&si| SumRow::TotalStatus(si)));
        sum_rows.push(SumRow::DevHead);
        sum_rows.extend((0..m.devs.len()).map(SumRow::Dev));
        y -= 1.5;
        for row in &sum_rows {
            let h = match row {
                SumRow::Cat(_) | SumRow::Total | SumRow::DevHead | SumRow::Dev(_) => MATRIX_SUM_H,
                _ => MATRIX_SUB_H,
            };
            if matches!(row, SumRow::DevHead) {
                y -= 1.5;
            }
            if y - h < MATRIX_BOT {
                footer(&mut shapes);
                pages.push(std::mem::take(&mut shapes));
                y = new_page(&mut shapes);
            }
            let base = y - h / 2.0 - 1.0;
            match row {
                SumRow::Cat(ci) => {
                    let cat = m.categories[*ci];
                    shapes.extend(rect_fill(
                        MATRIX_X0,
                        y - h,
                        MATRIX_X0 + MATRIX_LABEL_W,
                        y,
                        u32_rgb(category_color(cat)),
                    ));
                    let name = truncate_to_w(
                        &matrix_category_name(app, cat),
                        MATRIX_FONT,
                        MATRIX_LABEL_W - 3.0,
                    );
                    shapes.extend(text_left(MATRIX_X0 + 1.5, base, &name, MATRIX_FONT, true, BLACK));
                    for (i, &c) in cols.iter().enumerate() {
                        let n = m.summary[*ci][c];
                        if n > 0 {
                            shapes.extend(text_center(
                                col_x(i) + col_w / 2.0,
                                base,
                                &n.to_string(),
                                MATRIX_FONT,
                                true,
                                BLACK,
                            ));
                        }
                    }
                }
                SumRow::Total => {
                    shapes.extend(rect_fill(MATRIX_X0, y - h, x1, y, GRAY_FOOTER));
                    shapes.extend(text_left(
                        MATRIX_X0 + 1.5,
                        base,
                        "Total projects",
                        MATRIX_FONT,
                        true,
                        BLACK,
                    ));
                    for (i, &c) in cols.iter().enumerate() {
                        shapes.extend(text_center(
                            col_x(i) + col_w / 2.0,
                            base,
                            &m.totals[c].to_string(),
                            MATRIX_FONT,
                            true,
                            BLACK,
                        ));
                    }
                }
                SumRow::DevHead => {
                    shapes.extend(rect_fill(MATRIX_X0, y - h, x1, y, GRAY_FOOTER));
                    shapes.extend(text_left(
                        MATRIX_X0 + 1.5,
                        base,
                        "Hours by dev",
                        MATRIX_FONT,
                        true,
                        BLACK,
                    ));
                    for (i, &c) in cols.iter().enumerate() {
                        let total = crate::worker_matrix::worker_hours(&m.dev_hours[c]);
                        shapes.extend(text_center(
                            col_x(i) + col_w / 2.0,
                            base,
                            &format!("{total}h"),
                            MATRIX_FONT,
                            true,
                            BLACK,
                        ));
                    }
                }
                SumRow::Dev(di) => {
                    let (dev, name) = &m.devs[*di];
                    let bg = dev_rgb(*dev);
                    let fg = app
                        .devs
                        .get(*dev)
                        .map(|d| u32_rgb(d.font_color() as u32))
                        .unwrap_or_else(|| text_on(bg));
                    shapes.extend(rect_fill(MATRIX_X0, y - h, MATRIX_X0 + MATRIX_LABEL_W, y, bg));
                    let name = truncate_to_w(name, MATRIX_FONT, MATRIX_LABEL_W - 3.0);
                    shapes.extend(text_left(MATRIX_X0 + 1.5, base, &name, MATRIX_FONT, true, fg));
                    for (i, &c) in cols.iter().enumerate() {
                        if let Some((hours, pct)) = crate::worker_matrix::dev_share(&m.dev_hours[c], *dev) {
                            let s = crate::worker_matrix::dev_share_label(hours, pct);
                            shapes.extend(text_center(
                                col_x(i) + col_w / 2.0,
                                base,
                                &truncate_to_w(&s, MATRIX_SUB_FONT, col_w - 2.0),
                                MATRIX_SUB_FONT,
                                false,
                                BLACK,
                            ));
                        }
                    }
                }
                SumRow::CatStatus(_, si) | SumRow::TotalStatus(si) => {
                    let counts = |c: usize| match row {
                        SumRow::CatStatus(ci, _) => m.summary_status[*ci][c][*si],
                        _ => m.status_totals[c][*si],
                    };
                    shapes.extend(text_left(
                        MATRIX_X0 + 4.5,
                        base,
                        MATRIX_STATUS_EN[*si],
                        MATRIX_SUB_FONT,
                        false,
                        TEXT_GRAY,
                    ));
                    for (i, &c) in cols.iter().enumerate() {
                        shapes.extend(text_center(
                            col_x(i) + col_w / 2.0,
                            base,
                            &counts(c).to_string(),
                            MATRIX_SUB_FONT,
                            false,
                            TEXT_GRAY,
                        ));
                    }
                }
            }
            shapes.extend(line(MATRIX_X0, y - h, x1, y - h, 0.2, GRAY_DK));
            vlines(&mut shapes, y - h, y);
            y -= h;
        }
        footer(&mut shapes);
        pages.push(shapes);
    }
    pages
}

/// PDF della matrice progetti × worker con l'intervallo `[from, to]` (giorni),
/// gli anni (`years`) e le categorie (`categories`, `None` = senza categoria)
/// scelti nelle combo in testa (vuoti = nessun filtro). `None` se non ci sono
/// worker o progetti da mostrare.
pub fn build_matrix_pdf(
    app: &App,
    m: &crate::worker_matrix::MatrixData,
    from: i32,
    to: i32,
    years: &[i32],
    categories: &[Option<crate::categories::CategoryId>],
) -> Option<Vec<u8>> {
    let categories: Vec<String> = categories
        .iter()
        .map(|c| matrix_category_name(app, *c))
        .collect();
    if m.workers.is_empty() || m.rows.is_empty() {
        return None;
    }
    let created = today().format(US_DATE).to_string();
    let today = today_days();
    let pages = matrix_pages(app, m, from, to, years, &categories, today, &created);
    pdf_document("Projects x workers", &pages)
}

// --- Torte dev/categorie per worker ---------------------------------------------

/// Pannelli della pagina (sinistra: dev e categorie, destra: categorie): in
/// ognuno il titolo, la torta centrata e sotto la sua tabella.
const PIE_PANEL_W: f32 = 140.0;
const PIE_TITLE_Y: f32 = 173.5;
/// Centro verticale, raggio esterno e raggio dell'anello dei dev (mm).
const PIE_R_MM: f32 = 44.0;
const PIE_CY: f32 = PIE_TITLE_Y - 4.0 - PIE_R_MM;
const PIE_R_IN_MM: f32 = 27.5;
const PIE_GAP_MM: f32 = 1.1;
/// Percentuali dentro le fette.
const PIE_LABEL_FONT: f32 = 8.0;
/// Tabella sotto la torta: prima riga, passo, font.
const PIE_TABLE_TOP: f32 = PIE_CY - PIE_R_MM - 6.0;
const PIE_ROW_H: f32 = 4.2;
const PIE_FONT: f32 = 7.0;

/// Colore del dev nel PDF.
fn dev_rgb(app: &App, dev: DevId) -> (f32, f32, f32) {
    app.devs
        .get(dev)
        .map(|d| u32_rgb(d.bg_color() as u32))
        .unwrap_or(GRAY_FOOTER)
}

/// Un anello della torta: fette, bordo bianco fra le fette e percentuale
/// (sul totale del worker) dove ci sta.
fn pie_ring<K>(
    cx: f32,
    r0: f32,
    r1: f32,
    segs: &[crate::worker_pies::Seg<K>],
    total: u32,
    color: impl Fn(&K) -> (f32, f32, f32),
) -> Vec<Shape> {
    use crate::worker_pies::{arc_point, label_pos, pct_label, sector};
    let mut out = Vec::new();
    for sg in segs {
        out.extend(poly_fill(&sector(cx, PIE_CY, r0, r1, sg.from, sg.to, true), color(&sg.key)));
    }
    if segs.len() > 1 {
        for sg in segs {
            let a = arc_point(cx, PIE_CY, r0, sg.from, true);
            let b = arc_point(cx, PIE_CY, r1, sg.from, true);
            out.extend(line(a.0, a.1, b.0, b.1, 0.8, WHITE));
        }
    }
    let h = PIE_LABEL_FONT * 0.352_778;
    for sg in segs {
        let label = pct_label(sg.hours, total);
        let w = text_w_mm(&label, PIE_LABEL_FONT) * 1.1;
        if let Some((x, y)) = label_pos(cx, PIE_CY, r0, r1, sg.from, sg.to, w, h, true) {
            out.extend(text_center(
                x,
                y - h * 0.35,
                &label,
                PIE_LABEL_FONT,
                true,
                text_on(color(&sg.key)),
            ));
        }
    }
    out
}

/// Una riga di tabella: quadratino colorato (se c'è), nome e valori allineati
/// a destra sulle colonne `cols` (bordo destro di ogni colonna).
#[allow(clippy::too_many_arguments)]
fn pie_table_row(
    x: f32,
    y: f32,
    font: f32,
    swatch: Option<(f32, f32, f32)>,
    name: &str,
    name_w: f32,
    cols: &[(f32, String)],
    bold: bool,
) -> Vec<Shape> {
    let mut out = Vec::new();
    let mut nx = x;
    if let Some(c) = swatch {
        let s = font * 0.352_778 * 0.8;
        out.extend(rect_fill(x, y - 0.3, x + s, y - 0.3 + s, c));
        nx += s + 1.2;
    }
    let name = truncate_to_w(name, font, name_w - (nx - x));
    out.extend(text_left(nx, y, &name, font, bold, BLACK));
    for (right, v) in cols {
        out.extend(text_right(*right, y, v, font, bold, BLACK));
    }
    out
}

/// Righe di tabella che entrano fra `PIE_TABLE_TOP` (sotto la torta) e il footer: se sono di
/// più, riga e font si riducono insieme.
fn pie_table_scale(rows: usize) -> (f32, f32) {
    let avail = PIE_TABLE_TOP - (FOOTER_TOP + 4.0);
    let k = (avail / (rows as f32 * PIE_ROW_H)).min(1.0);
    (PIE_ROW_H * k, (PIE_FONT * k).max(3.5))
}

/// Riga di una tabella delle torte: valori allineati alle intestazioni
/// numeriche (stringa vuota = cella vuota).
struct PieRow {
    swatch: Option<(f32, f32, f32)>,
    name: String,
    indent: bool,
    bold: bool,
    vals: Vec<String>,
}

/// Larghezza di una colonna numerica delle tabelle delle torte (mm).
const PIE_NUM_W: f32 = 17.0;

/// Celle delle ore: `fatte, pianificate, totale` se `split`, altrimenti solo il
/// totale.
fn pie_hour_cells(h: crate::worker_pies::Hours, split: bool) -> Vec<String> {
    if split {
        vec![format!("{}h", h.done), format!("{}h", h.planned()), format!("{}h", h.total)]
    } else {
        vec![format!("{}h", h.total)]
    }
}

/// Intestazioni delle colonne delle ore (vedi `pie_hour_cells`).
fn pie_hour_heads(split: bool) -> Vec<&'static str> {
    if split { vec!["Done", "Planned", "Total"] } else { vec!["Hours"] }
}

/// Tabella sotto una torta, fra `x0` e `right`: intestazione, righe e riga del
/// totale, con riga e font ridotti se non entrano (`pie_table_scale`).
fn pie_table(x0: f32, right: f32, first: &str, heads: &[&str], rows: &[PieRow], total: &PieRow) -> Vec<Shape> {
    let n = heads.len();
    let cols: Vec<f32> = (0..n).map(|i| right - (n - 1 - i) as f32 * PIE_NUM_W).collect();
    let name_w = cols[0] - PIE_NUM_W - x0;
    let (rh, font) = pie_table_scale(rows.len() + 2);
    let cells = |vals: &[String]| -> Vec<(f32, String)> {
        cols.iter()
            .zip(vals)
            .filter(|(_, v)| !v.is_empty())
            .map(|(x, v)| (*x, v.clone()))
            .collect()
    };
    let mut out = Vec::new();
    let mut y = PIE_TABLE_TOP;
    let head: Vec<String> = heads.iter().map(|h| h.to_string()).collect();
    out.extend(pie_table_row(x0, y, font, None, first, name_w, &cells(&head), true));
    out.extend(line(x0, y - rh * 0.35, right, y - rh * 0.35, 0.3, GRAY_DK));
    y -= rh;
    for r in rows {
        let ind = if r.indent { 3.0 } else { 0.0 };
        out.extend(pie_table_row(x0 + ind, y, font, r.swatch, &r.name, name_w - ind, &cells(&r.vals), r.bold));
        y -= rh;
    }
    out.extend(line(x0, y + rh * 0.65, right, y + rh * 0.65, 0.3, GRAY_DK));
    out.extend(pie_table_row(x0, y, font, None, &total.name, name_w, &cells(&total.vals), true));
    out
}

/// Pagina di un worker: titolo e periodo, a sinistra la torta dev/categorie con
/// la sua tabella, a destra quella delle categorie con la sua. `created` =
/// `None` per l'SVG (niente footer).
/// `split`: colonne fatte/pianificate (periodo oltre la settimana corrente).
fn pies_page(
    app: &App,
    pie: &crate::worker_pies::WorkerPie,
    subtitle: &str,
    split: bool,
    created: Option<&str>,
) -> Vec<Shape> {
    use crate::worker_matrix::category_color;
    use crate::worker_pies::{Hours, pct_label};
    let cat_rgb = |c: Option<crate::categories::CategoryId>| u32_rgb(category_color(c));
    let dev_name = |d: DevId| app.devs.get(d).map(|d| d.get_name()).unwrap_or_default();
    let mut out = Vec::new();
    out.extend(text_left(
        X_LABEL,
        PAGE_H - 14.0,
        &format!("Hours by dev and category — {}", pie.name),
        16.0,
        true,
        BLACK,
    ));
    // Sottotitolo: periodo e filtri; poi le ore (totale e capacità) e, con un
    // periodo oltre la settimana corrente, una riga per le ore libere (rossa se
    // oltre la capacità). Righe intere e non pezzi accostati: la larghezza dei
    // testi è solo stimata (`text_w_mm`).
    out.extend(text_left(X_LABEL, PAGE_H - 20.0, subtitle, 10.0, false, BLACK));
    let mut hours = if split {
        format!(
            "Total: {}h ({}h done, {}h planned)",
            pie.total,
            pie.done,
            pie.planned()
        )
    } else {
        format!("Total: {}h", pie.total)
    };
    hours = format!(
        "{hours}  ·  Capacity: {}h ({} used)",
        pie.capacity,
        pct_label(pie.total, pie.capacity)
    );
    out.extend(text_left(X_LABEL, PAGE_H - 25.5, &hours, 9.0, false, BLACK));
    if split {
        let free = pie.free_hours();
        // Ore pianificate di tutte le categorie, anche con il filtro.
        let all = if pie.filtered { " (all categories)" } else { "" };
        let head = format!(
            "From now to the end{all}: {}h planned of {}h available, ",
            pie.planned_all, pie.future_capacity
        );
        let (line, bold, color) = if free >= 0 {
            (format!("{head}{free}h free"), false, BLACK)
        } else {
            (format!("{head}{}h over capacity", -free), true, RED)
        };
        out.extend(text_left(X_LABEL, PAGE_H - 30.5, &line, 9.0, bold, color));
    }

    // Pannello sinistro: dev (anello interno) e categorie di ogni dev.
    let x0 = X_LABEL;
    let cx = x0 + PIE_PANEL_W / 2.0;
    out.extend(text_left(x0, PIE_TITLE_Y, "By dev and category", 10.0, true, BLACK));
    out.extend(pie_ring(cx, 0.0, PIE_R_IN_MM, &pie.dev_ring(), pie.total, |d| dev_rgb(app, *d)));
    out.extend(pie_ring(
        cx,
        PIE_R_IN_MM + PIE_GAP_MM,
        PIE_R_MM,
        &pie.dev_category_ring(),
        pie.total,
        |(_, c)| cat_rgb(*c),
    ));
    let total_cells = |pct_dev: bool| {
        let mut v = pie_hour_cells(Hours { total: pie.total, done: pie.done }, split);
        v.push("100%".to_string());
        if pct_dev {
            v.push(String::new());
        }
        v
    };
    let mut heads = pie_hour_heads(split);
    heads.extend(["% total", "% dev"]);
    let mut rows = Vec::new();
    for d in &pie.devs {
        let mut vals = pie_hour_cells(d.hours, split);
        vals.extend([pct_label(d.hours.total, pie.total), String::new()]);
        rows.push(PieRow {
            swatch: Some(dev_rgb(app, d.dev)),
            name: dev_name(d.dev),
            indent: false,
            bold: true,
            vals,
        });
        for &(c, h) in &d.categories {
            let mut vals = pie_hour_cells(h, split);
            vals.extend([pct_label(h.total, pie.total), pct_label(h.total, d.hours.total)]);
            rows.push(PieRow {
                swatch: Some(cat_rgb(c)),
                name: matrix_category_name(app, c),
                indent: true,
                bold: false,
                vals,
            });
        }
    }
    let total = PieRow {
        swatch: None,
        name: "Total".into(),
        indent: false,
        bold: true,
        vals: total_cells(true),
    };
    out.extend(pie_table(x0, x0 + PIE_PANEL_W, "Dev / Category", &heads, &rows, &total));

    // Pannello destro: categorie.
    let x0 = X_LABEL + PIE_PANEL_W + 5.0;
    let right = (x0 + PIE_PANEL_W).min(PAGE_W - X_LABEL);
    let cx = (x0 + right) / 2.0;
    out.extend(text_left(x0, PIE_TITLE_Y, "By category", 10.0, true, BLACK));
    out.extend(pie_ring(cx, 0.0, PIE_R_MM, &pie.category_ring(), pie.total, |c| cat_rgb(*c)));
    let mut heads = pie_hour_heads(split);
    heads.push("% total");
    let rows: Vec<PieRow> = pie
        .categories
        .iter()
        .map(|&(c, h)| {
            let mut vals = pie_hour_cells(h, split);
            vals.push(pct_label(h.total, pie.total));
            PieRow {
                swatch: Some(cat_rgb(c)),
                name: matrix_category_name(app, c),
                indent: false,
                bold: false,
                vals,
            }
        })
        .collect();
    let total = PieRow {
        swatch: None,
        name: "Total".into(),
        indent: false,
        bold: true,
        vals: total_cells(false),
    };
    out.extend(pie_table(x0, right, "Category", &heads, &rows, &total));

    if let Some(created) = created {
        out.extend(rect_fill(X_LABEL, FOOTER_BOT, PAGE_W - X_LABEL, FOOTER_TOP, GRAY_FOOTER));
        out.extend(text_center(
            PAGE_W / 2.0,
            (FOOTER_BOT + FOOTER_TOP) / 2.0 - 1.4,
            created,
            8.0,
            false,
            TEXT_GRAY,
        ));
    }
    out
}

/// Sottotitolo delle torte: intervallo, anni e categorie scelti (come la matrice).
fn pies_subtitle(
    app: &App,
    from: i32,
    to: i32,
    years: &[i32],
    categories: &[Option<crate::categories::CategoryId>],
) -> String {
    let names: Vec<String> = categories
        .iter()
        .map(|c| matrix_category_name(app, *c))
        .collect();
    matrix_subtitle(from, to, today_days(), years, &names)
}

/// PDF delle torte: una pagina per worker con ore nel periodo `[from, to]`
/// (`years`/`categories` = scelte delle combo, nel sottotitolo). `None` se
/// nessun worker ha ore.
pub fn build_pies_pdf(
    app: &App,
    pies: &[crate::worker_pies::WorkerPie],
    from: i32,
    to: i32,
    years: &[i32],
    categories: &[Option<crate::categories::CategoryId>],
) -> Option<Vec<u8>> {
    let subtitle = pies_subtitle(app, from, to, years, categories);
    let split = crate::worker_pies::split_shown(to, current_monday_days());
    let created = self::today().format(US_DATE).to_string();
    let pages: Vec<Vec<Shape>> = pies
        .iter()
        .filter(|p| p.total > 0)
        .map(|p| pies_page(app, p, &subtitle, split, Some(&created)))
        .collect();
    pdf_document("Hours by dev and category", &pages)
}

/// SVG delle torte: `(nome del worker, svg)` per ogni worker con ore.
pub fn build_pies_svgs(
    app: &App,
    pies: &[crate::worker_pies::WorkerPie],
    from: i32,
    to: i32,
    years: &[i32],
    categories: &[Option<crate::categories::CategoryId>],
) -> Vec<(String, String)> {
    let subtitle = pies_subtitle(app, from, to, years, categories);
    let split = crate::worker_pies::split_shown(to, current_monday_days());
    pies.iter()
        .filter(|p| p.total > 0)
        .map(|p| (p.name.clone(), render_svg(&pies_page(app, p, &subtitle, split, None))))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::milestones::MilestoneKind;
    use crate::single_dev_utils::single_dev::WeekId;
    use crate::single_effort_utils::single_effort::Effort;
    use crate::workers_utils::worker::WorkerId;

    /// Matrice progetti × worker: tanti worker (più blocchi di colonne) e tanti
    /// progetti (più pagine per blocco) → PDF valido con più pagine; senza
    /// progetti nell'intervallo → `None`.
    /// Riepilogo lungo (molte categorie × 4 righe): continua sulle pagine
    /// successive e nessuna riga della tabella scende sotto il margine.
    /// Intestazione del worker nel PDF della matrice: a 50/50 fra due dev, due
    /// bande dei colori dei dev larghe uguali; senza ore, fondo grigio.
    #[test]
    fn matrix_pdf_worker_header_is_split_by_dev_hours() {
        let mut app = App::new();
        let mario = app.workers.add("Mario");
        let luigi = app.workers.add("Luigi");
        let fe = app.devs.add("Frontend");
        let be = app.devs.add("Backend");
        let p = app.projects.add("x", Some("P1"), None);
        app.projects.add_dev(p, fe);
        app.projects.add_dev(p, be);
        app.projects.add_effort(p, fe, WeekId(20000), mario, Effort(8));
        app.projects.add_effort(p, be, WeekId(20000), mario, Effort(8));
        app.projects.add_effort(p, fe, WeekId(20000), luigi, Effort(0));
        let sel: std::collections::HashSet<_> = [mario, luigi].into_iter().collect();
        let m = crate::worker_matrix::build_filtered(
            &app,
            &sel,
            20000,
            20006,
            &Default::default(),
            &Default::default(),
        );
        let pages = matrix_pages(&app, &m, 20000, 20006, &[], &[], 20000, "x");
        let color = |d| u32_rgb(app.devs.get(d).unwrap().bg_color() as u32);
        let widths = |c: (f32, f32, f32)| -> Vec<f32> {
            pages[0]
                .iter()
                .filter_map(|sh| match sh {
                    // Le etichette delle righe «ore per dev» hanno lo stesso
                    // colore ma stanno nella colonna Project.
                    Shape::Rect { x0, x1, color, .. }
                        if *color == c && *x0 >= MATRIX_X0 + MATRIX_LABEL_W - 0.01 =>
                    {
                        Some(x1 - x0)
                    }
                    _ => None,
                })
                .collect()
        };
        let (wf, wb) = (widths(color(fe)), widths(color(be)));
        assert_eq!((wf.len(), wb.len()), (1, 1), "una banda per dev");
        assert!((wf[0] - wb[0]).abs() < 0.01, "metà e metà: {wf:?} {wb:?}");
        assert_eq!(widths(MATRIX_NO_HOURS_BG).len(), 1, "Luigi senza ore: grigio");
    }

    /// PDF della matrice: righe «Hours by dev» (ore del worker, poi ogni dev
    /// del file con ore e %) e bordi marcati neri fra i worker.
    #[test]
    fn matrix_pdf_has_dev_hour_rows_and_thick_worker_borders() {
        let mut app = App::new();
        let mario = app.workers.add("Mario");
        let luigi = app.workers.add("Luigi");
        let fe = app.devs.add("Frontend");
        let be = app.devs.add("Backend");
        app.devs.add("Design");
        let p = app.projects.add("x", Some("P1"), None);
        app.projects.add_dev(p, fe);
        app.projects.add_dev(p, be);
        app.projects.add_effort(p, fe, WeekId(20000), mario, Effort(30));
        app.projects.add_effort(p, be, WeekId(20000), mario, Effort(10));
        app.projects.add_effort(p, fe, WeekId(20000), luigi, Effort(8));
        let sel: std::collections::HashSet<_> = [mario, luigi].into_iter().collect();
        let m = crate::worker_matrix::build(&app, &sel, 20000, 20006);
        let pages = matrix_pages(&app, &m, 20000, 20006, &[], &[], 20000, "x");
        let all: Vec<String> = pages.iter().flat_map(|pg| texts(pg)).collect();
        for t in ["Hours by dev", "40h", "8h", "Design", "30h · 75%", "10h · 25%", "8h · 100%"] {
            assert!(all.iter().any(|s| s == t), "manca «{t}»: {all:?}");
        }
        let thick: std::collections::BTreeSet<i32> = pages[0]
            .iter()
            .filter_map(|sh| match sh {
                Shape::Line { x0, x1, thick, color, .. }
                    if *thick == MATRIX_SEP_W && x0 == x1 && *color == BLACK =>
                {
                    Some((x0 * 100.0).round() as i32)
                }
                _ => None,
            })
            .collect();
        assert_eq!(thick.len(), 3, "etichette|Mario, Mario|Luigi, dopo Luigi: {thick:?}");
    }

    /// Torte dei worker: una pagina per worker con ore (chi ne ha 0 è
    /// saltato), fette come poligoni, tabelle con ore e percentuali; SVG uno
    /// per worker, senza footer.
    #[test]
    fn pies_pdf_has_a_page_per_worker_with_hours() {
        let mut app = App::new();
        let mario = app.workers.add("Mario");
        let luigi = app.workers.add("Luigi");
        let anna = app.workers.add("Anna");
        let fe = app.devs.add("Frontend");
        let be = app.devs.add("Backend");
        let ca = app.categories.add("Cliente A");
        let p = app.projects.add("x", Some("P1"), None);
        app.projects.set_category(p, Some(ca));
        app.projects.add_dev(p, fe);
        app.projects.add_dev(p, be);
        app.projects.add_effort(p, fe, WeekId(20000), mario, Effort(30));
        app.projects.add_effort(p, be, WeekId(20000), mario, Effort(10));
        app.projects.add_effort(p, fe, WeekId(20000), anna, Effort(8));
        app.projects.add_effort(p, be, WeekId(20000), luigi, Effort(0));
        let sel: std::collections::HashSet<_> = [mario, luigi, anna].into_iter().collect();
        // «Oggi» la settimana dopo: le ore di 20000 sono fatte.
        let pies = crate::worker_pies::build(&app, &sel, 20000, 20006, &Default::default(), 20007);
        let m = pies.iter().find(|p| p.worker == mario).unwrap();
        let page = pies_page(&app, m, "From x to y", false, Some("created"));
        let t = texts(&page);
        for s in ["Hours by dev and category — Mario", "Frontend", "Backend", "Cliente A", "30h", "75%", "25%", "% dev", "created"] {
            assert!(t.iter().any(|x| x == s), "manca «{s}»: {t:?}");
        }
        let polys = page.iter().filter(|s| matches!(s, Shape::Poly { .. })).count();
        assert_eq!(polys, 5, "2 dev + 2 dev·categoria + 1 categoria");
        let svgs = build_pies_svgs(&app, &pies, 20000, 20006, &[], &[]);
        let names: Vec<_> = svgs.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(names, ["Anna", "Mario"], "Luigi senza ore è saltato");
        assert!(svgs[1].1.starts_with("<svg") && !svgs[1].1.contains("created"));
        let pdf = build_pies_pdf(&app, &pies, 20000, 20006, &[2026], &[Some(ca)]).expect("PDF");
        assert_eq!(
            pies_subtitle(&app, 20000, 20006, &[], &[Some(ca), None]),
            matrix_subtitle(20000, 20006, today_days(), &[], &["Cliente A".into(), "No category".into()])
        );
        assert!(pdf.starts_with(b"%PDF"));
        let none = crate::worker_pies::build(
            &app,
            &[luigi].into_iter().collect(),
            20000,
            20006,
            &Default::default(),
            20000,
        );
        assert!(build_pies_pdf(&app, &none, 20000, 20006, &[], &[]).is_none());
        // Con il periodo oltre la settimana corrente: colonne Done / Planned /
        // Total e sottotitolo con la divisione.
        let split = pies_page(&app, m, "From x to y", true, None);
        let t = texts(&split);
        for s in [
            "Done",
            "Planned",
            "Total",
            // 20000..20006 non parte di lunedì: tocca due settimane da 40h.
            "From x to y",
            "Total: 40h (40h done, 0h planned)  ·  Capacity: 80h (50% used)",
        ] {
            assert!(t.iter().any(|x| x == s), "manca «{s}»: {t:?}");
        }
        assert!(!texts(&page).iter().any(|x| x == "Planned"));
    }

    /// Larghezza vera dal font: le lettere larghe pesano più di quelle strette
    /// (la stima `text_w_mm` le considera uguali), il grassetto è più largo.
    #[test]
    fn font_width_follows_the_glyphs() {
        let wide = font_text_w_mm("MMMM", 10.0, false);
        let thin = font_text_w_mm("iiii", 10.0, false);
        assert!(wide > 2.5 * thin, "{wide} / {thin}");
        assert!(font_text_w_mm("Test", 10.0, true) > font_text_w_mm("Test", 10.0, false));
        let cut = truncate_to_font_w("Sviluppo firmware", 7.5, false, 10.0);
        assert!(cut.ends_with('…') && font_text_w_mm(&cut, 7.5, false) <= 10.0);
        assert_eq!(truncate_to_font_w("ELE", 7.5, false, 10.0), "ELE");
    }

    /// Riga di cella: il nome tiene almeno 4 caratteri, poi si rimpicciolisce
    /// il font; ore e percentuale restano intere; se c'è spazio, tutto intero.
    #[test]
    fn matrix_cell_keeps_part_of_the_dev_name() {
        let size = matrix_cell_font("Test SYS", " 226h 50%", 16.7);
        assert!((MATRIX_MIN_CELL_FONT..MATRIX_FONT).contains(&size), "{size}");
        let s = matrix_cell_text("Test SYS", " 226h 50%", 16.7, size);
        assert!(s.starts_with("Test") && s.ends_with(" 226h 50%"), "{s}");
        assert!(font_text_w_mm(&s, size, false) <= 16.7 || size == MATRIX_MIN_CELL_FONT);
        assert_eq!(matrix_cell_font("ELE", " 3h 1%", 16.7), MATRIX_FONT);
        assert_eq!(matrix_cell_text("ELE", " 3h 1%", 16.7, MATRIX_FONT), "ELE 3h 1%");
        // Allo stesso font più piccolo di un'altra riga, un nome corto resta
        // intero.
        assert_eq!(matrix_cell_text("SMS", " 32h 6%", 16.7, size), "SMS 32h 6%");
    }

    /// PDF della matrice: dev con le ore nella cella; il nome lungo si tronca,
    /// le ore restano.
    #[test]
    fn matrix_pdf_cells_show_hours() {
        let mut app = App::new();
        let mario = app.workers.add("Mario");
        let fe = app.devs.add("Frontend");
        let long = app.devs.add("Sviluppo firmware di sistema");
        let p = app.projects.add("x", Some("P1"), None);
        app.projects.add_dev(p, fe);
        app.projects.add_dev(p, long);
        app.projects.add_effort(p, fe, WeekId(20000), mario, Effort(30));
        app.projects.add_effort(p, long, WeekId(20000), mario, Effort(120));
        let sel = [mario].into_iter().collect();
        let m = crate::worker_matrix::build(&app, &sel, 20000, 20006);
        let pages = matrix_pages(&app, &m, 20000, 20006, &[], &[], 20000, "x");
        let t: Vec<String> = pages.iter().flat_map(|pg| texts(pg)).collect();
        // Percentuali sulle 150h di Mario; i nomi si accorciano, ore e % no.
        assert!(t.iter().any(|s| s.starts_with("Fron") && s.ends_with(" 30h 20%")), "{t:?}");
        assert!(t.iter().any(|s| s.starts_with("Svil") && s.ends_with("… 120h 80%")), "{t:?}");
    }

    #[test]
    fn matrix_summary_breaks_across_pages() {
        let mut app = App::new();
        let fe = app.devs.add("Frontend");
        let w = app.workers.add("Mario");
        for i in 0..12 {
            let cat = app.categories.add(&format!("Cat {i}"));
            let p = app.projects.add("x", Some(&format!("P{i:02}")), None);
            app.projects.set_category(p, Some(cat));
            app.projects.add_dev(p, fe);
            app.projects.add_effort(p, fe, WeekId(20000), w, Effort(1));
        }
        let sel = [w].into_iter().collect();
        let m = crate::worker_matrix::build(&app, &sel, 20000, 20006);
        let pages = matrix_pages(&app, &m, 20000, 20006, &[], &[], 20000, "x");
        assert!(pages.len() >= 2, "il riepilogo deve continuare: {}", pages.len());
        for page in &pages {
            for sh in page {
                if let Shape::Line { y0, y1, .. } = sh {
                    assert!(y0.min(*y1) >= MATRIX_BOT - 0.01, "riga sotto il margine");
                }
            }
        }
        // Tutte le sotto-righe di stato ci sono: 12 categorie + totale.
        let actives = pages
            .iter()
            .flat_map(|pg| texts(pg))
            .filter(|t| t == "Active")
            .count();
        assert_eq!(actives, 13);
    }

    /// Filtro «Stato» nel PDF: sotto-righe solo per gli stati scelti e stato
    /// riportato nel sottotitolo.
    #[test]
    fn matrix_pdf_follows_the_status_filter() {
        use crate::project_utils::project::ProjectStatus;
        let mut app = App::new();
        let fe = app.devs.add("Frontend");
        let w = app.workers.add("Mario");
        for i in 0..3 {
            let p = app.projects.add("x", Some(&format!("P{i}")), None);
            app.projects.add_dev(p, fe);
            app.projects.add_effort(p, fe, WeekId(20000), w, Effort(1));
            app.projects.set_status(p, ProjectStatus::ALL[i]);
        }
        let sel = [w].into_iter().collect();
        let pick = [ProjectStatus::OnHold].into();
        let m = crate::worker_matrix::build_filtered(
            &app,
            &sel,
            20000,
            20006,
            &Default::default(),
            &pick,
        );
        let pages = matrix_pages(&app, &m, 20000, 20006, &[], &[], 20000, "x");
        let all: Vec<String> = pages.iter().flat_map(|pg| texts(pg)).collect();
        let count = |t: &str| all.iter().filter(|x| *x == t).count();
        assert_eq!(count("On-Hold"), 2, "categoria + totale");
        assert_eq!(count("Active"), 0);
        assert_eq!(count("Closed"), 0);
        assert!(all.iter().any(|t| t.ends_with("Status: On-Hold")), "sottotitolo");
        assert!(!all.iter().any(|t| t == "P0" || t == "P2"), "solo il progetto on-hold");
    }

    #[test]
    fn worker_matrix_pdf_paginates() {
        let mut app = App::new();
        let dev = app.devs.add("Frontend");
        let cat = app.categories.add("Cat");
        let workers: Vec<WorkerId> = (0..12).map(|i| app.workers.add(&format!("W{i}"))).collect();
        for i in 0..60 {
            let p = app
                .projects
                .add(&format!("Progetto {i}"), Some(&format!("P{i:02}")), None);
            if i % 2 == 0 {
                app.projects.set_category(p, Some(cat));
            }
            app.projects.add_dev(p, dev);
            app.projects
                .add_effort(p, dev, WeekId(20000), workers[i % 12], Effort(0));
        }
        let sel = workers.iter().copied().collect();
        let m = crate::worker_matrix::build(&app, &sel, 20000, 20006);
        assert_eq!(m.rows.len(), 60);
        let pages = matrix_pages(&app, &m, 20000, 20006, &[], &[], 20000, "x");
        // 12 worker stanno in un solo blocco di colonne; 60 progetti (più le
        // intestazioni dei 2 gruppi e il riepilogo) vanno su 3 pagine.
        assert_eq!(pages.len(), 3);
        let bytes =
            build_matrix_pdf(&app, &m, 20000, 20006, &[2024], &[Some(cat), None]).expect("Some");
        assert!(bytes.starts_with(b"%PDF"));
        let empty = crate::worker_matrix::build(&app, &sel, 30000, 30006);
        assert!(build_matrix_pdf(&app, &empty, 30000, 30006, &[], &[]).is_none());
    }

    /// Sottotitolo: gli anni scelti seguono l'intervallo; senza anni solo
    /// l'intervallo.
    #[test]
    fn matrix_subtitle_lists_selected_years() {
        let d = |y, m, dd| local_to_days(&NaiveDate::from_ymd_opt(y, m, dd).unwrap());
        let (from, to, today) = (d(2024, 1, 1), d(2026, 12, 31), d(2026, 10, 17));
        assert_eq!(
            matrix_subtitle(from, to, today, &[], &[]),
            "From 01/01/2024 to 12/31/2026"
        );
        assert_eq!(
            matrix_subtitle(from, to, today, &[2024, 2026], &[]),
            "From 01/01/2024 to 12/31/2026  ·  Years: 2024, 2026"
        );
        assert_eq!(
            matrix_subtitle(d(2026, 1, 1), to, today, &[2026], &["Cliente A".into()]),
            "From 01/01/2026 to 12/31/2026  ·  Year: 2026  ·  Category: Cliente A"
        );
        assert_eq!(
            matrix_subtitle(from, to, today, &[], &["A".into(), "No category".into()]),
            "From 01/01/2024 to 12/31/2026  ·  Categories: A, No category"
        );
    }

    #[test]
    fn no_eligible_projects_returns_none() {
        let app = App::new();
        assert!(build_pdf(&app, BarFormat::Continuous, &ExportOptions::default()).is_none());
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

        let bytes = build_pdf(&app, BarFormat::Continuous, &ExportOptions::default())
            .expect("un progetto idoneo → Some");
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
        assert!(build_pdf(&app, BarFormat::Continuous, &ExportOptions::default()).is_none());
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
        let bytes =
            build_pdf_selected(&app, &[a], BarFormat::Continuous, &ExportOptions::default())
                .expect("progetto selezionato idoneo → Some");
        assert!(bytes.starts_with(b"%PDF"));

        // Nessun progetto selezionato → None.
        assert!(
            build_pdf_selected(&app, &[], BarFormat::Continuous, &ExportOptions::default())
                .is_none()
        );

        // Progetto selezionato ma senza fine → None (non idoneo).
        let c = app
            .projects
            .add("Senza fine", Some("CCC"), Some(WeekId(20000)));
        assert!(
            build_pdf_selected(&app, &[c], BarFormat::Continuous, &ExportOptions::default())
                .is_none()
        );
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
        let bytes = build_pdf_project(
            &app,
            pid,
            &[dev_empty, dev_eff],
            BarFormat::Continuous,
            &ExportOptions::default(),
        )
        .expect("progetto con inizio/fine e dev → Some");
        assert!(bytes.starts_with(b"%PDF"));

        // Nessun dev selezionato → esporta comunque il resto (milestone, asse…).
        let bytes = build_pdf_project(
            &app,
            pid,
            &[],
            BarFormat::Continuous,
            &ExportOptions::default(),
        )
        .expect("senza dev → Some");
        assert!(bytes.starts_with(b"%PDF"));
    }

    #[test]
    fn single_project_without_end_returns_none() {
        let mut app = App::new();
        let pid = app.projects.add("Prog", Some("ABC"), Some(WeekId(20000)));
        // niente fine → None anche col percorso singolo progetto
        assert!(
            build_pdf_project(
                &app,
                pid,
                &[],
                BarFormat::Continuous,
                &ExportOptions::default()
            )
            .is_none()
        );
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

        let svg = build_svg_project(
            &app,
            pid,
            &[dev],
            BarFormat::Continuous,
            &ExportOptions::default(),
        )
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
        assert!(
            build_svg_project(
                &app,
                p2,
                &[],
                BarFormat::Continuous,
                &ExportOptions::default()
            )
            .is_none()
        );
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
        let goal_svg = build_svg_project(
            &app,
            pid,
            &[dev],
            BarFormat::Continuous,
            &ExportOptions::default(),
        )
        .unwrap();
        assert_eq!(bolt(&goal_svg), 0, "senza trigger niente fulmine");

        // Stessa milestone marcata come trigger: il triangolo lascia il posto
        // al fulmine, uno per uno.
        app.milestones.set_kind(mid, MilestoneKind::Trigger);
        let svg = build_svg_project(
            &app,
            pid,
            &[dev],
            BarFormat::Continuous,
            &ExportOptions::default(),
        )
        .unwrap();
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

        let scoped = |scope| ExportOptions {
            scope,
            ..Default::default()
        };
        // Mix: tutte e due le bandierine.
        let svg = build_svg_project(
            &app,
            pid,
            &[dev],
            BarFormat::Continuous,
            &scoped(MilestoneScope::Mix),
        )
        .unwrap();
        assert!(svg.contains("SoloInterna"));
        assert!(svg.contains("AncheEsterna"));

        // Solo internal: resta fuori quella marcata solo External.
        let svg = build_svg_project(
            &app,
            pid,
            &[dev],
            BarFormat::Continuous,
            &scoped(MilestoneScope::Internal),
        )
        .unwrap();
        assert!(svg.contains("SoloInterna"));
        assert_eq!(
            svg.contains("AncheEsterna"),
            app.milestones
                .has_category(esterna, MilestoneCategory::Internal),
        );

        // External: solo quella External.
        let svg = build_svg_project(
            &app,
            pid,
            &[dev],
            BarFormat::Continuous,
            &scoped(MilestoneScope::External),
        )
        .unwrap();
        assert!(
            !svg.contains("SoloInterna"),
            "la milestone Internal non va nella stampa External"
        );
        assert!(svg.contains("AncheEsterna"));
    }

    /// Periodo scelto nella dialog del singolo progetto: l'asse copre solo
    /// quello, le milestone fuori non si vedono, un dev senza effort nel
    /// periodo diventa la riga sottile (niente date) e un dev tagliato al
    /// bordo mantiene nell'etichetta le sue date reali.
    #[test]
    fn export_range_limits_axis_milestones_and_devs() {
        let mut app = App::new();
        // 20000 = 4 ottobre 2024, 20140 = 21 febbraio 2025.
        let pid = app.projects.add("Prog", Some("ABC"), Some(WeekId(20000)));
        app.projects.set_project_end_week(pid, Some(WeekId(20140)));
        let solo_ottobre = app.devs.add("SoloOttobre");
        let a_cavallo = app.devs.add("ACavallo");
        for d in [solo_ottobre, a_cavallo] {
            app.projects.add_dev(pid, d);
        }
        for w in [20000, 20007] {
            app.projects
                .add_effort(pid, solo_ottobre, WeekId(w), WorkerId(0), Effort(8));
        }
        // 6 dicembre e 3 gennaio: il periodo parte dal 20 dicembre.
        for w in [20063, 20091] {
            app.projects
                .add_effort(pid, a_cavallo, WeekId(w), WorkerId(0), Effort(8));
        }
        let inizio = app.milestones.add("Kickoff");
        let rilascio = app.milestones.add("Rilascio");
        app.projects.add_project_milestone(pid, inizio, WeekId(20007));
        app.projects.add_project_milestone(pid, rilascio, WeekId(20098));
        let devs = [solo_ottobre, a_cavallo];

        // Senza periodo: tutto il progetto, ottobre compreso.
        let full = build_svg_project(
            &app,
            pid,
            &devs,
            BarFormat::Continuous,
            &ExportOptions::default(),
        )
        .unwrap();
        assert!(full.contains(">Oct<") && full.contains("Kickoff"));
        assert!(full.contains("Oct 4"), "date del dev di ottobre");

        let ranged = ExportOptions {
            range: Some((20077, 20140)),
            ..Default::default()
        };
        let svg = build_svg_project(&app, pid, &devs, BarFormat::Continuous, &ranged).unwrap();
        assert!(!svg.contains(">Oct<"), "ottobre è fuori dall'asse");
        assert!(svg.contains(">Jan<"));
        assert!(!svg.contains("Kickoff"), "milestone fuori periodo omessa");
        assert!(svg.contains("Rilascio"));
        assert!(svg.contains("SoloOttobre"));
        // Dev tagliato al bordo sinistro: date reali (dal 6 dicembre).
        assert!(svg.contains("Dec 6"), "date reali del dev tagliato");
    }

    /// Periodo che inizia di giovedì 1° gennaio: l'asse parte dal lunedì 29
    /// dicembre (settimane intere), ma l'anno a sinistra è quello del primo
    /// mese leggibile (2026), non quello del ritaglio di dicembre (2025).
    /// Caso reale: progetto «OVT 3» esportato da gennaio 2026 ad agosto 2027.
    #[test]
    fn year_labels_follow_the_first_and_last_readable_month() {
        let d = |y, m, dd| local_to_days(&NaiveDate::from_ymd_opt(y, m, dd).unwrap());
        let mut app = App::new();
        let pid = app
            .projects
            .add("OVT", Some("OVT 3"), Some(WeekId(d(2026, 11, 9) as usize)));
        app.projects
            .set_project_end_week(pid, Some(WeekId(d(2027, 7, 5) as usize)));
        let dev = app.devs.add("Frontend");
        app.projects.add_dev(pid, dev);
        app.projects
            .add_effort(pid, dev, WeekId(d(2026, 11, 9) as usize), WorkerId(0), Effort(8));
        let opts = ExportOptions {
            range: Some((d(2026, 1, 1), d(2027, 8, 31))),
            ..Default::default()
        };
        let window = opts.window().unwrap();
        assert_eq!(day_to_date(window.0), NaiveDate::from_ymd_opt(2025, 12, 29).unwrap());

        let svg = build_svg_project(&app, pid, &[dev], BarFormat::Continuous, &opts).unwrap();
        assert!(svg.contains(">2026<"), "anno a sinistra: quello di Jan 2026");
        assert!(!svg.contains(">2025<"), "il ritaglio di dicembre 2025 non dà l'anno");
        assert!(svg.contains(">2027<"), "anno a destra");
    }

    /// Dev con effort solo fuori dal periodo scelto: niente barra né linea
    /// sottile, solo le sue date reali — a sinistra se ha già finito, a destra
    /// se deve ancora iniziare, al centro se il periodo cade in un suo buco.
    #[test]
    fn devs_outside_the_export_range_show_their_dates_at_the_edges() {
        let mut app = App::new();
        // 20000 = 4 ottobre 2024 … 20210 = 2 maggio 2025.
        let pid = app.projects.add("Prog", Some("ABC"), Some(WeekId(20000)));
        app.projects.set_project_end_week(pid, Some(WeekId(20210)));
        let finito = app.devs.add("Finito");
        let futuro = app.devs.add("Futuro");
        let buco = app.devs.add("ConBuco");
        let dentro = app.devs.add("Dentro");
        let mut add = |dev, ws: &[usize]| {
            app.projects.add_dev(pid, dev);
            for w in ws {
                app.projects
                    .add_effort(pid, dev, WeekId(*w), WorkerId(0), Effort(8));
            }
        };
        add(finito, &[20000, 20007]); // ottobre
        add(futuro, &[20161]); // 14 marzo
        add(buco, &[20000, 20161]);
        add(dentro, &[20091]); // 3 gennaio
        let order = [finito, futuro, buco, dentro];

        // Periodo: 20 dicembre 2024 → settimana del 21 febbraio 2025.
        let shapes = project_shapes_fmt(
            &app,
            pid,
            "Prog",
            &dev_info_map(&app),
            20091,
            "",
            Some(&order),
            true,
            &|_| DevBar::from(BarFormat::Continuous),
            &ExportOptions {
                range: Some((20077, 20140)),
                ..Default::default()
            },
        )
        .unwrap();

        // (x iniziale, larghezza) del testo che contiene `pat`.
        let at = |pat: &str| -> (f32, f32) {
            shapes
                .iter()
                .find_map(|sh| match sh {
                    Shape::Text { x, s, size, .. } if s.contains(pat) => {
                        Some((*x, text_w_mm(s, *size)))
                    }
                    _ => None,
                })
                .unwrap_or_else(|| panic!("manca il testo {pat:?}: {:?}", texts(&shapes)))
        };
        let (x, _) = at("Oct 4 - Oct 15");
        assert!((x - (CHART_X0 + 1.0)).abs() < 0.01, "già finito: a sinistra ({x})");
        let (x, w) = at("Mar 14 - Mar 18");
        assert!(
            (x + w - (CHART_X1 - 1.0)).abs() < 0.01,
            "deve iniziare: a destra ({x}+{w})"
        );
        let (x, w) = at("Oct 4 '24 - Mar 18");
        let mid = (CHART_X0 + CHART_X1) / 2.0;
        assert!((x + w / 2.0 - mid).abs() < 0.01, "buco: al centro ({x}+{w})");

        // Nessuna linea sottile a tutta larghezza: il solo dev con effort nel
        // periodo ha la sua barra, gli altri tre solo il testo.
        let full_width = shapes
            .iter()
            .filter(|sh| {
                matches!(sh, Shape::Rect { x0, x1, .. }
                    if (*x0 - CHART_X0).abs() < 0.01 && (*x1 - CHART_X1).abs() < 0.01)
            })
            .count();
        assert_eq!(full_width, 0, "niente riga sottile per i dev fuori periodo");
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

    fn shapes_all_devs(app: &App, pid: ProjectId, opts: &ExportOptions) -> Vec<Shape> {
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
            opts,
        )
        .unwrap()
    }

    /// Opzioni di default con la stampa dei ghost attiva o no.
    fn ghost_opts(show_ghost: bool) -> ExportOptions {
        ExportOptions {
            show_ghost,
            ..Default::default()
        }
    }

    /// Con la spunta «Includi i worker ghost» il ghost si stampa anche dove ha
    /// effort 0, e il dev che ha solo lui compare lo stesso; senza spunta la
    /// pagina torna a ignorarli.
    #[test]
    fn zero_effort_ghost_is_printed_only_when_enabled() {
        let (app, pid, ..) = app_with_zero_effort_ghost();

        let on = shapes_all_devs(&app, pid, &ghost_opts(true));
        assert!(
            texts(&on).iter().any(|t| t == "Backend"),
            "il dev con il solo ghost a effort 0 deve comparire"
        );
        let off = shapes_all_devs(&app, pid, &ghost_opts(false));
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
    }

    /// Con «Colore del dev» il ghost resta stampato (il dev col solo ghost a
    /// effort 0 compare, le settimane sono marcate) ma senza nulla di rosso:
    /// marcature nel colore del dev e nome nero.
    #[test]
    fn ghost_in_dev_color_prints_no_red() {
        let (app, pid, ..) = app_with_zero_effort_ghost();
        let red_texts = |shapes: &[Shape]| {
            shapes
                .iter()
                .filter(|sh| matches!(sh, Shape::Text { color, .. } if *color == RED))
                .count()
        };

        let off = shapes_all_devs(&app, pid, &ghost_opts(false));
        let red = shapes_all_devs(&app, pid, &ghost_opts(true));
        let dev_col = shapes_all_devs(
            &app,
            pid,
            &ExportOptions {
                ghost_red: false,
                ..ghost_opts(true)
            },
        );

        assert!(
            texts(&dev_col).iter().any(|t| t == "Backend"),
            "il dev con il solo ghost a effort 0 compare anche nel colore del dev"
        );
        assert_eq!(
            red_rects(&dev_col),
            red_rects(&off),
            "nessuna marcatura rossa per i ghost"
        );
        assert_eq!(red_texts(&dev_col), red_texts(&off), "nomi dei dev neri");
        assert!(red_texts(&red) > red_texts(&off), "in rosso il nome è rosso");
        // Le marcature ci sono comunque, nel colore del dev: stesso numero di
        // forme della versione rossa.
        assert_eq!(dev_col.len(), red.len());
    }

    /// Nella pagina del singolo progetto (dev scelti a mano) il ghost a effort 0
    /// marca la riga sottile del dev senza ore.
    #[test]
    fn zero_effort_ghost_marks_the_thin_row_of_a_dev_without_effort() {
        let (app, pid, _, solo_ghost) = app_with_zero_effort_ghost();
        let page = |ghost_on: bool| -> Vec<Shape> {
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
                &ghost_opts(ghost_on),
            )
            .unwrap()
        };

        assert_eq!(
            red_rects(&page(true)) - red_rects(&page(false)),
            1,
            "la settimana del ghost è marcata sulla riga sottile"
        );
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
                &ghost_opts(true),
            )
            .unwrap();
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
                &ghost_opts(false),
            )
            .unwrap();
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

        let shapes_of = |app: &App| shapes_all_devs(app, pid, &ExportOptions::default());
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
        let pts = round_rect_pts(10.0, 20.0, 30.0, 24.0, 100);
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
        let pts = round_rect_pts(10.0, 20.0, 30.0, 24.0, 50);
        let top_left_x = pts
            .iter()
            .filter(|(_, y)| (*y - 24.0).abs() < 0.01)
            .map(|(x, _)| *x)
            .fold(f32::MAX, f32::min);
        assert!((top_left_x - 11.0).abs() < 0.05, "rientro atteso 1mm: {top_left_x}");
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

        let svg_with = |app: &App, allow: Option<Vec<MilestoneId>>, scope| {
            let opts = ExportOptions {
                milestones: allow.map(|v| v.into_iter().collect()),
                scope,
                ..Default::default()
            };
            build_svg_project(app, pid, &[dev], BarFormat::Continuous, &opts).unwrap()
        };
        // Solo la prima è spuntata nella dialog: l'altra non viene disegnata.
        let svg = svg_with(&app, Some(vec![tenuta]), MilestoneScope::Mix);
        assert!(svg.contains("Tenuta"));
        assert!(!svg.contains("Scartata"));

        // Il filtro si combina in AND con l'ambito: in External resta fuori
        // anche quella spuntata, se non è marcata External.
        let svg = svg_with(&app, Some(vec![tenuta]), MilestoneScope::External);
        assert!(!svg.contains("Tenuta"));
        app.milestones
            .set_category(tenuta, MilestoneCategory::External, true);
        let svg = svg_with(&app, Some(vec![tenuta]), MilestoneScope::External);
        assert!(svg.contains("Tenuta"));

        // Nessuna selezione = nessuna bandierina.
        let svg = svg_with(&app, Some(Vec::new()), MilestoneScope::Mix);
        assert!(!svg.contains("Tenuta"));
        assert!(!svg.contains("Scartata"));

        // Senza filtro (export multi-progetto) tornano tutte.
        let svg = svg_with(&app, None, MilestoneScope::Mix);
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

        let pct = ExportOptions {
            show_pct: true,
            ..Default::default()
        };
        // Flag off (default): niente percentuali.
        let svg = build_svg_project(
            &app,
            pid,
            &[dev, dev_empty],
            BarFormat::Continuous,
            &ExportOptions::default(),
        )
        .unwrap();
        assert!(!svg.contains("80%/60%"), "senza flag non devono comparire");

        // Flag on: compare "presunta/dichiarata" solo per il dev con effort.
        let svg =
            build_svg_project(&app, pid, &[dev, dev_empty], BarFormat::Continuous, &pct).unwrap();
        assert!(svg.contains("80%/60%"), "con flag deve comparire 80%/60%");
        assert!(!svg.contains("33%"), "il dev senza effort non mostra la %");
    }

    /// La spunta «anno» aggiunge `'AA` alle date del dev (solo alla fine se
    /// inizio e fine sono nello stesso anno) e a nient'altro nella pagina.
    /// Inizio e fine in anni diversi: l'anno compare in entrambe le date.
    #[test]
    fn dev_range_shows_both_years_across_new_year() {
        let d = |y, m, dd| local_to_days(&NaiveDate::from_ymd_opt(y, m, dd).unwrap());
        assert_eq!(dev_range(d(2026, 12, 14), d(2027, 1, 8), true), "Dec 14 '26 - Jan 8 '27");
        assert_eq!(dev_range(d(2026, 9, 21), d(2026, 10, 9), true), "Sep 21 - Oct 9 '26");
        assert_eq!(dev_range(d(2026, 12, 14), d(2027, 1, 8), false), "Dec 14 - Jan 8");
    }

    /// Etichetta del dev: la fine è il venerdì dell'ultima settimana di effort,
    /// quindi un effort in una sola settimana mostra «lunedì - venerdì».
    #[test]
    fn dev_label_ends_on_friday_of_last_week() {
        let monday = local_to_days(&NaiveDate::from_ymd_opt(2026, 9, 21).unwrap());
        let mut app = App::new();
        let pid = app.projects.add("Prog", Some("ABC"), Some(WeekId(monday as usize - 14)));
        app.projects
            .set_project_end_week(pid, Some(WeekId(monday as usize + 28)));
        let dev = app.devs.add("Frontend");
        app.projects.add_dev(pid, dev);
        app.projects
            .add_effort(pid, dev, WeekId(monday as usize), WorkerId(0), Effort(8));
        // Anno nelle date: è il default delle opzioni.
        let shapes = project_shapes(
            &app,
            pid,
            &project_name(&app, pid),
            &dev_info_map(&app),
            monday,
            "",
            Some(&[dev]),
            true,
            BarFormat::Continuous,
            &ExportOptions::default(),
        )
        .unwrap();
        let t = texts(&shapes);
        assert!(
            t.iter().any(|s| s == "Sep 21 - Sep 25 '26"),
            "attesa «Sep 21 - Sep 25 '26»: {t:?}"
        );
    }

    #[test]
    fn show_year_adds_short_year_to_dev_dates() {
        let mut app = App::new();
        let pid = app.projects.add("Prog", Some("ABC"), Some(WeekId(20000)));
        app.projects.set_project_end_week(pid, Some(WeekId(20070)));
        let dev = app.devs.add("Frontend");
        app.projects.add_dev(pid, dev);
        app.projects
            .add_effort(pid, dev, WeekId(20007), WorkerId(0), Effort(8));
        let page = |show_year: bool| {
            project_shapes(
                &app,
                pid,
                &project_name(&app, pid),
                &dev_info_map(&app),
                20030,
                "",
                Some(&[dev]),
                true,
                BarFormat::Continuous,
                &ExportOptions {
                    show_year,
                    ..Default::default()
                },
            )
            .unwrap()
        };

        let off = texts(&page(false));
        let on = texts(&page(true));
        let y = format!("'{:02}", day_to_date(20007).year() % 100);
        let dated: Vec<_> = on.iter().filter(|t| t.contains(" - ")).collect();
        assert_eq!(dated.len(), 1, "una sola etichetta date: {on:?}");
        assert_eq!(
            dated[0].matches(&y).count(),
            1,
            "stesso anno: anno abbreviato solo dopo la fine: {}",
            dated[0]
        );
        assert!(dated[0].ends_with(&y), "{}", dated[0]);
        assert!(!off.iter().any(|t| t.contains(&y)), "senza spunta niente anno");
        assert_eq!(on.len(), off.len(), "nessun altro testo cambia");
    }

    #[test]
    fn show_pct_adds_project_progress_next_to_today() {
        // Progetto che contiene "oggi" nell'asse, così il marker Today è disegnato.
        let base = today_days();
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

        let svg = build_svg_project(
            &app,
            pid,
            &[dev],
            BarFormat::Continuous,
            &ExportOptions::default(),
        )
        .unwrap();
        assert!(
            svg.contains("Today"),
            "oggi è nell'asse: Today deve comparire"
        );
        assert!(
            !svg.contains("(50%/70%)"),
            "senza flag niente % sotto Today"
        );

        let pct = ExportOptions {
            show_pct: true,
            ..Default::default()
        };
        let svg = build_svg_project(&app, pid, &[dev], BarFormat::Continuous, &pct).unwrap();
        assert!(svg.contains("Today"), "Today resta presente");
        assert!(
            svg.contains("(50%/70%)"),
            "col flag presunta/attuale tra parentesi sotto Today"
        );
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

    /// «Date sotto»: ogni tratto ha inizio (lunedì) e fine (venerdì) sotto di
    /// sé, l'intervallo complessivo a destra sparisce; con la barra continua il
    /// flag è ignorato.
    #[test]
    fn segment_dates_go_below_each_run() {
        let d = |m, dd| local_to_days(&NaiveDate::from_ymd_opt(2026, m, dd).unwrap());
        let mut app = App::new();
        let pid = app
            .projects
            .add("Prog", Some("ABC"), Some(WeekId(d(9, 7) as usize)));
        app.projects
            .set_project_end_week(pid, Some(WeekId(d(11, 30) as usize)));
        let dev = app.devs.add("Frontend");
        app.projects.add_dev(pid, dev);
        // Tratto 1: settimane del 21/9 e 28/9; tratto 2: settimana del 19/10.
        for (m, dd) in [(9, 21), (9, 28), (10, 19)] {
            app.projects
                .add_effort(pid, dev, WeekId(d(m, dd) as usize), WorkerId(0), Effort(8));
        }
        // Opzioni di default: anno nelle date sì, percentuali no.
        let page = |bar: DevBar| {
            texts(
                &project_shapes_fmt(
                    &app,
                    pid,
                    "Prog",
                    &dev_info_map(&app),
                    d(10, 1),
                    "",
                    Some(&[dev]),
                    true,
                    &|_| bar,
                    &ExportOptions::default(),
                )
                .unwrap(),
            )
        };
        for fmt in [BarFormat::Segmented, BarFormat::Proportional] {
            let t = page(DevBar::new(fmt, true));
            for want in ["Sep 21", "Oct 2 '26", "Oct 19", "Oct 23 '26"] {
                assert!(t.iter().any(|s| s == want), "manca «{want}»: {t:?}");
            }
            assert!(!t.iter().any(|s| s.contains(" - ")), "niente intervallo: {t:?}");
        }
        // Barra continua: il flag non ha effetto.
        let cont = page(DevBar::new(BarFormat::Continuous, true));
        assert!(cont.iter().any(|s| s == "Sep 21 - Oct 23 '26"), "{cont:?}");
    }

    /// Formato barra **per dev**: nella stessa pagina un dev segmentato e uno
    /// continuo; il conteggio delle forme piene cambia solo per il dev scelto.
    #[test]
    fn bar_format_is_chosen_per_dev() {
        let mut app = App::new();
        let pid = app.projects.add("Prog", Some("ABC"), Some(WeekId(20000)));
        app.projects.set_project_end_week(pid, Some(WeekId(20070)));
        let a = app.devs.add("Frontend");
        let b = app.devs.add("Backend");
        for d in [a, b] {
            app.projects.add_dev(pid, d);
            // Due tratti: 20007 e 20035 (buco in mezzo).
            app.projects
                .add_effort(pid, d, WeekId(20007), WorkerId(0), Effort(8));
            app.projects
                .add_effort(pid, d, WeekId(20035), WorkerId(0), Effort(8));
        }
        let fills = |devs: &[(DevId, DevBar)]| {
            let svg = build_svg_project_fmt(&app, pid, devs, &ExportOptions::default()).unwrap();
            svg.matches("<rect").count() + svg.matches("<polygon").count()
        };
        let (c, sg) = (
            DevBar::from(BarFormat::Continuous),
            DevBar::from(BarFormat::Segmented),
        );
        let base = fills(&[(a, c), (b, c)]);
        assert_eq!(fills(&[(a, sg), (b, c)]), base + 1, "solo A in 2 tratti");
        assert_eq!(fills(&[(a, c), (b, sg)]), base + 1, "solo B in 2 tratti");
        assert_eq!(fills(&[(a, sg), (b, sg)]), base + 2, "entrambi in 2 tratti");
        // Il wrapper col formato unico equivale a darlo a tutti i dev.
        assert_eq!(
            build_svg_project(
                &app,
                pid,
                &[a, b],
                BarFormat::Segmented,
                &ExportOptions::default()
            )
            .unwrap(),
            build_svg_project_fmt(&app, pid, &[(a, sg), (b, sg)], &ExportOptions::default())
                .unwrap()
        );
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
            let svg = build_svg_project(&app, pid, &[dev], fmt, &ExportOptions::default()).unwrap();
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
            let bytes = build_pdf_project(&app, pid, &[dev], fmt, &ExportOptions::default())
                .expect("PDF valido");
            assert!(bytes.starts_with(b"%PDF"));
            let svg = build_svg_project(&app, pid, &[dev], fmt, &ExportOptions::default())
                .expect("SVG valido");
            assert!(svg.contains("</svg>"));
        }
    }
}
