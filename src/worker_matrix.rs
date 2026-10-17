//! Matrice progetti × worker (Vista ▸ Matrice progetti/worker…): dati condivisi
//! dalla finestra (`ui::matrix`) e dall'export PDF (`pdf_export::build_matrix_pdf`).
//!
//! Colonne = worker selezionati, righe = progetti (tutti, aperti e chiusi) in cui
//! almeno un worker selezionato è **assegnato** (anche a effort 0) in una
//! settimana che si **sovrappone** all'intervallo `[from, to]` (giorni). Le righe
//! sono raggruppate per categoria; nella cella i dev con cui il worker è
//! assegnato nell'intervallo. In fondo, per worker, il numero di progetti per
//! categoria.

use std::collections::{BTreeSet, HashMap, HashSet};

use crate::app::App;
use crate::categories::CategoryId;
use crate::project_utils::project::ProjectId;
use crate::workers_utils::worker::{WORKER_ID_ZERO, WorkerId};

/// Una riga della matrice (un progetto).
pub struct MatrixRow {
    pub proj: ProjectId,
    pub tripletta: String,
    pub name: String,
    pub category: Option<CategoryId>,
    /// Progetto chiuso: resta nel gruppo della sua categoria ma la riga è
    /// disegnata grigia a strisce.
    pub closed: bool,
    /// Per ogni worker (stesso ordine di `MatrixData.workers`): i nomi dei dev
    /// con cui è assegnato nell'intervallo, nell'ordine dei dev del progetto.
    pub cells: Vec<Vec<String>>,
}

pub struct MatrixData {
    pub workers: Vec<(WorkerId, String)>,
    /// Righe già raggruppate per categoria (ordine di `categories`), e dentro
    /// il gruppo nell'ordine di visualizzazione dei progetti.
    pub rows: Vec<MatrixRow>,
    /// Categorie presenti nelle righe, nell'ordine dei gruppi: per id, con
    /// «Senza categoria» (`None`) in fondo.
    pub categories: Vec<Option<CategoryId>>,
    /// `summary[c][w]` = progetti della categoria `categories[c]` in cui il
    /// worker `workers[w]` è presente.
    pub summary: Vec<Vec<u32>>,
    /// `totals[w]` = progetti (di qualunque categoria, aperti e chiusi) in cui
    /// il worker `workers[w]` è presente.
    pub totals: Vec<u32>,
}

/// La settimana che inizia il giorno `week_start` (lunedì, 7 giorni) si
/// sovrappone a `[from, to]` (estremi inclusi)?
pub fn week_overlaps(week_start: i32, from: i32, to: i32) -> bool {
    week_start <= to && week_start + 6 >= from
}

/// Costruisce la matrice per i worker `selected` (ordinati come `workers.list()`)
/// nell'intervallo di giorni `[from, to]`.
pub fn build(app: &App, selected: &HashSet<WorkerId>, from: i32, to: i32) -> MatrixData {
    build_filtered(app, selected, from, to, &BTreeSet::new())
}

/// Come `build`, limitata ai progetti delle categorie in `cats` (`None` nel set
/// = «Senza categoria»; set vuoto = tutte). Riepilogo e totali contano solo i
/// progetti rimasti.
pub fn build_filtered(
    app: &App,
    selected: &HashSet<WorkerId>,
    from: i32,
    to: i32,
    cats: &BTreeSet<Option<CategoryId>>,
) -> MatrixData {
    let workers: Vec<(WorkerId, String)> = app
        .workers
        .list()
        .into_iter()
        .filter(|(id, _)| *id != WORKER_ID_ZERO && selected.contains(id))
        .collect();
    let dev_names: HashMap<_, _> = app.devs.list().into_iter().collect();

    let mut rows: Vec<MatrixRow> = Vec::new();
    for (proj, _) in app.projects.list() {
        if !cats.is_empty() && !cats.contains(&app.projects.get_category(proj)) {
            continue;
        }
        let mut cells: Vec<Vec<String>> = vec![Vec::new(); workers.len()];
        for dev in app.projects.list_devs(proj) {
            let Some(sd) = app.projects.get_single_dev(proj, dev) else {
                continue;
            };
            for wk in sd.get_weeks() {
                if !week_overlaps(wk.0 as i32, from, to) {
                    continue;
                }
                let Some(sew) = sd.get_all(wk) else { continue };
                for (wi, (wid, _)) in workers.iter().enumerate() {
                    if sew.worker_id.contains_key(wid) {
                        let name = dev_names.get(&dev).cloned().unwrap_or_default();
                        if !cells[wi].contains(&name) {
                            cells[wi].push(name);
                        }
                    }
                }
            }
        }
        if cells.iter().all(|c| c.is_empty()) {
            continue;
        }
        rows.push(MatrixRow {
            proj,
            tripletta: app.projects.get_tripletta(proj),
            name: app.projects.get_info(proj),
            category: app.projects.get_category(proj),
            closed: app.projects.is_closed(proj),
            cells,
        });
    }

    // Raggruppamento: categorie per id, «Senza categoria» in fondo. Il sort è
    // stabile, quindi dentro il gruppo resta l'ordine dei progetti.
    let group_key = |c: Option<CategoryId>| (c.is_none(), c);
    rows.sort_by_key(|r| group_key(r.category));
    let mut categories: Vec<Option<CategoryId>> = Vec::new();
    for r in &rows {
        if categories.last() != Some(&r.category) {
            categories.push(r.category);
        }
    }
    let summary = categories
        .iter()
        .map(|cat| {
            (0..workers.len())
                .map(|wi| {
                    rows.iter()
                        .filter(|r| r.category == *cat && !r.cells[wi].is_empty())
                        .count() as u32
                })
                .collect()
        })
        .collect();
    let totals = (0..workers.len())
        .map(|wi| rows.iter().filter(|r| !r.cells[wi].is_empty()).count() as u32)
        .collect();

    MatrixData {
        workers,
        rows,
        categories,
        summary,
        totals,
    }
}

