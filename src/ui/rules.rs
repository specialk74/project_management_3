//! Finestra «Regole di automazione» (Aggiungi ▸ Regole di automazione…):
//! elenco delle regole di `rules.rs`, una per riga, modificabili con elenchi
//! a tendina — attiva, nome, ambito, milestone, ancora, scostamento in
//! settimane — con lo stato (in quanti progetti agisce, ciclo, riferimenti
//! eliminati). Ogni modifica è un'`Action` (`AddRule` / `UpdateRule` /
//! `DeleteRule`); le regole si riapplicano in `mark_changed`. Attivare una
//! regola (o cambiarne ambito, stati, cosa, ancora) che toccherebbe dei
//! progetti passa prima dall'anteprima `rule_activation_window`.

use super::*;
use crate::project_utils::project::ProjectStatus;
use crate::rules::{Anchor, Cmp, Inactive, Rule, RuleId, RuleScope, Target};

/// Larghezze delle colonne (combo).
pub(crate) const RULE_NAME_W: f32 = 300.0;
const RULE_SCOPE_W: f32 = 170.0;
/// Stati di progetto (accanto all'ambito).
const RULE_STATES_W: f32 = 120.0;
const RULE_TARGET_W: f32 = 220.0;
const RULE_ANCHOR_W: f32 = 200.0;
/// Punto controllato da un avviso (accanto a «Avviso»).
const RULE_POINT_W: f32 = 170.0;

/// Combo con scelte `(valore, etichetta)`: ritorna la nuova scelta se
/// cambiata. Un valore che non è fra le scelte (riferimento eliminato) si
/// legge «⚠ inesistente».
fn choice_combo<T: PartialEq + Copy>(
    ui: &mut egui::Ui,
    salt: impl std::hash::Hash + std::fmt::Debug,
    width: f32,
    current: T,
    options: &[(T, String)],
) -> Option<T> {
    let text = options
        .iter()
        .find(|(v, _)| *v == current)
        .map_or_else(|| "⚠ inesistente".to_string(), |(_, l)| l.clone());
    let mut picked = None;
    egui::ComboBox::from_id_salt(salt)
        .selected_text(text)
        .width(width)
        .height(400.0)
        .show_ui(ui, |ui| {
            for (v, label) in options {
                if ui.selectable_label(*v == current, label).clicked() && *v != current {
                    picked = Some(*v);
                }
            }
        });
    picked
}

/// Tendina a scelta multipla di ancore (punti o ancore di un avviso): una
/// spunta per voce, almeno una resta (l'ultima non si toglie). Ritorna il
/// nuovo elenco, nell'ordine delle voci, se cambiato. Il testo mostra le
/// voci scelte separate da virgole (tutte nel suggerimento).
fn multi_anchor_combo(
    ui: &mut egui::Ui,
    salt: impl std::hash::Hash + std::fmt::Debug,
    width: f32,
    selected: &[Anchor],
    options: &[(Anchor, String)],
) -> Option<Vec<Anchor>> {
    let label = |a: &Anchor| {
        options.iter().find(|(v, _)| v == a).map_or_else(|| "⚠ inesistente".to_string(), |(_, l)| l.clone())
    };
    let text = selected.iter().map(label).collect::<Vec<_>>().join(", ");
    let mut picked = None;
    let resp = egui::ComboBox::from_id_salt(salt)
        .selected_text(text.clone())
        .width(width)
        .height(400.0)
        .show_ui(ui, |ui| {
            for (v, l) in options {
                let mut on = selected.contains(v);
                let last = on && selected.len() == 1;
                if ui.add_enabled(!last, egui::Checkbox::new(&mut on, l)).changed() {
                    // Nell'ordine delle voci; quelle scelte non più in elenco
                    // (riferimenti eliminati) restano in fondo.
                    let mut next: Vec<Anchor> = options
                        .iter()
                        .map(|(x, _)| *x)
                        .filter(|x| if x == v { on } else { selected.contains(x) })
                        .collect();
                    next.extend(selected.iter().filter(|x| !options.iter().any(|(o, _)| o == *x)));
                    picked = Some(next);
                }
            }
        });
    if selected.len() > 1 {
        resp.response.on_hover_text(text);
    }
    picked
}

/// Testo dell'ambito nella combo: «Tutti i progetti», «Categoria: …»,
/// «Priorità: …», «Progetto: ABC» o «Progetti: ABC, XYZ».
fn scope_label(app: &App, scope: &RuleScope) -> String {
    match scope {
        RuleScope::All => "Tutti i progetti".into(),
        RuleScope::Category(c) => format!("Categoria: {}", app.categories.get_name(*c).unwrap_or("⚠ inesistente")),
        RuleScope::Priority(p) => format!("Priorità: {}", app.priorities.get_name(*p).unwrap_or("⚠ inesistente")),
        RuleScope::Project(_) | RuleScope::Projects(_) => {
            let ps = scope.projects();
            let names: Vec<String> = ps.iter().map(|p| project_label(app, *p)).collect();
            if ps.len() == 1 { format!("Progetto: {}", names[0]) } else { format!("Progetti: {}", names.join(", ")) }
        }
        RuleScope::AllExcept(ps) => {
            let names: Vec<String> = ps.iter().map(|p| project_label(app, *p)).collect();
            if names.is_empty() {
                "Tutti tranne: —".into()
            } else {
                format!("Tutti tranne: {}", names.join(", "))
            }
        }
    }
}

