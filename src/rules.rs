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
//! finestra la segnala. Fase 3: aggiunta di dev (sempre) e spostamento delle
//! ore di un dev (su comando). Fase 4: avvisi, che non cambiano nulla.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use serde::{Deserialize, Serialize};

use crate::app::App;
use crate::categories::CategoryId;
use crate::dev_utils::dev::DevId;
use crate::milestones::MilestoneId;
use crate::priorities::PriorityId;
use crate::project_utils::project::{ProjectId, ProjectStatus};
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
    /// Fase 4: avviso. Il punto (un'ancora qualunque) non deve stare prima /
    /// dopo l'ancora della regola ± N; non cambia mai nulla, segnala.
    Check(Anchor, Cmp),
}

/// Confronto di una regola d'avviso.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Cmp {
    /// Il punto deve essere dalla soglia in poi (avviso se prima).
    NotBefore,
    /// Il punto deve essere entro la soglia (avviso se dopo).
    NotAfter,
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
    /// Avviso: non cambia nulla (`warnings`).
    Warning,
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
            Target::Check(..) => RuleKind::Warning,
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
        (Target::Check(Anchor::DevPresent(_), _), _) => false,
        (Target::DevStart(b), Anchor::DevStart(a) | Anchor::DevEnd(a)) => a != b,
        (Target::DevStart(_), Anchor::FirstEffort | Anchor::LastEffort) => false,
        _ => true,
    }
}


/// Da dove si conta (la settimana di partenza della regola).
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
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
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum RuleScope {
    All,
    Category(CategoryId),
    Priority(PriorityId),
    /// Un solo progetto (le regole scritte prima della scelta multipla).
    Project(ProjectId),
    /// I progetti scelti (uno o più).
    Projects(Vec<ProjectId>),
    /// Tutti i progetti tranne quelli elencati.
    AllExcept(Vec<ProjectId>),
}

impl RuleScope {
    /// Il progetto è nell'ambito?
    pub fn matches(&self, app: &App, proj: ProjectId) -> bool {
        match self {
            RuleScope::All => true,
            RuleScope::Category(c) => app.projects.get_category(proj) == Some(*c),
            RuleScope::Priority(p) => app.projects.get_priority(proj) == Some(*p),
            RuleScope::Project(p) => *p == proj,
            RuleScope::Projects(ps) => ps.contains(&proj),
            RuleScope::AllExcept(ps) => !ps.contains(&proj),
        }
    }

    /// Specificità: vince il valore più alto (progetti scelti > categoria >
    /// priorità > tutti tranne … > tutti).
    pub fn rank(&self) -> u8 {
        match self {
            RuleScope::All => 0,
            RuleScope::AllExcept(_) => 1,
            RuleScope::Priority(_) => 2,
            RuleScope::Category(_) => 3,
            RuleScope::Project(_) | RuleScope::Projects(_) => 4,
        }
    }

    /// I progetti esclusi (`AllExcept`); vuoto per gli altri ambiti.
    pub fn excluded(&self) -> Vec<ProjectId> {
        match self {
            RuleScope::AllExcept(ps) => ps.clone(),
            _ => Vec::new(),
        }
    }

    /// I progetti scelti (`Project` / `Projects`); vuoto per gli altri ambiti.
    pub fn projects(&self) -> Vec<ProjectId> {
        match self {
            RuleScope::Project(p) => vec![*p],
            RuleScope::Projects(ps) => ps.clone(),
            _ => Vec::new(),
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
    /// Progetti non attivi a cui la regola vale lo stesso (di norma nessuno:
    /// solo i progetti **attivi**).
    #[serde(default, skip_serializing_if = "Inactive::is_none")]
    pub inactive: Inactive,
    /// Solo per gli avvisi: punti e ancore in più (ogni coppia punto ×
    /// ancora si controlla) e la spunta «Blocca».
    #[serde(default, skip_serializing_if = "CheckOpts::is_default")]
    pub check: CheckOpts,
}

/// Opzioni di una regola d'avviso. Il primo punto sta in `Target::Check`, la
/// prima ancora in `Rule::anchor`; qui gli **altri**, così una regola dice
/// «dev1, dev2 non prima di dev3, dev4, Milestone1» e controlla tutte le
/// coppie (`Rule::points` × `Rule::anchors`). Con `block` una modifica che
/// crea o peggiora una violazione viene annullata (`block_regression`).
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
pub struct CheckOpts {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub points: Vec<Anchor>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub anchors: Vec<Anchor>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub block: bool,
}

impl CheckOpts {
    pub fn is_default(&self) -> bool {
        *self == CheckOpts::default()
    }
}

fn is_false(b: &bool) -> bool {
    !*b
}

/// Stati di progetto, oltre ad «Attivo», a cui una regola vale. Di norma
/// nessuno: una regola non tocca (né controlla) i progetti on-hold o chiusi,
/// che altrimenti si vedrebbero spostare milestone e date già consegnate.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Inactive {
    #[serde(default)]
    pub on_hold: bool,
    #[serde(default)]
    pub closed: bool,
}

impl Inactive {
    /// Solo progetti attivi.
    pub fn is_none(&self) -> bool {
        !self.on_hold && !self.closed
    }

    /// La regola vale per un progetto in questo stato?
    pub fn allows(self, s: ProjectStatus) -> bool {
        match s {
            ProjectStatus::Active => true,
            ProjectStatus::OnHold => self.on_hold,
            ProjectStatus::Closed => self.closed,
        }
    }
}

impl Rule {
    /// Punti controllati da un avviso (il primo e gli altri, senza
    /// doppioni); vuoto per le altre regole.
    pub fn points(&self) -> Vec<Anchor> {
        let Target::Check(p, _) = self.target else {
            return Vec::new();
        };
        let mut out = vec![p];
        for x in &self.check.points {
            if !out.contains(x) {
                out.push(*x);
            }
        }
        out
    }

    /// Ancore della regola: la principale e, per un avviso, le altre (senza
    /// doppioni).
    pub fn anchors(&self) -> Vec<Anchor> {
        let mut out = vec![self.anchor];
        if matches!(self.target, Target::Check(..)) {
            for x in &self.check.anchors {
                if !out.contains(x) {
                    out.push(*x);
                }
            }
        }
        out
    }

    /// Ancore (e punti) adatti al bersaglio (`anchor_fits`), anche quelli in
    /// più di un avviso.
    pub fn fits(&self) -> bool {
        self.anchors().iter().all(|a| anchor_fits(self.target, *a))
            && self.points().iter().all(|p| !matches!(p, Anchor::DevPresent(_)))
    }

    /// Avviso con la spunta «Blocca».
    pub fn blocks(&self) -> bool {
        self.check.block && matches!(self.target, Target::Check(..))
    }

    /// La regola vale per il progetto: è nell'ambito e il suo stato è fra
    /// quelli ammessi (`inactive`).
    pub fn applies_to(&self, app: &App, proj: ProjectId) -> bool {
        self.scope.matches(app, proj) && self.inactive.allows(app.projects.status(proj))
    }

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
    #[serde(default)]
    inactive: Inactive,
    #[serde(default)]
    check: CheckOpts,
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
            inactive: f.inactive,
            check: f.check,
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
            .filter(|(_, r)| {
                r.target == Target::Milestone(m)
                    || r.anchors().contains(&Anchor::Milestone(m))
                    || r.points().contains(&Anchor::Milestone(m))
            })
            .map(|(id, _)| *id)
            .collect()
    }

