//! Torte dev/categorie per worker (Vista ▸ Torte dev/categorie per worker…):
//! dati e geometria condivisi dalla finestra (`ui::pies`) e dall'export PDF/SVG
//! (`pdf_export::build_pies_pdf` / `build_pies_svgs`).
//!
//! Per ogni worker scelto si sommano le **ore** (le assegnazioni a effort 0 non
//! contano) di tutti i progetti, aperti e chiusi, nelle settimane che si
//! sovrappongono all'intervallo `[from, to]` (giorni), come la matrice:
//! - prima torta: anello interno = dev, anello esterno = per ogni dev le sue
//!   categorie (allineate sopra lo spicchio del dev);
//! - seconda torta: categorie.
//!
//! Le frazioni sono tutte sul totale del worker. Angoli: 0 in alto (ore 12),
//! senso orario.

use std::collections::{BTreeMap, BTreeSet, HashSet};

use crate::app::App;
use crate::categories::CategoryId;
use crate::dev_utils::dev::DevId;
use crate::worker_matrix::week_overlaps;
use crate::workers_utils::worker::{WORKER_ID_ZERO, WorkerId};

/// Ore di un dev del worker, divise per categoria.
pub struct DevHours {
    pub dev: DevId,
    pub hours: u32,
    /// Ordine delle categorie: per id, «Senza categoria» in fondo; solo > 0.
    pub categories: Vec<(Option<CategoryId>, u32)>,
}

/// Dati delle due torte di un worker.
pub struct WorkerPie {
    pub worker: WorkerId,
    pub name: String,
    /// Ore totali nel periodo (0 = niente torte).
    pub total: u32,
    /// Dev in ordine di id, solo quelli con ore.
    pub devs: Vec<DevHours>,
    /// Categorie (stesso ordine di `DevHours::categories`), solo > 0.
    pub categories: Vec<(Option<CategoryId>, u32)>,
}

/// Una fetta: `from`/`to` frazioni 0..1 del giro, sul totale del worker.
#[derive(Clone, Debug, PartialEq)]
pub struct Seg<K> {
    pub key: K,
    pub hours: u32,
    pub from: f32,
    pub to: f32,
}

/// Percentuale arrotondata di `h` su `total` (0 se il totale è 0).
pub fn pct_of(h: u32, total: u32) -> u32 {
    (h * 100 + total / 2).checked_div(total).unwrap_or(0)
}

/// Percentuale da stampare: `25%`, oppure `<1%` se ci sono ore ma
/// l'arrotondamento darebbe 0 (uno «0%» farebbe pensare a nessuna ora).
pub fn pct_label(h: u32, total: u32) -> String {
    match pct_of(h, total) {
        0 if h > 0 => "<1%".to_string(),
        p => format!("{p}%"),
    }
}

/// Fette consecutive per i valori dati, sul totale `total`, a partire da `start`.
fn segs<K: Clone>(items: &[(K, u32)], total: u32, start: u32) -> Vec<Seg<K>> {
    let mut acc = start;
    items
        .iter()
        .filter(|(_, h)| *h > 0 && total > 0)
        .map(|(k, h)| {
            let from = acc as f32 / total as f32;
            acc += h;
            Seg {
                key: k.clone(),
                hours: *h,
                from,
                to: acc as f32 / total as f32,
            }
        })
        .collect()
}

impl WorkerPie {
    /// Anello interno della prima torta: un fetta per dev.
    pub fn dev_ring(&self) -> Vec<Seg<DevId>> {
        let items: Vec<(DevId, u32)> = self.devs.iter().map(|d| (d.dev, d.hours)).collect();
        segs(&items, self.total, 0)
    }

    /// Anello esterno della prima torta: le categorie di ogni dev, nello
    /// spicchio del dev.
    pub fn dev_category_ring(&self) -> Vec<Seg<(DevId, Option<CategoryId>)>> {
        let mut out = Vec::new();
        let mut start = 0;
        for d in &self.devs {
            let items: Vec<_> = d.categories.iter().map(|&(c, h)| ((d.dev, c), h)).collect();
            out.extend(segs(&items, self.total, start));
            start += d.hours;
        }
        out
    }