/// Combo dell'ambito: in cima le scelte singole (tutti, «tutti tranne…», una
/// categoria, una priorità), sotto una spunta per progetto: con «tutti
/// tranne…» sono i progetti **esclusi**, altrimenti quelli **scelti** (uno o
/// più: togliendo l'ultimo si torna a «tutti»). Ritorna il nuovo ambito se
/// cambiato.
fn scope_combo(ui: &mut egui::Ui, app: &App, salt: impl std::hash::Hash + std::fmt::Debug, scope: &RuleScope) -> Option<RuleScope> {
    let mut picked = None;
    egui::ComboBox::from_id_salt(salt)
        .selected_text(scope_label(app, scope))
        .width(RULE_SCOPE_W)
        .height(400.0)
        .show_ui(ui, |ui| {
            let except = matches!(scope, RuleScope::AllExcept(_));
            let mut singles = vec![RuleScope::All];
            singles.extend(app.categories.list().into_iter().map(|(c, _)| RuleScope::Category(c)));
            singles.extend(app.priorities.list().into_iter().map(|(p, _)| RuleScope::Priority(p)));
            for s in singles {
                if ui.selectable_label(*scope == s, scope_label(app, &s)).clicked() && *scope != s {
                    picked = Some(s);
                }
            }
            if ui
                .selectable_label(except, "Tutti i progetti tranne…")
                .on_hover_text("Poi spunta qui sotto i progetti da escludere.")
                .clicked()
                && !except
            {
                picked = Some(RuleScope::AllExcept(Vec::new()));
            }
            ui.separator();
            let head = if except { "Progetti esclusi:" } else { "Progetti (uno o più):" };
            ui.label(egui::RichText::new(head).small().color(text_dim()));
            let chosen = if except { scope.excluded() } else { scope.projects() };
            for (p, _) in app.projects.list() {
                let mut on = chosen.contains(&p);
                if ui.checkbox(&mut on, project_label(app, p)).changed() {
                    // Nell'ordine dei progetti.
                    let set: Vec<ProjectId> = app
                        .projects
                        .list()
                        .into_iter()
                        .map(|(x, _)| x)
                        .filter(|x| if *x == p { on } else { chosen.contains(x) })
                        .collect();
                    picked = Some(if except {
                        RuleScope::AllExcept(set)
                    } else if set.is_empty() {
                        RuleScope::All
                    } else {
                        RuleScope::Projects(set)
                    });
                }
            }
        });
    picked
}

/// Scelte della milestone bersaglio: tutte (quelle di un progetto con la sua
/// tripletta: valgono solo lì).
fn milestone_options(app: &App) -> Vec<(MilestoneId, String)> {
    app.milestones
        .list()
        .into_iter()
        .map(|(id, name, _)| {
            let label = match app.milestones.owner(id) {
                Some(p) => format!("{name} · {}", project_label(app, p)),
                None => name,
            };
            (id, label)
        })
        .collect()
}

/// Avviso di partenza quando si sceglie «Avviso» in «Cosa»: la prima
/// milestone (o la fine del progetto) non prima dell'ancora.
fn default_check(app: &App) -> Target {
    let point = app.milestones.list().first().map_or(Anchor::ProjectEnd, |m| Anchor::Milestone(m.0));
    Target::Check(point, Cmp::NotBefore)
}

/// Scelte del bersaglio: le due date del progetto, le milestone, poi per
/// ogni dev «Aggiungi dev» e lo spostamento delle sue ore (su comando), e
/// in fondo «Avviso» (il punto e il confronto si scelgono accanto).
/// `current`: se è già un avviso, la voce «Avviso» è quello.
fn target_options(app: &App, current: Target) -> Vec<(Target, String)> {
    let mut out = vec![
        (Target::ProjectStart, "Inizio progetto (data)".to_string()),
        (Target::ProjectEnd, "Fine progetto (data)".to_string()),
    ];
    out.extend(milestone_options(app).into_iter().map(|(m, n)| (Target::Milestone(m), n)));
    for (d, n) in app.devs.list() {
        out.push((Target::AddDev(d), format!("Aggiungi dev {n}")));
    }
    for (d, n) in app.devs.list() {
        out.push((Target::DevStart(d), format!("Ore del dev {n} (su comando)")));
    }
    let check = if matches!(current, Target::Check(..)) { current } else { default_check(app) };
    out.push((check, "Avviso".to_string()));
    out
}

/// Punti controllabili da un avviso: le stesse ancore delle milestone.
fn point_options(app: &App) -> Vec<(Anchor, String)> {
    anchor_options(app, Target::ProjectEnd)
}

/// Scelte dell'ancora adatte al bersaglio (`anchor_fits`): per «Aggiungi
/// dev» le condizioni «Se c'è il dev …»; altrimenti fine / inizio di ogni dev,
/// primo / ultimo effort di tutti i dev, date del progetto, prima / ultima
/// milestone, ogni milestone.
fn anchor_options(app: &App, target: Target) -> Vec<(Anchor, String)> {
    let devs: Vec<DevId> = app.devs.list().into_iter().map(|(d, _)| d).collect();
    let mut all: Vec<Anchor> = devs.iter().map(|d| Anchor::DevPresent(*d)).collect();
    for d in &devs {
        all.push(Anchor::DevEnd(*d));
        all.push(Anchor::DevStart(*d));
    }
    all.extend([
        Anchor::FirstEffort,
        Anchor::LastEffort,
        Anchor::ProjectStart,
        Anchor::ProjectEnd,
        Anchor::FirstMilestone,
        Anchor::LastMilestone,
    ]);
    all.extend(app.milestones.list().into_iter().map(|(m, _, _)| Anchor::Milestone(m)));
    all.into_iter()
        .filter(|a| crate::rules::anchor_fits(target, *a))
        .map(|a| (a, crate::rules::anchor_label(app, a)))
        .collect()
}

/// Ambito della regola con bersaglio `target`: per una milestone di un solo
/// progetto è sempre quel progetto (altrove non si può collocare), altrimenti
/// resta `scope`.
fn auto_scope(app: &App, target: Target, scope: RuleScope) -> RuleScope {
    match target.milestone().and_then(|m| app.milestones.owner(m)) {
        Some(o) if scope.projects() != [o] => RuleScope::Projects(vec![o]),
        _ => scope,
    }
}

/// Regola nuova: spenta (le scelte di partenza non devono toccare i progetti
/// finché non è configurata): prima milestone (o, se non ce ne sono, la fine
/// del progetto), fine del primo dev (o ultimo effort).
pub(crate) fn new_rule(app: &App) -> Rule {
    let target = app.milestones.list().first().map_or(Target::ProjectEnd, |m| Target::Milestone(m.0));
    let anchor = match (target, app.devs.list().first()) {
        (Target::Milestone(_), Some((d, _))) => Anchor::DevEnd(*d),
        _ => Anchor::LastEffort,
    };
    Rule {
        name: String::new(),
        enabled: false,
        scope: auto_scope(app, target, RuleScope::All),
        target,
        anchor,
        offset_weeks: 0,
        inactive: Default::default(),
        check: Default::default(),
    }
}

