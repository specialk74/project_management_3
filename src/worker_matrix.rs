//! Matrice progetti × worker (Vista ▸ Matrice progetti/worker…): dati condivisi
//! dalla finestra (`ui::matrix`) e dall'export PDF (`pdf_export::build_matrix_pdf`).
//!
//! Colonne = worker selezionati, righe = progetti (tutti, aperti e chiusi) in cui
//! almeno un worker selezionato è **assegnato** (anche a effort 0) in una
//! settimana che si **sovrappone** all'intervallo `[from, to]` (giorni). Le righe
//! sono raggruppate per categoria; nella cella i dev con cui il worker è
//! assegnato nell'intervallo. In fondo, per worker, il numero di progetti per
//! categoria e, per ogni dev, ore e percentuale sul totale del worker.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use crate::app::App;
use crate::categories::CategoryId;
use crate::dev_utils::dev::DevId;
use crate::project_utils::project::{ProjectId, ProjectStatus};
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
    /// Stato del progetto: on-hold e closed sono entrambi `closed` (stessa
    /// resa), cambia solo l'etichetta.
    pub status: ProjectStatus,
    /// Per ogni worker (stesso ordine di `MatrixData.workers`): i nomi dei dev
    /// con cui è assegnato nell'intervallo, nell'ordine dei dev del progetto.
    pub cells: Vec<Vec<String>>,
    /// Ore di ogni dev della cella nell'intervallo, stesso ordine di `cells`
    /// (0 = solo assegnato a effort 0).
    pub hours: Vec<Vec<u32>>,
}

/// Parte fissa di una riga di cella, dopo il nome del dev: ` 120h 18%`, con la
/// percentuale sulle ore del worker nell'intervallo (`worker_total`, quelle di
/// `worker_hours`: le percentuali di una colonna sommano a 100%). Con 0 ore
/// (solo assegnato) solo ` 0h`. Il nome del dev si tronca (a video e nel PDF),
/// questa parte mai.
pub fn cell_hours_label(hours: u32, worker_total: u32) -> String {
    if hours == 0 {
        " 0h".to_string()
    } else {
        format!(" {hours}h {}", crate::worker_pies::pct_label(hours, worker_total))
    }
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
    /// `summary_status[c][w]` = come `summary`, divisi per stato nell'ordine di
    /// `ProjectStatus::ALL` (attivi, on-hold, closed).
    pub summary_status: Vec<Vec<[u32; 3]>>,
    /// `totals[w]` = progetti (di qualunque categoria, aperti e chiusi) in cui
    /// il worker `workers[w]` è presente.
    pub totals: Vec<u32>,
    /// `status_totals[w]` = progetti del worker `workers[w]` per stato, nell'ordine
    /// di `ProjectStatus::ALL` (attivi, on-hold, closed).
    pub status_totals: Vec<[u32; 3]>,
    /// Stati mostrati (indici in `ProjectStatus::ALL`, in ordine): solo per
    /// questi il riepilogo ha la sotto-riga. Tutti e tre senza filtro.
    pub statuses: Vec<usize>,
    /// Per ogni worker (stesso ordine di `workers`): ore per dev, sommate su
    /// tutti i progetti della matrice nelle settimane che si sovrappongono al
    /// periodo, in ordine di id del dev e solo i dev con ore > 0. Colora
    /// l'intestazione del worker (vedi `dev_bands`).
    pub dev_hours: Vec<Vec<(DevId, u32)>>,
    /// Tutti i dev del file (ordine di id): le righe «ore per dev» in fondo
    /// alla matrice, anche quelli senza ore per nessun worker.
    pub devs: Vec<(DevId, String)>,
}

/// Ore totali di un worker nel periodo (somma di `dev_hours[w]`).
pub fn worker_hours(hours: &[(DevId, u32)]) -> u32 {
    hours.iter().map(|(_, h)| h).sum()
}

/// Ore del worker nel dev e percentuale (arrotondata) sul suo totale; `None`
/// se nel dev non ha ore (cella vuota nelle righe «ore per dev»).
pub fn dev_share(hours: &[(DevId, u32)], dev: DevId) -> Option<(u32, u32)> {
    let total = worker_hours(hours);
    let h = hours.iter().find(|(d, _)| *d == dev).map_or(0, |(_, h)| *h);
    (h > 0 && total > 0).then(|| (h, (h * 100 + total / 2) / total))
}