/// Colore (0xRRGGBB, tenue: il testo sopra è nero) del gruppo di una categoria:
/// palette fissa indicizzata dall'id, grigio per «Senza categoria».
pub fn category_color(cat: Option<CategoryId>) -> u32 {
    const PALETTE: [u32; 10] = [
        0xBBDEFB, // blu
        0xC8E6C9, // verde
        0xFFE0B2, // arancio
        0xE1BEE7, // viola
        0xFFF59D, // giallo
        0xB2EBF2, // ciano
        0xF8BBD0, // rosa
        0xDCEDC8, // lime
        0xD7CCC8, // marrone
        0xFFCDD2, // rosso
    ];
    match cat {
        Some(CategoryId(id)) => PALETTE[id % PALETTE.len()],
        None => 0xE0E0E0,
    }
}

/// Colori (0xRRGGBB) della riga di un progetto **chiuso**: la tinta schiarita
/// della categoria spinta in parte (~16%) verso il grigio (si riconosce il
/// gruppo) e, più scura, quella delle strisce diagonali.
pub fn closed_colors(cat: Option<CategoryId>) -> (u32, u32) {
    let c = category_color(cat);
    let ch = |shift: u32, k: f32| {
        let v = ((c >> shift) & 0xFF) as f32;
        let light = 255.0 - (255.0 - v) * 0.45;
        // ~16% di grigio (50% ridotto quattro volte del 25%).
        let grayish = light * 0.842 + 200.0 * 0.158;
        ((grayish * k) as u32).min(255) << shift
    };
    let fill = ch(16, 1.0) | ch(8, 1.0) | ch(0, 1.0);
    let hatch = ch(16, 0.88) | ch(8, 0.88) | ch(0, 0.88);
    (fill, hatch)
}