/// Copia di una regola per «Clona»: stessi valori, nome «… (copia)» e
/// **spenta** (due regole uguali attive si farebbero concorrenza: la copia si
/// modifica e poi si attiva).
pub(crate) fn cloned_rule(app: &App, id: RuleId, rule: &Rule) -> Rule {
    Rule {
        name: format!("{} (copia)", crate::rules::rule_label(app, id)),
        enabled: false,
        ..rule.clone()
    }
}

/// Testo dello stato di una regola e il suo colore.
fn status_text(app: &App, id: RuleId, rule: &Rule) -> (String, Color32, String) {
    use crate::rules::RuleKind;
    let st = crate::rules::status(app, id);
    if !st.missing.is_empty() {
        let text = format!("⚠ {} inesistente", st.missing.join(", "));
        return (text, warn_amber(), "Un riferimento della regola è stato eliminato: sceglierne un altro.".into());
    }
    if st.bad_anchor {
        return ("⚠ ancora non adatta".into(), warn_amber(), "Scegli un'ancora dall'elenco.".into());
    }
    if st.blocked {
        return (
            "⚠ ciclo".into(),
            error_red(),
            "La regola dipende (anche indirettamente) da sé stessa: resta ferma finché il ciclo non si rompe.".into(),
        );
    }
    if !rule.enabled {
        return ("spenta".into(), text_dim(), "Spunta «Attiva» per applicarla.".into());
    }
    // Dopo il testo principale, i progetti in cui non agisce e perché.
    let mut extra = Vec::new();
    let mut extra_tip = Vec::new();
    if rule.target.kind() == RuleKind::Aligned {
        if st.no_anchor() > 0 {
            extra.push(format!("{} senza ancora", st.no_anchor()));
            extra_tip.push(format!(
                "{} senza ancora: lì l'ancora manca, quindi la milestone non c'è (una data resta com'è).",
                st.no_anchor()
            ));
        }
        if st.beaten > 0 {
            extra.push(format!("{} ad altre regole", st.beaten));
            extra_tip.push(format!(
                "{} ad altre regole: lì lo stesso bersaglio lo governa una regola più specifica (o, a parità, più vecchia).",
                st.beaten
            ));
        }
    }
    if st.excluded_state > 0 {
        extra.push(format!("{} esclusi per stato", st.excluded_state));
        extra_tip.push(format!(
            "{} esclusi per stato: progetti dell'ambito on-hold / closed, che la regola non tocca \
             (si cambia con la tendina degli stati accanto ai progetti).",
            st.excluded_state
        ));
    }
    let (main, color, tip) = match rule.target.kind() {
        RuleKind::Aligned => (
            format!("in {} progetti", st.placed),
            text_dim(),
            format!(
                "In {} progetti la regola governa il bersaglio; in {} l'ancora c'è e il bersaglio è al suo posto.",
                st.governed, st.placed
            ),
        ),
        RuleKind::AddDev => (
            format!("in {} progetti", st.placed),
            text_dim(),
            format!(
                "Progetti nell'ambito: {}; in {} c'è il dev della condizione, quindi il dev \
                 viene aggiunto (e non si toglie finché la condizione vale).",
                st.governed, st.placed
            ),
        ),
        RuleKind::MoveDev => (
            if st.placed == 0 { "allineata".to_string() } else { format!("{} da spostare", st.placed) },
            text_dim(),
            "Si applica su comando: «Applica ora…» mostra cosa si sposta prima di farlo.".into(),
        ),
        RuleKind::Warning => {
            let tip = format!(
                "Progetti nell'ambito: {}; con l'avviso: {} (controllati solo dove punto e ancora \
                 esistono). L'elenco è in «Avvisi…».",
                st.governed, st.placed
            );
            if st.placed == 0 {
                ("rispettata".into(), text_dim(), tip)
            } else {
                (format!("⚠ {} avvisi", st.placed), warn_amber(), tip)
            }
        }
    };
    let text = std::iter::once(main).chain(extra).collect::<Vec<_>>().join(" · ");
    let tip = std::iter::once(tip).chain(extra_tip).collect::<Vec<_>>().join("\n");
    (text, color, tip)
}

/// Voci delle tendine dei filtri.
const RULE_TYPE_CHOICES: [(RuleTypeFilter, &str); 5] = [
    (RuleTypeFilter::All, "Tutti i tipi"),
    (RuleTypeFilter::Milestone, "Milestone"),
    (RuleTypeFilter::Date, "Date del progetto"),
    (RuleTypeFilter::Dev, "Dev"),
    (RuleTypeFilter::Warning, "Avvisi"),
];
const RULE_ENABLED_CHOICES: [(RuleEnabledFilter, &str); 3] = [
    (RuleEnabledFilter::All, "Attive e spente"),
    (RuleEnabledFilter::Enabled, "Solo attive"),
    (RuleEnabledFilter::Disabled, "Solo spente"),
];

/// La regola passa i filtri della finestra? Ricerca (senza maiuscole) nel
/// nome e nel riassunto (`rule_summary`: milestone, dev, ancora…), tipo
/// del bersaglio, attiva / spenta.
pub(crate) fn rule_passes_filters(
    app: &App,
    id: RuleId,
    r: &Rule,
    search: &str,
    kind: RuleTypeFilter,
    enabled: RuleEnabledFilter,
) -> bool {
    let kind_ok = match kind {
        RuleTypeFilter::All => true,
        RuleTypeFilter::Milestone => matches!(r.target, Target::Milestone(_)),
        RuleTypeFilter::Date => matches!(r.target, Target::ProjectStart | Target::ProjectEnd),
        RuleTypeFilter::Dev => matches!(r.target, Target::AddDev(_) | Target::DevStart(_)),
        RuleTypeFilter::Warning => matches!(r.target, Target::Check(..)),
    };
    let enabled_ok = match enabled {
        RuleEnabledFilter::All => true,
        RuleEnabledFilter::Enabled => r.enabled,
        RuleEnabledFilter::Disabled => !r.enabled,
    };
    let text = format!("{} {}", crate::rules::rule_label(app, id), crate::rules_text::rule_summary(app, r));
    kind_ok && enabled_ok && search_matches(&search.trim().to_lowercase(), &text)
}

