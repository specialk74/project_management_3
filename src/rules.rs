//! Regole di automazione (MIGLIORAMENTI #58, fase 1): milestone posizionate
//! rispetto a un'ancora, senza scrivere codice. Una regola dice
//! «*bersaglio* = *ancora* ± N settimane» in un *ambito* di progetti, es.
//! «milestone Collaudo = fine del dev Backend + 2 settimane, per tutti i
//! progetti». Le regole stanno nel `.ron` (`App.rules`) e si scrivono dalla
//! finestra «Regole…» (`ui::rules`).
//!
//! **Sempre allineate**: `apply` ricalcola dopo ogni modifica (da
//! `PjmApp::mark_changed`) e all'apertura del file. Per ogni progetto e
//! milestone c'è al più una regola che la *governa* (`governing`: la più
//! specifica fra quelle attive il cui ambito comprende il progetto). La
//! milestone governata **esiste se e solo se esiste l'ancora**, nella
//! settimana dell'ancora + `offset_weeks`; nel menù della griglia non si
//! sposta a mano. Le catene (milestone che dipende da una milestone governata)
//! si risolvono in ordine; una regola in un **ciclo** (o che dipende da un
//! ciclo) resta ferma e la finestra la segnala.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use serde::{Deserialize, Serialize};

use crate::app::App;
use crate::categories::CategoryId;
use crate::dev_utils::dev::DevId;
use crate::milestones::MilestoneId;
use crate::priorities::PriorityId;
use crate::project_utils::project::ProjectId;
use crate::single_dev_utils::single_dev::WeekId;

#[derive(Serialize, Deserialize, Hash, PartialEq, Eq, Clone, Copy, PartialOrd, Ord, Debug, Default)]
pub struct RuleId(pub usize);

/// Da dove si conta (la settimana di partenza della regola).
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Anchor {
    /// Prima settimana con effort (> 0) del dev nel progetto.
    DevStart(DevId),
    /// Ultima settimana con effort (> 0) del dev nel progetto.
    DevEnd(DevId),
    /// Data di inizio del progetto.
    ProjectStart,
    /// Data di fine del progetto.
    ProjectEnd,
    /// Settimana in cui la milestone è collocata nel progetto.
    Milestone(MilestoneId),
}

/// A quali progetti vale la regola.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuleScope {
    All,
    Category(CategoryId),
    Priority(PriorityId),
    Project(ProjectId),
}

impl RuleScope {
    /// Il progetto è nell'ambito?
    pub fn matches(self, app: &App, proj: ProjectId) -> bool {
        match self {
            RuleScope::All => true,
            RuleScope::Category(c) => app.projects.get_category(proj) == Some(c),
            RuleScope::Priority(p) => app.projects.get_priority(proj) == Some(p),
            RuleScope::Project(p) => p == proj,
        }
    }

    /// Specificità: vince il valore più alto (progetto > categoria >
    /// priorità > tutti).
    pub fn rank(self) -> u8 {
        match self {
            RuleScope::All => 0,
            RuleScope::Priority(_) => 1,
            RuleScope::Category(_) => 2,
            RuleScope::Project(_) => 3,
        }
    }
}

fn yes() -> bool {
    true
}

/// Una regola: `target` = `anchor` + `offset_weeks` settimane, nei progetti di
/// `scope`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Rule {
    #[serde(default)]
    pub name: String,
    #[serde(default = "yes")]
    pub enabled: bool,
    pub scope: RuleScope,
    pub target: MilestoneId,
    pub anchor: Anchor,
    #[serde(default)]
    pub offset_weeks: i32,
}

/// Le regole del file, con id stabili.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct Rules {
    last_id: usize,
    rules: BTreeMap<RuleId, Rule>,
}

