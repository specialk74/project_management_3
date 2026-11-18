//! Regole di automazione (MIGLIORAMENTI #58): milestone e date del progetto
//! posizionate rispetto a un'ancora, senza scrivere codice. Una regola dice
//! «*bersaglio* = *ancora* ± N settimane» in un *ambito* di progetti, es.
//! «milestone Collaudo = fine del dev Backend + 2 settimane, per tutti i
//! progetti» (fase 1) o «fine progetto = ultima milestone + 1» (fase 2). Le
//! regole stanno nel `.ron` (`App.rules`) e si scrivono dalla finestra
//! «Regole di automazione…» (`ui::rules`).
//!
//! **Sempre allineate**: `apply` ricalcola dopo ogni modifica (da
//! `PjmApp::mark_changed`) e all'apertura del file. Per ogni progetto e
//! bersaglio c'è al più una regola che lo *governa* (`governing`: la più
//! specifica fra quelle attive il cui ambito comprende il progetto). Una
//! milestone governata **esiste se e solo se esiste l'ancora**; una data del
//! progetto governata segue l'ancora ma, se l'ancora manca, **resta com'è**
//! (togliere la data di un progetto farebbe sparire il periodo). I bersagli
//! governati non si modificano a mano. Le catene si risolvono in ordine; una
//! regola in un **ciclo** (o che dipende da un ciclo) resta ferma e la
//! finestra la segnala.

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

/// Cosa posiziona la regola.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Target {
    /// Una milestone (collocata nel progetto, o tolta se l'ancora manca).
    Milestone(MilestoneId),
    /// La data di inizio del progetto.
    ProjectStart,
    /// La data di fine del progetto.
    ProjectEnd,
    /// Fase 3: aggiunge il dev al progetto quando c'è il dev della
    /// condizione (ancora `DevPresent`); non lo toglie mai.
    AddDev(DevId),
    /// Fase 3: prima settimana delle ore del dev. Si applica **su comando**
    /// («Applica ora…»): tutte le ore del dev si spostano in blocco.
    DevStart(DevId),
}

/// Come agisce una regola, secondo il suo bersaglio.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuleKind {
    /// Milestone e date: sempre allineate (`apply`).
    Aligned,
    /// Aggiunta di un dev: sempre (`apply`), mai rimosso.
    AddDev,
    /// Spostamento delle ore di un dev: solo su comando (`pending_moves`).
    MoveDev,
}

impl Target {
    pub fn milestone(self) -> Option<MilestoneId> {
        match self {
            Target::Milestone(m) => Some(m),
            _ => None,
        }
    }

    pub fn kind(self) -> RuleKind {
        match self {
            Target::Milestone(_) | Target::ProjectStart | Target::ProjectEnd => RuleKind::Aligned,
            Target::AddDev(_) => RuleKind::AddDev,
            Target::DevStart(_) => RuleKind::MoveDev,
        }
    }
}

/// L'ancora va bene per il bersaglio? «Aggiungi dev B» vuole la condizione
/// «Se c'è il dev A» (A ≠ B), e solo lui la usa; lo spostamento delle ore di
/// B non può contare da B stesso né da primo / ultimo effort (che
/// comprendono B): a ogni applicazione B si sposterebbe di nuovo.
pub fn anchor_fits(target: Target, anchor: Anchor) -> bool {
    match (target, anchor) {
        (Target::AddDev(b), Anchor::DevPresent(a)) => a != b,
        (Target::AddDev(_), _) | (_, Anchor::DevPresent(_)) => false,
        (Target::DevStart(b), Anchor::DevStart(a) | Anchor::DevEnd(a)) => a != b,
        (Target::DevStart(_), Anchor::FirstEffort | Anchor::LastEffort) => false,
        _ => true,
    }
}


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
    /// Prima settimana con effort (> 0) di qualunque dev del progetto.
    FirstEffort,
    /// Ultima settimana con effort (> 0) di qualunque dev del progetto.
    LastEffort,
    /// La milestone collocata più presto nel progetto (escluso il bersaglio
    /// stesso).
    FirstMilestone,
    /// La milestone collocata più tardi nel progetto (escluso il bersaglio
    /// stesso).
    LastMilestone,
    /// Condizione (fase 3, solo per «Aggiungi dev»): il dev è nel progetto,
    /// anche senza ore.
    DevPresent(DevId),
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
/// `scope`. Nel file il bersaglio è `goal`; le regole scritte con la fase 1
/// hanno invece `target: (id)` (solo milestone) e si rileggono uguali
/// (`RuleFile`).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(from = "RuleFile")]
pub struct Rule {
    pub name: String,
    pub enabled: bool,
    pub scope: RuleScope,
    #[serde(rename = "goal")]
    pub target: Target,
    pub anchor: Anchor,
    pub offset_weeks: i32,
}