/// Stati di progetto a cui vale la regola, oltre agli attivi.
const STATE_CHOICES: [(bool, bool, &str); 4] = [
    (false, false, "Solo attivi"),
    (true, false, "Attivi + on-hold"),
    (false, true, "Attivi + closed"),
    (true, true, "Tutti gli stati"),
];

/// Combo degli stati di progetto (`Rule::inactive`): ritorna la nuova scelta
/// se cambiata.
fn states_combo(ui: &mut egui::Ui, salt: impl std::hash::Hash + std::fmt::Debug, current: Inactive) -> Option<Inactive> {
    let options: Vec<(Inactive, String)> = STATE_CHOICES
        .iter()
        .map(|&(on_hold, closed, l)| (Inactive { on_hold, closed }, l.to_string()))
        .collect();
    choice_combo(ui, salt, RULE_STATES_W, current, &options)
}

/// Una modifica di una regola dalla finestra. Se è una modifica «di
/// struttura» — accesa / spenta, ambito, stati, cosa, ancora — prima si
/// guarda cosa comporta: le modifiche che la regola (se attiva) farebbe ai
/// progetti e le milestone che resterebbero senza regola (se era attiva);
/// se c'è qualcosa, la conferma (`rule_activation_window`), altrimenti si
/// salva subito. Settimane e nome passano diretti (si regolano a piccoli
/// passi).
pub(crate) fn update_or_confirm(
    app: &App,
    state: &mut UiState,
    actions: &mut Vec<Action>,
    id: RuleId,
    before: &Rule,
    after: Rule,
) {
    let structural = after.enabled != before.enabled
        || after.scope != before.scope
        || after.inactive != before.inactive
        || after.target != before.target
        || after.anchor != before.anchor;
    if structural {
        let changes = if after.enabled { crate::rules::preview_changes(app, id, &after) } else { Vec::new() };
        let orphans = if before.enabled { crate::rules::orphans(app, id, Some(&after)) } else { Vec::new() };
        if !changes.is_empty() || !orphans.is_empty() {
            let pending = RulePending::Update { id, rule: after, was_enabled: before.enabled };
            state.rule_activation = Some(RuleActivation { pending, changes, orphans, remove_orphans: false });
            return;
        }
    }
    actions.push(Action::UpdateRule(id, after));
}

/// 🗑 di una regola: se lascerebbe delle milestone senza regola prima si
/// chiede (lasciarle o toglierle), altrimenti si elimina subito.
pub(crate) fn delete_or_confirm(app: &App, state: &mut UiState, actions: &mut Vec<Action>, id: RuleId) {
    let orphans = crate::rules::orphans(app, id, None);
    if orphans.is_empty() {
        actions.push(Action::DeleteRule(id));
    } else {
        state.rule_activation =
            Some(RuleActivation { pending: RulePending::Delete(id), changes: Vec::new(), orphans, remove_orphans: false });
    }
}

/// Spazio sotto l'elenco delle regole: quanto occupa la barra di
/// scorrimento orizzontale, più un margine.
pub(crate) fn rules_bottom_pad(ui: &egui::Ui) -> f32 {
    let sc = &ui.spacing().scroll;
    sc.bar_width + sc.bar_inner_margin + sc.bar_outer_margin + 6.0
}