    /// Seconda torta: una fetta per categoria.
    pub fn category_ring(&self) -> Vec<Seg<Option<CategoryId>>> {
        segs(&self.categories, self.total, 0)
    }
}

/// Torte dei worker `selected` (ordinati come `workers.list()`), anche quelli
/// senza ore nel periodo (`total` 0). `cats` limita ai progetti di quelle
/// categorie (`None` nel set = «Senza categoria»; vuoto = tutte), come la
/// combo della matrice: totale e percentuali contano solo quei progetti.
pub fn build(
    app: &App,
    selected: &HashSet<WorkerId>,
    from: i32,
    to: i32,
    cats: &BTreeSet<Option<CategoryId>>,
) -> Vec<WorkerPie> {
    let workers: Vec<(WorkerId, String)> = app
        .workers
        .list()
        .into_iter()
        .filter(|(id, _)| *id != WORKER_ID_ZERO && selected.contains(id))
        .collect();
    // hours[w][dev][categoria]; la chiave di categoria mette «Senza» in fondo.
    type CatKey = (bool, Option<CategoryId>);
    let mut hours: Vec<BTreeMap<DevId, BTreeMap<CatKey, u32>>> =
        vec![BTreeMap::new(); workers.len()];
    for (proj, _) in app.projects.list() {
        let cat = app.projects.get_category(proj);
        if !cats.is_empty() && !cats.contains(&cat) {
            continue;
        }
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
                    let h = sew.worker_id.get(wid).map_or(0, |se| se.get_effort().0 as u32);
                    if h > 0 {
                        *hours[wi]
                            .entry(dev)
                            .or_default()
                            .entry((cat.is_none(), cat))
                            .or_default() += h;
                    }
                }
            }
        }
    }
    workers
        .into_iter()
        .zip(hours)
        .map(|((worker, name), by_dev)| {
            let mut cats: BTreeMap<CatKey, u32> = BTreeMap::new();
            let devs: Vec<DevHours> = by_dev
                .into_iter()
                .map(|(dev, by_cat)| {
                    for (k, h) in &by_cat {
                        *cats.entry(*k).or_default() += h;
                    }
                    DevHours {
                        dev,
                        hours: by_cat.values().sum(),
                        categories: by_cat.into_iter().map(|((_, c), h)| (c, h)).collect(),
                    }
                })
                .collect();
            WorkerPie {
                worker,
                name,
                total: devs.iter().map(|d| d.hours).sum(),
                devs,
                categories: cats.into_iter().map(|((_, c), h)| (c, h)).collect(),
            }
        })
        .collect()
}

/// Punto a raggio `r` e frazione di giro `frac` (0 in alto, senso orario) dal
/// centro `(cx, cy)`. `y_up`: asse verticale verso l'alto (PDF) o verso il
/// basso (schermo).
pub fn arc_point(cx: f32, cy: f32, r: f32, frac: f32, y_up: bool) -> (f32, f32) {
    let a = frac * std::f32::consts::TAU;
    let dy = r * a.cos();
    (cx + r * a.sin(), if y_up { cy + dy } else { cy - dy })
}

/// Punti dell'arco a raggio `r` da `from` a `to` (estremi compresi), con un
/// passo abbastanza fitto da sembrare curvo.
pub fn arc(cx: f32, cy: f32, r: f32, from: f32, to: f32, y_up: bool) -> Vec<(f32, f32)> {
    let steps = (((to - from) * 128.0).ceil() as usize).max(2);
    (0..=steps)
        .map(|i| arc_point(cx, cy, r, from + (to - from) * i as f32 / steps as f32, y_up))
        .collect()
}

/// Contorno di una fetta fra i raggi `r0` (0 = spicchio fino al centro) e
/// `r1`: arco esterno in avanti, poi quello interno all'indietro (o il centro).
pub fn sector(cx: f32, cy: f32, r0: f32, r1: f32, from: f32, to: f32, y_up: bool) -> Vec<(f32, f32)> {
    let mut pts = arc(cx, cy, r1, from, to, y_up);
    if r0 <= 0.0 {
        pts.push((cx, cy));
    } else {
        pts.extend(arc(cx, cy, r0, from, to, y_up).into_iter().rev());
    }
    pts
}

