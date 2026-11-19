//! Finestra «Regole di automazione» (Aggiungi ▸ Regole di automazione…):
//! elenco delle regole di `rules.rs`, una per riga, modificabili con elenchi
//! a tendina — attiva, nome, ambito, milestone, ancora, scostamento in
//! settimane — con lo stato (in quanti progetti agisce, ciclo, riferimenti
//! eliminati). Ogni modifica è un'`Action` (`AddRule` / `UpdateRule` /
//! `DeleteRule`); le regole si riapplicano in `mark_changed`.

use super::*;
use crate::rules::{Anchor, Cmp, Rule, RuleId, RuleScope, Target};

/// Larghezze delle colonne (combo).
pub(crate) const RULE_NAME_W: f32 = 300.0;
const RULE_SCOPE_W: f32 = 170.0;
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
    match rule.target.kind() {
        RuleKind::Aligned => {}
        RuleKind::AddDev => {
            return (
                format!("in {} progetti", st.placed),
                text_dim(),
                format!(
                    "Progetti nell'ambito: {}; in {} c'è il dev della condizione, quindi il dev \
                     viene aggiunto (e non si toglie finché la condizione vale).",
                    st.governed, st.placed
                ),
            );
        }
        RuleKind::MoveDev => {
            let text = if st.placed == 0 { "allineata".to_string() } else { format!("{} da spostare", st.placed) };
            return (
                text,
                text_dim(),
                "Si applica su comando: «Applica ora…» mostra cosa si sposta prima di farlo.".into(),
            );
        }
        RuleKind::Warning => {
            let tip = format!(
                "Progetti nell'ambito: {}; con l'avviso: {} (controllati solo dove punto e ancora \
                 esistono). L'elenco è in «Avvisi…».",
                st.governed, st.placed
            );
            return if st.placed == 0 {
                ("rispettata".into(), text_dim(), tip)
            } else {
                (format!("⚠ {} avvisi", st.placed), warn_amber(), tip)
            };
        }
    }
    (
        format!("in {} progetti", st.placed),
        text_dim(),
        format!(
            "Progetti in cui la regola governa la milestone: {}; con l'ancora presente (milestone collocata): {}. \
             Dove un'altra regola più specifica governa la stessa milestone, vince quella.",
            st.governed, st.placed
        ),
    )
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
                     sta prima / dopo l'ancora ± N.",
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
            let rules: Vec<(RuleId, Rule)> = app.rules.list().into_iter().map(|(id, r)| (id, r.clone())).collect();
            if rules.is_empty() {
                ui.label(egui::RichText::new("Nessuna regola.").color(text_dim()));
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
                            // «Cosa»; per un avviso, accanto il punto e il confronto.
                            ui.horizontal(|ui| {
                                let targets = target_options(app, r.target);
                                if let Some(t) = choice_combo(ui, ("rule_target", id), RULE_TARGET_W, r.target, &targets) {
                                    r.target = t;
                                }
                                if let Target::Check(point, cmp) = r.target {
                                    let points = point_options(app);
                                    let p = choice_combo(ui, ("rule_point", id), RULE_POINT_W, point, &points).unwrap_or(point);
                                    let cmps = [(Cmp::NotBefore, "non prima di".to_string()), (Cmp::NotAfter, "non dopo".to_string())];
                                    let c = choice_combo(ui, ("rule_cmp", id), 110.0, cmp, &cmps).unwrap_or(cmp);
                                    r.target = Target::Check(p, c);
                                }
                            });
                            r.scope = auto_scope(app, r.target, r.scope.clone());
                            // Ancore adatte al bersaglio: se il bersaglio
                            // cambia tipo, l'ancora passa alla prima adatta.
                            let anchors = anchor_options(app, r.target);
                            if !crate::rules::anchor_fits(r.target, r.anchor)
                                && let Some((a, _)) = anchors.first()
                            {
                                r.anchor = *a;
                            }
                            let add_dev = r.target.kind() == crate::rules::RuleKind::AddDev;
                            let check = matches!(r.target, Target::Check(..));
                            ui.label(if add_dev { "se" } else if check { "" } else { "=" });
                            if let Some(a) = choice_combo(ui, ("rule_anchor", id), RULE_ANCHOR_W, r.anchor, &anchors) {
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
                                if ui.button("🗑").on_hover_text("Elimina la regola (le milestone restano dove sono).").clicked() {
                                    actions.push(Action::DeleteRule(id));
                                }
                            });
                            ui.end_row();
                            if r != rule {
                                actions.push(Action::UpdateRule(id, r));
                            }
                        }
                    });
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