/// Testo della cella «ore per dev»: `16h · 50%`. Condiviso da finestra e PDF.
pub fn dev_share_label(hours: u32, pct: u32) -> String {
    format!("{hours}h · {pct}%")
}

/// Bande dell'intestazione di un worker: `(dev, da, a)` con `da`/`a` frazioni
/// 0..1 della larghezza, proporzionali alle ore, nell'ordine dato. Vuoto se non
/// ci sono ore (il worker è solo assegnato a effort 0): l'intestazione è grigia.
/// Condiviso da finestra e PDF, così le proporzioni coincidono.
pub fn dev_bands(hours: &[(DevId, u32)]) -> Vec<(DevId, f32, f32)> {
    let total: u32 = hours.iter().map(|(_, h)| h).sum();
    if total == 0 {
        return Vec::new();
    }
    let mut acc = 0u32;
    hours
        .iter()
        .filter(|(_, h)| *h > 0)
        .map(|&(dev, h)| {
            let from = acc as f32 / total as f32;
            acc += h;
            (dev, from, acc as f32 / total as f32)
        })
        .collect()
}

/// La settimana che inizia il giorno `week_start` (lunedì, 7 giorni) si
/// sovrappone a `[from, to]` (estremi inclusi)?
pub fn week_overlaps(week_start: i32, from: i32, to: i32) -> bool {
    week_start <= to && week_start + 6 >= from
}

/// Costruisce la matrice per i worker `selected` (ordinati come `workers.list()`)
/// nell'intervallo di giorni `[from, to]`.
#[cfg(test)]
pub fn build(app: &App, selected: &HashSet<WorkerId>, from: i32, to: i32) -> MatrixData {
    build_filtered(app, selected, from, to, &BTreeSet::new(), &BTreeSet::new())
}

