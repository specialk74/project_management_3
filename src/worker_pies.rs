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
//!
//! Ogni valore porta anche le ore **fatte** (settimane già concluse, prima di
//! quella corrente); il resto, **dalla settimana corrente** in poi, sono ore
//! **pianificate**: a metà settimana quelle ore non sono ancora fatte, e così
//! le ore libere partono dalla stessa settimana del numero verde del footer
//! (`worker_remaining_hours`). Le torte usano solo il totale; fatte/pianificate
//! vanno nelle tabelle quando il periodo arriva alla settimana corrente
//! (`split_shown`).
//!
//! La **capacità** del worker nel periodo è la somma, sulle settimane del
//! periodo, del suo massimo ore effettivo (limite della settimana se c'è,
//! altrimenti il massimo del worker): la stessa regola della Saturazione,
//! quindi ferie e malattia contano solo attraverso il limite della settimana.

use std::collections::{BTreeMap, BTreeSet, HashSet};

use crate::app::App;
use crate::categories::CategoryId;
use crate::dev_utils::dev::DevId;
use crate::worker_matrix::week_overlaps;
use crate::workers_utils::worker::{WORKER_ID_ZERO, WorkerId};

/// Ore totali di una voce e, di queste, quelle fatte (settimane prima di
/// quella corrente).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Hours {
    pub total: u32,
    pub done: u32,
}

impl Hours {
    /// Ore pianificate: dalla settimana corrente in poi.
    pub fn planned(&self) -> u32 {
        self.total - self.done
    }

    fn add(&mut self, h: u32, done: bool) {
        self.total += h;
        if done {
            self.done += h;
        }
    }
}

impl std::ops::AddAssign for Hours {
    fn add_assign(&mut self, o: Hours) {
        self.total += o.total;
        self.done += o.done;
    }
}

/// Ore di un dev del worker, divise per categoria.
pub struct DevHours {
    pub dev: DevId,
    pub hours: Hours,
    /// Ordine delle categorie: per id, «Senza categoria» in fondo; solo > 0.
    pub categories: Vec<(Option<CategoryId>, Hours)>,
}

/// Dati delle due torte di un worker.
pub struct WorkerPie {
    pub worker: WorkerId,
    pub name: String,
    /// Ore totali nel periodo (0 = niente torte).
    pub total: u32,
    /// Di `total`, le ore fatte (settimane prima di quella corrente).
    pub done: u32,
    /// Ore disponibili nel periodo (vedi la nota del modulo).
    pub capacity: u32,
    /// Di `capacity`, quelle delle settimane **dalla corrente** in poi: le
    /// stesse in cui le ore sono «pianificate».
    pub future_capacity: u32,
    /// Ore pianificate di **tutte** le categorie, anche con il filtro: le ore
    /// libere si calcolano su queste, perché il lavoro delle categorie escluse
    /// occupa comunque il worker.
    pub planned_all: u32,
    /// Il filtro Categorie era attivo: `planned_all` può differire da
    /// `planned()` e la riga delle ore libere lo dice.
    pub filtered: bool,
    /// Dev in ordine di id, solo quelli con ore.
    pub devs: Vec<DevHours>,
    /// Categorie (stesso ordine di `DevHours::categories`), solo > 0.
    pub categories: Vec<(Option<CategoryId>, Hours)>,
}

/// Lunedì (giorni) delle settimane che si sovrappongono a `[from, to]`: le
/// stesse settimane di cui si contano le ore.
pub fn period_weeks(from: i32, to: i32) -> Vec<i32> {
    use crate::date_utils::dates::{days_to_local, local_to_days, primo_giorno_settimana_corrente};
    if from > to {
        return Vec::new();
    }
    let first = local_to_days(&primo_giorno_settimana_corrente(&days_to_local(from)));
    (0..)
        .map(|i| first + 7 * i)
        .take_while(|w| week_overlaps(*w, from, to))
        .collect()
}

/// Ore disponibili del worker nelle settimane `weeks` (vedi la nota del
/// modulo).
pub fn capacity(app: &App, worker: WorkerId, weeks: &[i32]) -> u32 {
    weeks
        .iter()
        .map(|&w| app.workers.get_effective_max_hours(worker, w as usize))
        .sum()
}

