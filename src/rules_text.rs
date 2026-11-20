//! Regole di automazione in **testo** (MIGLIORAMENTI #58, fase 5): esporta /
//! importa le regole in un file leggibile, per copiarle fra file `.ron`
//! diversi. I riferimenti (milestone, dev, categorie, priorità, progetti)
//! sono **per nome**, quindi valgono in ogni file che ha gli stessi nomi.
//!
//! Formato: un blocco per regola, righe `chiave = valore`, `#` = commento.
//!
//! ```text
//! [regola]
//! nome = Collaudo dopo il Backend
//! attiva = sì
//! progetti = tutti
//! stati = attivi
//! cosa = milestone "Collaudo"
//! ancora = fine dev "Backend"
//! settimane = +2
//! ```
//!
//! Valori (parole chiave in minuscolo, nomi fra virgolette, `\"` per una
//! virgoletta nel nome):
//! - `progetti`: `tutti` · `categoria "X"` · `priorità "X"` · `progetto "ABC"` ·
//!   `progetti "ABC", "XYZ"` · `tutti tranne "ABC", "XYZ"`
//! - `stati` (facoltativa, assente = `attivi`): `attivi` più, separati da
//!   virgole, `on-hold` e / o `closed` (o `chiusi`)
//! - `cosa`: `milestone "X"` · `inizio progetto` · `fine progetto` ·
//!   `aggiungi dev "X"` · `ore dev "X"` · `avviso <punto> non prima di` ·
//!   `avviso <punto> non dopo`
//! - `ancora` e `<punto>`: `fine dev "X"` · `inizio dev "X"` ·
//!   `se c'è il dev "X"` · `primo effort` · `ultimo effort` ·
//!   `inizio progetto` · `fine progetto` · `prima milestone` ·
//!   `ultima milestone` · `milestone "X"`
//! - una milestone di un solo progetto: `milestone "X" del progetto "ABC"`.
//! - avvisi: più punti e più ancore separati da virgole (`avviso fine dev
//!   "A", inizio dev "B" non prima di` / `ancora = fine dev "C", milestone
//!   "M"`: si controllano tutte le coppie) e `blocca = sì` (facoltativa).

use crate::app::App;
use crate::dev_utils::dev::DevId;
use crate::milestones::MilestoneId;
use crate::project_utils::project::ProjectId;
use crate::rules::{Anchor, Cmp, Inactive, Rule, RuleScope, Target};