impl Rules {
    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }

    /// Regole in ordine di creazione.
    pub fn list(&self) -> Vec<(RuleId, &Rule)> {
        self.rules.iter().map(|(id, r)| (*id, r)).collect()
    }

    pub fn get(&self, id: RuleId) -> Option<&Rule> {
        self.rules.get(&id)
    }

    pub fn add(&mut self, rule: Rule) -> RuleId {
        self.last_id += 1;
        let id = RuleId(self.last_id);
        self.rules.insert(id, rule);
        id
    }

    /// Sostituisce la regola; `true` se è cambiata.
    pub fn update(&mut self, id: RuleId, rule: Rule) -> bool {
        match self.rules.get_mut(&id) {
            Some(r) if *r != rule => {
                *r = rule;
                true
            }
            _ => false,
        }
    }

    pub fn remove(&mut self, id: RuleId) -> bool {
        self.rules.remove(&id).is_some()
    }

    /// Regole che usano la milestone (come bersaglio o ancora).
    pub fn using_milestone(&self, m: MilestoneId) -> Vec<RuleId> {
        self.rules
            .iter()
            .filter(|(_, r)| r.target == m || r.anchor == Anchor::Milestone(m))
            .map(|(id, _)| *id)
            .collect()
    }

    /// Regole che usano il dev come ancora.
    pub fn using_dev(&self, d: DevId) -> Vec<RuleId> {
        self.rules
            .iter()
            .filter(|(_, r)| matches!(r.anchor, Anchor::DevStart(x) | Anchor::DevEnd(x) if x == d))
            .map(|(id, _)| *id)
            .collect()
    }
}

/// La milestone si può collocare nel progetto (esiste ed è di sistema o di
/// quel progetto)?
fn placeable(app: &App, proj: ProjectId, m: MilestoneId) -> bool {
    app.milestones.available_for(m, proj)
}

/// Per ogni milestone del progetto governata da una regola, la regola che la
/// governa: fra le attive con ambito che comprende il progetto e bersaglio
/// collocabile lì, la più specifica (a parità, la creata prima).
pub fn governing(app: &App, proj: ProjectId) -> BTreeMap<MilestoneId, RuleId> {
    let mut out: BTreeMap<MilestoneId, (u8, RuleId)> = BTreeMap::new();
    for (id, r) in app.rules.list() {
        if !r.enabled || !r.scope.matches(app, proj) || !placeable(app, proj, r.target) {
            continue;
        }
        let rank = r.scope.rank();
        match out.get(&r.target) {
            Some(&(best, _)) if best >= rank => {}
            _ => {
                out.insert(r.target, (rank, id));
            }
        }
    }
    out.into_iter().map(|(m, (_, id))| (m, id)).collect()
}

/// La regola che governa la milestone `m` nel progetto, se c'è (la griglia
/// non la fa spostare a mano).
#[cfg(test)]
pub fn governing_rule(app: &App, proj: ProjectId, m: MilestoneId) -> Option<RuleId> {
    governing(app, proj).get(&m).copied()
}

/// Valore di un'ancora non-milestone nel progetto.
fn plain_anchor(app: &App, proj: ProjectId, a: Anchor) -> Option<WeekId> {
    match a {
        Anchor::DevStart(d) => app.projects.get_single_dev(proj, d)?.effort_span().map(|s| s.0),
        Anchor::DevEnd(d) => app.projects.get_single_dev(proj, d)?.effort_span().map(|s| s.1),
        Anchor::ProjectStart => app.projects.get_project_start_week(proj),
        Anchor::ProjectEnd => app.projects.get_project_end_week(proj),
        Anchor::Milestone(_) => None,
    }
}

/// Settimana dell'ancora spostata di `weeks` settimane (mai prima del giorno 0).
fn shifted(w: WeekId, weeks: i32) -> WeekId {
    WeekId((w.0 as i64 + 7 * weeks as i64).max(0) as usize)
}

/// Esito del calcolo delle regole di un progetto.
#[derive(Debug, Default, PartialEq)]
pub struct Evaluation {
    /// Per ogni milestone governata e risolta: dove deve stare (`None` =
    /// ancora mancante, quindi la milestone non c'è).
    pub values: BTreeMap<MilestoneId, Option<WeekId>>,
    /// Regole ferme perché in un ciclo o dipendenti da un ciclo.
    pub blocked: BTreeSet<RuleId>,
}