impl Rule {
    /// Il dev del bersaglio (per «Aggiungi dev» e lo spostamento delle ore).
    pub fn target_dev(&self) -> DevId {
        match self.target {
            Target::AddDev(d) | Target::DevStart(d) => d,
            _ => DevId(usize::MAX),
        }
    }
}

/// Una regola come sta nel file, compreso il vecchio campo `target`.
#[derive(Deserialize)]
struct RuleFile {
    #[serde(default)]
    name: String,
    #[serde(default = "yes")]
    enabled: bool,
    scope: RuleScope,
    #[serde(default, deserialize_with = "plain_some")]
    goal: Option<Target>,
    #[serde(default, deserialize_with = "plain_some")]
    target: Option<MilestoneId>,
    anchor: Anchor,
    #[serde(default)]
    offset_weeks: i32,
}

/// Campo facoltativo scritto come valore semplice (non `Some(…)`).
fn plain_some<'de, D: serde::Deserializer<'de>, T: Deserialize<'de>>(d: D) -> Result<Option<T>, D::Error> {
    T::deserialize(d).map(Some)
}

impl From<RuleFile> for Rule {
    fn from(f: RuleFile) -> Self {
        // Senza nessuno dei due campi (file rovinato): la fine del progetto,
        // la regola si vede e si corregge nella finestra.
        let target = f.goal.or(f.target.map(Target::Milestone)).unwrap_or(Target::ProjectEnd);
        Rule {
            name: f.name,
            enabled: f.enabled,
            scope: f.scope,
            target,
            anchor: f.anchor,
            offset_weeks: f.offset_weeks,
        }
    }
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
            .filter(|(_, r)| r.target == Target::Milestone(m) || r.anchor == Anchor::Milestone(m))
            .map(|(id, _)| *id)
            .collect()
    }

    /// Regole che usano il dev (come ancora, condizione o bersaglio).
    pub fn using_dev(&self, d: DevId) -> Vec<RuleId> {
        self.rules
            .iter()
            .filter(|(_, r)| {
                matches!(r.anchor, Anchor::DevStart(x) | Anchor::DevEnd(x) | Anchor::DevPresent(x) if x == d)
                    || matches!(r.target, Target::AddDev(x) | Target::DevStart(x) if x == d)
            })
            .map(|(id, _)| *id)
            .collect()
    }
}

/// Il bersaglio ha senso nel progetto? Le date sempre; una milestone se
/// esiste ed è di sistema o di quel progetto.
fn placeable(app: &App, proj: ProjectId, t: Target) -> bool {
    match t {
        Target::Milestone(m) => app.milestones.available_for(m, proj),
        Target::ProjectStart | Target::ProjectEnd => true,
        Target::AddDev(d) | Target::DevStart(d) => app.devs.get(d).is_some(),
    }
}