/// Nome fra virgolette (con `\"` e `\\`).
fn quote(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

fn project_name(app: &App, p: ProjectId) -> String {
    app.projects.get(p).map(|x| x.label(p)).unwrap_or_else(|| "?".into())
}

fn dev_text(app: &App, d: DevId) -> String {
    quote(&app.devs.get(d).map(|x| x.get_name()).unwrap_or_else(|| "?".into()))
}

fn milestone_text(app: &App, m: MilestoneId) -> String {
    let name = quote(app.milestones.get_name(m).unwrap_or("?"));
    match app.milestones.owner(m) {
        Some(p) => format!("milestone {name} del progetto {}", quote(&project_name(app, p))),
        None => format!("milestone {name}"),
    }
}

pub(crate) fn anchor_text(app: &App, a: Anchor) -> String {
    match a {
        Anchor::DevEnd(d) => format!("fine dev {}", dev_text(app, d)),
        Anchor::DevStart(d) => format!("inizio dev {}", dev_text(app, d)),
        Anchor::DevPresent(d) => format!("se c'è il dev {}", dev_text(app, d)),
        Anchor::FirstEffort => "primo effort".into(),
        Anchor::LastEffort => "ultimo effort".into(),
        Anchor::ProjectStart => "inizio progetto".into(),
        Anchor::ProjectEnd => "fine progetto".into(),
        Anchor::FirstMilestone => "prima milestone".into(),
        Anchor::LastMilestone => "ultima milestone".into(),
        Anchor::Milestone(m) => milestone_text(app, m),
    }
}

pub(crate) fn target_text(app: &App, t: Target) -> String {
    match t {
        Target::Milestone(m) => milestone_text(app, m),
        Target::ProjectStart => "inizio progetto".into(),
        Target::ProjectEnd => "fine progetto".into(),
        Target::AddDev(d) => format!("aggiungi dev {}", dev_text(app, d)),
        Target::DevStart(d) => format!("ore dev {}", dev_text(app, d)),
        Target::Check(p, Cmp::NotBefore) => format!("avviso {} non prima di", anchor_text(app, p)),
        Target::Check(p, Cmp::NotAfter) => format!("avviso {} non dopo", anchor_text(app, p)),
    }
}

pub(crate) fn scope_text(app: &App, s: &RuleScope) -> String {
    match s {
        RuleScope::All => "tutti".into(),
        RuleScope::Category(c) => format!("categoria {}", quote(app.categories.get_name(*c).unwrap_or("?"))),
        RuleScope::Priority(p) => format!("priorità {}", quote(app.priorities.get_name(*p).unwrap_or("?"))),
        RuleScope::Project(p) => format!("progetto {}", quote(&project_name(app, *p))),
        RuleScope::Projects(ps) => {
            let names: Vec<String> = ps.iter().map(|p| quote(&project_name(app, *p))).collect();
            format!("progetti {}", names.join(", "))
        }
        RuleScope::AllExcept(ps) => {
            let names: Vec<String> = ps.iter().map(|p| quote(&project_name(app, *p))).collect();
            format!("tutti tranne {}", names.join(", ")).trim_end().to_string()
        }
    }
}

/// Elenco di progetti fra virgolette separati da virgole (`"ABC", "XYZ"`);
/// vuoto se `s` è vuoto.
fn project_list(app: &App, s: &str) -> Result<Vec<ProjectId>, String> {
    let mut list = Vec::new();
    let mut rest = s.trim();
    if rest.is_empty() {
        return Ok(list);
    }
    loop {
        let (name, after) = quoted(rest)?;
        let p = find_project(app, &name)?;
        if !list.contains(&p) {
            list.push(p);
        }
        let after = after.trim_start();
        if after.is_empty() {
            return Ok(list);
        }
        rest = after
            .strip_prefix(',')
            .ok_or_else(|| format!("attesa una virgola fra i progetti: «{}»", after.trim()))?;
    }
}

/// La regola in una riga, con le parole del formato testo: «milestone
/// "Collaudo" = fine dev "Backend" +2 sett.», «aggiungi dev "Test" se c'è il
/// dev "Backend"», «avviso … non dopo fine dev "Backend"».
/// «cosa» di una regola, con tutti i punti di un avviso (`avviso fine dev
/// "A", inizio dev "B" non prima di`).
pub(crate) fn rule_target_text(app: &App, r: &Rule) -> String {
    match r.target {
        Target::Check(_, cmp) => {
            let pts: Vec<String> = r.points().into_iter().map(|p| anchor_text(app, p)).collect();
            let word = if cmp == Cmp::NotBefore { "non prima di" } else { "non dopo" };
            format!("avviso {} {word}", pts.join(", "))
        }
        t => target_text(app, t),
    }
}

/// «ancora» di una regola, con tutte le ancore di un avviso.
pub(crate) fn rule_anchor_text(app: &App, r: &Rule) -> String {
    r.anchors().into_iter().map(|a| anchor_text(app, a)).collect::<Vec<_>>().join(", ")
}

/// Divide `s` alle virgole fuori dalle virgolette (i nomi possono
/// contenerne; `\"` è una virgoletta nel nome).
fn split_list(s: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let (mut start, mut quoted, mut escaped) = (0, false, false);
    for (i, c) in s.char_indices() {
        match c {
            _ if escaped => escaped = false,
            '\\' if quoted => escaped = true,
            '"' => quoted = !quoted,
            ',' if !quoted => {
                out.push(&s[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    out.push(&s[start..]);
    out
}

/// Elenco di ancore separate da virgole (almeno una).
fn parse_anchor_list(app: &App, s: &str) -> Result<Vec<Anchor>, String> {
    let parts = split_list(s);
    if parts.iter().any(|p| p.trim().is_empty()) {
        return Err(format!("elemento vuoto nell'elenco: «{}»", s.trim()));
    }
    parts.into_iter().map(|p| parse_anchor(app, p)).collect()
}

pub(crate) fn rule_summary(app: &App, r: &Rule) -> String {
    let weeks = match r.offset_weeks {
        0 => String::new(),
        n if n > 0 => format!(" +{n} sett."),
        n => format!(" {n} sett."),
    };
    let (t, a) = (rule_target_text(app, r), rule_anchor_text(app, r));
    match r.target {
        Target::AddDev(_) => format!("{t} {a}"),
        Target::Check(..) => format!("{t} {a}{weeks}"),
        _ => format!("{t} = {a}{weeks}"),
    }
}

/// Stati a cui vale la regola: `attivi`, più `on-hold` / `closed`.
pub(crate) fn states_text(i: Inactive) -> String {
    let mut out = vec!["attivi"];
    if i.on_hold {
        out.push("on-hold");
    }
    if i.closed {
        out.push("closed");
    }
    out.join(", ")
}

/// Legge `stati`: parole separate da virgole fra `attivi` (obbligatoria: le
/// regole valgono sempre per i progetti attivi), `on-hold`, `closed` /
/// `chiusi`.
fn parse_states(s: &str) -> Result<Inactive, String> {
    let mut out = Inactive::default();
    let mut active = false;
    for w in s.split(',').map(|w| w.trim().to_lowercase()) {
        match w.as_str() {
            "attivi" => active = true,
            "on-hold" => out.on_hold = true,
            "closed" | "chiusi" => out.closed = true,
            "" => return Err("«stati» con un elemento vuoto".into()),
            v => return Err(format!("«stati» non riconosciuto: «{v}» (attivi, on-hold, closed)")),
        }
    }
    if !active {
        return Err("«stati» deve comprendere «attivi»".into());
    }
    Ok(out)
}

/// Il file di testo con tutte le regole del `.ron`.
pub fn export_rules(app: &App) -> String {
    let mut out = String::from(
        "# Regole di automazione (Aggiungi ▸ Regole di automazione… ▸ Importa da testo…).\n\
         # Un blocco [regola] per regola; le righe che iniziano con # sono commenti.\n\
         # Valori: vedi il manuale, capitolo 11, «Regole in testo».\n",
    );
    for (_, r) in app.rules.list() {
        let plus = if r.offset_weeks > 0 { "+" } else { "" };
        out.push_str(&format!(
            "\n[regola]\nnome = {}\nattiva = {}\nprogetti = {}\nstati = {}\ncosa = {}\nancora = {}\nsettimane = {plus}{}\n",
            r.name.replace('\n', " "),
            if r.enabled { "sì" } else { "no" },
            scope_text(app, &r.scope),
            states_text(r.inactive),
            rule_target_text(app, r),
            rule_anchor_text(app, r),
            r.offset_weeks
        ));
        if r.blocks() {
            out.push_str("blocca = sì\n");
        }
    }
    out
}

// ── Lettura ─────────────────────────────────────────────────────────────────

/// Toglie `prefix` dall'inizio di `s` (dopo gli spazi).
fn strip<'a>(s: &'a str, prefix: &str) -> Option<&'a str> {
    s.trim_start().strip_prefix(prefix)
}

/// Un nome fra virgolette all'inizio di `s`: (nome, resto).
fn quoted(s: &str) -> Result<(String, &str), String> {
    let s = s.trim_start();
    let Some(body) = s.strip_prefix('"') else {
        return Err(format!("atteso un nome fra virgolette in «{}»", s.trim()));
    };
    let mut name = String::new();
    let mut chars = body.char_indices();
    while let Some((i, c)) = chars.next() {
        match c {
            '\\' => {
                if let Some((_, n)) = chars.next() {
                    name.push(n);
                }
            }
            '"' => return Ok((name, &body[i + 1..])),
            _ => name.push(c),
        }
    }
    Err(format!("virgolette non chiuse in «{}»", s.trim()))
}

/// Un nome fra virgolette che deve essere tutto il valore.
fn only_quoted(s: &str) -> Result<String, String> {
    let (name, rest) = quoted(s)?;
    if rest.trim().is_empty() {
        Ok(name)
    } else {
        Err(format!("testo in più dopo «{name}»: «{}»", rest.trim()))
    }
}

fn same(a: &str, b: &str) -> bool {
    a.trim().to_lowercase() == b.trim().to_lowercase()
}

fn find_dev(app: &App, name: &str) -> Result<DevId, String> {
    app.devs
        .list()
        .into_iter()
        .find(|(_, n)| same(n, name))
        .map(|(d, _)| d)
        .ok_or_else(|| format!("dev «{name}» inesistente"))
}

fn find_project(app: &App, name: &str) -> Result<ProjectId, String> {
    app.projects
        .list()
        .into_iter()
        .map(|(p, _)| p)
        .find(|p| same(&project_name(app, *p), name) || same(&app.projects.get_tripletta(*p), name))
        .ok_or_else(|| format!("progetto «{name}» inesistente"))
}

/// `"X"` o `"X" del progetto "ABC"` (il resto di `milestone …`).
fn find_milestone(app: &App, s: &str) -> Result<MilestoneId, String> {
    let (name, rest) = quoted(s)?;
    let owner = match strip(rest, "del progetto") {
        Some(p) => Some(find_project(app, &only_quoted(p)?)?),
        None if rest.trim().is_empty() => None,
        None => return Err(format!("testo in più dopo «{name}»: «{}»", rest.trim())),
    };
    app.milestones
        .list()
        .into_iter()
        .map(|(m, _, _)| m)
        .find(|m| app.milestones.get_name(*m).is_some_and(|n| same(n, &name)) && app.milestones.owner(*m) == owner)
        .ok_or_else(|| match owner {
            Some(_) => format!("milestone «{name}» di quel progetto inesistente"),
            None => format!("milestone «{name}» inesistente (se è di un solo progetto: «milestone \"{name}\" del progetto \"…\"»)"),
        })
}

fn parse_anchor(app: &App, s: &str) -> Result<Anchor, String> {
    let s = s.trim();
    if let Some(r) = strip(s, "se c'è il dev") {
        return Ok(Anchor::DevPresent(find_dev(app, &only_quoted(r)?)?));
    }
    if let Some(r) = strip(s, "fine dev") {
        return Ok(Anchor::DevEnd(find_dev(app, &only_quoted(r)?)?));
    }
    if let Some(r) = strip(s, "inizio dev") {
        return Ok(Anchor::DevStart(find_dev(app, &only_quoted(r)?)?));
    }
    if let Some(r) = strip(s, "milestone") {
        return Ok(Anchor::Milestone(find_milestone(app, r)?));
    }
    Ok(match s {
        "primo effort" => Anchor::FirstEffort,
        "ultimo effort" => Anchor::LastEffort,
        "inizio progetto" => Anchor::ProjectStart,
        "fine progetto" => Anchor::ProjectEnd,
        "prima milestone" => Anchor::FirstMilestone,
        "ultima milestone" => Anchor::LastMilestone,
        _ => return Err(format!("ancora non riconosciuta: «{s}»")),
    })
}

/// «cosa»: il bersaglio e, per un avviso, gli altri punti dell'elenco.
fn parse_target(app: &App, s: &str) -> Result<(Target, Vec<Anchor>), String> {
    let s = s.trim();
    if let Some(r) = strip(s, "avviso") {
        let r = r.trim();
        for (suffix, cmp) in [("non prima di", Cmp::NotBefore), ("non dopo", Cmp::NotAfter)] {
            if let Some(p) = r.strip_suffix(suffix) {
                let mut pts = parse_anchor_list(app, p)?;
                let first = pts.remove(0);
                return Ok((Target::Check(first, cmp), pts));
            }
        }
        return Err(format!("avviso senza «non prima di» / «non dopo» alla fine: «{s}»"));
    }
    if let Some(r) = strip(s, "aggiungi dev") {
        return Ok((Target::AddDev(find_dev(app, &only_quoted(r)?)?), Vec::new()));
    }
    if let Some(r) = strip(s, "ore dev") {
        return Ok((Target::DevStart(find_dev(app, &only_quoted(r)?)?), Vec::new()));
    }
    if let Some(r) = strip(s, "milestone") {
        return Ok((Target::Milestone(find_milestone(app, r)?), Vec::new()));
    }
    match s {
        "inizio progetto" => Ok((Target::ProjectStart, Vec::new())),
        "fine progetto" => Ok((Target::ProjectEnd, Vec::new())),
        _ => Err(format!("«cosa» non riconosciuto: «{s}»")),
    }
}

fn parse_scope(app: &App, s: &str) -> Result<RuleScope, String> {
    let s = s.trim();
    if s == "tutti" {
        return Ok(RuleScope::All);
    }
    if let Some(r) = strip(s, "tutti tranne") {
        return Ok(RuleScope::AllExcept(project_list(app, r)?));
    }
    if let Some(r) = strip(s, "categoria") {
        let name = only_quoted(r)?;
        return app
            .categories
            .list()
            .into_iter()
            .find(|(_, n)| same(n, &name))
            .map(|(c, _)| RuleScope::Category(c))
            .ok_or_else(|| format!("categoria «{name}» inesistente"));
    }
    if let Some(r) = strip(s, "priorità") {
        let name = only_quoted(r)?;
        return app
            .priorities
            .list()
            .into_iter()
            .find(|(_, n)| same(n, &name))
            .map(|(p, _)| RuleScope::Priority(p))
            .ok_or_else(|| format!("priorità «{name}» inesistente"));
    }
    if let Some(r) = strip(s, "progetti") {
        // `progetti "ABC", "XYZ"` (almeno uno).
        let list = project_list(app, r)?;
        if list.is_empty() {
            return Err("«progetti» senza nessun progetto".into());
        }
        return Ok(RuleScope::Projects(list));
    }
    if let Some(r) = strip(s, "progetto") {
        return Ok(RuleScope::Project(find_project(app, &only_quoted(r)?)?));
    }
    Err(format!("«progetti» non riconosciuto: «{s}»"))
}

/// Un blocco `[regola]` letto: le coppie chiave/valore con la loro riga.
struct Block {
    line: usize,
    fields: Vec<(usize, String, String)>,
}

fn parse_block(app: &App, b: &Block) -> Result<Rule, String> {
    let mut rule = Rule {
        name: String::new(),
        enabled: true,
        scope: RuleScope::All,
        target: Target::ProjectEnd,
        anchor: Anchor::LastEffort,
        offset_weeks: 0,
        inactive: Default::default(),
        check: Default::default(),
    };
    let (mut has_target, mut has_anchor) = (false, false);
    for (line, key, value) in &b.fields {
        let at = |e: String| format!("riga {line}: {e}");
        match key.as_str() {
            "nome" => rule.name = value.clone(),
            "attiva" => {
                rule.enabled = match value.to_lowercase().as_str() {
                    "sì" | "si" => true,
                    "no" => false,
                    v => return Err(at(format!("«attiva» vuole sì / no, non «{v}»"))),
                }
            }
            "progetti" => rule.scope = parse_scope(app, value).map_err(at)?,
            "stati" => rule.inactive = parse_states(value).map_err(at)?,
            "cosa" => {
                let (t, pts) = parse_target(app, value).map_err(at)?;
                rule.target = t;
                rule.check.points = pts;
                has_target = true;
            }
            "ancora" => {
                let mut list = parse_anchor_list(app, value).map_err(at)?;
                rule.anchor = list.remove(0);
                rule.check.anchors = list;
                has_anchor = true;
            }
            "blocca" => {
                rule.check.block = match value.to_lowercase().as_str() {
                    "sì" | "si" => true,
                    "no" => false,
                    v => return Err(at(format!("«blocca» vuole sì / no, non «{v}»"))),
                }
            }
            "settimane" => {
                rule.offset_weeks = value
                    .trim_start_matches('+')
                    .parse()
                    .map_err(|_| at(format!("«settimane» vuole un numero, non «{value}»")))?
            }
            k => return Err(at(format!("chiave sconosciuta «{k}»"))),
        }
    }
    if !has_target || !has_anchor {
        return Err(format!("riga {}: mancano «cosa» o «ancora»", b.line));
    }
    if !matches!(rule.target, Target::Check(..)) {
        if !rule.check.anchors.is_empty() {
            return Err(format!("riga {}: più ancore solo per gli avvisi", b.line));
        }
        if rule.check.block {
            return Err(format!("riga {}: «blocca» solo per gli avvisi", b.line));
        }
    }
    if !rule.fits() {
        return Err(format!("riga {}: l'ancora non va bene per «cosa»", b.line));
    }
    Ok(rule)
}

/// Le regole lette da un testo: quelle valide e, per le altre, il motivo
/// (con la riga). Errori fuori dai blocchi (righe senza `[regola]` prima,
/// righe senza `=`) sono segnalati a parte.
pub fn parse_rules(app: &App, text: &str) -> (Vec<Rule>, Vec<String>) {
    let mut blocks: Vec<Block> = Vec::new();
    let mut errors = Vec::new();
    for (i, raw) in text.lines().enumerate() {
        let line = i + 1;
        let l = raw.trim().trim_start_matches('\u{FEFF}');
        if l.is_empty() || l.starts_with('#') {
            continue;
        }
        if l == "[regola]" {
            blocks.push(Block { line, fields: Vec::new() });
            continue;
        }
        let Some((k, v)) = l.split_once('=') else {
            errors.push(format!("riga {line}: attesa «chiave = valore» o «[regola]»"));
            continue;
        };
        match blocks.last_mut() {
            Some(b) => b.fields.push((line, k.trim().to_lowercase(), v.trim().to_string())),
            None => errors.push(format!("riga {line}: manca «[regola]» prima")),
        }
    }
    let mut rules = Vec::new();
    for b in &blocks {
        match parse_block(app, b) {
            Ok(r) => rules.push(r),
            Err(e) => errors.push(e),
        }
    }
    (rules, errors)
}

/// Resoconto di un'importazione.
#[derive(Debug, Default, PartialEq)]
pub struct ImportReport {
    pub added: usize,
    /// Regole identiche a una già presente: saltate.
    pub duplicates: usize,
    pub errors: Vec<String>,
}

/// Un'importazione preparata ma non ancora fatta: le regole da aggiungere
/// (già senza le identiche a una presente o a una precedente del testo), con
/// i conteggi del resoconto. Si fa con `ImportPlan::apply`, dopo l'eventuale
/// anteprima delle modifiche (`rules::preview_added`).
#[derive(Debug, Default, Clone, PartialEq)]
pub struct ImportPlan {
    pub rules: Vec<Rule>,
    pub duplicates: usize,
    pub errors: Vec<String>,
}

impl ImportPlan {
    /// Aggiunge le regole al `.ron` (nessuna regola esistente cambia).
    pub fn apply(self, app: &mut App) -> ImportReport {
        let added = self.rules.len();
        for r in self.rules {
            app.rules.add(r);
        }
        ImportReport { added, duplicates: self.duplicates, errors: self.errors }
    }
}

/// Le regole del testo da aggiungere al `.ron` (le identiche a una già
/// presente si saltano), senza ancora aggiungerle.
pub fn plan_import(app: &App, text: &str) -> ImportPlan {
    let (rules, errors) = parse_rules(app, text);
    let mut plan = ImportPlan { errors, ..Default::default() };
    for r in rules {
        if app.rules.list().iter().any(|(_, x)| **x == r) || plan.rules.contains(&r) {
            plan.duplicates += 1;
        } else {
            plan.rules.push(r);
        }
    }
    plan
}

/// Aggiunge al `.ron` le regole del testo (le identiche a una già presente
/// si saltano; nessuna regola esistente cambia).
#[cfg(test)]
pub fn import_rules(app: &mut App, text: &str) -> ImportReport {
    plan_import(app, text).apply(app)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> App {
        let mut app = App::new();
        let be = app.devs.add("Backend");
        let test = app.devs.add("Test");
        let cat = app.categories.add("Cliente \"A\"");
        let p = app.projects.add("x", Some("ABC"), None);
        let collaudo = app.milestones.add("Collaudo");
        let own = app.milestones.add_owned("Demo interna", Default::default(), Vec::new(), Some(p));
        let must = app.priorities.list()[0].0;
        let rule = |name: &str, scope, target, anchor, offset_weeks, enabled| Rule {
            name: name.into(),
            enabled,
            scope,
            target,
            anchor,
            offset_weeks,
            inactive: Default::default(),
            check: Default::default(),
        };
        app.rules.add(rule("Collaudo", RuleScope::All, Target::Milestone(collaudo), Anchor::DevEnd(be), 2, true));
        app.rules.add(rule("", RuleScope::Category(cat), Target::ProjectEnd, Anchor::LastMilestone, -1, false));
        app.rules.add(rule("Test", RuleScope::Priority(must), Target::AddDev(test), Anchor::DevPresent(be), 0, true));
        app.rules.add(rule("Sposta", RuleScope::Project(p), Target::DevStart(test), Anchor::Milestone(own), 3, true));
        app.rules.add(rule(
            "Avviso",
            RuleScope::All,
            Target::Check(Anchor::Milestone(collaudo), Cmp::NotAfter),
            Anchor::ProjectEnd,
            0,
            true,
        ));
        app
    }

    /// Esporta → reimporta in un file con gli stessi nomi: regole identiche;
    /// nello stesso file sono tutte doppie (saltate).
    #[test]
    fn export_and_import_round_trip_by_name() {
        let app = sample();
        let text = export_rules(&app);
        assert!(text.contains("cosa = milestone \"Collaudo\"\nancora = fine dev \"Backend\"\nsettimane = +2"), "{text}");
        assert!(text.contains("progetti = categoria \"Cliente \\\"A\\\"\""), "{text}");
        assert!(text.contains("milestone \"Demo interna\" del progetto \"ABC\""), "{text}");
        assert!(text.contains("cosa = avviso milestone \"Collaudo\" non dopo"), "{text}");
        let mut fresh = sample();
        let before: Vec<Rule> = fresh.rules.list().into_iter().map(|(_, r)| r.clone()).collect();
        fresh.rules = Default::default();
        let rep = import_rules(&mut fresh, &text);
        assert_eq!(rep, ImportReport { added: 5, duplicates: 0, errors: vec![] });
        let after: Vec<Rule> = fresh.rules.list().into_iter().map(|(_, r)| r.clone()).collect();
        assert_eq!(after, before);
        let mut same_file = sample();
        let rep = import_rules(&mut same_file, &text);
        assert_eq!((rep.added, rep.duplicates), (0, 5));
    }

    /// Errori con la riga: nomi inesistenti, valori sbagliati, chiavi
    /// sconosciute; le regole buone si importano lo stesso.
    #[test]
    fn import_reports_bad_lines_and_keeps_the_good_rules() {
        let mut app = sample();
        app.rules = Default::default();
        let text = "\
# commento
nome = fuori
[regola]
cosa = milestone \"Inesistente\"
ancora = fine progetto

[regola]
nome = Buona
cosa = fine progetto
ancora = ultimo effort
settimane = 1

[regola]
cosa = aggiungi dev \"Test\"
ancora = fine dev \"Backend\"

[regola]
attiva = forse
cosa = inizio progetto
ancora = primo effort

[regola]
colore = rosso
";
        let rep = import_rules(&mut app, text);
        assert_eq!(rep.added, 1);
        assert_eq!(app.rules.list()[0].1.name, "Buona");
        let e = rep.errors.join("\n");
        for part in [
            "riga 2: manca «[regola]» prima",
            "riga 4: milestone «Inesistente» inesistente",
            "riga 13: l'ancora non va bene per «cosa»",
            "riga 18: «attiva» vuole sì / no, non «forse»",
            "riga 23: chiave sconosciuta «colore»",
        ] {
            assert!(e.contains(part), "manca «{part}» in:\n{e}");
        }
    }

    /// Più progetti: `progetti "ABC", "XYZ"` va e torna; virgola mancante o
    /// progetto inesistente sono errori.
    #[test]
    fn several_projects_round_trip_and_bad_lists_are_reported() {
        let mut app = App::new();
        let p = app.projects.add("x", Some("ABC"), None);
        let q = app.projects.add("y", Some("XYZ"), None);
        app.rules.add(Rule {
            name: "Due".into(),
            enabled: true,
            scope: RuleScope::Projects(vec![p, q]),
            target: Target::ProjectEnd,
            anchor: Anchor::LastEffort,
            offset_weeks: 1,
            inactive: Default::default(),
            check: Default::default(),
        });
        let text = export_rules(&app);
        assert!(text.contains("progetti = progetti \"ABC\", \"XYZ\""), "{text}");
        let mut other = App::new();
        other.projects.add("x", Some("ABC"), None);
        other.projects.add("y", Some("XYZ"), None);
        assert_eq!(import_rules(&mut other, &text).added, 1);
        assert_eq!(other.rules.list()[0].1, app.rules.list()[0].1);
        for (bad, err) in [
            ("progetti \"ABC\" \"XYZ\"", "attesa una virgola"),
            ("progetti \"ABC\", \"NOPE\"", "progetto «NOPE» inesistente"),
        ] {
            let t = format!("[regola]\nprogetti = {bad}\ncosa = fine progetto\nancora = ultimo effort\n");
            let rep = import_rules(&mut other, &t);
            assert!(rep.errors.iter().any(|e| e.contains(err)), "{bad}: {:?}", rep.errors);
        }
    }

    /// `tutti tranne "ABC", "XYZ"` (anche senza progetti) va e torna.
    #[test]
    fn all_except_round_trips() {
        let mut app = App::new();
        let p = app.projects.add("x", Some("ABC"), None);
        let q = app.projects.add("y", Some("XYZ"), None);
        for scope in [RuleScope::AllExcept(vec![p, q]), RuleScope::AllExcept(vec![])] {
            app.rules.add(Rule {
                name: String::new(),
                enabled: true,
                scope,
                target: Target::ProjectEnd,
                anchor: Anchor::LastEffort,
                offset_weeks: 0,
                inactive: Default::default(),
                check: Default::default(),
            });
        }
        let text = export_rules(&app);
        assert!(text.contains("progetti = tutti tranne \"ABC\", \"XYZ\"\n"), "{text}");
        assert!(text.contains("progetti = tutti tranne\n"), "{text}");
        let mut other = App::new();
        other.projects.add("x", Some("ABC"), None);
        other.projects.add("y", Some("XYZ"), None);
        let rep = import_rules(&mut other, &text);
        assert_eq!((rep.added, rep.errors.len()), (2, 0), "{:?}", rep.errors);
        let a: Vec<&Rule> = app.rules.list().into_iter().map(|(_, r)| r).collect();
        let b: Vec<&Rule> = other.rules.list().into_iter().map(|(_, r)| r).collect();
        assert_eq!(a, b);
    }

    /// `stati`: scritta sempre, letta facoltativa (assente = solo attivi),
    /// errori per parole sconosciute o senza «attivi».
    #[test]
    fn states_round_trip_and_bad_values_are_reported() {
        let mut app = sample();
        let mut r = app.rules.get(crate::rules::RuleId(1)).unwrap().clone();
        r.inactive = Inactive { on_hold: true, closed: true };
        app.rules.update(crate::rules::RuleId(1), r);
        let text = export_rules(&app);
        assert!(text.contains("stati = attivi, on-hold, closed\n"), "{text}");
        assert!(text.contains("stati = attivi\n"), "{text}");
        let mut other = sample();
        other.rules = Default::default();
        import_rules(&mut other, &text);
        let a: Vec<&Rule> = app.rules.list().into_iter().map(|(_, r)| r).collect();
        let b: Vec<&Rule> = other.rules.list().into_iter().map(|(_, r)| r).collect();
        assert_eq!(a, b);
        assert_eq!(parse_states("Attivi, chiusi"), Ok(Inactive { on_hold: false, closed: true }));
        for (bad, msg) in [("on-hold", "deve comprendere «attivi»"), ("attivi, sospesi", "non riconosciuto"), ("attivi,", "vuoto")] {
            let t = format!("[regola]\nstati = {bad}\ncosa = fine progetto\nancora = ultimo effort\n");
            let (rules, errs) = parse_rules(&app, &t);
            assert!(rules.is_empty() && errs.len() == 1 && errs[0].contains(msg), "{bad}: {errs:?}");
        }
    }

    /// Riassunto in una riga: «=», settimane solo se diverse da 0, «Aggiungi
    /// dev» senza «=».
    #[test]
    fn rule_summary_reads_like_the_text_format() {
        let app = sample();
        let rules: Vec<&Rule> = app.rules.list().into_iter().map(|(_, r)| r).collect();
        assert_eq!(rule_summary(&app, rules[0]), "milestone \"Collaudo\" = fine dev \"Backend\" +2 sett.");
        assert_eq!(rule_summary(&app, rules[1]), "fine progetto = ultima milestone -1 sett.");
        let mut add = rules[0].clone();
        add.target = Target::AddDev(crate::dev_utils::dev::DevId(99));
        add.anchor = Anchor::FirstEffort;
        add.offset_weeks = 0;
        assert!(rule_summary(&app, &add).starts_with("aggiungi dev "), "{}", rule_summary(&app, &add));
        assert!(rule_summary(&app, &add).ends_with(" primo effort"));
    }

    /// Avvisi con più punti e ancore e «blocca» vanno e tornano; le virgole
    /// dentro i nomi fra virgolette non dividono; liste e «blocca» fuori
    /// dagli avvisi sono errori.
    #[test]
    fn warning_lists_and_block_round_trip() {
        let mut app = sample();
        let c = app.milestones.add("Fase 1, parte A");
        let be = app.devs.list()[0].0;
        let test = app.devs.list()[1].0;
        app.rules = Default::default();
        app.rules.add(Rule {
            name: "Ordine".into(),
            enabled: true,
            scope: RuleScope::All,
            target: Target::Check(Anchor::DevStart(test), Cmp::NotBefore),
            anchor: Anchor::DevEnd(be),
            offset_weeks: 1,
            inactive: Default::default(),
            check: crate::rules::CheckOpts {
                points: vec![Anchor::DevEnd(test)],
                anchors: vec![Anchor::Milestone(c)],
                block: true,
            },
        });
        let text = export_rules(&app);
        assert!(text.contains("cosa = avviso inizio dev \"Test\", fine dev \"Test\" non prima di\n"), "{text}");
        assert!(text.contains("ancora = fine dev \"Backend\", milestone \"Fase 1, parte A\"\n"), "{text}");
        assert!(text.contains("blocca = sì\n"), "{text}");
        let (rules, errs) = parse_rules(&app, &text);
        assert!(errs.is_empty(), "{errs:?}");
        assert_eq!(rules[0], *app.rules.get(crate::rules::RuleId(1)).unwrap());
        for (body, msg) in [
            ("cosa = fine progetto\nancora = ultimo effort, primo effort\n", "più ancore solo per gli avvisi"),
            ("cosa = fine progetto\nancora = ultimo effort\nblocca = sì\n", "«blocca» solo per gli avvisi"),
            ("cosa = avviso inizio dev \"Test\", non dopo\nancora = ultimo effort\n", "elemento vuoto"),
        ] {
            let (rules, errs) = parse_rules(&app, &format!("[regola]\n{body}"));
            assert!(rules.is_empty() && errs.len() == 1 && errs[0].contains(msg), "{body}: {errs:?}");
        }
    }
}