impl WorkerPie {
    /// Ore pianificate (dalla settimana corrente in poi).
    pub fn planned(&self) -> u32 {
        self.total - self.done
    }

    /// Ore ancora libere da qui a fine periodo: capacità delle settimane future
    /// meno le ore pianificate di tutte le categorie (`planned_all`). Negativo =
    /// oltre la capacità.
    pub fn free_hours(&self) -> i64 {
        self.future_capacity as i64 - self.planned_all as i64
    }
}

/// Le colonne fatte/pianificate servono solo se il periodo (che finisce a
/// `to`) arriva alla settimana corrente (lunedì `now`): altrimenti tutto è
/// «fatto».
pub fn split_shown(to: i32, now: i32) -> bool {
    to >= now
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
        let items: Vec<(DevId, u32)> = self.devs.iter().map(|d| (d.dev, d.hours.total)).collect();
        segs(&items, self.total, 0)
    }

    /// Anello esterno della prima torta: le categorie di ogni dev, nello
    /// spicchio del dev.
    pub fn dev_category_ring(&self) -> Vec<Seg<(DevId, Option<CategoryId>)>> {
        let mut out = Vec::new();
        let mut start = 0;
        for d in &self.devs {
            let items: Vec<_> = d.categories.iter().map(|&(c, h)| ((d.dev, c), h.total)).collect();
            out.extend(segs(&items, self.total, start));
            start += d.hours.total;
        }
        out
    }

    /// Seconda torta: una fetta per categoria.
    pub fn category_ring(&self) -> Vec<Seg<Option<CategoryId>>> {
        let items: Vec<_> = self.categories.iter().map(|&(c, h)| (c, h.total)).collect();
        segs(&items, self.total, 0)
    }
}