/// Per ogni bersaglio del progetto governato da una regola, la regola che lo
/// governa: fra le attive con ambito che comprende il progetto e bersaglio
/// sensato lì, la più specifica (a parità, la creata prima).
pub fn governing(app: &App, proj: ProjectId) -> BTreeMap<Target, RuleId> {
    let mut out: BTreeMap<Target, (u8, RuleId)> = BTreeMap::new();
    for (id, r) in app.rules.list() {
        if !r.enabled
            || r.target.kind() != RuleKind::Aligned
            || !anchor_fits(r.target, r.anchor)
            || !r.scope.matches(app, proj)
            || !placeable(app, proj, r.target)
        {
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
    out.into_iter().map(|(t, (_, id))| (t, id)).collect()
}

/// La regola che governa il bersaglio `t` nel progetto, se c'è (la griglia
/// non lo fa modificare a mano).
pub fn governing_rule(app: &App, proj: ProjectId, t: Target) -> Option<RuleId> {
    governing(app, proj).get(&t).copied()
}

/// Valore attuale (senza regole) di un bersaglio nel progetto.
fn current(app: &App, proj: ProjectId, t: Target) -> Option<WeekId> {
    match t {
        Target::Milestone(m) => app
            .projects
            .list_project_milestones(proj)
            .into_iter()
            .find(|(x, _)| *x == m)
            .map(|(_, w)| w),
        Target::ProjectStart => app.projects.get_project_start_week(proj),
        Target::ProjectEnd => app.projects.get_project_end_week(proj),
        Target::DevStart(d) => app.projects.get_single_dev(proj, d).and_then(|sd| sd.effort_span()).map(|s| s.0),
        Target::AddDev(_) => None,
    }
}

/// Prima / ultima settimana con effort di tutti i dev del progetto.
fn effort_bounds(app: &App, proj: ProjectId) -> Option<(WeekId, WeekId)> {
    app.projects
        .list_devs(proj)
        .into_iter()
        .filter_map(|d| app.projects.get_single_dev(proj, d)?.effort_span())
        .reduce(|a, b| (a.0.min(b.0), a.1.max(b.1)))
}

/// Settimana dell'ancora spostata di `weeks` settimane (mai prima del giorno 0).
fn shifted(w: WeekId, weeks: i32) -> WeekId {
    WeekId((w.0 as i64 + 7 * weeks as i64).max(0) as usize)
}

/// Esito del calcolo delle regole di un progetto.
#[derive(Debug, Default, PartialEq)]
pub struct Evaluation {
    /// Per ogni bersaglio governato e risolto: dove deve stare (`None` =
    /// ancora mancante: la milestone non c'è, la data resta com'è).
    pub values: BTreeMap<Target, Option<WeekId>>,
    /// Regole ferme perché in un ciclo o dipendenti da un ciclo.
    pub blocked: BTreeSet<RuleId>,
}

#[derive(Clone, Copy)]
enum St {
    Visiting,
    Done(Option<WeekId>),
    Failed,
}

/// Contesto del calcolo di un progetto.
struct Ctx<'a> {
    app: &'a App,
    proj: ProjectId,
    gov: BTreeMap<Target, RuleId>,
    memo: HashMap<Target, St>,
}

impl Ctx<'_> {
    /// Valore del bersaglio: calcolato dalla sua regola se è governato,
    /// altrimenti quello attuale. `Err` se la catena incontra un ciclo.
    fn resolve(&mut self, t: Target) -> Result<Option<WeekId>, ()> {
        let Some(&rid) = self.gov.get(&t) else {
            return Ok(current(self.app, self.proj, t));
        };
        match self.memo.get(&t) {
            Some(St::Done(v)) => return Ok(*v),
            Some(St::Visiting) | Some(St::Failed) => {
                self.memo.insert(t, St::Failed);
                return Err(());
            }
            None => {}
        }
        self.memo.insert(t, St::Visiting);
        let rule = self.app.rules.get(rid).expect("regola governante esistente");
        let (anchor, offset) = (rule.anchor, rule.offset_weeks);
        match self.anchor(anchor, t) {
            Ok(v) => {
                let v = v.map(|w| shifted(w, offset));
                // Data con l'ancora mancante: resta quella attuale.
                let v = if v.is_none() && t.milestone().is_none() { current(self.app, self.proj, t) } else { v };
                self.memo.insert(t, St::Done(v));
                Ok(v)
            }
            Err(()) => {
                self.memo.insert(t, St::Failed);
                Err(())
            }
        }
    }

    /// Valore di un'ancora per il bersaglio `own` (escluso dalle ancore
    /// «prima / ultima milestone»).
    fn anchor(&mut self, a: Anchor, own: Target) -> Result<Option<WeekId>, ()> {
        let (app, proj) = (self.app, self.proj);
        Ok(match a {
            Anchor::DevStart(d) => app.projects.get_single_dev(proj, d).and_then(|sd| sd.effort_span()).map(|s| s.0),
            Anchor::DevEnd(d) => app.projects.get_single_dev(proj, d).and_then(|sd| sd.effort_span()).map(|s| s.1),
            Anchor::DevPresent(_) => None,
            Anchor::FirstEffort => effort_bounds(app, proj).map(|b| b.0),
            Anchor::LastEffort => effort_bounds(app, proj).map(|b| b.1),
            Anchor::ProjectStart => self.resolve(Target::ProjectStart)?,
            Anchor::ProjectEnd => self.resolve(Target::ProjectEnd)?,
            Anchor::Milestone(m) => self.resolve(Target::Milestone(m))?,
            Anchor::FirstMilestone | Anchor::LastMilestone => {
                // Le milestone collocate più quelle governate (che possono
                // comparire o spostarsi), tranne il bersaglio stesso.
                let mut ms: BTreeSet<MilestoneId> =
                    app.projects.list_project_milestones(proj).into_iter().map(|(m, _)| m).collect();
                ms.extend(self.gov.keys().filter_map(|t| t.milestone()));
                if let Some(m) = own.milestone() {
                    ms.remove(&m);
                }
                let mut weeks = Vec::new();
                for m in ms {
                    if let Some(w) = self.resolve(Target::Milestone(m))? {
                        weeks.push(w);
                    }
                }
                if a == Anchor::FirstMilestone { weeks.into_iter().min() } else { weeks.into_iter().max() }
            }
        })
    }
}