    /// Regole che usano il dev (come ancora, condizione o bersaglio).
    pub fn using_dev(&self, d: DevId) -> Vec<RuleId> {
        self.rules
            .iter()
            .filter(|(_, r)| {
                let uses = |a: &Anchor| matches!(a, Anchor::DevStart(x) | Anchor::DevEnd(x) | Anchor::DevPresent(x) if *x == d);
                r.anchors().iter().any(uses)
                    || r.points().iter().any(uses)
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
        Target::Check(..) => true,
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
            || !r.fits()
            || !r.applies_to(app, proj)
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
        Target::AddDev(_) | Target::Check(..) => None,
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

/// Una modifica che una regola farebbe a un progetto (anteprima
/// dell'attivazione, `preview_changes`).
#[derive(Debug, Clone, PartialEq)]
pub struct RuleChange {
    pub proj: ProjectId,
    pub text: String,
}

/// Cosa cambierebbe nei progetti mettendo `rule` al posto della regola `id`
/// (o aggiungendola, se `id` non c'è): le regole si applicano a due copie dei
/// dati, prima e dopo, e si confrontano dev, date e milestone. Vuoto se non
/// cambia nulla (anche per avvisi e spostamenti di ore, che non agiscono da
/// soli).
pub fn preview_changes(app: &App, id: RuleId, rule: &Rule) -> Vec<RuleChange> {
    preview_with(app, |rules| {
        if !rules.update(id, rule.clone()) && rules.get(id).is_none() {
            rules.add(rule.clone());
        }
    })
}

/// Cosa cambierebbe nei progetti aggiungendo le regole (anteprima
/// dell'importazione da testo).
pub fn preview_added(app: &App, added: &[Rule]) -> Vec<RuleChange> {
    preview_with(app, |rules| {
        for r in added {
            rules.add(r.clone());
        }
    })
}

/// Applica le regole a due copie dei dati, prima e dopo `edit` delle regole,
/// e confronta.
fn preview_with(app: &App, edit: impl FnOnce(&mut Rules)) -> Vec<RuleChange> {
    let mut before = app.clone();
    apply(&mut before);
    let mut after = app.clone();
    edit(&mut after.rules);
    apply(&mut after);
    changes_between(&before, &after)
}

/// Una milestone collocata da una regola che, dopo una modifica (spenta,
/// eliminata, ambito ristretto, altro bersaglio…), non sarebbe più governata
/// da nessuna regola: resterebbe dov'è, da spostare a mano.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Orphan {
    pub proj: ProjectId,
    pub milestone: MilestoneId,
    pub week: WeekId,
}

/// Le milestone che la regola `id` governa e ha collocato e che, mettendo
/// `after` al suo posto (`None` = regola eliminata), nessuna regola
/// governerebbe più. Una milestone che passa a un'altra regola non è
/// orfana (quella la tiene allineata). Le date del progetto e i dev aggiunti
/// non sono qui: restano comunque.
pub fn orphans(app: &App, id: RuleId, after: Option<&Rule>) -> Vec<Orphan> {
    let mut next = app.rules.clone();
    match after {
        Some(r) => {
            next.update(id, r.clone());
        }
        None => {
            next.remove(id);
        }
    }
    let mut after_app: Option<App> = None;
    let mut out = Vec::new();
    for (proj, _) in app.projects.list() {
        let placed: BTreeMap<MilestoneId, WeekId> = app.projects.list_project_milestones(proj).into_iter().collect();
        for (t, rid) in governing(app, proj) {
            let Target::Milestone(m) = t else { continue };
            let Some(&week) = placed.get(&m) else { continue };
            if rid != id {
                continue;
            }
            // Le regole dopo la modifica, sugli stessi dati: chi governa?
            let a = after_app.get_or_insert_with(|| {
                let mut a = app.clone();
                a.rules = next.clone();
                a
            });
            if !governing(a, proj).contains_key(&t) {
                out.push(Orphan { proj, milestone: m, week });
            }
        }
    }
    out
}

/// Differenze di dev, date e milestone fra due versioni degli stessi
/// progetti, in ordine di griglia.
fn changes_between(before: &App, after: &App) -> Vec<RuleChange> {
    let date = |w: Option<WeekId>| w.map_or_else(|| "—".to_string(), |w| crate::date_utils::dates::week_label(w.0 as i32));
    let ms_name = |m: MilestoneId| after.milestones.get_name(m).unwrap_or("?").to_string();
    let mut out = Vec::new();
    for (proj, _) in after.projects.list() {
        let mut push = |text: String| out.push(RuleChange { proj, text });
        let old_devs = before.projects.list_devs(proj);
        for d in after.projects.list_devs(proj) {
            if !old_devs.contains(&d) {
                let name = after.devs.get(d).map(|x| x.get_name()).unwrap_or_else(|| "?".into());
                push(format!("aggiunto il dev {name}"));
            }
        }
        for (label, b, a) in [
            ("inizio", before.projects.get_project_start_week(proj), after.projects.get_project_start_week(proj)),
            ("fine", before.projects.get_project_end_week(proj), after.projects.get_project_end_week(proj)),
        ] {
            if a != b {
                push(format!("{label} progetto: {} → {}", date(b), date(a)));
            }
        }
        let old: BTreeMap<MilestoneId, WeekId> = before.projects.list_project_milestones(proj).into_iter().collect();
        let new: BTreeMap<MilestoneId, WeekId> = after.projects.list_project_milestones(proj).into_iter().collect();
        for (m, w) in &new {
            match old.get(m) {
                None => push(format!("milestone {} collocata il {}", ms_name(*m), date(Some(*w)))),
                Some(o) if o != w => {
                    push(format!("milestone {}: {} → {}", ms_name(*m), date(Some(*o)), date(Some(*w))))
                }
                _ => {}
            }
        }
        for (m, w) in &old {
            if !new.contains_key(m) {
                push(format!("milestone {} tolta (era il {})", ms_name(*m), date(Some(*w))));
            }
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
            && r.applies_to(app, proj)
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
    if !r.fits() {
        return Vec::new();
    }
    let mut out = Vec::new();
    for (proj, _) in app.projects.list() {
        if !r.applies_to(app, proj) {
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

/// Nome di un'ancora (o del punto di un avviso), come negli elenchi della
/// finestra delle regole.
pub fn anchor_label(app: &App, a: Anchor) -> String {
    let dev = |d: DevId| app.devs.get(d).map(|x| x.get_name()).unwrap_or_else(|| "?".into());
    let ms = |m: MilestoneId| {
        let name = app.milestones.get_name(m).unwrap_or("?").to_string();
        match app.milestones.owner(m) {
            Some(p) => format!("{name} · {}", app.projects.get(p).map(|x| x.label(p)).unwrap_or_default()),
            None => name,
        }
    };
    match a {
        Anchor::DevPresent(d) => format!("Se c'è il dev {}", dev(d)),
        Anchor::DevEnd(d) => format!("Fine dev {}", dev(d)),
        Anchor::DevStart(d) => format!("Inizio dev {}", dev(d)),
        Anchor::FirstEffort => "Primo effort (tutti i dev)".into(),
        Anchor::LastEffort => "Ultimo effort (tutti i dev)".into(),
        Anchor::ProjectStart => "Inizio progetto".into(),
        Anchor::ProjectEnd => "Fine progetto".into(),
        Anchor::FirstMilestone => "Prima milestone".into(),
        Anchor::LastMilestone => "Ultima milestone".into(),
        Anchor::Milestone(m) => format!("Milestone {}", ms(m)),
    }
}

/// Un avviso: una regola d'avviso violata in un progetto.
#[derive(Debug, Clone, PartialEq)]
pub struct RuleWarning {
    pub rule: RuleId,
    /// Es. «Milestone Rilascio (26-03-02) è prima di Fine dev Backend + 1
    /// (26-03-09)».
    pub text: String,
    /// La coppia che viola (punto, ancora) e di quanti giorni (> 0): serve a
    /// capire se una modifica la peggiora (`block_regression`).
    pub point: Anchor,
    pub anchor: Anchor,
    pub gap: i64,
}

/// Avvisi del progetto: le regole d'avviso attive, con l'ambito che lo
/// comprende, in cui sia il punto sia l'ancora esistono e il punto sta prima
/// (o dopo) dell'ancora ± N. Valori come dopo le regole (milestone e date
/// governate già al loro posto).
pub fn warnings(app: &App, proj: ProjectId) -> Vec<RuleWarning> {
    let checks: Vec<(RuleId, &Rule)> = app
        .rules
        .list()
        .into_iter()
        .filter(|(_, r)| r.enabled && r.target.kind() == RuleKind::Warning && r.fits())
        .collect();
    if checks.is_empty() {
        return Vec::new();
    }
    let mut ctx = Ctx { app, proj, gov: governing(app, proj), memo: HashMap::new() };
    let mut out = Vec::new();
    for (id, r) in checks {
        let Target::Check(_, cmp) = r.target else { continue };
        if !r.applies_to(app, proj) {
            continue;
        }
        // Ogni coppia punto × ancora in cui esistono entrambi.
        for point in r.points() {
            let Ok(Some(p)) = ctx.anchor(point, r.target) else { continue };
            for anchor in r.anchors() {
                let Ok(Some(a)) = ctx.anchor(anchor, r.target) else { continue };
                let limit = shifted(a, r.offset_weeks);
                let (gap, word) = match cmp {
                    Cmp::NotBefore => (limit.0 as i64 - p.0 as i64, "prima di"),
                    Cmp::NotAfter => (p.0 as i64 - limit.0 as i64, "dopo"),
                };
                if gap <= 0 {
                    continue;
                }
                let off = match r.offset_weeks {
                    0 => String::new(),
                    n if n > 0 => format!(" + {n}"),
                    n => format!(" - {}", -n),
                };
                let date = |w: WeekId| crate::date_utils::dates::week_label(w.0 as i32);
                out.push(RuleWarning {
                    rule: id,
                    text: format!(
                        "{} ({}) è {word} {}{off} ({})",
                        anchor_label(app, point),
                        date(p),
                        anchor_label(app, anchor),
                        date(limit)
                    ),
                    point,
                    anchor,
                    gap,
                });
            }
        }
    }
    out
}

/// Le violazioni delle regole d'avviso con «Blocca», in tutti i progetti:
/// (progetto, regola, punto, ancora) → (giorni di troppo, testo).
type Violations = HashMap<(ProjectId, RuleId, Anchor, Anchor), (i64, String)>;

fn blocking_violations(app: &App) -> Violations {
    let mut out = Violations::new();
    if !app.rules.list().iter().any(|(_, r)| r.enabled && r.blocks()) {
        return out;
    }
    for (proj, _) in app.projects.list() {
        for w in warnings(app, proj) {
            if app.rules.get(w.rule).is_some_and(|r| r.blocks()) {
                out.insert((proj, w.rule, w.point, w.anchor), (w.gap, w.text));
            }
        }
    }
    out
}

/// C'è almeno una regola attiva con «Blocca»? (Solo allora si tiene la
/// copia dei dati prima di ogni modifica.)
pub fn has_blocking(app: &App) -> bool {
    app.rules.list().iter().any(|(_, r)| r.enabled && r.blocks())
}

/// Una modifica da `before` ad `after` viola una regola con «Blocca»? Sì se
/// crea una coppia in violazione che prima non c'era o ne **peggiora** una
/// (più giorni di troppo); le violazioni già presenti che restano uguali o
/// migliorano passano. Le modifiche alle regole stesse non si bloccano
/// (sono loro a decidere i vincoli). Ritorna il messaggio della prima.
pub fn block_regression(before: &App, after: &App) -> Option<String> {
    if before.rules != after.rules {
        return None;
    }
    let old = blocking_violations(before);
    let new = blocking_violations(after);
    let mut bad: Vec<_> = new
        .iter()
        .filter(|(k, (gap, _))| old.get(*k).is_none_or(|(g, _)| gap > g))
        .collect();
    bad.sort_by_key(|((p, r, _, _), _)| (after.projects.list().iter().position(|(x, _)| x == p), *r));
    let ((proj, rule, _, _), (_, text)) = bad.first()?;
    let label = after.projects.get(*proj).map(|x| x.label(*proj)).unwrap_or_default();
    let more = if bad.len() > 1 { format!(" (e altre {})", bad.len() - 1) } else { String::new() };
    Some(format!(
        "Modifica annullata: la regola «{}» la blocca — {label}: {text}{more}.",
        rule_label(after, *rule)
    ))
}

/// Avvisi di tutti i progetti, nell'ordine dei progetti.
pub fn all_warnings(app: &App) -> Vec<(ProjectId, RuleWarning)> {
    if !app.rules.list().iter().any(|(_, r)| r.enabled && r.target.kind() == RuleKind::Warning) {
        return Vec::new();
    }
    app.projects
        .list()
        .into_iter()
        .flat_map(|(p, _)| warnings(app, p).into_iter().map(move |w| (p, w)))
        .collect()
}

/// Perché l'ancora `a` manca nel progetto (per la diagnosi).
fn missing_reason(app: &App, proj: ProjectId, a: Anchor) -> String {
    let dev = |d: DevId| app.devs.get(d).map(|x| x.get_name()).unwrap_or_else(|| "?".into());
    match a {
        Anchor::DevStart(d) | Anchor::DevEnd(d) | Anchor::DevPresent(d) => {
            if app.projects.list_devs(proj).contains(&d) {
                format!("il dev {} non ha ore", dev(d))
            } else {
                format!("il dev {} non è nel progetto", dev(d))
            }
        }
        Anchor::FirstEffort | Anchor::LastEffort => "nessun dev del progetto ha ore".into(),
        Anchor::ProjectStart => "il progetto non ha la data di inizio".into(),
        Anchor::ProjectEnd => "il progetto non ha la data di fine".into(),
        Anchor::FirstMilestone | Anchor::LastMilestone => {
            "nessuna milestone collocata (oltre a quella della regola)".into()
        }
        Anchor::Milestone(m) => format!(
            "la milestone {} non è collocata nel progetto",
            app.milestones.get_name(m).unwrap_or("?")
        ),
    }
}

/// Cosa fa la regola `id` nel progetto (già nell'ambito) e perché.
fn diagnose_project(app: &App, id: RuleId, r: &Rule, proj: ProjectId) -> String {
    let date = |w: WeekId| crate::date_utils::dates::week_label(w.0 as i32);
    let mut ctx = Ctx { app, proj, gov: governing(app, proj), memo: HashMap::new() };
    let anchor = ctx.anchor(r.anchor, r.target);
    match r.target.kind() {
        RuleKind::Aligned => {
            if !placeable(app, proj, r.target) {
                return "la milestone è di un altro progetto: qui non si colloca".into();
            }
            match ctx.gov.get(&r.target) {
                Some(&other) if other != id => {
                    return format!(
                        "non agisce: vince la regola «{}» (ambito più specifico, o a parità creata prima)",
                        rule_label(app, other)
                    );
                }
                None => return "non agisce (regola non applicabile qui)".into(),
                _ => {}
            }
            let ev = evaluate(app, proj);
            if ev.blocked.contains(&id) {
                return "ferma: ciclo (la regola dipende da sé stessa, anche attraverso altre regole)".into();
            }
            let is_date = r.target.milestone().is_none();
            match (anchor, ev.values.get(&r.target).copied().flatten()) {
                (Ok(Some(a)), Some(w)) => {
                    let what = if is_date { "data" } else { "milestone" };
                    format!("{what} il {} (ancora il {})", date(w), date(a))
                }
                (Ok(None), _) if is_date => format!(
                    "ancora mancante ({}): la data resta com'è",
                    missing_reason(app, proj, r.anchor)
                ),
                (Ok(None), _) => format!(
                    "ancora mancante ({}): la milestone non è collocata",
                    missing_reason(app, proj, r.anchor)
                ),
                _ => "nessun valore calcolato".into(),
            }
        }
        RuleKind::AddDev => {
            let Anchor::DevPresent(a) = r.anchor else {
                return "ancora non adatta".into();
            };
            let b = app.devs.get(r.target_dev()).map(|x| x.get_name()).unwrap_or_else(|| "?".into());
            if app.projects.list_devs(proj).contains(&a) {
                format!("c'è la condizione: il dev {b} è (o viene messo) nel progetto")
            } else {
                format!("{}: niente da aggiungere", missing_reason(app, proj, r.anchor))
            }
        }
        RuleKind::MoveDev => {
            let dev = r.target_dev();
            let Some((first, _)) = app.projects.get_single_dev(proj, dev).and_then(|sd| sd.effort_span()) else {
                return format!("{}: niente da spostare", missing_reason(app, proj, Anchor::DevStart(dev)));
            };
            match anchor {
                Err(()) => "ferma: ciclo nelle regole da cui dipende l'ancora".into(),
                Ok(None) => format!("ancora mancante ({}): niente da spostare", missing_reason(app, proj, r.anchor)),
                Ok(Some(a)) => {
                    let to = shifted(a, r.offset_weeks);
                    if to == first {
                        format!("allineata: le ore partono il {}", date(first))
                    } else {
                        let w = (to.0 as i64 - first.0 as i64) / 7;
                        format!(
                            "da spostare: prima settimana {} → {} ({}{w} sett.), usa «Applica ora…»",
                            date(first),
                            date(to),
                            if w > 0 { "+" } else { "" }
                        )
                    }
                }
            }
        }
        RuleKind::Warning => {
            if !matches!(r.target, Target::Check(..)) {
                return String::new();
            }
            // Coppie controllate (punto e ancora esistono entrambi).
            let mut checked = 0;
            let mut unchecked = Vec::new();
            let mut last = None;
            for point in r.points() {
                for a in r.anchors() {
                    match (ctx.anchor(point, r.target), ctx.anchor(a, r.target)) {
                        (Err(()), _) | (_, Err(())) => unchecked.push("ciclo nelle regole da cui dipende".to_string()),
                        (Ok(None), _) => unchecked.push(missing_reason(app, proj, point)),
                        (_, Ok(None)) => unchecked.push(missing_reason(app, proj, a)),
                        (Ok(Some(pw)), Ok(Some(aw))) => {
                            checked += 1;
                            last = Some((pw, shifted(aw, r.offset_weeks)));
                        }
                    }
                }
            }
            let ws: Vec<String> = warnings(app, proj).into_iter().filter(|w| w.rule == id).map(|w| w.text).collect();
            if !ws.is_empty() {
                return format!("AVVISO: {}", ws.join("; "));
            }
            unchecked.dedup();
            match (checked, last) {
                (0, _) => format!("non controllata: {}", unchecked.first().cloned().unwrap_or_default()),
                (1, Some((pw, limit))) if unchecked.is_empty() => {
                    format!("rispettata: punto il {}, soglia il {}", date(pw), date(limit))
                }
                _ if unchecked.is_empty() => format!("rispettata ({checked} coppie controllate)"),
                _ => format!(
                    "rispettata ({checked} coppie controllate; non controllate: {})",
                    unchecked.join(", ")
                ),
            }
        }
    }
}

/// Diagnosi di tutte le regole, da salvare in un file di testo («Log…»):
/// per ogni regola cosa dice, i riferimenti mancanti e, progetto per
/// progetto (nell'ambito), cosa fa e perché; i progetti fuori ambito sono
/// solo contati ed elencati.
pub fn diagnose(app: &App, when: &str) -> String {
    use crate::rules_text::scope_text;
    let mut out = format!("Diagnosi delle regole di automazione — {when}\n");
    if app.rules.is_empty() {
        out.push_str("\nNessuna regola.\n");
        return out;
    }
    let projects: Vec<ProjectId> = app.projects.list().into_iter().map(|(p, _)| p).collect();
    let pname = |p: ProjectId| app.projects.get(p).map(|x| x.label(p)).unwrap_or_default();
    for (id, r) in app.rules.list() {
        let st = status(app, id);
        let plus = if r.offset_weeks > 0 { "+" } else { "" };
        out.push_str(&format!(
            "\n== Regola {} «{}» — {} ==\n   cosa = {}\n   ancora = {}\n   settimane = {plus}{}\n   progetti = {}\n   stati = {}\n",
            id.0,
            rule_label(app, id),
            if r.enabled { "attiva" } else { "SPENTA" },
            crate::rules_text::rule_target_text(app, r),
            crate::rules_text::rule_anchor_text(app, r),
            r.offset_weeks,
            scope_text(app, &r.scope),
            crate::rules_text::states_text(r.inactive)
        ));
        if !st.missing.is_empty() {
            out.push_str(&format!("   ⚠ inesistente: {} — la regola non agisce.\n", st.missing.join(", ")));
            continue;
        }
        if st.bad_anchor {
            out.push_str("   ⚠ l'ancora non va bene per «cosa» — la regola non agisce.\n");
            continue;
        }
        if !r.enabled {
            out.push_str("   spenta: non agisce in nessun progetto (spunta «Attiva»).\n");
            continue;
        }
        if r.target.kind() == RuleKind::MoveDev {
            out.push_str("   (si applica solo su comando, con «Applica ora…»)\n");
        }
        if r.blocks() {
            out.push_str("   blocca: le modifiche che creano o peggiorano una violazione vengono annullate\n");
        }
        let (inside, outside): (Vec<ProjectId>, Vec<ProjectId>) =
            projects.iter().partition(|p| r.applies_to(app, **p));
        if inside.is_empty() {
            out.push_str("   nessun progetto nell'ambito.\n");
        }
        for p in inside {
            out.push_str(&format!("   - {}: {}\n", pname(p), diagnose_project(app, id, r, p)));
        }
        if !outside.is_empty() {
            let mut names: Vec<String> = outside.iter().take(10).map(|p| pname(*p)).collect();
            if outside.len() > 10 {
                names.push("…".into());
            }
            out.push_str(&format!(
                "   fuori ambito (o in uno stato escluso): {} progetti ({})\n",
                outside.len(),
                names.join(", ")
            ));
        }
    }
    out
}

/// Una regola vista da un progetto («Regole di questo progetto…»): se agisce
/// lì e cosa fa (la diagnosi del Log), o perché no.
#[derive(Debug, Clone, PartialEq)]
pub struct ProjectRuleLine {
    pub rule: RuleId,
    pub applies: bool,
    pub text: String,
}

/// Tutte le regole, dal punto di vista del progetto: quelle che valgono lì
/// (attive, nell'ambito, stato ammesso, riferimenti esistenti) con cosa
/// fanno e perché, le altre con il motivo per cui non agiscono.
pub fn project_rules(app: &App, proj: ProjectId) -> Vec<ProjectRuleLine> {
    let status = app.projects.status(proj);
    app.rules
        .list()
        .into_iter()
        .map(|(id, r)| {
            let missing = missing_refs(app, r);
            let no = |text: String| ProjectRuleLine { rule: id, applies: false, text };
            if !missing.is_empty() {
                no(format!("non agisce: {} inesistente", missing.join(", ")))
            } else if !r.fits() {
                no("non agisce: l'ancora non va bene per «cosa»".into())
            } else if !r.enabled {
                no("spenta".into())
            } else if !r.scope.matches(app, proj) {
                no(format!("fuori ambito (progetti: {})", crate::rules_text::scope_text(app, &r.scope)))
            } else if !r.inactive.allows(status) {
                no(format!(
                    "il progetto è {}: la regola vale solo per {}",
                    status.label(),
                    crate::rules_text::states_text(r.inactive)
                ))
            } else {
                ProjectRuleLine { rule: id, applies: true, text: diagnose_project(app, id, r, proj) }
            }
        })
        .collect()
}

/// Il progetto ha qualcosa che le regole governano **ed è presente** — una
/// milestone collocata, una data di inizio / fine che c'è — o un dev che una
/// regola vuole? (Il «⚙» accanto alla tripletta, come i «⚙» in griglia: una
/// regola per tutti i progetti senza ancora qui non conta.)
pub fn project_is_governed(app: &App, proj: ProjectId) -> bool {
    if app.rules.is_empty() {
        return false;
    }
    let present = |t: &Target| match t {
        Target::Milestone(m) => app.projects.list_project_milestones(proj).iter().any(|(x, _)| x == m),
        Target::ProjectStart => app.projects.get_project_start_week(proj).is_some(),
        Target::ProjectEnd => app.projects.get_project_end_week(proj).is_some(),
        _ => false,
    };
    governing(app, proj).keys().any(present) || !required_devs(app, proj).is_empty()
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
    /// Attiva: progetti nell'ambito ma in uno stato escluso (on-hold /
    /// closed senza `inactive`).
    pub excluded_state: usize,
    /// Allineate: progetti in cui la regola varrebbe ma il bersaglio lo
    /// governa un'altra regola (più specifica o, a parità, più vecchia).
    pub beaten: usize,
}

impl RuleStatus {
    /// Allineate: progetti governati in cui l'ancora manca (la milestone non
    /// c'è; la data resta com'era).
    pub fn no_anchor(&self) -> usize {
        self.governed.saturating_sub(self.placed)
    }
}

/// I riferimenti della regola che non esistono più (milestone, dev,
/// categoria, priorità, progetti): con uno di questi la regola non agisce.
fn missing_refs(app: &App, r: &Rule) -> Vec<&'static str> {
    let mut missing = Vec::new();
    match r.target {
        Target::Milestone(m) if app.milestones.get_name(m).is_none() => missing.push("milestone"),
        Target::AddDev(d) | Target::DevStart(d) if app.devs.get(d).is_none() => missing.push("dev"),
        _ => {}
    }
    for p in r.points() {
        match p {
            Anchor::DevStart(d) | Anchor::DevEnd(d) if app.devs.get(d).is_none() => missing.push("dev controllato"),
            Anchor::Milestone(m) if app.milestones.get_name(m).is_none() => missing.push("milestone controllata"),
            _ => {}
        }
    }
    for a in r.anchors() {
        match a {
            Anchor::DevStart(d) | Anchor::DevEnd(d) | Anchor::DevPresent(d) if app.devs.get(d).is_none() => {
                missing.push("dev di partenza")
            }
            Anchor::Milestone(m) if app.milestones.get_name(m).is_none() => missing.push("milestone di partenza"),
            _ => {}
        }
    }
    missing.dedup();
    match &r.scope {
        RuleScope::Category(c) if app.categories.get_name(*c).is_none() => missing.push("categoria"),
        RuleScope::Priority(p) if app.priorities.get_name(*p).is_none() => missing.push("priorità"),
        RuleScope::Project(_) | RuleScope::Projects(_)
            if r.scope.projects().iter().all(|p| app.projects.get(*p).is_none()) =>
        {
            missing.push("progetto")
        }
        _ => {}
    }
    missing
}

pub fn status(app: &App, id: RuleId) -> RuleStatus {
    let mut st = RuleStatus::default();
    let Some(r) = app.rules.get(id) else {
        return st;
    };
    st.missing = missing_refs(app, r);
    st.bad_anchor = !r.fits();
    if r.enabled {
        st.excluded_state = app
            .projects
            .list()
            .into_iter()
            .filter(|(p, _)| r.scope.matches(app, *p) && !r.inactive.allows(app.projects.status(*p)))
            .count();
    }
    match r.target.kind() {
        RuleKind::Aligned => {}
        RuleKind::AddDev => {
            for (proj, _) in app.projects.list() {
                if r.enabled && r.applies_to(app, proj) {
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
        RuleKind::Warning => {
            for (proj, _) in app.projects.list() {
                if r.applies_to(app, proj) {
                    st.governed += 1;
                    if warnings(app, proj).iter().any(|w| w.rule == id) {
                        st.placed += 1;
                    }
                }
            }
            return st;
        }
    }
    for (proj, _) in app.projects.list() {
        match governing(app, proj).get(&r.target) {
            Some(&g) if g == id => {}
            Some(_) => {
                if r.enabled && r.fits() && r.applies_to(app, proj) {
                    st.beaten += 1;
                }
                continue;
            }
            None => continue,
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
        Rule { name: String::new(), enabled: true, scope, target, anchor, offset_weeks, inactive: Inactive::default(), check: CheckOpts::default() }
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

    /// Fase 4: «Rilascio non prima di fine Backend + 1»: avviso quando il
    /// Rilascio sta prima, niente se è dopo o se manca; «non dopo» al
    /// contrario; spenta = niente. Non cambia nulla.
    #[test]
    fn warning_rules_flag_violations_only_when_both_points_exist() {
        let mut f = fx();
        let be_end = f.base + 21;
        let r = f.app.rules.add(date_rule(
            RuleScope::All,
            Target::Check(Anchor::Milestone(f.rilascio), Cmp::NotBefore),
            Anchor::DevEnd(f.be),
            1,
        ));
        assert!(warnings(&f.app, f.p).is_empty(), "Rilascio non collocato: niente avviso");
        f.app.projects.add_project_milestone(f.p, f.rilascio, WeekId(be_end));
        let before = f.app.clone();
        assert!(!apply(&mut f.app).changed, "un avviso non cambia nulla");
        assert_eq!(f.app.projects.list_project_milestones(f.p), before.projects.list_project_milestones(f.p));
        let w = warnings(&f.app, f.p);
        assert_eq!(w.len(), 1);
        assert_eq!(w[0].rule, r);
        let d = |x: usize| crate::date_utils::dates::week_label(x as i32);
        assert_eq!(
            w[0].text,
            format!("Milestone Rilascio ({}) è prima di Fine dev Backend + 1 ({})", d(be_end), d(be_end + 7))
        );
        assert_eq!(status(&f.app, r).placed, 1);
        assert_eq!(all_warnings(&f.app).len(), 1);
        // Spostato dopo: rispettata.
        f.app.projects.add_project_milestone(f.p, f.rilascio, WeekId(be_end + 7));
        assert!(warnings(&f.app, f.p).is_empty());
        // «Non dopo»: il Rilascio oltre la soglia è un avviso.
        let mut after = f.app.rules.get(r).unwrap().clone();
        after.target = Target::Check(Anchor::Milestone(f.rilascio), Cmp::NotAfter);
        after.offset_weeks = 0;
        f.app.rules.update(r, after.clone());
        assert_eq!(warnings(&f.app, f.p).len(), 1);
        assert!(warnings(&f.app, f.p)[0].text.contains("è dopo Fine dev Backend ("));
        after.enabled = false;
        f.app.rules.update(r, after);
        assert!(warnings(&f.app, f.p).is_empty(), "spenta");
    }

    /// Diagnosi: per ogni progetto dell'ambito cosa fa la regola e perché
    /// (collocata, ancora mancante con il motivo, vince un'altra regola,
    /// ciclo), e i progetti fuori ambito.
    #[test]
    fn diagnosis_explains_why_a_rule_does_or_does_not_act() {
        let mut f = fx();
        let cat = f.app.categories.add("Cliente");
        let q = f.app.projects.add("y", Some("XYZ"), None);
        f.app.projects.set_category(q, Some(cat));
        f.app.projects.add_dev(q, f.be); // senza ore
        let r = f.app.rules.add(rule(RuleScope::All, f.collaudo, Anchor::DevEnd(f.be), 2));
        let win = f.app.rules.add(rule(RuleScope::Project(f.p), f.demo, Anchor::DevEnd(f.fe), 0));
        f.app.rules.add(rule(RuleScope::All, f.demo, Anchor::DevEnd(f.be), 0));
        let off = f.app.rules.add(Rule { enabled: false, ..rule(RuleScope::Category(cat), f.rilascio, Anchor::ProjectEnd, 0) });
        apply(&mut f.app);
        let log = diagnose(&f.app, "oggi");
        let d = |x: usize| crate::date_utils::dates::week_label(x as i32);
        let be_end = f.base + 21;
        for part in [
            format!("== Regola {} «Regola {}» — attiva ==", r.0, r.0),
            "   cosa = milestone \"Collaudo\"\n   ancora = fine dev \"Backend\"\n   settimane = +2".to_string(),
            format!("   - ABC: milestone il {} (ancora il {})", d(be_end + 14), d(be_end)),
            "   - XYZ: ancora mancante (il dev Backend non ha ore): la milestone non è collocata".to_string(),
            format!("   - ABC: non agisce: vince la regola «Regola {}»", win.0),
            format!("== Regola {} «Regola {}» — SPENTA ==", off.0, off.0),
            "   spenta: non agisce in nessun progetto".to_string(),
        ] {
            assert!(log.contains(&part), "manca «{part}» in:\n{log}");
        }
        let scoped = f.app.rules.add(rule(RuleScope::Category(cat), f.rilascio, Anchor::DevEnd(f.fe), 0));
        let log = diagnose(&f.app, "oggi");
        assert!(log.contains("   - XYZ: ancora mancante (il dev Frontend non è nel progetto)"), "{log}");
        assert!(log.contains("   fuori ambito (o in uno stato escluso): 1 progetti (ABC)"), "{log}");
        let _ = scoped;
    }

    /// Ambito con più progetti: vale in quelli scelti, specifico come un
    /// progetto solo (batte categoria / priorità / tutti).
    #[test]
    fn a_rule_can_cover_several_chosen_projects() {
        let mut f = fx();
        let w = f.app.workers.list()[0].0;
        let q = f.app.projects.add("y", Some("XYZ"), None);
        let z = f.app.projects.add("z", Some("ZZZ"), None);
        for p in [q, z] {
            f.app.projects.add_dev(p, f.be);
            f.app.projects.add_effort(p, f.be, WeekId(f.base), w, Effort(8));
        }
        f.app.rules.add(rule(RuleScope::All, f.collaudo, Anchor::DevEnd(f.be), 9));
        let r = f.app.rules.add(rule(RuleScope::Projects(vec![f.p, q]), f.collaudo, Anchor::DevEnd(f.be), 1));
        assert!(RuleScope::Projects(vec![f.p, q]).matches(&f.app, q));
        assert!(!RuleScope::Projects(vec![f.p, q]).matches(&f.app, z));
        assert_eq!(RuleScope::Projects(vec![f.p]).rank(), RuleScope::Project(f.p).rank());
        apply(&mut f.app);
        assert_eq!(governing_rule(&f.app, q, Target::Milestone(f.collaudo)), Some(r));
        assert_eq!(at(&f.app, f.p, f.collaudo), Some(WeekId(f.base + 21 + 7)));
        assert_eq!(at(&f.app, q, f.collaudo), Some(WeekId(f.base + 7)));
        assert_eq!(at(&f.app, z, f.collaudo), Some(WeekId(f.base + 63)), "ZZZ: la regola per tutti");
        assert_eq!(status(&f.app, r).placed, 2);
    }

    /// «Tutti tranne …»: vale ovunque tranne nei progetti esclusi; batte
    /// «tutti» ma perde da priorità, categoria e progetti scelti.
    #[test]
    fn all_except_skips_the_excluded_projects() {
        let mut f = fx();
        let w = f.app.workers.list()[0].0;
        let q = f.app.projects.add("y", Some("XYZ"), None);
        f.app.projects.add_dev(q, f.be);
        f.app.projects.add_effort(q, f.be, WeekId(f.base), w, Effort(8));
        let all = f.app.rules.add(rule(RuleScope::All, f.collaudo, Anchor::DevEnd(f.be), 9));
        let except = f.app.rules.add(rule(RuleScope::AllExcept(vec![q]), f.collaudo, Anchor::DevEnd(f.be), 1));
        assert!(RuleScope::AllExcept(vec![q]).matches(&f.app, f.p));
        assert!(!RuleScope::AllExcept(vec![q]).matches(&f.app, q));
        apply(&mut f.app);
        assert_eq!(governing_rule(&f.app, f.p, Target::Milestone(f.collaudo)), Some(except));
        assert_eq!(governing_rule(&f.app, q, Target::Milestone(f.collaudo)), Some(all), "XYZ escluso: vale «tutti»");
        let must = f.app.priorities.list()[0].0;
        f.app.projects.set_priority(f.p, Some(must));
        let pri = f.app.rules.add(rule(RuleScope::Priority(must), f.collaudo, Anchor::DevEnd(f.be), 2));
        assert_eq!(governing_rule(&f.app, f.p, Target::Milestone(f.collaudo)), Some(pri));
        assert!(RuleScope::AllExcept(vec![]).rank() > RuleScope::All.rank());
    }

    /// Di norma una regola vale solo per i progetti attivi: in un progetto
    /// on-hold o chiuso non colloca nulla e non sposta ciò che c'è (la
    /// milestone già collocata resta dov'è, non più governata), non aggiunge
    /// dev e non avvisa; con `inactive` acceso vale anche lì.
    #[test]
    fn rules_act_only_on_active_projects_unless_asked() {
        let mut f = fx();
        let test = f.app.devs.add("Test");
        let r = f.app.rules.add(rule(RuleScope::All, f.collaudo, Anchor::DevEnd(f.be), 2));
        let add = f.app.rules.add(date_rule(RuleScope::All, Target::AddDev(test), Anchor::DevPresent(f.be), 0));
        let warn = f.app.rules.add(date_rule(
            RuleScope::All,
            Target::Check(Anchor::Milestone(f.collaudo), Cmp::NotAfter),
            Anchor::DevEnd(f.be),
            0,
        ));
        f.app.projects.set_status(f.p, ProjectStatus::Closed);
        apply(&mut f.app);
        assert_eq!(at(&f.app, f.p, f.collaudo), None, "chiuso: niente milestone");
        assert!(!f.app.projects.list_devs(f.p).contains(&test), "chiuso: niente dev");
        assert!(governing(&f.app, f.p).is_empty());
        // Attivo: la regola agisce; poi on-hold: la milestone resta dov'è
        // anche se l'ancora si sposta.
        f.app.projects.set_status(f.p, ProjectStatus::Active);
        apply(&mut f.app);
        let placed = WeekId(f.base + 7 * 5);
        assert_eq!(at(&f.app, f.p, f.collaudo), Some(placed));
        assert_eq!(warnings(&f.app, f.p).len(), 1, "Collaudo dopo la fine del Backend");
        f.app.projects.set_status(f.p, ProjectStatus::OnHold);
        f.app.projects.add_effort(f.p, f.be, WeekId(f.base + 7 * 4), f.app.workers.list()[0].0, Effort(8));
        apply(&mut f.app);
        assert_eq!(at(&f.app, f.p, f.collaudo), Some(placed), "on-hold: non si sposta");
        assert!(warnings(&f.app, f.p).is_empty(), "on-hold: nessun avviso");
        assert_eq!(status(&f.app, r).governed, 0);
        // Con on-hold ammesso, la regola torna ad agire.
        for id in [r, add, warn] {
            let mut x = f.app.rules.get(id).unwrap().clone();
            x.inactive.on_hold = true;
            f.app.rules.update(id, x);
        }
        apply(&mut f.app);
        assert_eq!(at(&f.app, f.p, f.collaudo), Some(WeekId(f.base + 7 * 6)));
        assert!(f.app.projects.list_devs(f.p).contains(&test));
        assert_eq!(warnings(&f.app, f.p).len(), 1);
        // On-hold ammesso, closed no.
        f.app.projects.set_status(f.p, ProjectStatus::Closed);
        assert!(governing(&f.app, f.p).is_empty());
    }

    /// `inactive` nel `.ron`: assente nelle regole vecchie (= solo attivi) e
    /// scritto solo quando non è quello di norma.
    #[test]
    fn inactive_states_round_trip_and_default_to_active_only() {
        let mut f = fx();
        f.app.rules.add(rule(RuleScope::All, f.collaudo, Anchor::DevEnd(f.be), 2));
        let ron = f.app.to_ron_string();
        assert!(!ron.contains("inactive"), "di norma non si scrive: {ron}");
        let mut x = f.app.rules.get(RuleId(1)).unwrap().clone();
        x.inactive = Inactive { on_hold: false, closed: true };
        f.app.rules.update(RuleId(1), x);
        let ron = f.app.to_ron_string();
        assert!(ron.contains("inactive"), "{ron}");
        assert_eq!(App::from_ron_str(&ron).expect("rilettura").rules, f.app.rules);
    }

    /// Anteprima: cosa farebbe la regola se attivata, progetto per progetto;
    /// nulla per una regola che non cambierebbe niente.
    #[test]
    fn preview_lists_what_a_rule_would_change() {
        let mut f = fx();
        f.app.projects.set_project_end_week(f.p, Some(WeekId(f.base + 7 * 10)));
        f.app.projects.add_project_milestone(f.p, f.rilascio, WeekId(f.base));
        let mut ms = rule(RuleScope::All, f.collaudo, Anchor::DevEnd(f.be), 2);
        ms.enabled = false;
        let id = f.app.rules.add(ms.clone());
        assert!(preview_changes(&f.app, id, &ms).is_empty(), "spenta: nulla");
        ms.enabled = true;
        let ch = preview_changes(&f.app, id, &ms);
        assert_eq!(ch.len(), 1, "{ch:?}");
        assert_eq!(ch[0].proj, f.p);
        assert!(ch[0].text.starts_with("milestone Collaudo collocata il "), "{}", ch[0].text);
        // Data e milestone spostata / tolta.
        let end = date_rule(RuleScope::All, Target::ProjectEnd, Anchor::LastEffort, 1);
        let texts: Vec<String> = preview_changes(&f.app, RuleId(99), &end).into_iter().map(|c| c.text).collect();
        assert_eq!(texts.len(), 1);
        assert!(texts[0].starts_with("fine progetto: "), "{texts:?}");
        let gone = rule(RuleScope::All, f.rilascio, Anchor::Milestone(f.demo), 0);
        let texts: Vec<String> = preview_changes(&f.app, RuleId(99), &gone).into_iter().map(|c| c.text).collect();
        assert_eq!(texts.len(), 1);
        assert!(texts[0].starts_with("milestone Rilascio tolta"), "{texts:?}");
        // I dati non cambiano.
        assert_eq!(at(&f.app, f.p, f.collaudo), None);
    }

    /// Orfane: le milestone collocate dalla regola che, spenta, eliminata o
    /// ristretta, nessuna regola governerebbe più; non quelle che passano a
    /// un'altra regola, né quelle mai collocate.
    #[test]
    fn orphans_are_placements_no_rule_would_govern_any_more() {
        let mut f = fx();
        let q = f.app.projects.add("y", Some("XYZ"), None);
        let r = f.app.rules.add(rule(RuleScope::All, f.collaudo, Anchor::DevEnd(f.be), 2));
        // Rilascio: nessuna ancora in XYZ né in ABC (Demo non collocata).
        f.app.rules.add(rule(RuleScope::All, f.rilascio, Anchor::Milestone(f.demo), 0));
        apply(&mut f.app);
        let placed = WeekId(f.base + 7 * 5);
        let want = vec![Orphan { proj: f.p, milestone: f.collaudo, week: placed }];
        assert_eq!(orphans(&f.app, r, None), want, "eliminata");
        let mut off = f.app.rules.get(r).unwrap().clone();
        off.enabled = false;
        assert_eq!(orphans(&f.app, r, Some(&off)), want, "spenta");
        let mut narrow = f.app.rules.get(r).unwrap().clone();
        narrow.scope = RuleScope::Projects(vec![q]);
        assert_eq!(orphans(&f.app, r, Some(&narrow)), want, "ambito ristretto");
        let mut moved = f.app.rules.get(r).unwrap().clone();
        moved.offset_weeks = 4;
        assert!(orphans(&f.app, r, Some(&moved)).is_empty(), "stessa milestone, ancora governata");
        // Un'altra regola (meno specifica, creata dopo) la prenderebbe in carico.
        f.app.rules.add(rule(RuleScope::AllExcept(vec![]), f.collaudo, Anchor::LastEffort, 1));
        assert!(orphans(&f.app, r, None).is_empty(), "passa all'altra regola");
        assert_eq!(at(&f.app, f.p, f.collaudo), Some(placed), "nulla cambia nei dati");
    }

    /// Le regole viste dal progetto: chi agisce (con la diagnosi), chi no e
    /// perché — spenta, fuori ambito, stato escluso, riferimento eliminato.
    #[test]
    fn project_rules_explain_each_rule_from_the_project() {
        let mut f = fx();
        let q = f.app.projects.add("y", Some("XYZ"), None);
        let ok = f.app.rules.add(rule(RuleScope::All, f.collaudo, Anchor::DevEnd(f.be), 2));
        let mut off_rule = rule(RuleScope::All, f.rilascio, Anchor::ProjectEnd, 0);
        off_rule.enabled = false;
        let off = f.app.rules.add(off_rule);
        let out = f.app.rules.add(rule(RuleScope::Projects(vec![q]), f.demo, Anchor::ProjectEnd, 0));
        let gone = f.app.rules.add(date_rule(RuleScope::All, Target::ProjectEnd, Anchor::DevEnd(DevId(999)), 0));
        apply(&mut f.app);
        assert!(project_is_governed(&f.app, f.p));
        assert!(!project_is_governed(&f.app, q), "XYZ non ha ancore");
        let lines = project_rules(&f.app, f.p);
        let get = |id| lines.iter().find(|l| l.rule == id).unwrap();
        assert!(get(ok).applies && get(ok).text.starts_with("milestone il "), "{:?}", get(ok));
        assert_eq!((get(off).applies, get(off).text.as_str()), (false, "spenta"));
        assert!(!get(out).applies && get(out).text.starts_with("fuori ambito (progetti: progetti \"XYZ\")"), "{:?}", get(out));
        assert!(!get(gone).applies && get(gone).text.contains("dev di partenza inesistente"), "{:?}", get(gone));
        f.app.projects.set_status(f.p, ProjectStatus::Closed);
        let lines = project_rules(&f.app, f.p);
        let l = lines.iter().find(|l| l.rule == ok).unwrap();
        assert_eq!((l.applies, l.text.as_str()), (false, "il progetto è Closed: la regola vale solo per attivi"));
    }

    /// Stato di una regola: oltre ai progetti in cui agisce, quelli senza
    /// ancora, quelli dove vince un'altra regola e quelli esclusi per stato.
    #[test]
    fn status_counts_no_anchor_beaten_and_excluded_projects() {
        let mut f = fx();
        let q = f.app.projects.add("y", Some("XYZ"), None); // senza Backend
        let c = f.app.projects.add("z", Some("CLO"), None);
        f.app.projects.add_dev(c, f.be);
        f.app.projects.add_effort(c, f.be, WeekId(f.base), f.app.workers.list()[0].0, Effort(8));
        f.app.projects.set_status(c, ProjectStatus::Closed);
        let all = f.app.rules.add(rule(RuleScope::All, f.collaudo, Anchor::DevEnd(f.be), 2));
        apply(&mut f.app);
        let st = status(&f.app, all);
        assert_eq!((st.governed, st.placed, st.no_anchor(), st.beaten, st.excluded_state), (2, 1, 1, 0, 1));
        // Una regola per ABC vince lì: la generale perde ABC.
        f.app.rules.add(rule(RuleScope::Projects(vec![f.p]), f.collaudo, Anchor::DevEnd(f.be), 3));
        apply(&mut f.app);
        let st = status(&f.app, all);
        assert_eq!((st.governed, st.placed, st.beaten, st.excluded_state), (1, 0, 1, 1), "{st:?}");
        assert!(f.app.projects.list_devs(q).is_empty());
        // Spenta: niente conteggi d'esclusione.
        let mut off = f.app.rules.get(all).unwrap().clone();
        off.enabled = false;
        f.app.rules.update(all, off);
        assert_eq!(status(&f.app, all).excluded_state, 0);
    }

    /// Avviso con più punti e più ancore: si controlla ogni coppia in cui
    /// esistono entrambi; ogni coppia che viola è un avviso a sé (con quanti
    /// giorni di troppo). Gli elementi in più contano per i riferimenti.
    #[test]
    fn warning_lists_check_every_pair() {
        let mut f = fx();
        // Frontend parte a base+14, Backend finisce a base+21.
        f.app.projects.add_project_milestone(f.p, f.collaudo, WeekId(f.base + 7));
        let mut r = date_rule(
            RuleScope::All,
            Target::Check(Anchor::DevStart(f.fe), Cmp::NotBefore),
            Anchor::DevEnd(f.be),
            0,
        );
        r.check.points = vec![Anchor::Milestone(f.demo)]; // non collocata: niente
        r.check.anchors = vec![Anchor::Milestone(f.collaudo), Anchor::ProjectEnd]; // fine: assente
        let id = f.app.rules.add(r.clone());
        let ws = warnings(&f.app, f.p);
        assert_eq!(ws.len(), 1, "{ws:?}");
        assert_eq!((ws[0].point, ws[0].anchor, ws[0].gap), (Anchor::DevStart(f.fe), Anchor::DevEnd(f.be), 7));
        assert!(ws[0].text.starts_with("Inizio dev Frontend"), "{}", ws[0].text);
        // Collaudo dopo l'inizio del Frontend: una seconda coppia viola.
        f.app.projects.add_project_milestone(f.p, f.collaudo, WeekId(f.base + 35));
        assert_eq!(warnings(&f.app, f.p).len(), 2);
        let d = diagnose_project(&f.app, id, &r, f.p);
        assert!(d.starts_with("AVVISO: ") && d.contains("; "), "{d}");
        // Riferimenti: anche gli elementi in più.
        assert!(f.app.rules.using_milestone(f.demo).contains(&id));
        assert!(f.app.rules.using_milestone(f.collaudo).contains(&id));
        let mut gone = r.clone();
        gone.check.anchors.push(Anchor::DevEnd(DevId(999)));
        assert_eq!(missing_refs(&f.app, &gone), ["dev di partenza"]);
        let mut bad = r.clone();
        bad.check.points.push(Anchor::DevPresent(f.be));
        assert!(!bad.fits());
    }

    /// «Blocca»: una modifica che crea una violazione o ne peggiora una si
    /// riconosce; una che la lascia uguale o la migliora no; le regole senza
    /// «Blocca» e le modifiche alle regole stesse non bloccano.
    #[test]
    fn block_regression_spots_new_and_worse_violations_only() {
        let mut f = fx();
        let w = f.app.workers.list()[0].0;
        let mut r = date_rule(
            RuleScope::All,
            Target::Check(Anchor::DevStart(f.fe), Cmp::NotBefore),
            Anchor::DevEnd(f.be),
            0,
        );
        f.app.rules.add(r.clone());
        let before = f.app.clone();
        let mut early = f.app.clone();
        early.projects.add_effort(f.p, f.fe, WeekId(f.base + 7), w, Effort(8));
        assert_eq!(block_regression(&before, &early), None, "senza «Blocca» solo avviso");
        r.check.block = true;
        let mut blocking = f.app.clone();
        blocking.rules.update(RuleId(1), r.clone());
        assert!(has_blocking(&blocking));
        // Già in violazione di 7 giorni: anticipare ancora peggiora.
        let mut worse = blocking.clone();
        worse.projects.add_effort(f.p, f.fe, WeekId(f.base + 7), w, Effort(8));
        let msg = block_regression(&blocking, &worse).expect("peggiora");
        assert!(msg.starts_with("Modifica annullata: la regola «Regola 1» la blocca — ABC: Inizio dev Frontend"), "{msg}");
        // Uguale (ore in una settimana dove già lavora) o migliore: passa.
        let mut same = blocking.clone();
        same.projects.add_effort(f.p, f.fe, WeekId(f.base + 21), w, Effort(4));
        assert_eq!(block_regression(&blocking, &same), None);
        let mut better = blocking.clone();
        better.projects.reset_effort(f.p, f.fe, WeekId(f.base + 14));
        assert_eq!(block_regression(&blocking, &better), None);
        // Una violazione nuova (prima in regola): bloccata.
        let mut ok = blocking.clone();
        ok.projects.reset_effort(f.p, f.fe, WeekId(f.base + 14));
        ok.projects.reset_effort(f.p, f.fe, WeekId(f.base + 21));
        assert!(warnings(&ok, f.p).is_empty());
        let mut again = ok.clone();
        again.projects.add_effort(f.p, f.fe, WeekId(f.base + 14), w, Effort(8));
        assert!(block_regression(&ok, &again).is_some());
        // Cambiano le regole: non si blocca.
        assert_eq!(block_regression(&f.app, &worse), None);
    }
}