/// Torte dei worker `selected` (ordinati come `workers.list()`), anche quelli
/// senza ore nel periodo (`total` 0). `cats` limita ai progetti di quelle
/// categorie (`None` nel set = «Senza categoria»; vuoto = tutte), come la
/// combo della matrice: totale e percentuali contano solo quei progetti.
/// `now` = lunedì della settimana corrente (giorni): le settimane che iniziano
/// prima sono «fatte», quella corrente e le successive «pianificate».
pub fn build(
    app: &App,
    selected: &HashSet<WorkerId>,
    from: i32,
    to: i32,
    cats: &BTreeSet<Option<CategoryId>>,
    now: i32,
) -> Vec<WorkerPie> {
    let workers: Vec<(WorkerId, String)> = app
        .workers
        .list()
        .into_iter()
        .filter(|(id, _)| *id != WORKER_ID_ZERO && selected.contains(id))
        .collect();
    // hours[w][dev][categoria]; la chiave di categoria mette «Senza» in fondo.
    type CatKey = (bool, Option<CategoryId>);
    let mut hours: Vec<BTreeMap<DevId, BTreeMap<CatKey, Hours>>> =
        vec![BTreeMap::new(); workers.len()];
    // Ore pianificate di tutte le categorie, filtro o no (per le ore libere).
    let mut planned_all = vec![0u32; workers.len()];
    for (proj, _) in app.projects.list() {
        let cat = app.projects.get_category(proj);
        let shown = cats.is_empty() || cats.contains(&cat);
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
                    let done = (wk.0 as i32) < now;
                    if !done {
                        planned_all[wi] += h;
                    }
                    if h > 0 && shown {
                        hours[wi]
                            .entry(dev)
                            .or_default()
                            .entry((cat.is_none(), cat))
                            .or_default()
                            .add(h, done);
                    }
                }
            }
        }
    }
    let weeks = period_weeks(from, to);
    let filtered = !cats.is_empty();
    let future: Vec<i32> = weeks.iter().copied().filter(|w| *w >= now).collect();
    workers
        .into_iter()
        .zip(hours)
        .zip(planned_all)
        .map(|(((worker, name), by_dev), planned_all)| {
            let mut by_cat_all: BTreeMap<CatKey, Hours> = BTreeMap::new();
            let mut all = Hours::default();
            let devs: Vec<DevHours> = by_dev
                .into_iter()
                .map(|(dev, by_cat)| {
                    let mut hours = Hours::default();
                    for (k, h) in &by_cat {
                        *by_cat_all.entry(*k).or_default() += *h;
                        hours += *h;
                    }
                    all += hours;
                    DevHours {
                        dev,
                        hours,
                        categories: by_cat.into_iter().map(|((_, c), h)| (c, h)).collect(),
                    }
                })
                .collect();
            WorkerPie {
                capacity: capacity(app, worker, &weeks),
                future_capacity: capacity(app, worker, &future),
                planned_all,
                filtered,
                worker,
                name,
                total: all.total,
                done: all.done,
                devs,
                categories: by_cat_all.into_iter().map(|((_, c), h)| (c, h)).collect(),
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
        // «Oggi» nella terza settimana: le prime due sono concluse, fatte.
        let pies = build(&app, &sel, 20000, 20013, &BTreeSet::new(), 20014);
        let m = pies.iter().find(|p| p.worker == mario).unwrap();
        let h = |total, done| Hours { total, done };
        assert_eq!((m.total, m.done), (40, 40));
        assert_eq!(m.devs.len(), 2);
        assert_eq!((m.devs[0].dev, m.devs[0].hours), (fe, h(30, 30)));
        assert_eq!(m.devs[0].categories, [(Some(ca), h(20, 20)), (None, h(10, 10))]);
        assert_eq!(m.categories, [(Some(ca), h(30, 30)), (None, h(10, 10))]);
        // «Oggi» nella seconda settimana: la settimana corrente è ancora
        // pianificata, quindi le sue 10h di Backend lo sono.
        let early = build(&app, &sel, 20000, 20013, &BTreeSet::new(), 20007);
        let e = early.iter().find(|p| p.worker == mario).unwrap();
        assert_eq!((e.total, e.done), (40, 30));
        assert_eq!(e.devs[1].hours, h(10, 0));
        assert_eq!(e.devs[1].hours.planned(), 10);
        assert_eq!(e.categories[0].1, h(30, 20));
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
        let f = build(&app, &sel, 20000, 20013, &only_a, 20007);
        let m = f.iter().find(|p| p.worker == mario).unwrap();
        assert_eq!(m.total, 30);
        // Le 10h della settimana corrente (20007) sono pianificate.
        assert_eq!(m.categories, [(Some(ca), h(30, 20))]);
        assert_eq!(m.dev_ring()[0].to, 20.0 / 30.0);
        let no_cat: BTreeSet<_> = [None].into();
        let f = build(&app, &sel, 20000, 20013, &no_cat, 20007);
        assert_eq!(f.iter().find(|p| p.worker == mario).unwrap().total, 10);
        assert_eq!(pct_label(4, 1380), "<1%");
        assert_eq!(pct_label(0, 1380), "0%");
        assert_eq!(pct_label(1, 4), "25%");
        // Colonne fatte/pianificate solo se il periodo va oltre la settimana
        // corrente.
        assert!(!split_shown(19999, 20000), "periodo concluso: tutto fatto");
        assert!(split_shown(20000, 20000), "arriva alla settimana corrente");
    }

    /// Capacità: massimo effettivo di ogni settimana del periodo (limite della
    /// settimana se c'è, altrimenti il massimo del worker, 40h di default).
    #[test]
    fn capacity_sums_the_effective_max_of_the_period_weeks() {
        use crate::date_utils::dates::{local_to_days, primo_giorno_settimana_corrente};
        let monday = local_to_days(&primo_giorno_settimana_corrente(
            &chrono::NaiveDate::from_ymd_opt(2026, 10, 19).unwrap(),
        ));
        // Da mercoledì a martedì della settimana dopo: due settimane.
        assert_eq!(period_weeks(monday + 2, monday + 8), [monday, monday + 7]);
        assert_eq!(period_weeks(monday, monday + 6), [monday]);
        assert!(period_weeks(monday + 7, monday).is_empty());
        let mut app = App::new();
        let mario = app.workers.add("Mario");
        let weeks = period_weeks(monday, monday + 20);
        assert_eq!(weeks.len(), 3);
        assert_eq!(capacity(&app, mario, &weeks), 120, "40h di default");
        app.workers.set_week_override(mario, monday as usize, 16);
        assert_eq!(capacity(&app, mario, &weeks), 96);
        let sel: HashSet<_> = [mario].into_iter().collect();
        let pies = build(&app, &sel, monday, monday + 20, &BTreeSet::new(), monday);
        assert_eq!(pies[0].capacity, 96);
        // «Oggi» nella prima settimana: conta già lei (come il footer).
        assert_eq!(pies[0].future_capacity, 96);
        assert_eq!(pies[0].free_hours(), 96);
        // «Oggi» nella seconda: la prima è conclusa, restano le altre due.
        let pies = build(&app, &sel, monday, monday + 20, &BTreeSet::new(), monday + 7);
        assert_eq!(pies[0].future_capacity, 80);
    }

    /// Ore libere da qui a fine periodo: capacità delle settimane future meno
    /// le ore pianificate; negative se oltre la capacità.
    #[test]
    fn free_hours_are_future_capacity_minus_planned() {
        use crate::date_utils::dates::{local_to_days, primo_giorno_settimana_corrente};
        use crate::single_dev_utils::single_dev::WeekId;
        use crate::single_effort_utils::single_effort::Effort;
        let monday = local_to_days(&primo_giorno_settimana_corrente(
            &chrono::NaiveDate::from_ymd_opt(2026, 10, 19).unwrap(),
        ));
        let mut app = App::new();
        let mario = app.workers.add("Mario");
        let fe = app.devs.add("Frontend");
        let p = app.projects.add("x", Some("P1"), None);
        app.projects.add_dev(p, fe);
        let wk = |i: i32| WeekId((monday + 7 * i) as usize);
        app.projects.add_effort(p, fe, wk(0), mario, Effort(40));
        app.projects.add_effort(p, fe, wk(1), mario, Effort(30));
        app.projects.add_effort(p, fe, wk(2), mario, Effort(20));
        let sel: HashSet<_> = [mario].into_iter().collect();
        let to = monday + 20;
        // «Oggi» nella prima settimana: è già tra le pianificate e disponibili.
        let m = &build(&app, &sel, monday, to, &BTreeSet::new(), monday)[0];
        assert_eq!((m.planned(), m.future_capacity, m.free_hours()), (90, 120, 30));
        // «Oggi» nella seconda: la prima (40h) è fatta e fuori dal conto.
        let now = monday + 7;
        let m = &build(&app, &sel, monday, to, &BTreeSet::new(), now)[0];
        assert_eq!((m.planned(), m.future_capacity, m.free_hours()), (50, 80, 30));
        // Oltre la capacità: 60h in una settimana da 40h.
        app.projects.add_effort(p, fe, wk(2), mario, Effort(60));
        let m = &build(&app, &sel, monday, to, &BTreeSet::new(), now)[0];
        assert_eq!(m.free_hours(), -10);
        // Filtro Categorie: le ore libere contano comunque il lavoro delle
        // categorie escluse (P1 è senza categoria, il filtro tiene solo «A»).
        let ca = app.categories.add("Cliente A");
        let pa = app.projects.add("x", Some("PA"), None);
        app.projects.set_category(pa, Some(ca));
        app.projects.add_dev(pa, fe);
        app.projects.add_effort(pa, fe, wk(1), mario, Effort(5));
        let only_a: BTreeSet<_> = [Some(ca)].into();
        let m = &build(&app, &sel, monday, to, &only_a, now)[0];
        assert_eq!(m.planned(), 5, "le tabelle seguono il filtro");
        assert_eq!(m.planned_all, 95, "30 + 60 di P1 e 5 di PA");
        assert_eq!(m.free_hours(), 80 - 95);
        assert!(m.filtered);
        let m = &build(&app, &sel, monday, to, &BTreeSet::new(), now)[0];
        assert!(!m.filtered, "senza filtro");
        assert_eq!(m.planned(), m.planned_all);
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