pub(crate) fn rules_window(ctx: &egui::Context, app: &App, state: &mut UiState, actions: &mut Vec<Action>) {
    if !state.show_rules {
        return;
    }
    let mut open = true;
    egui::Window::new("Regole di automazione")
        .collapsible(false)
        .resizable(true)
        .default_width(1600.0)
        .default_pos(egui::pos2(60.0, 80.0))
        .open(&mut open)
        .show(ctx, |ui| {
            ui.label(
                egui::RichText::new(
                    "Ogni regola mette una milestone (o la data di inizio / fine del progetto) N \
                     settimane prima (-) o dopo (+) un'ancora, nei progetti dell'ambito, e la tiene \
                     allineata: se l'ancora si sposta, la segue; se l'ancora manca, la milestone \
                     non c'è (la data invece resta com'è). Quello che una regola governa non si \
                     modifica a mano. Con più regole sulla stessa \
                     milestone vince la più specifica (progetto, poi categoria, priorità, tutti). \
                     «Aggiungi dev B se c'è il dev A» aggiunge B ai progetti in cui c'è A (e non lo \
                     toglie). «Ore del dev B» sposta in blocco tutte le ore di B perché partano N \
                     settimane dopo l'ancora, solo con «Applica ora…». «Avviso» non cambia nulla: \
                     segnala (⚠ accanto alla tripletta, notifica, «Avvisi…») quando il punto scelto \
                     sta prima / dopo l'ancora ± N. Una regola vale solo per i progetti \
                     attivi, salvo scegliere anche on-hold / closed accanto all'ambito. Prima di \
                     attivare una regola (o di cambiarne ambito, stati, cosa o ancora) si vede \
                     l'elenco di quello che cambierebbe, da confermare.",
                )
                .small()
                .color(text_dim()),
            );
            ui.horizontal(|ui| {
                if ui.button("+ Nuova regola").clicked() {
                    actions.push(Action::AddRule(new_rule(app)));
                }
                ui.separator();
                if ui
                    .add_enabled(!app.rules.is_empty(), egui::Button::new("Esporta in testo…"))
                    .on_hover_text("Salva le regole in un file di testo leggibile (riferimenti per nome).")
                    .clicked()
                {
                    actions.push(Action::ExportRulesText);
                }
                if ui
                    .button("Importa da testo…")
                    .on_hover_text("Aggiunge le regole di un file di testo (quelle identiche si saltano).")
                    .clicked()
                {
                    actions.push(Action::ImportRulesText);
                }
                if ui
                    .add_enabled(!app.rules.is_empty(), egui::Button::new("Log…"))
                    .on_hover_text("Salva un file di testo che spiega, regola per regola e progetto per progetto, cosa fa ogni regola e perché.")
                    .clicked()
                {
                    actions.push(Action::ExportRulesLog);
                }
                ui.separator();
                let n = crate::rules::all_warnings(app).len();
                let label = if n == 0 { "Avvisi…".to_string() } else { format!("⚠ Avvisi ({n})…") };
                if ui.button(label).on_hover_text("Elenco degli avvisi delle regole in tutti i progetti.").clicked() {
                    state.show_rule_warnings = true;
                }
                ui.label(egui::RichText::new("(nasce spenta: configurala, poi spunta «Attiva»)").small().color(text_dim()));
            });
            ui.separator();
            let all: Vec<(RuleId, Rule)> = app.rules.list().into_iter().map(|(id, r)| (id, r.clone())).collect();
            if all.is_empty() {
                ui.label(egui::RichText::new("Nessuna regola.").color(text_dim()));
                return;
            }
            // Filtri: ricerca, tipo, attive / spente.
            ui.horizontal(|ui| {
                ui.label("Cerca:");
                ui.add(
                    egui::TextEdit::singleline(&mut state.rules_search)
                        .desired_width(220.0)
                        .hint_text("nome, milestone, dev…"),
                );
                let types: Vec<(RuleTypeFilter, String)> =
                    RULE_TYPE_CHOICES.iter().map(|(v, l)| (*v, l.to_string())).collect();
                if let Some(t) = choice_combo(ui, "rules_type_filter", 150.0, state.rules_type_filter, &types) {
                    state.rules_type_filter = t;
                }
                let ens: Vec<(RuleEnabledFilter, String)> =
                    RULE_ENABLED_CHOICES.iter().map(|(v, l)| (*v, l.to_string())).collect();
                if let Some(e) = choice_combo(ui, "rules_enabled_filter", 130.0, state.rules_enabled_filter, &ens) {
                    state.rules_enabled_filter = e;
                }
                let filtering = !state.rules_search.trim().is_empty()
                    || state.rules_type_filter != RuleTypeFilter::All
                    || state.rules_enabled_filter != RuleEnabledFilter::All;
                if filtering && ui.button("✖ Filtri").on_hover_text("Mostra tutte le regole").clicked() {
                    state.rules_search.clear();
                    state.rules_type_filter = RuleTypeFilter::All;
                    state.rules_enabled_filter = RuleEnabledFilter::All;
                }
            });
            let rules: Vec<(RuleId, Rule)> = all
                .iter()
                .filter(|(id, r)| {
                    rule_passes_filters(app, *id, r, &state.rules_search, state.rules_type_filter, state.rules_enabled_filter)
                })
                .cloned()
                .collect();
            if rules.len() < all.len() {
                ui.label(
                    egui::RichText::new(format!("{} di {} regole", rules.len(), all.len())).small().color(text_dim()),
                );
            }
            if rules.is_empty() {
                ui.label(egui::RichText::new("Nessuna regola corrisponde ai filtri.").color(text_dim()));
                return;
            }

            egui::ScrollArea::both()
                .scroll_source(drag_scroll())
                .max_height(ui.available_height().max(120.0))
                .show(ui, |ui| {
                    egui::Grid::new("rules_grid").striped(true).spacing(egui::vec2(8.0, 6.0)).show(ui, |ui| {
                        for h in ["Attiva", "Nome", "Progetti", "Cosa", "", "Ancora", "Settimane", "Stato", ""] {
                            ui.label(egui::RichText::new(h).strong());
                        }
                        ui.end_row();
                        for (id, rule) in rules {
                            let mut r = rule.clone();
                            ui.checkbox(&mut r.enabled, "");
                            // Nome: si conferma uscendo dal campo (Esc annulla).
                            let buf = state.rule_names.entry(id).or_insert_with(|| rule.name.clone());
                            // Dimensione esatta: dentro una griglia una
                            // `TextEdit` non va oltre la larghezza che la
                            // colonna ha preso al primo frame.
                            let resp = ui.add_sized(
                                [RULE_NAME_W, ui.spacing().interact_size.y],
                                egui::TextEdit::singleline(buf).hint_text(format!("Regola {}", id.0)),
                            );
                            if resp.lost_focus() {
                                if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                                    *buf = rule.name.clone();
                                } else {
                                    r.name = buf.trim().to_string();
                                }
                            }
                            // Milestone di un progetto: vale solo lì, quindi
                            // l'ambito è quel progetto e non si sceglie.
                            let own = r.target.milestone().and_then(|m| app.milestones.owner(m)).is_some();
                            ui.horizontal(|ui| {
                                let scope = ui.add_enabled_ui(!own, |ui| {
                                    scope_combo(ui, app, ("rule_scope", id), &r.scope)
                                });
                                if own {
                                    scope.response.on_hover_text(
                                        "Milestone di un solo progetto: la regola vale solo lì.",
                                    );
                                }
                                if let Some(s) = scope.inner {
                                    r.scope = s;
                                }
                                if let Some(i) = states_combo(ui, ("rule_states", id), r.inactive) {
                                    r.inactive = i;
                                }
                            });
                            // «Cosa»; per un avviso, accanto il punto e il confronto.
                            ui.horizontal(|ui| {
                                let targets = target_options(app, r.target);
                                if let Some(t) = choice_combo(ui, ("rule_target", id), RULE_TARGET_W, r.target, &targets) {
                                    r.target = t;
                                }
                                if let Target::Check(_, cmp) = r.target {
                                    ui.checkbox(&mut r.check.block, "Blocca").on_hover_text(
                                        "Impedisce le modifiche che creano o peggiorano una violazione: \
                                         vengono annullate e una notifica spiega perché.",
                                    );
                                    // Uno o più punti (dev, milestone…): si
                                    // controlla ogni coppia punto × ancora.
                                    let points = point_options(app);
                                    let mut pts = r.points();
                                    if let Some(next) = multi_anchor_combo(ui, ("rule_point", id), RULE_POINT_W, &pts, &points) {
                                        pts = next;
                                    }
                                    let cmps = [(Cmp::NotBefore, "non prima di".to_string()), (Cmp::NotAfter, "non dopo".to_string())];
                                    let c = choice_combo(ui, ("rule_cmp", id), 110.0, cmp, &cmps).unwrap_or(cmp);
                                    r.target = Target::Check(pts[0], c);
                                    r.check.points = pts[1..].to_vec();
                                }
                            });
                            // Fuori dagli avvisi le opzioni d'avviso non servono.
                            let check = matches!(r.target, Target::Check(..));
                            if !check {
                                r.check = Default::default();
                            }
                            r.scope = auto_scope(app, r.target, r.scope.clone());
                            // Ancore adatte al bersaglio: se il bersaglio
                            // cambia tipo, l'ancora passa alla prima adatta.
                            let anchors = anchor_options(app, r.target);
                            if !crate::rules::anchor_fits(r.target, r.anchor)
                                && let Some((a, _)) = anchors.first()
                            {
                                r.anchor = *a;
                            }
                            r.check.anchors.retain(|a| crate::rules::anchor_fits(r.target, *a));
                            let add_dev = r.target.kind() == crate::rules::RuleKind::AddDev;
                            ui.label(if add_dev { "se" } else if check { "" } else { "=" });
                            if check {
                                // Avviso: una o più ancore (dev, milestone…).
                                if let Some(next) =
                                    multi_anchor_combo(ui, ("rule_anchor", id), RULE_ANCHOR_W, &r.anchors(), &anchors)
                                {
                                    r.anchor = next[0];
                                    r.check.anchors = next[1..].to_vec();
                                }
                            } else if let Some(a) = choice_combo(ui, ("rule_anchor", id), RULE_ANCHOR_W, r.anchor, &anchors) {
                                r.anchor = a;
                            }
                            // «Aggiungi dev» non conta settimane.
                            ui.add_enabled_ui(!add_dev, |ui| {
                                let plus = if r.offset_weeks > 0 { "+" } else { "" };
                                ui.add(egui::DragValue::new(&mut r.offset_weeks).range(-520..=520).prefix(plus))
                                    .on_hover_text("Settimane dopo (+) o prima (-) dell'ancora.");
                            });
                            let (text, color, tip) = status_text(app, id, &rule);
                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new(text).color(color)).on_hover_text(tip);
                                // Spostamento delle ore: su comando, con anteprima.
                                if rule.target.kind() == crate::rules::RuleKind::MoveDev {
                                    let todo = rule.enabled && !crate::rules::pending_moves(app, id).is_empty();
                                    let btn = ui.add_enabled(todo, egui::Button::new("Applica ora…"));
                                    if btn
                                        .on_hover_text("Mostra quali ore si spostano, poi chiede conferma.")
                                        .clicked()
                                    {
                                        state.rule_preview = Some(id);
                                    }
                                }
                            });
                            ui.horizontal(|ui| {
                                if ui
                                    .button("Clona")
                                    .on_hover_text("Aggiunge in fondo una copia della regola (spenta, da modificare e poi attivare).")
                                    .clicked()
                                {
                                    actions.push(Action::AddRule(cloned_rule(app, id, &rule)));
                                }
                                if ui
                                    .button("🗑")
                                    .on_hover_text("Elimina la regola (se ha collocato delle milestone, chiede se lasciarle o toglierle).")
                                    .clicked()
                                {
                                    delete_or_confirm(app, state, actions, id);
                                }
                            });
                            ui.end_row();
                            if r != rule {
                                update_or_confirm(app, state, actions, id, &rule, r);
                            }
                        }
                    });
                    // Spazio sotto l'ultima regola: la barra di scorrimento
                    // orizzontale (fluttuante) non copre più l'ultima riga.
                    ui.add_space(rules_bottom_pad(ui));
                });
        });
    if !open {
        state.show_rules = false;
        state.rule_names.clear();
    }
}