/// Calcola i bersagli governati del progetto, risolvendo le catene.
pub fn evaluate(app: &App, proj: ProjectId) -> Evaluation {
    let gov = governing(app, proj);
    let mut ctx = Ctx { app, proj, gov: gov.clone(), memo: HashMap::new() };
    let mut out = Evaluation::default();
    for (&t, &rid) in &gov {
        match ctx.resolve(t) {
            Ok(v) => {
                out.values.insert(t, v);
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
    /// Qualche milestone o data è stata messa, spostata o tolta.
    pub changed: bool,
    /// Regole ferme per un ciclo, in almeno un progetto.
    pub blocked: BTreeSet<RuleId>,
}

/// Applica tutte le regole a tutti i progetti (aperti e chiusi): ogni
/// bersaglio governato va nella settimana calcolata; una milestone senza
/// ancora viene tolta, una data senza ancora resta. Le regole bloccate non
/// toccano nulla. Le celle di effort non cambiano mai.
pub fn apply(app: &mut App) -> Outcome {
    let mut out = Outcome::default();
    if app.rules.is_empty() {
        return out;
    }
    let projects: Vec<ProjectId> = app.projects.list().into_iter().map(|(p, _)| p).collect();
    // Prima i dev da aggiungere (un dev aggiunto può far scattare un'altra
    // regola, quindi fino a che non cambia più nulla): non hanno ore, quindi
    // non toccano le ancore delle milestone e delle date.
    loop {
        let mut added = false;
        for &proj in &projects {
            for d in required_devs(app, proj).into_keys() {
                if !app.projects.list_devs(proj).contains(&d) {
                    app.projects.add_dev(proj, d);
                    added = true;
                }
            }
        }
        if !added {
            break;
        }
        out.changed = true;
    }
    for proj in projects {
        let ev = evaluate(app, proj);
        out.blocked.extend(ev.blocked);
        for (t, v) in ev.values {
            let cur = current(app, proj, t);
            if cur == v {
                continue;
            }
            match (t, v) {
                (Target::Milestone(m), Some(w)) => app.projects.add_project_milestone(proj, m, w),
                (Target::Milestone(m), None) => app.projects.remove_project_milestone(proj, m),
                (Target::ProjectStart, Some(w)) => app.projects.set_project_start_week(proj, Some(w)),
                (Target::ProjectEnd, Some(w)) => app.projects.set_project_end_week(proj, Some(w)),
                _ => continue,
            }
            out.changed = true;
        }
    }
    out
}

/// Dev che le regole «Aggiungi dev» vogliono nel progetto (attive, con
/// l'ambito che lo comprende e la condizione vera), con la prima regola che
/// lo chiede: la finestra dei dev non li fa togliere.
pub fn required_devs(app: &App, proj: ProjectId) -> BTreeMap<DevId, RuleId> {
    let devs = app.projects.list_devs(proj);
    let mut out = BTreeMap::new();
    for (id, r) in app.rules.list() {
        if let (Target::AddDev(b), Anchor::DevPresent(a)) = (r.target, r.anchor)
            && r.enabled
            && a != b
            && app.devs.get(b).is_some()
            && r.scope.matches(app, proj)
            && devs.contains(&a)
        {
            out.entry(b).or_insert(id);
        }
    }
    out
}

/// Uno spostamento delle ore di un dev proposto da una regola «Inizio ore
/// dev» (fase 3, su comando).
#[derive(Debug, Clone, PartialEq)]
pub struct DevMove {
    pub proj: ProjectId,
    pub dev: DevId,
    /// Prima settimana con ore adesso e dopo lo spostamento.
    pub from: WeekId,
    pub to: WeekId,
    /// Dopo lo spostamento qualche ora cade fuori dal periodo del progetto
    /// (resta comunque: non si perde nulla).
    pub outside: bool,
}

impl DevMove {
    /// Spostamento in settimane (positivo = più tardi).
    pub fn weeks(&self) -> i64 {
        (self.to.0 as i64 - self.from.0 as i64) / 7
    }
}

/// Gli spostamenti che la regola farebbe adesso: nei progetti dell'ambito in
/// cui il dev ha ore e l'ancora c'è, quelli in cui la prima settimana del dev
/// non è già ancora + N. Vuoto per le regole di altro tipo.
pub fn pending_moves(app: &App, id: RuleId) -> Vec<DevMove> {
    let Some(r) = app.rules.get(id) else {
        return Vec::new();
    };
    let Target::DevStart(dev) = r.target else {
        return Vec::new();
    };
    if !anchor_fits(r.target, r.anchor) {
        return Vec::new();
    }
    let mut out = Vec::new();
    for (proj, _) in app.projects.list() {
        if !r.scope.matches(app, proj) {
            continue;
        }
        let Some((first, last)) = app.projects.get_single_dev(proj, dev).and_then(|sd| sd.effort_span()) else {
            continue;
        };
        let mut ctx = Ctx { app, proj, gov: governing(app, proj), memo: HashMap::new() };
        let Ok(Some(anchor)) = ctx.anchor(r.anchor, r.target) else {
            continue;
        };
        let to = shifted(anchor, r.offset_weeks);
        if to == first {
            continue;
        }
        let delta = to.0 as i64 - first.0 as i64;
        let new_last = last.0 as i64 + delta;
        let start = app.projects.get_project_start_week(proj);
        let end = app.projects.get_project_end_week(proj);
        let outside = start.is_some_and(|s| (to.0 as i64) < s.0 as i64) || end.is_some_and(|e| new_last > e.0 as i64);
        out.push(DevMove { proj, dev, from: first, to, outside });
    }
    out
}

/// Esegue gli spostamenti (tutte le ore del dev in blocco); ritorna quanti.
pub fn apply_moves(app: &mut App, moves: &[DevMove]) -> usize {
    moves
        .iter()
        .filter(|m| app.projects.shift_dev(m.proj, m.dev, m.to.0 as i64 - m.from.0 as i64))
        .count()
}

/// Nome della regola, o una descrizione se non ne ha («Regola 3»).
pub fn rule_label(app: &App, id: RuleId) -> String {
    match app.rules.get(id) {
        Some(r) if !r.name.trim().is_empty() => r.name.trim().to_string(),
        _ => format!("Regola {}", id.0),
    }
}

/// Situazione di una regola, per la finestra: in quanti progetti governa il
/// suo bersaglio, in quanti l'ha posizionato, se è ferma per un ciclo e i
/// riferimenti che non esistono più.
#[derive(Debug, Default, PartialEq)]
pub struct RuleStatus {
    pub governed: usize,
    /// Allineate: progetti in cui il bersaglio è posizionato. «Aggiungi
    /// dev»: progetti in cui la condizione vale. Spostamento: progetti da
    /// spostare adesso.
    pub placed: usize,
    pub blocked: bool,
    pub missing: Vec<&'static str>,
    /// L'ancora non va bene per il bersaglio (`anchor_fits`).
    pub bad_anchor: bool,
}

pub fn status(app: &App, id: RuleId) -> RuleStatus {
    let mut st = RuleStatus::default();
    let Some(r) = app.rules.get(id) else {
        return st;
    };
    match r.target {
        Target::Milestone(m) if app.milestones.get_name(m).is_none() => st.missing.push("milestone"),
        Target::AddDev(d) | Target::DevStart(d) if app.devs.get(d).is_none() => st.missing.push("dev"),
        _ => {}
    }
    st.bad_anchor = !anchor_fits(r.target, r.anchor);
    match r.anchor {
        Anchor::DevStart(d) | Anchor::DevEnd(d) | Anchor::DevPresent(d) if app.devs.get(d).is_none() => {
            st.missing.push("dev di partenza")
        }
        Anchor::Milestone(m) if app.milestones.get_name(m).is_none() => st.missing.push("milestone di partenza"),
        _ => {}
    }
    match r.scope {
        RuleScope::Category(c) if app.categories.get_name(c).is_none() => st.missing.push("categoria"),
        RuleScope::Priority(p) if app.priorities.get_name(p).is_none() => st.missing.push("priorità"),
        RuleScope::Project(p) if app.projects.get(p).is_none() => st.missing.push("progetto"),
        _ => {}
    }
    match r.target.kind() {
        RuleKind::Aligned => {}
        RuleKind::AddDev => {
            for (proj, _) in app.projects.list() {
                if r.enabled && r.scope.matches(app, proj) {
                    st.governed += 1;
                    if required_devs(app, proj).get(&r.target_dev()) == Some(&id) {
                        st.placed += 1;
                    }
                }
            }
            return st;
        }
        RuleKind::MoveDev => {
            st.placed = pending_moves(app, id).len();
            return st;
        }
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
        date_rule(scope, Target::Milestone(target), anchor, offset_weeks)
    }

    fn date_rule(scope: RuleScope, target: Target, anchor: Anchor, offset_weeks: i32) -> Rule {
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
        assert_eq!(governing_rule(&f.app, f.p, Target::Milestone(f.demo)), Some(demo));
        assert_eq!(governing_rule(&f.app, f.p, Target::Milestone(f.rilascio)), None);
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
        assert_eq!(governing_rule(&f.app, f.p, Target::Milestone(f.collaudo)), Some(pri));
        let catr = f.app.rules.add(rule(RuleScope::Category(cat), f.collaudo, Anchor::DevEnd(f.be), 3));
        assert_eq!(governing_rule(&f.app, f.p, Target::Milestone(f.collaudo)), Some(catr));
        let projr = f.app.rules.add(rule(RuleScope::Project(f.p), f.collaudo, Anchor::DevEnd(f.be), 4));
        assert_eq!(governing_rule(&f.app, f.p, Target::Milestone(f.collaudo)), Some(projr));
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
        assert_eq!(status(&f.app, r).missing, ["milestone", "dev di partenza", "progetto"]);
        assert_eq!(f.app.rules.using_dev(f.be).len(), 1);
        assert_eq!(f.app.rules.using_milestone(own).len(), 1);
    }

    /// Fase 2: «inizio progetto = primo effort», «fine progetto = ultima
    /// milestone + 1»; la fine segue la milestone governata; ancora mancante
    /// = la data resta com'è.
    #[test]
    fn project_dates_follow_efforts_and_milestones() {
        let mut f = fx();
        f.app.rules.add(date_rule(RuleScope::All, Target::ProjectStart, Anchor::FirstEffort, 0));
        f.app.rules.add(rule(RuleScope::All, f.collaudo, Anchor::LastEffort, 2));
        f.app.rules.add(date_rule(RuleScope::All, Target::ProjectEnd, Anchor::LastMilestone, 1));
        assert!(apply(&mut f.app).changed);
        let last = f.base + 7 * 5; // ultima settimana del Frontend
        assert_eq!(f.app.projects.get_project_start_week(f.p), Some(WeekId(f.base)));
        assert_eq!(at(&f.app, f.p, f.collaudo), Some(WeekId(last + 14)));
        assert_eq!(f.app.projects.get_project_end_week(f.p), Some(WeekId(last + 21)));
        // Una milestone messa a mano più tardi sposta la fine.
        f.app.projects.add_project_milestone(f.p, f.demo, WeekId(last + 70));
        apply(&mut f.app);
        assert_eq!(f.app.projects.get_project_end_week(f.p), Some(WeekId(last + 77)));
        assert_eq!(f.app.projects.get_project_start_week(f.p), Some(WeekId(f.base)));
        // Niente effort: Collaudo sparisce, la fine segue la Demo, l'inizio resta.
        f.app.projects.clear_efforts(f.p, None);
        apply(&mut f.app);
        assert_eq!(at(&f.app, f.p, f.collaudo), None);
        assert_eq!(f.app.projects.get_project_start_week(f.p), Some(WeekId(f.base)), "ancora mancante: data invariata");
        assert_eq!(f.app.projects.get_project_end_week(f.p), Some(WeekId(last + 77)));
        assert_eq!(governing_rule(&f.app, f.p, Target::ProjectEnd).map(|r| r.0), Some(3));
    }

    /// «Prima / ultima milestone» escludono il bersaglio stesso; una data che
    /// dipende da una milestone che dipende da quella data è un ciclo.
    #[test]
    fn first_last_milestone_exclude_the_target_and_date_cycles_are_blocked() {
        let mut f = fx();
        f.app.projects.add_project_milestone(f.p, f.demo, WeekId(f.base + 70));
        f.app.rules.add(rule(RuleScope::All, f.rilascio, Anchor::LastMilestone, 1));
        apply(&mut f.app);
        assert_eq!(at(&f.app, f.p, f.rilascio), Some(WeekId(f.base + 77)), "dopo la Demo, non dopo sé stessa");
        let a = f.app.rules.add(rule(RuleScope::All, f.collaudo, Anchor::ProjectEnd, -1));
        let b = f.app.rules.add(date_rule(RuleScope::All, Target::ProjectEnd, Anchor::LastMilestone, 1));
        let out = apply(&mut f.app);
        assert!(out.blocked.contains(&a) && out.blocked.contains(&b), "{:?}", out.blocked);
        assert_eq!(f.app.projects.get_project_end_week(f.p), None, "ferma: la data non cambia");
    }

    /// Le regole scritte prima della fase 2 (`target: (id)`) si rileggono
    /// come milestone; quelle nuove si salvano con `goal`.
    #[test]
    fn old_rules_with_a_milestone_target_still_load() {
        let mut f = fx();
        f.app.rules.add(rule(RuleScope::All, f.collaudo, Anchor::DevEnd(f.be), 2));
        f.app.rules.add(date_rule(RuleScope::All, Target::ProjectEnd, Anchor::LastEffort, 0));
        let ron = f.app.to_ron_string();
        let new_form = format!("goal: Milestone(({}))", f.collaudo.0);
        assert!(ron.contains(&new_form), "{ron}");
        let old = ron.replace(&new_form, &format!("target: ({})", f.collaudo.0));
        let back = App::from_ron_str(&old).expect("formato vecchio");
        assert_eq!(back.rules, f.app.rules);
        assert_eq!(App::from_ron_str(&ron).expect("formato nuovo").rules, f.app.rules);
    }

    /// Fase 3: «Aggiungi dev Test se c'è il dev Backend» aggiunge Test dove
    /// c'è il Backend (anche a catena), non lo toglie quando il Backend se ne
    /// va; spenta = niente.
    #[test]
    fn add_dev_rules_add_and_never_remove() {
        let mut f = fx();
        let test = f.app.devs.add("Test");
        let doc = f.app.devs.add("Doc");
        let q = f.app.projects.add("y", Some("XYZ"), None);
        f.app.projects.add_dev(q, f.fe);
        let r = f.app.rules.add(date_rule(RuleScope::All, Target::AddDev(test), Anchor::DevPresent(f.be), 0));
        f.app.rules.add(date_rule(RuleScope::All, Target::AddDev(doc), Anchor::DevPresent(test), 0));
        assert!(apply(&mut f.app).changed);
        let devs = |app: &App, p| app.projects.list_devs(p);
        assert!(devs(&f.app, f.p).contains(&test) && devs(&f.app, f.p).contains(&doc), "anche a catena");
        assert!(!devs(&f.app, q).contains(&test), "XYZ non ha il Backend");
        assert_eq!(required_devs(&f.app, f.p).get(&test), Some(&r));
        assert_eq!(status(&f.app, r).placed, 1);
        assert!(!apply(&mut f.app).changed);
        // Via il Backend: Test resta.
        f.app.projects.del_dev(f.p, f.be);
        apply(&mut f.app);
        assert!(devs(&f.app, f.p).contains(&test));
        assert!(!required_devs(&f.app, f.p).contains_key(&test));
        // Regola spenta: non aggiunge.
        f.app.projects.add_dev(q, f.be);
        let mut off = f.app.rules.get(r).unwrap().clone();
        off.enabled = false;
        f.app.rules.update(r, off);
        apply(&mut f.app);
        assert!(!devs(&f.app, q).contains(&test));
    }

    /// Ancore adatte: «Aggiungi dev» solo con «Se c'è il dev» (non sé stesso);
    /// le ore di un dev non contano da sé stesse né dal primo / ultimo effort.
    #[test]
    fn anchors_fit_their_targets() {
        let (a, b) = (DevId(1), DevId(2));
        assert!(anchor_fits(Target::AddDev(b), Anchor::DevPresent(a)));
        assert!(!anchor_fits(Target::AddDev(b), Anchor::DevPresent(b)));
        assert!(!anchor_fits(Target::AddDev(b), Anchor::DevEnd(a)));
        assert!(!anchor_fits(Target::ProjectEnd, Anchor::DevPresent(a)));
        assert!(anchor_fits(Target::DevStart(b), Anchor::DevEnd(a)));
        assert!(!anchor_fits(Target::DevStart(b), Anchor::DevEnd(b)));
        assert!(!anchor_fits(Target::DevStart(b), Anchor::LastEffort));
        assert!(anchor_fits(Target::DevStart(b), Anchor::ProjectEnd));
        assert!(anchor_fits(Target::Milestone(MilestoneId(1)), Anchor::LastEffort));
    }

    /// Fase 3: «Ore del dev Frontend = fine Backend + 2» propone lo
    /// spostamento in blocco e lo fa solo quando richiesto; poi è allineata.
    /// Fuori dal periodo del progetto le ore restano (segnalate).
    #[test]
    fn dev_moves_are_proposed_and_applied_on_demand() {
        let mut f = fx();
        f.app.projects.set_project_end_week(f.p, Some(WeekId(f.base + 7 * 7)));
        let r = f.app.rules.add(date_rule(RuleScope::All, Target::DevStart(f.fe), Anchor::DevEnd(f.be), 2));
        assert!(!apply(&mut f.app).changed, "non si applica da sola");
        let moves = pending_moves(&f.app, r);
        let be_end = f.base + 7 * 3;
        assert_eq!(moves.len(), 1);
        let m = &moves[0];
        assert_eq!((m.from, m.to), (WeekId(f.base + 14), WeekId(be_end + 14)));
        assert_eq!(m.weeks(), 3);
        assert!(m.outside, "il Frontend finirebbe alla settimana 8, la fine è la 7");
        assert_eq!(status(&f.app, r).placed, 1);
        let hours = |app: &App| app.projects.get_single_dev(f.p, f.fe).unwrap().effort_span();
        assert_eq!(apply_moves(&mut f.app, &moves), 1);
        assert_eq!(hours(&f.app), Some((WeekId(be_end + 14), WeekId(be_end + 14 + 21))));
        let total: usize = f.app.projects.get_single_dev(f.p, f.fe).unwrap().get_weeks().len();
        assert_eq!(total, 4, "nessuna settimana persa, anche fuori dal periodo");
        assert!(pending_moves(&f.app, r).is_empty(), "allineata");
    }
}
