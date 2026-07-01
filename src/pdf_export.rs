//! Esportazione PDF: una pagina per progetto (solo progetti visibili con
//! inizio e fine). Ogni pagina mostra tripletta, descrizione e una timeline
//! orizzontale con bandierine (inizio, fine, milestone) posizionate in scala
//! sulle rispettive date.

use printpdf::*;

use crate::app::App;
use crate::date_utils::dates::days_to_local;

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
const MARGIN: f32 = 20.0;
const X_LEFT: f32 = MARGIN;
const X_RIGHT: f32 = PAGE_W - MARGIN;

// Barra timeline.
const BAR_TOP: f32 = 110.0;
const BAR_BOTTOM: f32 = 100.0;

const BLACK: (f32, f32, f32) = (0.0, 0.0, 0.0);
const BAR_GRAY: (f32, f32, f32) = (0.85, 0.85, 0.85);
const START_BLUE: (f32, f32, f32) = (70.0 / 255.0, 130.0 / 255.0, 180.0 / 255.0);
const END_GREEN: (f32, f32, f32) = (0.0, 128.0 / 255.0, 0.0);

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

fn date_of(week: crate::single_dev_utils::single_dev::WeekId) -> String {
    days_to_local(week.0 as i32).format("%Y-%m-%d").to_string()
}

/// Larghezza approssimata di un testo Helvetica in mm (media 0.5·size per carattere).
fn text_w_mm(text: &str, size_pt: f32) -> f32 {
    text.chars().count() as f32 * size_pt * 0.5 * 0.352_778
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

// Una bandierina sulla timeline.
struct Flag {
    days: i32,
    title: String,
    date: String,
    color: (f32, f32, f32),
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

/// Come `wrap` ma preserva gli a-capo espliciti (`\n`): ogni riga del testo
/// diventa un paragrafo a sé, poi mandato a capo automaticamente. Le righe
/// vuote restano come righe vuote (a-capo visibile). Gestisce anche `\r\n`.
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

/// Genera gli `Op` di una pagina per un progetto. `flags` già ordinate per data.
fn page_ops(
    fonts: &Fonts,
    tripletta: &str,
    descr: &str,
    start: i32,
    end: i32,
    mut flags: Vec<Flag>,
    created: &str,
) -> Vec<Op> {
    let mut ops = Vec::new();

    // Piè di pagina: data di creazione del PDF, centrata in basso.
    ops.extend(text_center(
        fonts,
        PAGE_W / 2.0,
        8.0,
        &format!("Creato il {created}"),
        8.0,
        false,
        (0.4, 0.4, 0.4),
    ));

    // Tripletta (in alto, grande) e descrizione (a capo automatico).
    if !tripletta.is_empty() {
        ops.extend(text_left(fonts, MARGIN, 190.0, tripletta, 22.0, true, BLACK));
    }
    // Descrizione: preserva gli a-capo del testo del progetto. Limitata alle
    // righe che stanno sopra la zona della timeline (per non sovrapporsi).
    let mut y = 176.0;
    for l in wrap_multiline(descr, 95).into_iter().take(6) {
        ops.extend(text_left(fonts, MARGIN, y, &l, 13.0, false, BLACK));
        y -= 6.0;
    }

    // Barra timeline.
    ops.extend(rect_fill(X_LEFT, BAR_BOTTOM, X_RIGHT, BAR_TOP, BAR_GRAY));

    // Mappa data → x sulla barra (clamp agli estremi).
    let span = (end - start).max(1) as f32;
    let x_of = |days: i32| -> f32 {
        let t = (days - start) as f32 / span;
        (X_LEFT + t * (X_RIGHT - X_LEFT)).clamp(X_LEFT, X_RIGHT)
    };

    flags.sort_by_key(|f| f.days);
    for (i, f) in flags.iter().enumerate() {
        let x = x_of(f.days);
        // Stagger per righe pari/dispari, così bandierine vicine non si sovrappongono.
        let pole_top = if i % 2 == 0 { 134.0 } else { 126.0 };
        let (ty_title, ty_date) = if i % 2 == 0 { (96.0, 91.5) } else { (85.0, 80.5) };

        // Asta + pennant (triangolino a destra) colorati.
        ops.extend(line(x, BAR_TOP, x, pole_top, 1.2, f.color));
        let pennant = [(x, pole_top), (x + 8.0, pole_top - 2.5), (x, pole_top - 5.0)];
        ops.push(Op::SetFillColor { col: rgb(f.color) });
        ops.push(Op::DrawPolygon {
            polygon: Polygon {
                rings: vec![PolygonRing {
                    points: pennant
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
        });

        // Etichette sotto la barra: titolo + data, centrate sull'asta.
        ops.extend(text_center(fonts, x, ty_title, &f.title, 9.0, true, BLACK));
        ops.extend(text_center(fonts, x, ty_date, &f.date, 8.0, false, BLACK));
    }

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

    // Data/ora di creazione (locale), uguale su tutte le pagine di questo PDF.
    let created = chrono::Local::now().format("%Y-%m-%d %H:%M").to_string();

    let mut pages = Vec::new();

    for (id, name, enable) in app.projects.list_full() {
        // solo progetti visibili
        if !enable.0 || app.projects.is_closed(id) {
            continue;
        }
        // servono inizio E fine
        let (Some(start_w), Some(end_w)) = (
            app.projects.get_project_start_week(id),
            app.projects.get_project_end_week(id),
        ) else {
            continue;
        };
        let start = start_w.0 as i32;
        let end = end_w.0 as i32;

        let mut flags = vec![
            Flag {
                days: start,
                title: "Inizio".to_string(),
                date: date_of(start_w),
                color: START_BLUE,
            },
            Flag {
                days: end,
                title: "Fine".to_string(),
                date: date_of(end_w),
                color: END_GREEN,
            },
        ];
        for (mid, week) in app.projects.list_project_milestones(id) {
            let mname = app.milestones.get_name(mid).unwrap_or("?").to_string();
            let mcol = app.milestones.get_color(mid).map(u32_rgb).unwrap_or(BLACK);
            flags.push(Flag {
                days: week.0 as i32,
                title: mname,
                date: date_of(week),
                color: mcol,
            });
        }

        let tripletta = app.projects.get_tripletta(id);
        let ops = page_ops(&fonts, &tripletta, &name, start, end, flags, &created);
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