/// Centro dell'etichetta della fetta, se un testo largo `w` e alto `h` ci sta
/// dentro (con un piccolo margine); `None` altrimenti. Sullo spicchio pieno
/// (`r0` = 0) l'etichetta sta a circa 2/3 del raggio.
#[allow(clippy::too_many_arguments)]
pub fn label_pos(
    cx: f32,
    cy: f32,
    r0: f32,
    r1: f32,
    seg_from: f32,
    seg_to: f32,
    w: f32,
    h: f32,
    y_up: bool,
) -> Option<(f32, f32)> {
    let frac = seg_to - seg_from;
    let rm = if r0 <= 0.0 { r1 * 0.62 } else { (r0 + r1) / 2.0 };
    // Spazio trasversale: la corda alla distanza dell'etichetta.
    let across = if frac >= 0.5 {
        2.0 * rm
    } else {
        2.0 * rm * (std::f32::consts::PI * frac).sin()
    };
    // Spazio radiale: lo spessore dell'anello (sullo spicchio, il raggio
    // utile attorno all'etichetta).
    let thick = if r0 <= 0.0 { r1 * 0.7 } else { r1 - r0 };
    let pad = h * 0.3;
    // Testo orizzontale in una fetta orientata in modo qualsiasi: si chiede
    // spazio per la diagonale del riquadro, in entrambe le direzioni.
    let need = (w * w + h * h).sqrt() + pad;
    (across >= need && thick >= h + pad || frac >= 0.999 && thick >= h + pad && 2.0 * rm >= need)
        .then(|| arc_point(cx, cy, rm, (seg_from + seg_to) / 2.0, y_up))
}