/// Calcola le milestone governate del progetto, risolvendo le catene.
pub fn evaluate(app: &App, proj: ProjectId) -> Evaluation {
    #[derive(Clone, Copy)]
    enum St {
        Visiting,
        Done(Option<WeekId>),
        Failed,
    }
    let gov = governing(app, proj);
    let placed: HashMap<MilestoneId, WeekId> = app.projects.list_project_milestones(proj).into_iter().collect();
    let mut memo: HashMap<MilestoneId, St> = HashMap::new();

    // Ok(valore) o Err(()) se la catena incontra un ciclo.
    fn resolve(
        m: MilestoneId,
        app: &App,
        proj: ProjectId,
        gov: &BTreeMap<MilestoneId, RuleId>,
        placed: &HashMap<MilestoneId, WeekId>,
        memo: &mut HashMap<MilestoneId, St>,
    ) -> Result<Option<WeekId>, ()> {
        let Some(&rid) = gov.get(&m) else {
            // Non governata: conta dove sta adesso.
            return Ok(placed.get(&m).copied());
        };
        match memo.get(&m) {
            Some(St::Done(v)) => return Ok(*v),
            Some(St::Visiting) | Some(St::Failed) => {
                memo.insert(m, St::Failed);
                return Err(());
            }
            None => {}
        }
        memo.insert(m, St::Visiting);
        let rule = app.rules.get(rid).expect("regola governante esistente");
        let anchor = match rule.anchor {
            Anchor::Milestone(a) => resolve(a, app, proj, gov, placed, memo),
            other => Ok(plain_anchor(app, proj, other)),
        };
        match anchor {
            Ok(v) => {
                let v = v.map(|w| shifted(w, rule.offset_weeks));
                memo.insert(m, St::Done(v));
                Ok(v)
            }
            Err(()) => {
                memo.insert(m, St::Failed);
                Err(())
            }
        }
    }

    let mut out = Evaluation::default();
    for (&m, &rid) in &gov {
        match resolve(m, app, proj, &gov, &placed, &mut memo) {
            Ok(v) => {
                out.values.insert(m, v);
            }
            Err(()) => {
                out.blocked.insert(rid);
            }
        }
    }
    out
}

/// Esito di `apply`.
#[derive(Debug, Default, PartialEq)]
pub struct Outcome {
    /// Qualche milestone è stata messa, spostata o tolta.
    pub changed: bool,
    /// Regole ferme per un ciclo, in almeno un progetto.
    pub blocked: BTreeSet<RuleId>,
}

/// Applica tutte le regole a tutti i progetti (aperti e chiusi): ogni
/// milestone governata va nella settimana calcolata, o viene tolta se
/// l'ancora manca. Le regole bloccate non toccano nulla.
pub fn apply(app: &mut App) -> Outcome {
    let mut out = Outcome::default();
    if app.rules.is_empty() {
        return out;
    }
    let projects: Vec<ProjectId> = app.projects.list().into_iter().map(|(p, _)| p).collect();
    for proj in projects {
        let ev = evaluate(app, proj);
        out.blocked.extend(ev.blocked);
        let placed: HashMap<MilestoneId, WeekId> = app.projects.list_project_milestones(proj).into_iter().collect();
        for (m, v) in ev.values {
            match (placed.get(&m), v) {
                (Some(cur), Some(w)) if *cur == w => {}
                (_, Some(w)) => {
                    app.projects.add_project_milestone(proj, m, w);
                    out.changed = true;
                }
                (Some(_), None) => {
                    app.projects.remove_project_milestone(proj, m);
                    out.changed = true;
                }
                (None, None) => {}
            }
        }
    }
    out
}

/// Nome della regola, o una descrizione se non ne ha («Regola 3»).
pub fn rule_label(app: &App, id: RuleId) -> String {
    match app.rules.get(id) {
        Some(r) if !r.name.trim().is_empty() => r.name.trim().to_string(),
        _ => format!("Regola {}", id.0),
    }
}

/// Situazione di una regola, per la finestra: in quanti progetti governa la
/// sua milestone, in quanti l'ha collocata, se è ferma per un ciclo e i
/// riferimenti che non esistono più.
#[derive(Debug, Default, PartialEq)]
pub struct RuleStatus {
    pub governed: usize,
    pub placed: usize,
    pub blocked: bool,
    pub missing: Vec<&'static str>,
}