/// Conferma dell'eliminazione di una milestone usata da qualche regola: le
/// regole restano ma, senza la milestone, vengono segnalate.
pub(crate) fn confirm_delete_milestone_window(
    ctx: &egui::Context,
    app: &App,
    state: &mut UiState,
    actions: &mut Vec<Action>,
) {
    let Some(m) = state.confirm_delete_milestone else {
        return;
    };
    let name = app.milestones.get_name(m).unwrap_or("?").to_string();
    let rules: Vec<String> = app
        .rules
        .using_milestone(m)
        .into_iter()
        .map(|id| format!("«{}»", crate::rules::rule_label(app, id)))
        .collect();
    let body = format!(
        "La milestone «{name}» è usata dalle regole {}. Eliminandola le regole restano, \
         ma non agiscono più e sono segnalate in «Regole di automazione».",
        rules.join(", ")
    );
    match confirm_window(ctx, "Eliminare la milestone?", "Elimina", |ui| {
        ui.label(body);
    }) {
        Some(true) => {
            actions.push(Action::DeleteMilestone { milestone: m });
            state.confirm_delete_milestone = None;
        }
        Some(false) => state.confirm_delete_milestone = None,
        None => {}
    }
}

/// Anteprima di «Applica ora…» di una regola «Ore del dev»: elenco degli
/// spostamenti (progetto, prima settimana prima → dopo, settimane, fuori dal
/// periodo), poi «Sposta le ore» → `Action::ApplyRuleNow`.
pub(crate) fn rule_preview_window(
    ctx: &egui::Context,
    app: &App,
    state: &mut UiState,
    actions: &mut Vec<Action>,
) {
    let Some(id) = state.rule_preview else {
        return;
    };
    let moves = crate::rules::pending_moves(app, id);
    let dev = app.rules.get(id).map(|r| r.target_dev());
    let dev_name = dev.and_then(|d| app.devs.get(d)).map(|d| d.get_name()).unwrap_or_default();
    let mut open = true;
    let mut close = false;
    egui::Window::new(format!("Applica la regola «{}»", crate::rules::rule_label(app, id)))
        .collapsible(false)
        .resizable(true)
        .default_width(560.0)
        .open(&mut open)
        .show(ctx, |ui| {
            if moves.is_empty() {
                ui.label("Niente da spostare: le ore sono già al loro posto.");
                if ui.button("Chiudi").clicked() {
                    close = true;
                }
                return;
            }
            ui.label(format!(
                "Le ore del dev «{dev_name}» si spostano in blocco (stessi worker, stessa forma) in {} progetti:",
                moves.len()
            ));
            egui::ScrollArea::vertical()
                .scroll_source(drag_scroll())
                .max_height(320.0)
                .show(ui, |ui| {
                    egui::Grid::new("rule_preview").striped(true).show(ui, |ui| {
                        for h in ["Progetto", "Prima settimana", "", "Spostamento"] {
                            ui.label(egui::RichText::new(h).strong());
                        }
                        ui.end_row();
                        for m in &moves {
                            ui.label(project_label(app, m.proj));
                            ui.label(format!("{} → {}", week_label(m.from.0 as i32), week_label(m.to.0 as i32)));
                            if m.outside {
                                ui.label(egui::RichText::new("⚠ fuori dal periodo").color(warn_amber()))
                                    .on_hover_text("Dopo lo spostamento alcune ore cadono fuori dal periodo del progetto: restano, ma vanno controllate.");
                            } else {
                                ui.label("");
                            }
                            let w = m.weeks();
                            ui.label(format!("{}{w} sett.", if w > 0 { "+" } else { "" }));
                            ui.end_row();
                        }
                    });
                });
            ui.separator();
            ui.horizontal(|ui| {
                if ui.button("Sposta le ore").clicked() {
                    actions.push(Action::ApplyRuleNow(id));
                    close = true;
                }
                if ui.button("Annulla").clicked() {
                    close = true;
                }
            });
        });
    if !open || close {
        state.rule_preview = None;
    }
}