/// Come `build`, limitata ai progetti delle categorie in `cats` (`None` nel set
/// = «Senza categoria»; set vuoto = tutte). Riepilogo e totali contano solo i
/// progetti rimasti. Allo stesso modo `statuses` limita agli stati scelti
/// (vuoto = tutti): righe, riepilogo, totali e sotto-righe per stato.
pub fn build_filtered(
    app: &App,
    selected: &HashSet<WorkerId>,
    from: i32,
    to: i32,
    cats: &BTreeSet<Option<CategoryId>>,
    statuses: &BTreeSet<ProjectStatus>,
) -> MatrixData {
    let workers: Vec<(WorkerId, String)> = app
        .workers
        .list()
        .into_iter()
        .filter(|(id, _)| *id != WORKER_ID_ZERO && selected.contains(id))
        .collect();
    let dev_names: HashMap<_, _> = app.devs.list().into_iter().collect();

    let mut rows: Vec<MatrixRow> = Vec::new();
    // Ore per worker e dev sui progetti che entrano nella matrice.
    let mut hours: Vec<BTreeMap<DevId, u32>> = vec![BTreeMap::new(); workers.len()];
    for (proj, _) in app.projects.list() {
        if !cats.is_empty() && !cats.contains(&app.projects.get_category(proj)) {
            continue;
        }
        if !statuses.is_empty() && !statuses.contains(&app.projects.status(proj)) {
            continue;
        }
        let mut cells: Vec<Vec<String>> = vec![Vec::new(); workers.len()];
        // Dev di ogni cella, nell'ordine dei nomi in `cells` (per le ore).
        let mut cell_devs: Vec<Vec<DevId>> = vec![Vec::new(); workers.len()];
        let mut proj_hours: Vec<BTreeMap<DevId, u32>> = vec![BTreeMap::new(); workers.len()];
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
                    if let Some(se) = sew.worker_id.get(wid) {
                        let name = dev_names.get(&dev).cloned().unwrap_or_default();
                        if !cell_devs[wi].contains(&dev) {
                            cells[wi].push(name);
                            cell_devs[wi].push(dev);
                        }
                        *proj_hours[wi].entry(dev).or_default() += se.get_effort().0 as u32;
                    }
                }
            }
        }
        if cells.iter().all(|c| c.is_empty()) {
            continue;
        }
        let row_hours: Vec<Vec<u32>> = cell_devs
            .iter()
            .zip(&proj_hours)
            .map(|(devs, h)| devs.iter().map(|d| h.get(d).copied().unwrap_or(0)).collect())
            .collect();
        for (total, proj) in hours.iter_mut().zip(proj_hours) {
            for (dev, h) in proj {
                *total.entry(dev).or_default() += h;
            }
        }
        rows.push(MatrixRow {
            proj,
            tripletta: app.projects.get_tripletta(proj),
            name: app.projects.get_info(proj),
            category: app.projects.get_category(proj),
            closed: app.projects.is_closed(proj),
            status: app.projects.status(proj),
            cells,
            hours: row_hours,
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

    let summary_status = categories
        .iter()
        .map(|cat| {
            (0..workers.len())
                .map(|wi| {
                    ProjectStatus::ALL.map(|st| {
                        rows.iter()
                            .filter(|r| {
                                r.category == *cat && r.status == st && !r.cells[wi].is_empty()
                            })
                            .count() as u32
                    })
                })
                .collect()
        })
        .collect();
    let status_totals = (0..workers.len())
        .map(|wi| {
            ProjectStatus::ALL.map(|st| {
                rows.iter()
                    .filter(|r| r.status == st && !r.cells[wi].is_empty())
                    .count() as u32
            })
        })
        .collect();

    MatrixData {
        workers,
        rows,
        categories,
        summary,
        summary_status,
        totals,
        status_totals,
        statuses: (0..ProjectStatus::ALL.len())
            .filter(|&i| statuses.is_empty() || statuses.contains(&ProjectStatus::ALL[i]))
            .collect(),
        dev_hours: hours
            .into_iter()
            .map(|m| m.into_iter().filter(|(_, h)| *h > 0).collect())
            .collect(),
        devs: app.devs.list(),
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
    use crate::single_effort_utils::single_effort::Effort;

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

    /// On-hold: resta «chiuso» (stessa resa) ma ha il suo stato e il suo conteggio.
    #[test]
    fn status_totals_split_active_on_hold_closed() {
        let mut app = App::new();
        let mario = app.workers.add("Mario");
        let fe = app.devs.add("Frontend");
        let w = WeekId(20000);
        let ps: Vec<_> = (0..4)
            .map(|i| app.projects.add("x", Some(&format!("P{i}")), None))
            .collect();
        for p in &ps {
            app.projects.add_dev(*p, fe);
            app.projects.add_effort(*p, fe, w, mario, Effort(8));
        }
        app.projects.set_status(ps[1], ProjectStatus::OnHold);
        app.projects.set_status(ps[2], ProjectStatus::OnHold);
        app.projects.set_status(ps[3], ProjectStatus::Closed);
        assert!(app.projects.is_closed(ps[1]), "on-hold si comporta da chiuso");
        let sel: HashSet<_> = [mario].into_iter().collect();
        let m = build(&app, &sel, 20000, 20006);
        assert_eq!(m.totals, [4]);
        assert_eq!(m.status_totals, [[1, 2, 1]]);
        // Tutti senza categoria: un solo gruppo con la stessa suddivisione.
        assert_eq!(m.summary_status, [vec![[1, 2, 1]]]);
        let p1 = m.rows.iter().find(|r| r.proj == ps[1]).unwrap();
        assert!(p1.closed);
        assert_eq!(p1.status, ProjectStatus::OnHold);
        // Riaprire toglie l'on-hold; chiudere «semplice» anche.
        app.projects.set_status(ps[1], ProjectStatus::Active);
        assert_eq!(app.projects.status(ps[1]), ProjectStatus::Active);
        app.projects.set_status(ps[2], ProjectStatus::OnHold);
        app.projects.set_closed(ps[2], true);
        assert_eq!(app.projects.status(ps[2]), ProjectStatus::Closed);
    }

    /// Filtro «Stato»: solo i progetti negli stati scelti, in righe, riepilogo
    /// e totali; `statuses` elenca solo quelli (per le sotto-righe).
    #[test]
    fn status_filter_limits_rows_summary_and_sub_rows() {
        let mut app = App::new();
        let mario = app.workers.add("Mario");
        let fe = app.devs.add("Frontend");
        let w = WeekId(20000);
        let ps: Vec<_> = (0..4)
            .map(|i| app.projects.add("x", Some(&format!("P{i}")), None))
            .collect();
        for p in &ps {
            app.projects.add_dev(*p, fe);
            app.projects.add_effort(*p, fe, w, mario, Effort(8));
        }
        app.projects.set_status(ps[1], ProjectStatus::OnHold);
        app.projects.set_status(ps[2], ProjectStatus::OnHold);
        app.projects.set_status(ps[3], ProjectStatus::Closed);
        let sel: HashSet<_> = [mario].into_iter().collect();
        let none = BTreeSet::new();
        let all = build_filtered(&app, &sel, 20000, 20006, &none, &BTreeSet::new());
        assert_eq!(all.statuses, [0, 1, 2]);
        let pick: BTreeSet<_> = [ProjectStatus::Active, ProjectStatus::Closed].into();
        let m = build_filtered(&app, &sel, 20000, 20006, &none, &pick);
        let trip: Vec<_> = m.rows.iter().map(|r| r.tripletta.as_str()).collect();
        assert_eq!(trip, ["P0", "P3"]);
        assert_eq!(m.totals, [2]);
        assert_eq!(m.summary, [vec![2]]);
        assert_eq!(m.status_totals, [[1, 0, 1]]);
        assert_eq!(m.statuses, [0, 2]);
    }

    #[test]
    fn dev_bands_are_proportional_to_hours() {
        let (a, b) = (DevId(1), DevId(2));
        assert_eq!(dev_bands(&[(a, 8), (b, 8)]), [(a, 0.0, 0.5), (b, 0.5, 1.0)]);
        assert_eq!(dev_bands(&[(a, 30), (b, 10)]), [(a, 0.0, 0.75), (b, 0.75, 1.0)]);
        assert!(dev_bands(&[]).is_empty(), "nessuna ora: nessuna banda (grigio)");
        assert!(dev_bands(&[(a, 0)]).is_empty());
    }

    /// Righe «ore per dev»: ore nel dev e percentuale arrotondata sul totale
    /// del worker; dev senza ore (o worker senza ore) → cella vuota.
    #[test]
    fn dev_share_is_hours_and_rounded_percent_of_the_worker_total() {
        let (a, b, c) = (DevId(1), DevId(2), DevId(3));
        let h = [(a, 30), (b, 10)];
        assert_eq!(dev_share(&h, a), Some((30, 75)));
        assert_eq!(dev_share(&h, b), Some((10, 25)));
        assert_eq!(dev_share(&h, c), None);
        assert_eq!(dev_share(&[(a, 2), (b, 1)], a), Some((2, 67)));
        assert_eq!(dev_share(&[], a), None);
        assert_eq!(dev_share_label(30, 75), "30h · 75%");
    }

    /// Ore per dev del worker: sommate su tutti i progetti della matrice, solo
    /// nelle settimane del periodo e solo nei progetti che passano i filtri; un
    /// worker con sole assegnazioni a effort 0 non ha ore.
    #[test]
    fn dev_hours_sum_projects_in_the_period_and_filters() {
        let mut app = App::new();
        let mario = app.workers.add("Mario");
        let luigi = app.workers.add("Luigi");
        let fe = app.devs.add("Frontend");
        let be = app.devs.add("Backend");
        let p1 = app.projects.add("x", Some("P1"), None);
        let p2 = app.projects.add("x", Some("P2"), None);
        for p in [p1, p2] {
            app.projects.add_dev(p, fe);
            app.projects.add_dev(p, be);
        }
        app.projects.add_effort(p1, fe, WeekId(20000), mario, Effort(8));
        app.projects.add_effort(p2, be, WeekId(20000), mario, Effort(4));
        app.projects.add_effort(p2, be, WeekId(20007), mario, Effort(4));
        app.projects.add_effort(p2, fe, WeekId(20100), mario, Effort(99)); // fuori periodo
        app.projects.add_effort(p1, be, WeekId(20000), luigi, Effort(0));
        let sel: HashSet<_> = [mario, luigi].into_iter().collect();
        let none = BTreeSet::new();
        let m = build_filtered(&app, &sel, 20000, 20013, &none, &BTreeSet::new());
        let col = |w| m.workers.iter().position(|(id, _)| *id == w).unwrap();
        // 8 h Frontend (P1) + 8 h Backend (P2, due settimane): metà e metà
        assert_eq!(m.dev_hours[col(mario)], [(fe, 8), (be, 8)]);
        assert_eq!(dev_bands(&m.dev_hours[col(mario)]), [(fe, 0.0, 0.5), (be, 0.5, 1.0)]);
        // Luigi è assegnato ma a 0 ore: intestazione grigia
        assert!(m.dev_hours[col(luigi)].is_empty());
        assert_eq!(worker_hours(&m.dev_hours[col(mario)]), 16);
        assert_eq!(dev_share(&m.dev_hours[col(mario)], fe), Some((8, 50)));
        assert_eq!(dev_share(&m.dev_hours[col(luigi)], fe), None, "senza ore: vuota");
        assert_eq!(m.devs.len(), app.devs.list().len(), "tutti i dev");
        // Ore nelle celle: per progetto e dev, nell'intervallo; 0 = solo
        // assegnato. (P2 ha anche 99h di Frontend fuori periodo: non c'è.)
        let row = |t: &str| m.rows.iter().find(|r| r.tripletta == t).unwrap();
        assert_eq!(row("P1").cells[col(mario)], ["Frontend"]);
        assert_eq!(row("P1").hours[col(mario)], [8]);
        assert_eq!(row("P2").cells[col(mario)], ["Backend"]);
        assert_eq!(row("P2").hours[col(mario)], [8], "due settimane da 4h");
        assert_eq!(row("P1").hours[col(luigi)], [0], "assegnato a effort 0");
        // Percentuale sulle ore di Mario nell'intervallo (16h): metà e metà.
        assert_eq!(cell_hours_label(8, 16), " 8h 50%");
        assert_eq!(cell_hours_label(0, 16), " 0h", "solo assegnato: niente %");
        assert_eq!(cell_hours_label(1, 300), " 1h <1%");
        // Filtro categoria che esclude P2: restano solo le ore di P1
        let cat = app.categories.add("Cat");
        app.projects.set_category(p1, Some(cat));
        let only: BTreeSet<_> = [Some(cat)].into();
        let m = build_filtered(&app, &sel, 20000, 20013, &only, &BTreeSet::new());
        assert_eq!(m.dev_hours[col(mario)], [(fe, 8)]);
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
        let all = build_filtered(&app, &sel, 20000, 20006, &BTreeSet::new(), &BTreeSet::new());
        assert_eq!(all.rows.len(), 2);
        assert_eq!(all.totals, [2]);
        let only_a: BTreeSet<_> = [Some(cat_a)].into();
        let m = build_filtered(&app, &sel, 20000, 20006, &only_a, &BTreeSet::new());
        let trip: Vec<_> = m.rows.iter().map(|r| r.tripletta.as_str()).collect();
        assert_eq!(trip, ["P1"]);
        assert_eq!(m.totals, [1]);
        let none: BTreeSet<_> = [None].into();
        let m = build_filtered(&app, &sel, 20000, 20006, &none, &BTreeSet::new());
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
        // Ore di ogni dev della cella, allineate ai nomi.
        assert_eq!(m.rows[1].hours[wi_mario].len(), 2);
        assert_eq!(m.rows[0].hours[wi_anna].len(), 1);
        assert_eq!(m.summary[0][wi_anna], 1);
        assert_eq!(m.summary[0][wi_mario], 0);
        assert_eq!(m.summary[1][wi_mario], 1);
        assert_eq!(m.totals[wi_mario], 1);
        assert_eq!(m.totals[wi_anna], 1);
        assert_eq!(m.status_totals[wi_mario], [1, 0, 0]);
    }
}
