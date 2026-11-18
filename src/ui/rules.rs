//! Finestra «Regole di automazione» (Aggiungi ▸ Regole di automazione…):
//! elenco delle regole di `rules.rs`, una per riga, modificabili con elenchi
//! a tendina — attiva, nome, ambito, milestone, ancora, scostamento in
//! settimane — con lo stato (in quanti progetti agisce, ciclo, riferimenti
//! eliminati). Ogni modifica è un'`Action` (`AddRule` / `UpdateRule` /
//! `DeleteRule`); le regole si riapplicano in `mark_changed`.

use super::*;
use crate::rules::{Anchor, Rule, RuleId, RuleScope};

/// Larghezze delle colonne (combo).
const RULE_NAME_W: f32 = 300.0;
const RULE_SCOPE_W: f32 = 170.0;
const RULE_TARGET_W: f32 = 150.0;
const RULE_ANCHOR_W: f32 = 170.0;

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

/// Scelte dell'ambito: tutti, ogni categoria, ogni priorità, ogni progetto.
fn scope_options(app: &App) -> Vec<(RuleScope, String)> {
    let mut out = vec![(RuleScope::All, "Tutti i progetti".to_string())];
    out.extend(app.categories.list().into_iter().map(|(c, n)| (RuleScope::Category(c), format!("Categoria: {n}"))));
    out.extend(app.priorities.list().into_iter().map(|(p, n)| (RuleScope::Priority(p), format!("Priorità: {n}"))));
    out.extend(
        app.projects
            .list()
            .into_iter()
            .map(|(p, _)| (RuleScope::Project(p), format!("Progetto: {}", project_label(app, p)))),
    );
    out
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

/// Scelte dell'ancora: fine / inizio di ogni dev, date del progetto, ogni
/// milestone.
fn anchor_options(app: &App) -> Vec<(Anchor, String)> {
    let mut out = Vec::new();
    for (d, n) in app.devs.list() {
        out.push((Anchor::DevEnd(d), format!("Fine dev {n}")));
        out.push((Anchor::DevStart(d), format!("Inizio dev {n}")));
    }
    out.push((Anchor::ProjectStart, "Inizio progetto".to_string()));
    out.push((Anchor::ProjectEnd, "Fine progetto".to_string()));
    out.extend(milestone_options(app).into_iter().map(|(m, n)| (Anchor::Milestone(m), format!("Milestone {n}"))));
    out
}

/// Ambito della regola con bersaglio `target`: per una milestone di un solo
/// progetto è sempre quel progetto (altrove non si può collocare), altrimenti
/// resta `scope`.
fn auto_scope(app: &App, target: MilestoneId, scope: RuleScope) -> RuleScope {
    app.milestones.owner(target).map_or(scope, RuleScope::Project)
}

/// Regola nuova: spenta (le scelte di partenza non devono toccare i progetti
/// finché non è configurata), prima milestone, fine del primo dev.
pub(crate) fn new_rule(app: &App) -> Option<Rule> {
    let target = app.milestones.list().first()?.0;
    let anchor = app.devs.list().first().map_or(Anchor::ProjectEnd, |(d, _)| Anchor::DevEnd(*d));
    Some(Rule {
        name: String::new(),
        enabled: false,
        scope: auto_scope(app, target, RuleScope::All),
        target,
        anchor,
        offset_weeks: 0,
    })
}

/// Testo dello stato di una regola e il suo colore.
fn status_text(app: &App, id: RuleId, rule: &Rule) -> (String, Color32, String) {
    let st = crate::rules::status(app, id);
    if !st.missing.is_empty() {
        let text = format!("⚠ {} inesistente", st.missing.join(", "));
        return (text, warn_amber(), "Un riferimento della regola è stato eliminato: sceglierne un altro.".into());
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
        .default_width(1300.0)
        .default_pos(egui::pos2(60.0, 80.0))
        .open(&mut open)
        .show(ctx, |ui| {
            ui.label(
                egui::RichText::new(
                    "Ogni regola mette una milestone N settimane prima (-) o dopo (+) un'ancora, \
                     nei progetti dell'ambito, e la tiene allineata: se l'ancora si sposta, la \
                     milestone la segue; se l'ancora manca, la milestone non c'è. Una milestone \
                     governata da una regola non si sposta a mano. Con più regole sulla stessa \
                     milestone vince la più specifica (progetto, poi categoria, priorità, tutti).",
                )
                .small()
                .color(text_dim()),
            );
            let fresh = new_rule(app);
            ui.horizontal(|ui| {
                let btn = ui.add_enabled(fresh.is_some(), egui::Button::new("+ Nuova regola"));
                let btn = btn.on_disabled_hover_text("Crea prima una milestone (Aggiungi ▸ Milestone).");
                if btn.clicked()
                    && let Some(r) = fresh
                {
                    actions.push(Action::AddRule(r));
                }
                ui.label(egui::RichText::new("(nasce spenta: configurala, poi spunta «Attiva»)").small().color(text_dim()));
            });
            ui.separator();
            let rules: Vec<(RuleId, Rule)> = app.rules.list().into_iter().map(|(id, r)| (id, r.clone())).collect();
            if rules.is_empty() {
                ui.label(egui::RichText::new("Nessuna regola.").color(text_dim()));
                return;
            }
            let scopes = scope_options(app);
            let targets = milestone_options(app);
            let anchors = anchor_options(app);
            egui::ScrollArea::both()
                .scroll_source(drag_scroll())
                .max_height(ui.available_height().max(120.0))
                .show(ui, |ui| {
                    egui::Grid::new("rules_grid").striped(true).spacing(egui::vec2(8.0, 6.0)).show(ui, |ui| {
                        for h in ["Attiva", "Nome", "Progetti", "Milestone", "", "Ancora", "Settimane", "Stato", ""] {
                            ui.label(egui::RichText::new(h).strong());
                        }
                        ui.end_row();
                        for (id, rule) in rules {
                            let mut r = rule.clone();
                            ui.checkbox(&mut r.enabled, "");
                            // Nome: si conferma uscendo dal campo (Esc annulla).
                            let buf = state.rule_names.entry(id).or_insert_with(|| rule.name.clone());
                            let resp = ui.add(egui::TextEdit::singleline(buf).desired_width(RULE_NAME_W).hint_text(format!("Regola {}", id.0)));
                            if resp.lost_focus() {
                                if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                                    *buf = rule.name.clone();
                                } else {
                                    r.name = buf.trim().to_string();
                                }
                            }
                            // Milestone di un progetto: vale solo lì, quindi
                            // l'ambito è quel progetto e non si sceglie.
                            let own = app.milestones.owner(r.target).is_some();
                            let scope = ui.add_enabled_ui(!own, |ui| {
                                choice_combo(ui, ("rule_scope", id), RULE_SCOPE_W, r.scope, &scopes)
                            });
                            if own {
                                scope.response.on_hover_text(
                                    "Milestone di un solo progetto: la regola vale solo lì.",
                                );
                            }
                            if let Some(s) = scope.inner {
                                r.scope = s;
                            }
                            if let Some(t) = choice_combo(ui, ("rule_target", id), RULE_TARGET_W, r.target, &targets) {
                                r.target = t;
                            }
                            r.scope = auto_scope(app, r.target, r.scope);
                            ui.label("=");
                            if let Some(a) = choice_combo(ui, ("rule_anchor", id), RULE_ANCHOR_W, r.anchor, &anchors) {
                                r.anchor = a;
                            }
                            let plus = if r.offset_weeks > 0 { "+" } else { "" };
                            ui.add(egui::DragValue::new(&mut r.offset_weeks).range(-520..=520).prefix(plus))
                                .on_hover_text("Settimane dopo (+) o prima (-) dell'ancora.");
                            let (text, color, tip) = status_text(app, id, &rule);
                            ui.label(egui::RichText::new(text).color(color)).on_hover_text(tip);
                            if ui.button("🗑").on_hover_text("Elimina la regola (le milestone restano dove sono).").clicked() {
                                actions.push(Action::DeleteRule(id));
                            }
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