/// Conferma di una modifica alle regole — una regola attivata, modificata,
/// spenta o eliminata, o le regole di un'importazione da testo: l'elenco
/// delle modifiche che farebbe ai progetti (con lo stato, se non è attivo) e
/// delle milestone che resterebbero senza regola, con la spunta per
/// toglierle; poi «Applica» (→ `UpdateRule` / `DeleteRule` / `ImportRules`,
/// più un `RemoveProjectMilestone` per ogni milestone da togliere) o
/// «Annulla» (nulla cambia).
pub(crate) fn rule_activation_window(
    ctx: &egui::Context,
    app: &App,
    state: &mut UiState,
    actions: &mut Vec<Action>,
) {
    let Some(act) = state.rule_activation.as_mut() else {
        return;
    };
    let rule_name = |id: RuleId| crate::rules::rule_label(app, id);
    let (title, who) = match &act.pending {
        RulePending::Update { id, rule, was_enabled } => (
            match (*was_enabled, rule.enabled) {
                (true, false) => "Spegnere la regola?",
                (true, true) => "Modificare la regola?",
                _ => "Attivare la regola?",
            },
            format!("La regola «{}» farebbe", rule_name(*id)),
        ),
        RulePending::Delete(id) => ("Eliminare la regola?", format!("La regola «{}» farebbe", rule_name(*id))),
        RulePending::Import(plan) => (
            "Importare le regole?",
            match plan.rules.len() {
                1 => "La regola da importare farebbe".to_string(),
                n => format!("Le {n} regole da importare farebbero"),
            },
        ),
    };
    // Progetti nell'ordine in cui compaiono.
    let in_order = |ps: &mut Vec<ProjectId>| {
        let mut seen = std::collections::HashSet::new();
        ps.retain(|p| seen.insert(*p));
    };
    let mut projects: Vec<ProjectId> = act.changes.iter().map(|c| c.proj).collect();
    in_order(&mut projects);
    let mut orphan_projects: Vec<ProjectId> = act.orphans.iter().map(|o| o.proj).collect();
    in_order(&mut orphan_projects);
    let project_head = |ui: &mut egui::Ui, p: ProjectId| {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(project_label(app, p)).strong());
            let st = app.projects.status(p);
            if st != ProjectStatus::Active {
                ui.label(egui::RichText::new(st.label()).small().color(warn_amber()));
            }
        });
    };
    let answer = confirm_window(ctx, title, "Applica", |ui| {
        let max_h = ui.ctx().content_rect().height() * if act.orphans.is_empty() || act.changes.is_empty() { 0.5 } else { 0.25 };
        if !act.changes.is_empty() {
            ui.label(format!("{who} {} modifiche in {} progetti:", act.changes.len(), projects.len()));
            egui::ScrollArea::vertical()
                .id_salt("rule_activation_changes")
                .scroll_source(drag_scroll())
                .max_height(max_h)
                .show(ui, |ui| {
                    for &p in &projects {
                        project_head(ui, p);
                        for c in act.changes.iter().filter(|c| c.proj == p) {
                            ui.label(format!("    • {}", c.text));
                        }
                    }
                });
        }
        if !act.orphans.is_empty() {
            if !act.changes.is_empty() {
                ui.separator();
            }
            ui.label(format!(
                "{} milestone in {} progetti resterebbero dove sono, ma senza più una regola che le tenga allineate:",
                act.orphans.len(),
                orphan_projects.len()
            ));
            egui::ScrollArea::vertical()
                .id_salt("rule_activation_orphans")
                .scroll_source(drag_scroll())
                .max_height(max_h)
                .show(ui, |ui| {
                    for &p in &orphan_projects {
                        project_head(ui, p);
                        for o in act.orphans.iter().filter(|o| o.proj == p) {
                            let name = app.milestones.get_name(o.milestone).unwrap_or("?");
                            ui.label(format!("    • {name} ({})", week_label(o.week.0 as i32)));
                        }
                    }
                });
            ui.checkbox(&mut act.remove_orphans, "Toglile dai progetti")
                .on_hover_text("Spenta: restano al loro posto e si spostano a mano.");
            ui.label(
                egui::RichText::new("Le date di inizio / fine e i dev aggiunti dalla regola restano comunque.")
                    .small()
                    .color(text_dim()),
            );
        }
        ui.label(
            egui::RichText::new("Con «Applica» le modifiche si fanno subito (Ctrl+Z le annulla).")
                .small()
                .color(text_dim()),
        );
        ui.separator();
    });
    match answer {
        Some(true) => {
            if let Some(act) = state.rule_activation.take() {
                match act.pending {
                    RulePending::Update { id, rule, .. } => actions.push(Action::UpdateRule(id, rule)),
                    RulePending::Delete(id) => actions.push(Action::DeleteRule(id)),
                    RulePending::Import(plan) => actions.push(Action::ImportRules(plan)),
                }
                // Dopo la regola: così nessuna regola le rimette a posto.
                if act.remove_orphans {
                    for o in act.orphans {
                        actions.push(Action::RemoveProjectMilestone { proj: o.proj, milestone: o.milestone });
                    }
                }
            }
        }
        Some(false) => {
            if let Some(RulePending::Import(_)) = state.rule_activation.take().map(|a| a.pending) {
                state.toast_info("Importazione delle regole annullata: nessuna regola aggiunta.");
            }
        }
        None => {}
    }
}