/// Fetta sotto il punto `(x, y)`: indice in `segs` se il punto è nell'anello
/// `[r0, r1]` (schermo, asse verso il basso).
pub fn hit<K>(cx: f32, cy: f32, r0: f32, r1: f32, segs: &[Seg<K>], x: f32, y: f32) -> Option<usize> {
    let (dx, dy) = (x - cx, y - cy);
    let d = (dx * dx + dy * dy).sqrt();
    if d < r0 || d > r1 {
        return None;
    }
    let frac = dx.atan2(-dy).rem_euclid(std::f32::consts::TAU) / std::f32::consts::TAU;
    segs.iter().position(|s| frac >= s.from && frac < s.to)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::single_dev_utils::single_dev::WeekId;
    use crate::single_effort_utils::single_effort::Effort;

    /// Ore per dev e categoria del worker nel periodo; l'anello esterno sta
    /// dentro lo spicchio del suo dev; «Senza categoria» in fondo; effort 0 e
    /// settimane fuori periodo non contano.
    #[test]
    fn pies_sum_hours_by_dev_and_category() {
        let mut app = App::new();
        let mario = app.workers.add("Mario");
        let luigi = app.workers.add("Luigi");
        let fe = app.devs.add("Frontend");
        let be = app.devs.add("Backend");
        let ca = app.categories.add("Cliente A");
        let p1 = app.projects.add("x", Some("P1"), None);
        let p2 = app.projects.add("x", Some("P2"), None);
        app.projects.set_category(p1, Some(ca));
        for p in [p1, p2] {
            app.projects.add_dev(p, fe);
            app.projects.add_dev(p, be);
        }
        app.projects.add_effort(p1, fe, WeekId(20000), mario, Effort(20));
        app.projects.add_effort(p2, fe, WeekId(20000), mario, Effort(10));
        app.projects.add_effort(p1, be, WeekId(20007), mario, Effort(10));
        app.projects.add_effort(p1, be, WeekId(20100), mario, Effort(99)); // fuori
        app.projects.add_effort(p2, be, WeekId(20000), luigi, Effort(0));
        let sel: HashSet<_> = [mario, luigi].into_iter().collect();
        let pies = build(&app, &sel, 20000, 20013, &BTreeSet::new());
        let m = pies.iter().find(|p| p.worker == mario).unwrap();
        assert_eq!(m.total, 40);
        assert_eq!(m.devs.len(), 2);
        assert_eq!((m.devs[0].dev, m.devs[0].hours), (fe, 30));
        assert_eq!(m.devs[0].categories, [(Some(ca), 20), (None, 10)]);
        assert_eq!(m.categories, [(Some(ca), 30), (None, 10)]);
        let inner = m.dev_ring();
        assert_eq!((inner[0].from, inner[0].to), (0.0, 0.75));
        assert_eq!((inner[1].from, inner[1].to), (0.75, 1.0));
        let outer = m.dev_category_ring();
        let bounds: Vec<_> = outer.iter().map(|s| (s.key, s.from, s.to)).collect();
        assert_eq!(
            bounds,
            [((fe, Some(ca)), 0.0, 0.5), ((fe, None), 0.5, 0.75), ((be, Some(ca)), 0.75, 1.0)]
        );
        let l = pies.iter().find(|p| p.worker == luigi).unwrap();
        assert_eq!(l.total, 0, "solo effort 0: nessuna ora");
        assert!(l.dev_ring().is_empty());
        assert_eq!(pct_of(1, 3), 33);
        assert_eq!(pct_of(5, 0), 0);
        // Filtro categoria: solo i progetti di «Cliente A» (P1); il totale e
        // le percentuali seguono.
        let only_a: BTreeSet<_> = [Some(ca)].into();
        let f = build(&app, &sel, 20000, 20013, &only_a);
        let m = f.iter().find(|p| p.worker == mario).unwrap();
        assert_eq!(m.total, 30);
        assert_eq!(m.categories, [(Some(ca), 30)]);
        assert_eq!(m.dev_ring()[0].to, 20.0 / 30.0);
        let no_cat: BTreeSet<_> = [None].into();
        let f = build(&app, &sel, 20000, 20013, &no_cat);
        assert_eq!(f.iter().find(|p| p.worker == mario).unwrap().total, 10);
        assert_eq!(pct_label(4, 1380), "<1%");
        assert_eq!(pct_label(0, 1380), "0%");
        assert_eq!(pct_label(1, 4), "25%");
    }

    /// Geometria: 0 in alto e senso orario; l'etichetta entra nelle fette
    /// grandi e non in quelle piccole; il punto colpito trova la sua fetta.
    #[test]
    fn pie_geometry_labels_and_hit() {
        let (x, y) = arc_point(0.0, 0.0, 10.0, 0.25, false);
        assert!((x - 10.0).abs() < 1e-4 && y.abs() < 1e-4, "un quarto di giro = a destra");
        let (_, y) = arc_point(0.0, 0.0, 10.0, 0.0, true);
        assert!((y - 10.0).abs() < 1e-4, "0 in alto anche con l'asse verso l'alto");
        assert!(label_pos(0.0, 0.0, 0.0, 100.0, 0.0, 0.5, 30.0, 12.0, false).is_some());
        assert!(label_pos(0.0, 0.0, 0.0, 100.0, 0.0, 0.01, 30.0, 12.0, false).is_none());
        assert!(label_pos(0.0, 0.0, 70.0, 100.0, 0.0, 0.3, 30.0, 12.0, false).is_some());
        assert!(label_pos(0.0, 0.0, 95.0, 100.0, 0.0, 0.3, 30.0, 12.0, false).is_none());
        let s = vec![
            Seg { key: 1, hours: 1, from: 0.0, to: 0.25 },
            Seg { key: 2, hours: 3, from: 0.25, to: 1.0 },
        ];
        assert_eq!(hit(0.0, 0.0, 0.0, 10.0, &s, 3.0, -5.0), Some(0), "in alto a destra");
        assert_eq!(hit(0.0, 0.0, 0.0, 10.0, &s, -3.0, 5.0), Some(1));
        assert_eq!(hit(0.0, 0.0, 5.0, 10.0, &s, 1.0, 1.0), None, "nel buco");
        let pts = sector(0.0, 0.0, 0.0, 10.0, 0.0, 0.25, true);
        assert_eq!(pts.last(), Some(&(0.0, 0.0)), "lo spicchio chiude sul centro");
    }
}