/// Segmenti del tratteggio diagonale (45°, passo `step`) dentro il rettangolo
/// `[x0, x1] × [top, bot]`, per la riga di un progetto chiuso. `bot` può stare
/// sopra o sotto `top` (coordinate schermo o PDF): le linee scendono verso
/// sinistra da `top` a `bot`.
pub fn hatch_segments(x0: f32, x1: f32, top: f32, bot: f32, step: f32) -> Vec<(f32, f32, f32, f32)> {
    let h = (bot - top).abs();
    let dir = (bot - top).signum();
    let mut out = Vec::new();
    let mut xs = x0 + step;
    while xs < x1 + h {
        // Punto alto (xs, top) e basso (xs - h, bot), ritagliati su [x0, x1].
        let (mut ax, mut ay) = (xs, top);
        if ax > x1 {
            ay += (ax - x1) * dir;
            ax = x1;
        }
        let (mut bx, mut by) = (xs - h, bot);
        if bx < x0 {
            by -= (x0 - bx) * dir;
            bx = x0;
        }
        if ax > bx {
            out.push((ax, ay, bx, by));
        }
        xs += step;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::single_dev_utils::single_dev::WeekId;
    use crate::single_effort_utils::sinlge_effort::Effort;

    #[test]
    fn hatch_stays_inside_the_rect() {
        for (top, bot) in [(10.0, 30.0), (30.0, 10.0)] {
            let segs = hatch_segments(0.0, 100.0, top, bot, 7.0);
            assert!(!segs.is_empty());
            let (lo, hi) = (top.min(bot), top.max(bot));
            for (ax, ay, bx, by) in segs {
                for (x, y) in [(ax, ay), (bx, by)] {
                    assert!((0.0..=100.0).contains(&x) && (lo - 1e-3..=hi + 1e-3).contains(&y));
                }
                // 45°: |dx| == |dy|
                assert!(((ax - bx).abs() - (ay - by).abs()).abs() < 1e-3);
            }
        }
    }

    #[test]
    fn category_filter_limits_rows_and_totals() {
        let mut app = App::new();
        let mario = app.workers.add("Mario");
        let fe = app.devs.add("Frontend");
        let cat_a = app.categories.add("A");
        let w = WeekId(20000);
        let p1 = app.projects.add("Uno", Some("P1"), None);
        let p2 = app.projects.add("Due", Some("P2"), None);
        app.projects.set_category(p1, Some(cat_a));
        for p in [p1, p2] {
            app.projects.add_dev(p, fe);
            app.projects.add_effort(p, fe, w, mario, Effort(8));
        }
        let sel: HashSet<_> = [mario].into_iter().collect();
        let all = build_filtered(&app, &sel, 20000, 20006, &BTreeSet::new());
        assert_eq!(all.rows.len(), 2);
        assert_eq!(all.totals, [2]);
        let only_a: BTreeSet<_> = [Some(cat_a)].into();
        let m = build_filtered(&app, &sel, 20000, 20006, &only_a);
        let trip: Vec<_> = m.rows.iter().map(|r| r.tripletta.as_str()).collect();
        assert_eq!(trip, ["P1"]);
        assert_eq!(m.totals, [1]);
        let none: BTreeSet<_> = [None].into();
        let m = build_filtered(&app, &sel, 20000, 20006, &none);
        assert_eq!(m.rows.len(), 1);
        assert_eq!(m.rows[0].tripletta, "P2");
    }

    #[test]
    fn week_overlap_counts_partial_weeks() {
        // settimana 100..=106
        assert!(week_overlaps(100, 106, 120)); // tocca solo la domenica
        assert!(week_overlaps(100, 90, 100)); // tocca solo il lunedì
        assert!(!week_overlaps(100, 107, 120));
        assert!(!week_overlaps(100, 80, 99));
    }

    #[test]
    fn matrix_groups_by_category_and_counts() {
        let mut app = App::new();
        let mario = app.workers.add("Mario");
        let anna = app.workers.add("Anna");
        let fe = app.devs.add("Frontend");
        let qa = app.devs.add("QA");
        let cat_a = app.categories.add("A");
        let w = WeekId(20000);

        let p1 = app.projects.add("Uno", Some("P1"), None);
        let p2 = app.projects.add("Due", Some("P2"), None);
        let p3 = app.projects.add("Tre", Some("P3"), None);
        let p4 = app.projects.add("Quattro", Some("P4"), None);
        app.projects.set_category(p2, Some(cat_a));
        for p in [p1, p2, p3, p4] {
            app.projects.add_dev(p, fe);
            app.projects.add_dev(p, qa);
        }
        // P1 (senza categoria): Mario su Frontend e QA, a effort 0 su QA.
        app.projects.add_effort(p1, fe, w, mario, Effort(8));
        app.projects.add_effort(p1, qa, w, mario, Effort(0));
        // P2 (cat A): Anna.
        app.projects.add_effort(p2, qa, w, anna, Effort(4));
        // P3: Mario ma fuori intervallo → escluso.
        app.projects
            .add_effort(p3, fe, WeekId(20000 + 70), mario, Effort(8));
        // P4: nessuno.

        let sel: HashSet<_> = [mario, anna].into_iter().collect();
        let m = build(&app, &sel, 20003, 20010);
        let trip: Vec<_> = m.rows.iter().map(|r| r.tripletta.as_str()).collect();
        // Gruppo A prima, «Senza categoria» in fondo.
        assert_eq!(trip, ["P2", "P1"]);
        assert!(m.categories == [Some(cat_a), None]);
        let wi_mario = m.workers.iter().position(|(id, _)| *id == mario).unwrap();
        let wi_anna = m.workers.iter().position(|(id, _)| *id == anna).unwrap();
        assert_eq!(m.rows[1].cells[wi_mario], ["Frontend", "QA"]);
        assert_eq!(m.rows[0].cells[wi_anna], ["QA"]);
        assert_eq!(m.summary[0][wi_anna], 1);
        assert_eq!(m.summary[0][wi_mario], 0);
        assert_eq!(m.summary[1][wi_mario], 1);
        assert_eq!(m.totals[wi_mario], 1);
        assert_eq!(m.totals[wi_anna], 1);
    }
}