/// Finestra «Regole di questo progetto»: per ogni regola, se agisce sul
/// progetto e cosa fa lì (la diagnosi del Log, `rules::project_rules`), o
/// perché no; in fondo il pulsante per la finestra delle regole. Si ricalcola
/// a ogni frame, così segue le modifiche fatte con la finestra aperta.
pub(crate) fn project_rules_window(ctx: &egui::Context, app: &App, state: &mut UiState) {
    let Some(proj) = state.project_rules else {
        return;
    };
    if app.projects.get(proj).is_none() {
        state.project_rules = None;
        return;
    }
    let lines = crate::rules::project_rules(app, proj);
    let (here, other): (Vec<_>, Vec<_>) = lines.iter().partition(|l| l.applies);
    let mut open = true;
    let mut open_rules = false;
    egui::Window::new(format!("Regole — {}", project_label(app, proj)))
        .id(egui::Id::unique("project_rules_window"))
        .collapsible(false)
        .resizable(true)
        .default_width(620.0)
        .open(&mut open)
        .show(ctx, |ui| {
            let st = app.projects.status(proj);
            if st != ProjectStatus::Active {
                ui.label(
                    egui::RichText::new(format!(
                        "Progetto {}: valgono solo le regole che comprendono questo stato.",
                        st.label()
                    ))
                    .color(warn_amber()),
                );
            }
            if lines.is_empty() {
                ui.label(egui::RichText::new("Nessuna regola.").color(text_dim()));
            }
            let row = |ui: &mut egui::Ui, l: &crate::rules::ProjectRuleLine| {
                let Some(r) = app.rules.get(l.rule) else { return };
                ui.label(egui::RichText::new(crate::rules::rule_label(app, l.rule)).strong());
                ui.label(
                    egui::RichText::new(crate::rules_text::rule_summary(app, r)).small().color(text_dim()),
                );
                let warn = l.text.starts_with("AVVISO") || l.text.starts_with("ferma");
                let color = if warn { warn_amber() } else { text() };
                ui.label(egui::RichText::new(format!("→ {}", l.text)).color(color));
                ui.add_space(4.0);
            };
            egui::ScrollArea::vertical()
                .scroll_source(drag_scroll())
                .max_height(ui.ctx().content_rect().height() * 0.6)
                .show(ui, |ui| {
                    if !lines.is_empty() {
                        ui.heading(format!("Agiscono qui ({})", here.len()));
                        if here.is_empty() {
                            ui.label(egui::RichText::new("Nessuna.").color(text_dim()));
                        }
                        for l in &here {
                            row(ui, l);
                        }
                    }
                    if !other.is_empty() {
                        ui.separator();
                        egui::CollapsingHeader::new(format!("Non agiscono qui ({})", other.len()))
                            .id_salt("project_rules_other")
                            .show(ui, |ui| {
                                for l in &other {
                                    row(ui, l);
                                }
                            });
                    }
                });
            ui.separator();
            if ui.button("Regole di automazione…").clicked() {
                open_rules = true;
            }
        });
    if open_rules {
        state.show_rules = true;
    }
    if !open {
        state.project_rules = None;
    }
}

/// Finestra «Avvisi delle regole»: per ogni progetto con avvisi, la
/// tripletta (clic = salta al progetto nella griglia), la regola e il testo.
pub(crate) fn rule_warnings_window(ctx: &egui::Context, app: &App, state: &mut UiState) {
    if !state.show_rule_warnings {
        return;
    }
    let list = crate::rules::all_warnings(app);
    let mut open = true;
    egui::Window::new("Avvisi delle regole")
        .collapsible(false)
        .resizable(true)
        .default_width(760.0)
        .default_pos(egui::pos2(120.0, 120.0))
        .open(&mut open)
        .show(ctx, |ui| {
            if list.is_empty() {
                ui.label(egui::RichText::new("Nessun avviso: le regole d'avviso sono tutte rispettate.").color(text_dim()));
                return;
            }
            ui.label(
                egui::RichText::new("Clic sul progetto: lo mostra nella griglia (se è nella vista e nei filtri).")
                    .small()
                    .color(text_dim()),
            );
            egui::ScrollArea::vertical()
                .scroll_source(drag_scroll())
                .max_height(ui.available_height().max(120.0))
                .show(ui, |ui| {
                    egui::Grid::new("rule_warnings").striped(true).spacing(egui::vec2(10.0, 4.0)).show(ui, |ui| {
                        for (proj, w) in &list {
                            if ui.link(project_label(app, *proj)).clicked() {
                                state.jump_to_project = Some(*proj);
                            }
                            ui.label(egui::RichText::new(crate::rules::rule_label(app, w.rule)).strong());
                            ui.label(egui::RichText::new(&w.text).color(warn_amber()));
                            ui.end_row();
                        }
                    });
                });
        });
    if !open {
        state.show_rule_warnings = false;
    }
}

/// Resoconto di «Importa da testo…»: le righe scartate e il motivo.
pub(crate) fn rules_import_report_window(ctx: &egui::Context, state: &mut UiState) {
    let Some(errors) = &state.rules_import_errors else {
        return;
    };
    let mut open = true;
    let mut close = false;
    egui::Window::new("Regole non importate")
        .collapsible(false)
        .resizable(true)
        .default_width(640.0)
        .open(&mut open)
        .show(ctx, |ui| {
            ui.label("Queste regole del file non sono state importate (le altre sì):");
            egui::ScrollArea::vertical()
                .scroll_source(drag_scroll())
                .max_height(320.0)
                .show(ui, |ui| {
                    for e in errors {
                        ui.label(egui::RichText::new(e).color(warn_amber()));
                    }
                });
            ui.separator();
            if ui.button("Chiudi").clicked() {
                close = true;
            }
        });
    if !open || close {
        state.rules_import_errors = None;
    }
}