pub fn status(app: &App, id: RuleId) -> RuleStatus {
    let mut st = RuleStatus::default();
    let Some(r) = app.rules.get(id) else {
        return st;
    };
    if app.milestones.get_name(r.target).is_none() {
        st.missing.push("milestone");
    }
    match r.anchor {
        Anchor::DevStart(d) | Anchor::DevEnd(d) if app.devs.get(d).is_none() => st.missing.push("dev"),
        Anchor::Milestone(m) if app.milestones.get_name(m).is_none() => st.missing.push("milestone di partenza"),
        _ => {}
    }
    match r.scope {
        RuleScope::Category(c) if app.categories.get_name(c).is_none() => st.missing.push("categoria"),
        RuleScope::Priority(p) if app.priorities.get_name(p).is_none() => st.missing.push("priorità"),
        RuleScope::Project(p) if app.projects.get(p).is_none() => st.missing.push("progetto"),
        _ => {}
    }
    for (proj, _) in app.projects.list() {
        if governing(app, proj).get(&r.target) != Some(&id) {
            continue;
        }
        st.governed += 1;
        let ev = evaluate(app, proj);
        if ev.blocked.contains(&id) {
            st.blocked = true;
        } else if ev.values.get(&r.target).is_some_and(Option::is_some) {
            st.placed += 1;
        }
    }
    st
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::single_effort_utils::single_effort::Effort;

    /// Progetto ABC con i dev Backend (settimane 0–3) e Frontend (2–5),
    /// milestone di sistema Collaudo / Rilascio / Demo.
    struct Fx {
        app: App,
        p: ProjectId,
        be: DevId,
        fe: DevId,
        collaudo: MilestoneId,
        rilascio: MilestoneId,
        demo: MilestoneId,
        base: usize,
    }

    fn fx() -> Fx {
        let mut app = App::new();
        let w = app.workers.add("Mario");
        let be = app.devs.add("Backend");
        let fe = app.devs.add("Frontend");
        let p = app.projects.add("x", Some("ABC"), None);
        app.projects.add_dev(p, be);
        app.projects.add_dev(p, fe);
        let base = 20004; // un lunedì
        for i in 0..4 {
            app.projects.add_effort(p, be, WeekId(base + 7 * i), w, Effort(8));
        }
        for i in 2..6 {
            app.projects.add_effort(p, fe, WeekId(base + 7 * i), w, Effort(8));
        }
        let collaudo = app.milestones.add("Collaudo");
        let rilascio = app.milestones.add("Rilascio");
        let demo = app.milestones.add("Demo");
        Fx { app, p, be, fe, collaudo, rilascio, demo, base }
    }

    fn rule(scope: RuleScope, target: MilestoneId, anchor: Anchor, offset_weeks: i32) -> Rule {
        Rule { name: String::new(), enabled: true, scope, target, anchor, offset_weeks }
    }

    fn at(app: &App, p: ProjectId, m: MilestoneId) -> Option<WeekId> {
        app.projects.list_project_milestones(p).into_iter().find(|(x, _)| *x == m).map(|(_, w)| w)
    }

    /// «Collaudo = fine Backend + 2»; «Rilascio = Collaudo + 8»: la catena
    /// segue l'ancora quando il dev cambia, e senza effort tutto sparisce.
    #[test]
    fn milestones_follow_their_anchor_through_a_chain() {
        let mut f = fx();
        f.app.rules.add(rule(RuleScope::All, f.collaudo, Anchor::DevEnd(f.be), 2));
        f.app.rules.add(rule(RuleScope::All, f.rilascio, Anchor::Milestone(f.collaudo), 8));
        let out = apply(&mut f.app);
        assert!(out.changed && out.blocked.is_empty());
        let end = f.base + 7 * 3;
        assert_eq!(at(&f.app, f.p, f.collaudo), Some(WeekId(end + 14)));
        assert_eq!(at(&f.app, f.p, f.rilascio), Some(WeekId(end + 14 + 56)));
        assert!(!apply(&mut f.app).changed, "già allineate: niente da fare");
        // Il Backend lavora una settimana in più: si sposta tutto.
        let w = f.app.workers.list()[0].0;
        f.app.projects.add_effort(f.p, f.be, WeekId(end + 7), w, Effort(4));
        assert!(apply(&mut f.app).changed);
        assert_eq!(at(&f.app, f.p, f.collaudo), Some(WeekId(end + 21)));
        assert_eq!(at(&f.app, f.p, f.rilascio), Some(WeekId(end + 21 + 56)));
        // Senza effort sul Backend l'ancora non c'è: spariscono entrambe.
        f.app.projects.clear_efforts(f.p, Some(f.be));
        apply(&mut f.app);
        assert_eq!(at(&f.app, f.p, f.collaudo), None);
        assert_eq!(at(&f.app, f.p, f.rilascio), None);
    }

    /// Inizio del dev, date del progetto, scostamento negativo; regola spenta
    /// = nessun effetto.
    #[test]
    fn other_anchors_negative_offset_and_disabled_rules() {
        let mut f = fx();
        f.app.projects.set_project_end_week(f.p, Some(WeekId(f.base + 70)));
        let demo = f.app.rules.add(rule(RuleScope::All, f.demo, Anchor::DevStart(f.fe), -1));
        f.app.rules.add(rule(RuleScope::All, f.collaudo, Anchor::ProjectEnd, -2));
        let mut off = rule(RuleScope::All, f.rilascio, Anchor::ProjectStart, 0);
        off.enabled = false;
        f.app.rules.add(off);
        apply(&mut f.app);
        assert_eq!(at(&f.app, f.p, f.demo), Some(WeekId(f.base + 14 - 7)));
        assert_eq!(at(&f.app, f.p, f.collaudo), Some(WeekId(f.base + 70 - 14)));
        assert_eq!(at(&f.app, f.p, f.rilascio), None, "regola spenta");
        assert_eq!(governing_rule(&f.app, f.p, f.demo), Some(demo));
        assert_eq!(governing_rule(&f.app, f.p, f.rilascio), None);
    }

    /// Vince la regola più specifica: progetto > categoria > priorità > tutti.
    #[test]
    fn the_most_specific_scope_wins() {
        let mut f = fx();
        let cat = f.app.categories.add("Cliente");
        let must = f.app.priorities.list()[0].0;
        f.app.projects.set_category(f.p, Some(cat));
        f.app.projects.set_priority(f.p, Some(must));
        let q = f.app.projects.add("y", Some("XYZ"), None);
        f.app.projects.add_dev(q, f.be);
        let w = f.app.workers.list()[0].0;
        f.app.projects.add_effort(q, f.be, WeekId(f.base), w, Effort(8));
        f.app.rules.add(rule(RuleScope::All, f.collaudo, Anchor::DevEnd(f.be), 1));
        let pri = f.app.rules.add(rule(RuleScope::Priority(must), f.collaudo, Anchor::DevEnd(f.be), 2));
        assert_eq!(governing_rule(&f.app, f.p, f.collaudo), Some(pri));
        let catr = f.app.rules.add(rule(RuleScope::Category(cat), f.collaudo, Anchor::DevEnd(f.be), 3));
        assert_eq!(governing_rule(&f.app, f.p, f.collaudo), Some(catr));
        let projr = f.app.rules.add(rule(RuleScope::Project(f.p), f.collaudo, Anchor::DevEnd(f.be), 4));
        assert_eq!(governing_rule(&f.app, f.p, f.collaudo), Some(projr));
        apply(&mut f.app);
        assert_eq!(at(&f.app, f.p, f.collaudo), Some(WeekId(f.base + 21 + 28)));
        // XYZ non ha categoria né priorità: vale la regola per tutti.
        assert_eq!(at(&f.app, q, f.collaudo), Some(WeekId(f.base + 7)));
    }

    /// Ciclo (Collaudo ← Rilascio ← Collaudo): le due regole restano ferme e
    /// sono segnalate; una terza che ne dipende anche.
    #[test]
    fn cycles_are_blocked_and_reported() {
        let mut f = fx();
        f.app.projects.add_project_milestone(f.p, f.collaudo, WeekId(f.base));
        let a = f.app.rules.add(rule(RuleScope::All, f.collaudo, Anchor::Milestone(f.rilascio), 1));
        let b = f.app.rules.add(rule(RuleScope::All, f.rilascio, Anchor::Milestone(f.collaudo), 1));
        let c = f.app.rules.add(rule(RuleScope::All, f.demo, Anchor::Milestone(f.rilascio), 1));
        let out = apply(&mut f.app);
        assert_eq!(out.blocked, [a, b, c].into());
        assert!(!out.changed, "regole ferme: niente cambia");
        assert_eq!(at(&f.app, f.p, f.collaudo), Some(WeekId(f.base)));
        let st = status(&f.app, a);
        assert!(st.blocked && st.governed == 1 && st.placed == 0);
    }

    /// Milestone di un altro progetto: non governabile qui; riferimenti
    /// eliminati segnalati da `status`.
    #[test]
    fn owned_milestones_and_missing_references() {
        let mut f = fx();
        let q = f.app.projects.add("y", Some("XYZ"), None);
        let own = f.app.milestones.add_owned("Solo XYZ", Default::default(), Vec::new(), Some(q));
        f.app.rules.add(rule(RuleScope::All, own, Anchor::DevEnd(f.be), 0));
        apply(&mut f.app);
        assert_eq!(at(&f.app, f.p, own), None, "milestone di XYZ: non va in ABC");
        let r = f.app.rules.add(rule(RuleScope::Project(ProjectId(999)), MilestoneId(999), Anchor::DevEnd(DevId(999)), 0));
        assert_eq!(status(&f.app, r).missing, ["milestone", "dev", "progetto"]);
        assert_eq!(f.app.rules.using_dev(f.be).len(), 1);
        assert_eq!(f.app.rules.using_milestone(own).len(), 1);
    }
}
