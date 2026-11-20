//! Test dell'interfaccia (`ui`): spostati qui da `mod.rs` (era un unico file di
//! ~6.500 righe). Sono un modulo figlio di `ui`, quindi `use super::*` vede tutto
//! il nucleo e i sottomoduli, come prima.

use super::*;

#[test]
fn restore_dev_entries_keeps_order_and_flags() {
    let (a, b, c, d) = (DevId(0), DevId(1), DevId(2), DevId(3));
    // Nessuna memoria: tutti spuntati nell'ordine del progetto.
    assert_eq!(
        restore_dev_entries(&[a, b], &[]),
        vec![(a, true), (b, true)]
    );
    // Memoria: c, a (a tolto); b è stato rimosso dal progetto, d è nuovo.
    let mem = [(c, true), (b, true), (a, false)];
    assert_eq!(
        restore_dev_entries(&[a, c, d], &mem),
        vec![(c, true), (a, false), (d, true)]
    );
}
use std::cell::Cell;

/// Renderizza `project_checklist` con `n` progetti dentro una finestra grande
/// come `screen_h` px e restituisce l'altezza verticale consumata dalla lista.
fn checklist_height(n: usize, screen_h: f32) -> f32 {
    let app = App::new();
    let mut entries: Vec<(ProjectId, bool)> = (0..n).map(|i| (ProjectId(i), false)).collect();

    let ctx = egui::Context::default();
    let input = egui::RawInput {
        screen_rect: Some(screen(1000.0, screen_h)),
        ..Default::default()
    };

    let consumed = Cell::new(0.0);
    // Due frame: la prima registra le aree, la seconda misura a layout stabile.
    for _ in 0..2 {
        let _ = ctx.run_ui_test(input.clone(), |ui| {
            let ctx = ui.ctx();
            egui::Window::new("test").show(ctx, |ui| {
                let before = ui.cursor().top();
                project_checklist(ui, &app, &mut entries);
                consumed.set(ui.cursor().top() - before);
            });
        });
    }
    consumed.get()
}

#[test]
fn project_checklist_shrinks_and_caps() {
    let screen_h = 400.0;
    let h_empty = checklist_height(0, screen_h);
    let h_one = checklist_height(1, screen_h);
    let h_many = checklist_height(50, screen_h);

    // Nessun panic (arrivare qui basta) e la lista non cresce illimitata:
    // con 50 progetti resta ampiamente sotto le ~50 righe che occuperebbe
    // senza cap, e comunque entro l'altezza dello schermo.
    assert!(
        h_many <= screen_h,
        "lista non limitata: {h_many} > schermo {screen_h}"
    );
    assert!(h_many < 500.0, "cap non applicato: {h_many}");

    // auto_shrink: con pochi progetti la lista è più bassa che con molti.
    assert!(
        h_one < h_many,
        "auto_shrink non attivo: 1 progetto ({h_one}) >= 50 ({h_many})"
    );
    // Con zero progetti la lista è comunque disegnata (Select All + area),
    // quindi non nulla ma piccola.
    assert!(h_empty > 0.0 && h_empty <= h_one + 40.0, "vuota: {h_empty}");
}

#[test]
fn project_checklist_shows_at_most_15_projects() {
    // Schermo alto: il limite è quello delle 15 righe, non lo spazio.
    let mut app = App::new();
    let mut entries = Vec::new();
    for i in 0..50 {
        let trip = format!("Q{i:02}");
        entries.push((app.projects.add("Progetto", Some(&trip), None), false));
    }
    let ctx = egui::Context::default();
    let input = egui::RawInput {
        screen_rect: Some(screen(1000.0, 2000.0)),
        ..Default::default()
    };
    let mut visible = 0;
    for _ in 0..3 {
        let out = ctx.run_ui_test(input.clone(), |ui| {
            // Finestra alta quanto lo schermo: lo spazio non fa da limite.
            egui::Window::new("test")
                .resizable(false)
                .min_height(1800.0)
                .show(ui.ctx(), |ui| {
                    project_checklist(ui, &app, &mut entries);
                });
        });
        // Righe interamente visibili (non tagliate dal clip dell'elenco).
        visible = out
            .shapes
            .iter()
            .filter(|cs| match &cs.shape {
                egui::epaint::Shape::Text(t) => {
                    t.galley.text().starts_with('Q')
                        && cs.clip_rect.contains_rect(t.visual_bounding_rect())
                }
                _ => false,
            })
            .count();
    }
    assert_eq!(visible, MAX_VISIBLE_PROJECTS);
}

/// Esporta multi-progetto su uno schermo basso con tanti progetti: il
/// bottone «Esporta PDF…» deve restare dentro lo schermo (l'elenco scorre).
#[test]
fn multi_export_button_stays_on_a_small_screen() {
    let screen = egui::vec2(1000.0, 600.0);
    let mut app = App::new();
    let mut entries = Vec::new();
    for i in 0..40 {
        let trip = format!("P{i:02}");
        entries.push((app.projects.add("Progetto", Some(&trip), None), true));
    }
    let mut state = UiState {
        pdf_multi_export: Some(PdfMultiExport { entries }),
        ..Default::default()
    };
    let ctx = egui::Context::default();
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, screen)),
        ..Default::default()
    };
    let mut button: Option<egui::Rect> = None;
    for _ in 0..5 {
        let mut actions = Vec::new();
        let out = ctx.run_ui_test(input.clone(), |ui| {
            pdf_multi_export_window(ui.ctx(), &app, &mut state, &mut actions);
        });
        button = None;
        for cs in &out.shapes {
            if let egui::epaint::Shape::Text(t) = &cs.shape
                && t.galley.text() == "Esporta PDF…"
            {
                let r = t.visual_bounding_rect();
                // Visibile solo se non tagliato dal clip della finestra.
                if cs.clip_rect.contains_rect(r) {
                    button = Some(r);
                }
            }
        }
    }
    let r = button.expect("bottone «Esporta PDF…» non visibile");
    assert!(r.bottom() <= screen.y, "bottone fuori schermo: {r:?}");
}

/// L'inserimento multiplo (tasto destro su cella vuota) scrive a tutti i
/// worker scelti per N settimane consecutive, sovrascrive quello che c'era
/// e non esce dalla griglia.
#[test]
fn bulk_fill_writes_all_workers_for_n_weeks() {
    let mut app = App::new();
    let w1 = app.workers.add("Alice");
    let w2 = app.workers.add("Bob");
    let dev = app.devs.add("Frontend");
    let proj = app.projects.add("Progetto", Some("A-B-C"), None);

    let start = app.start_week;
    // Valore preesistente che deve essere sovrascritto.
    app.projects.add_effort(proj, dev, start, w1, Effort(3));

    assert!(bulk_fill_effort(
        &mut app,
        proj,
        dev,
        start,
        &[w1, w2],
        Effort(8),
        3,
        None
    ));

    for i in 0..3 {
        let w = WeekId(start.0 + i * WEEK_STEP);
        let sew = app.projects.get_single_dev(proj, dev).unwrap().get_all(w);
        let sew = sew.unwrap_or_else(|| panic!("settimana {i} non scritta"));
        for wid in [w1, w2] {
            assert_eq!(sew.worker_id.get(&wid).unwrap().get_effort(), Effort(8));
        }
    }
    // La quarta settimana resta intatta.
    assert!(
        app.projects
            .get_single_dev(proj, dev)
            .unwrap()
            .get_all(WeekId(start.0 + 3 * WEEK_STEP))
            .is_none()
    );

    // Oltre la fine della griglia non scrive nulla.
    let after_end = WeekId(app.end_week.0 + WEEK_STEP);
    assert!(!bulk_fill_effort(
        &mut app,
        proj,
        dev,
        after_end,
        &[w1],
        Effort(8),
        5,
        None
    ));
    // Selezione vuota o zero settimane: nessuna scrittura.
    assert!(!bulk_fill_effort(
        &mut app,
        proj,
        dev,
        start,
        &[],
        Effort(8),
        3,
        None
    ));
    assert!(!bulk_fill_effort(
        &mut app,
        proj,
        dev,
        start,
        &[w1],
        Effort(8),
        0,
        None
    ));
}

/// La dialog dell'inserimento multiplo si disegna (due frame, come le altre
/// prove di UI) senza panic e resta aperta finché non si conferma/annulla.
#[test]
fn bulk_fill_window_renders() {
    let mut app = App::new();
    app.workers.add("Alice");
    let dev = app.devs.add("Frontend");
    let proj = app.projects.add("Progetto", Some("A-B-C"), None);

    let mut state = UiState {
        bulk_fill: Some(BulkFill {
            proj,
            dev,
            week: app.start_week.0 as i32,
            search: String::new(),
            selected: HashSet::new(),
            effort_text: "8".to_string(),
            weeks_text: "3".to_string(),
            to_end: false,
            to_milestone: None,
        }),
        ..Default::default()
    };

    let ctx = egui::Context::default();
    let input = egui::RawInput {
        screen_rect: Some(screen(1000.0, 800.0)),
        ..Default::default()
    };
    for _ in 0..2 {
        let mut actions: Vec<Action> = Vec::new();
        let _ = ctx.run_ui_test(input.clone(), |ui| {
            let ctx = ui.ctx();
            bulk_fill_window(ctx, &app, &mut state, &mut actions);
        });
        assert!(
            actions.is_empty(),
            "nessuna azione senza click su Inserisci"
        );
    }
    assert!(state.bulk_fill.is_some(), "la dialog deve restare aperta");
}

/// «oppure fino al:» della dialog di inserimento multiplo: la settimana che
/// contiene la data scelta è l'ultima (inclusa).
#[test]
fn bulk_fill_end_date_counts_weeks_inclusively() {
    let start = parse_date_str("26-11-02").unwrap(); // lunedì
    assert_eq!(bulk_fill_last_week(start, 1), start);
    assert_eq!(bulk_fill_last_week(start, 3), start + 14);
    assert_eq!(bulk_fill_last_week(start, 0), start);
    // Stessa settimana (anche di domenica) → 1; venerdì di due settimane dopo → 3.
    assert_eq!(weeks_until(start, start), 1);
    assert_eq!(weeks_until(start, start + 6), 1);
    assert_eq!(weeks_until(start, start + 18), 3);
    // Prima dell'inizio → ≤ 0 (la dialog lo segnala e disabilita Inserisci).
    assert_eq!(weeks_until(start, start - 1), 0);
    for n in 1..10 {
        assert_eq!(weeks_until(start, bulk_fill_last_week(start, n)), n as i64);
    }
}

/// La dialog mostra la data di fine collegata al numero di settimane.
#[test]
fn bulk_fill_window_shows_the_end_date_of_the_weeks() {
    let mut app = App::new();
    app.workers.add("Alice");
    let dev = app.devs.add("Frontend");
    let proj = app.projects.add("Progetto", Some("A-B-C"), None);
    let start = app.start_week.0 as i32;
    let mut state = UiState {
        bulk_fill: Some(BulkFill {
            weeks_text: "3".to_string(),
            ..BulkFill::new(proj, dev, start)
        }),
        ..Default::default()
    };
    let ctx = egui::Context::default();
    let input = egui::RawInput {
        screen_rect: Some(screen(1000.0, 800.0)),
        ..Default::default()
    };
    let mut texts = Vec::new();
    for _ in 0..2 {
        let mut actions = Vec::new();
        let out = ctx.run_ui_test(input.clone(), |ui| {
            bulk_fill_window(ui.ctx(), &app, &mut state, &mut actions);
        });
        texts = drawn_texts(&out);
    }
    let end = days_to_local(start + 14).format("%Y-%m-%d").to_string();
    assert!(texts.iter().any(|t| t == "oppure fino al:"), "{texts:?}");
    assert!(texts.iter().any(|t| t.starts_with(&end)), "{end}: {texts:?}");
}

/// Tasto destro sulla fascia milestone ▸ «Aggiungi efforts…» apre la dialog di
/// inserimento multiplo su quel dev e su quella settimana.
#[test]
fn add_efforts_menu_item_opens_the_bulk_fill_on_the_clicked_week() {
    let (app, _alice, week) = highlight_fixture();
    let dev = app.devs.list()[0].0;
    let ctx = egui::Context::default();
    let mut state = UiState::default();
    let origin = Cell::new(egui::Pos2::ZERO);
    let frame = |state: &mut UiState, ev: Option<egui::Event>| {
        let mut input = egui::RawInput {
            screen_rect: Some(screen(1600.0, 900.0)),
            ..Default::default()
        };
        input.events.extend(ev);
        let mut actions = Vec::new();
        ctx.run_ui_test(input, |ui| {
            origin.set(ui.cursor().min);
            let layout = body_layout(ui, &app, state, &Filter::default(), &None);
            grid(ui, &app, state, &mut actions, &Filter::default(), &layout);
        })
    };
    let _ = frame(&mut state, None);
    // Seconda colonna, fascia in alto.
    let strip = egui::pos2(origin.get().x + COL_W * 1.5, origin.get().y + 1.0);
    for ev in click_events(strip, egui::PointerButton::Secondary) {
        let _ = frame(&mut state, Some(ev));
    }
    let out = frame(&mut state, None);
    let texts = drawn_texts(&out);
    assert!(texts.iter().any(|t| t == "Sposta"), "{texts:?}");
    let item = text_pos(&out, "Aggiungi efforts…").expect("voce «Aggiungi efforts…»");
    for ev in click_events(item, egui::PointerButton::Primary) {
        let _ = frame(&mut state, Some(ev));
    }
    let bf = state.bulk_fill.as_ref().expect("dialog di inserimento multiplo aperta");
    assert_eq!((bf.proj, bf.dev, bf.week), (ProjectId(0), dev, week + WEEK_STEP as i32));
    assert_eq!(bf.weeks_text, "1");
}

/// Progetto 20000..=20070 (11 settimane) con un dev e un worker, per i
/// test di «Sposta inizio/fine progetto qua».
fn bound_fixture() -> (App, ProjectId, DevId, WorkerId) {
    let mut app = App::new();
    let proj = app.projects.add("Prog", Some("ABC"), Some(WeekId(20000)));
    app.projects.set_project_end_week(proj, Some(WeekId(20070)));
    let dev = app.devs.add("Frontend");
    app.projects.add_dev(proj, dev);
    let w = app.workers.add("Alice");
    (app, proj, dev, w)
}

#[test]
fn bound_menu_items_follow_the_click_position() {
    let (mut app, proj, _, _) = bound_fixture();
    let kinds = |app: &App, w: usize| -> Vec<(ProjectBound, bool)> {
        bound_menu_items(app, proj, WeekId(w))
            .into_iter()
            .map(|(b, _, on)| (b, on))
            .collect()
    };
    use ProjectBound::{End, Start};
    // Prima dell'inizio: solo l'inizio; dopo la fine: solo la fine.
    assert_eq!(kinds(&app, 19993), vec![(Start, true)]);
    assert_eq!(kinds(&app, 20077), vec![(End, true)]);
    // Dentro: entrambe; sull'inizio/sulla fine quella voce è disabilitata.
    assert_eq!(kinds(&app, 20035), vec![(Start, true), (End, true)]);
    assert_eq!(kinds(&app, 20000), vec![(Start, false), (End, true)]);
    assert_eq!(kinds(&app, 20070), vec![(Start, true), (End, false)]);
    // Fine mancante: «Imposta fine» ovunque dopo l'inizio, «Sposta inizio» ovunque.
    app.projects.set_project_end_week(proj, None);
    let items = bound_menu_items(&app, proj, WeekId(20100));
    assert_eq!(
        items.iter().map(|(_, l, _)| *l).collect::<Vec<_>>(),
        vec!["Sposta inizio progetto qua", "Imposta fine progetto qua"]
    );
}

#[test]
fn moving_a_bound_inward_reports_and_deletes_what_falls_outside() {
    let (mut app, proj, dev, w) = bound_fixture();
    for wk in [20000, 20007, 20014, 20063, 20070] {
        app.projects.add_effort(proj, dev, WeekId(wk), w, Effort(8));
    }
    // Anche un'assegnazione a effort 0 conta come dato inserito.
    app.projects.add_effort(proj, dev, WeekId(20021), w, Effort(0));
    let ms = app.milestones.add("Kickoff");
    app.projects.add_project_milestone(proj, ms, WeekId(20007));
    let keep = app.milestones.add("Demo");
    app.projects.add_project_milestone(proj, keep, WeekId(20035));

    let loss = bound_move_loss(&app, proj, ProjectBound::Start, WeekId(20028));
    assert_eq!(
        loss.efforts,
        [20000, 20007, 20014, 20021]
            .iter()
            .map(|w| (dev, WeekId(*w)))
            .collect::<Vec<_>>()
    );
    assert_eq!(loss.milestones, vec![(ms, WeekId(20007))]);

    assert!(move_project_bound(&mut app, proj, ProjectBound::Start, WeekId(20028)));
    assert_eq!(app.projects.get_project_start_week(proj), Some(WeekId(20028)));
    let sd = app.projects.get_single_dev(proj, dev).unwrap();
    assert_eq!(sd.get_weeks(), vec![WeekId(20063), WeekId(20070)]);
    assert_eq!(
        app.projects.list_project_milestones(proj),
        vec![(keep, WeekId(20035))]
    );

    // Fine anticipata: cadono gli effort dopo la nuova fine.
    let loss = bound_move_loss(&app, proj, ProjectBound::End, WeekId(20063));
    assert_eq!(loss.efforts, vec![(dev, WeekId(20070))]);
    assert!(loss.milestones.is_empty());
}

#[test]
fn moving_a_bound_outward_loses_nothing() {
    let (mut app, proj, dev, w) = bound_fixture();
    app.projects.add_effort(proj, dev, WeekId(20000), w, Effort(8));
    app.projects.add_effort(proj, dev, WeekId(20070), w, Effort(8));
    assert!(bound_move_loss(&app, proj, ProjectBound::Start, WeekId(19972)).is_empty());
    assert!(bound_move_loss(&app, proj, ProjectBound::End, WeekId(20140)).is_empty());
    assert!(move_project_bound(&mut app, proj, ProjectBound::End, WeekId(20140)));
    assert_eq!(app.projects.get_project_end_week(proj), Some(WeekId(20140)));
    // Già lì: nessun cambiamento (il file non va marcato modificato).
    assert!(!move_project_bound(&mut app, proj, ProjectBound::End, WeekId(20140)));
}

#[test]
fn clear_project_milestones_removes_placements_but_keeps_definitions() {
    let (mut app, proj, _dev, _w) = bound_fixture();
    let other = app.projects.add("Altro", Some("XYZ"), Some(WeekId(20000)));
    let global = app.milestones.add("Rilascio");
    let own = app.milestones.add_owned(
        "Collaudo",
        crate::milestones::MilestoneKind::default(),
        Vec::new(),
        Some(proj),
    );
    app.projects.add_project_milestone(proj, global, WeekId(20007));
    app.projects.add_project_milestone(proj, own, WeekId(20014));
    app.projects.add_project_milestone(other, global, WeekId(20021));

    assert!(app.projects.clear_project_milestones(proj));
    assert!(app.projects.list_project_milestones(proj).is_empty());
    assert_eq!(
        app.projects.list_project_milestones(other),
        vec![(global, WeekId(20021))],
        "gli altri progetti non cambiano"
    );
    assert!(app.milestones.get(global).is_some(), "la globale resta definita");
    assert!(app.milestones.get(own).is_some(), "la propria resta definita");
    assert!(!app.projects.clear_project_milestones(proj), "già vuoto: nessun cambio");
}

#[test]
fn clear_milestones_window_asks_then_confirms() {
    let (mut app, proj, _dev, _w) = bound_fixture();
    let m = app.milestones.add("Rilascio");
    app.projects.add_project_milestone(proj, m, WeekId(20007));
    let ctx = egui::Context::default();
    let input = egui::RawInput {
        screen_rect: Some(screen(1000.0, 800.0)),
        ..Default::default()
    };
    let mut state = UiState {
        confirm_clear_milestones: Some(proj),
        ..Default::default()
    };
    let mut button = None;
    let mut texts = Vec::new();
    for _ in 0..2 {
        let mut actions = Vec::new();
        let out = ctx.run_ui_test(input.clone(), |ui| {
            clear_milestones_window(ui.ctx(), &app, &mut state, &mut actions);
        });
        assert!(actions.is_empty(), "nessuna azione senza click");
        texts.clear();
        for cs in &out.shapes {
            if let egui::epaint::Shape::Text(t) = &cs.shape {
                let txt = t.galley.text().to_string();
                if txt == "Sì, cancella" {
                    button = Some(t.visual_bounding_rect().center());
                }
                texts.push(txt);
            }
        }
    }
    assert!(texts.iter().any(|t| t.contains("Rilascio")), "{texts:?}");
    assert!(state.confirm_clear_milestones.is_some(), "senza conferma resta aperta");

    let pos = button.expect("bottone «Sì, cancella»");
    let mut actions = Vec::new();
    for ev in click_events(pos, egui::PointerButton::Primary) {
        let mut i = input.clone();
        i.events = vec![ev];
        let _ = ctx.run_ui_test(i, |ui| {
            clear_milestones_window(ui.ctx(), &app, &mut state, &mut actions);
        });
    }
    assert!(
        matches!(actions.as_slice(), [Action::ClearProjectMilestones { proj: p }] if *p == proj),
        "attesa una sola ClearProjectMilestones"
    );
    assert!(state.confirm_clear_milestones.is_none());
}

#[test]
fn clear_efforts_empties_one_dev_or_all_devs_keeping_planned() {
    let (mut app, proj, dev, w) = bound_fixture();
    let dev2 = app.devs.add("Backend");
    app.projects.add_dev(proj, dev2);
    app.projects.add_dev_effort(proj, dev, Effort(100));
    app.projects.add_effort(proj, dev, WeekId(20000), w, Effort(8));
    app.projects.add_effort(proj, dev, WeekId(20007), w, Effort(0));
    app.projects.set_note(proj, dev, WeekId(20000), w, "nota");
    app.projects.set_dev_note(proj, dev, "nota dev");
    app.projects.add_effort(proj, dev2, WeekId(20014), w, Effort(4));

    // un solo dev: le sue celle (anche a effort 0) spariscono, l'altro resta
    assert!(app.projects.clear_efforts(proj, Some(dev)));
    let sd = app.projects.get_single_dev(proj, dev).unwrap();
    assert!(!sd.has_cells());
    assert_eq!(sd.planned_effort(), Effort(100), "il pianificato resta");
    assert_eq!(sd.get_note(), "nota dev", "la nota dev resta");
    assert!(app.projects.get_single_dev(proj, dev2).unwrap().has_cells());
    assert!(!app.projects.clear_efforts(proj, Some(dev)), "già vuoto: nessun cambio");

    // tutti i dev del progetto
    assert!(app.projects.clear_efforts(proj, None));
    assert!(!app.projects.get_single_dev(proj, dev2).unwrap().has_cells());
    assert!(!app.projects.clear_efforts(proj, None));
}

#[test]
fn clear_efforts_window_asks_then_confirms() {
    let (mut app, proj, dev, w) = bound_fixture();
    app.projects.add_effort(proj, dev, WeekId(20000), w, Effort(8));
    let ctx = egui::Context::default();
    let input = egui::RawInput {
        screen_rect: Some(screen(1000.0, 800.0)),
        ..Default::default()
    };
    let mut state = UiState {
        confirm_clear_efforts: Some(ClearEfforts {
            proj,
            dev: Some(dev),
        }),
        ..Default::default()
    };
    let mut button = None;
    let mut texts = Vec::new();
    for _ in 0..2 {
        let mut actions = Vec::new();
        let out = ctx.run_ui_test(input.clone(), |ui| {
            clear_efforts_window(ui.ctx(), &app, &mut state, &mut actions);
        });
        assert!(actions.is_empty(), "nessuna azione senza click");
        texts.clear();
        for cs in &out.shapes {
            if let egui::epaint::Shape::Text(t) = &cs.shape {
                let txt = t.galley.text().to_string();
                if txt == "Sì, cancella" {
                    button = Some(t.visual_bounding_rect().center());
                }
                texts.push(txt);
            }
        }
    }
    assert!(texts.iter().any(|t| t.contains("«Frontend»")), "{texts:?}");
    assert!(state.confirm_clear_efforts.is_some(), "senza conferma resta aperta");

    // click su «Sì, cancella» → azione + dialog chiusa
    let pos = button.expect("bottone «Sì, cancella»");
    let mut actions = Vec::new();
    for ev in click_events(pos, egui::PointerButton::Primary) {
        let mut i = input.clone();
        i.events = vec![ev];
        let _ = ctx.run_ui_test(i, |ui| {
            clear_efforts_window(ui.ctx(), &app, &mut state, &mut actions);
        });
    }
    assert!(
        matches!(
            actions.as_slice(),
            [Action::ClearEfforts { proj: p, dev: Some(d) }] if *p == proj && *d == dev
        ),
        "attesa una sola ClearEfforts"
    );
    assert!(state.confirm_clear_efforts.is_none());
}

#[test]
fn bound_move_window_confirms_or_cancels() {
    let (mut app, proj, dev, w) = bound_fixture();
    app.projects.add_effort(proj, dev, WeekId(20000), w, Effort(8));
    let ctx = egui::Context::default();
    let input = egui::RawInput {
        screen_rect: Some(screen(1000.0, 800.0)),
        ..Default::default()
    };
    let loss = bound_move_loss(&app, proj, ProjectBound::Start, WeekId(20014));
    let mut state = UiState {
        bound_move: Some(BoundMove {
            proj,
            bound: ProjectBound::Start,
            week: WeekId(20014),
            loss,
        }),
        ..Default::default()
    };
    let mut texts = Vec::new();
    for _ in 0..2 {
        let mut actions = Vec::new();
        let out = ctx.run_ui_test(input.clone(), |ui| {
            bound_move_window(ui.ctx(), &app, &mut state, &mut actions);
        });
        assert!(actions.is_empty(), "nessuna azione senza click");
        texts = drawn_texts(&out);
    }
    assert!(texts.iter().any(|t| t.contains("Frontend: 1 settimana, 8 h")), "{texts:?}");
    assert!(texts.iter().any(|t| t == "Cancella e sposta"));
    assert!(state.bound_move.is_some(), "la conferma deve restare aperta");
}

/// Periodo dell'export di un progetto: alla prima apertura inizio/fine
/// del progetto, poi resta quello scelto (anche se le date del progetto
/// cambiano); ogni progetto ha il suo.
#[test]
fn export_range_starts_from_project_dates_and_is_remembered_per_project() {
    let mut app = App::new();
    let a = app.projects.add("A", Some("AAA"), Some(WeekId(20000)));
    app.projects.set_project_end_week(a, Some(WeekId(20140)));
    let b = app.projects.add("B", Some("BBB"), Some(WeekId(20210)));
    app.projects.set_project_end_week(b, Some(WeekId(20350)));
    let mut pjm = PjmApp {
        app,
        ui: UiState::default(),
    };
    let ctx = egui::Context::default();
    let input = egui::RawInput {
        screen_rect: Some(screen(1536.0, 864.0)),
        ..Default::default()
    };
    // Mostra solo `p` (l'export di un solo progetto apre la sua dialog),
    // la apre, ci disegna sopra un frame e la chiude.
    let open = |pjm: &mut PjmApp, p: ProjectId, other: ProjectId| {
        pjm.app.projects.set_enable(p, Enable(true));
        pjm.app.projects.set_enable(other, Enable(false));
        pjm.apply(Action::ExportPdf);
        pjm.ui.pdf_export.as_ref().expect("dialog del progetto").range
    };
    let frame = |pjm: &mut PjmApp| {
        let mut actions = Vec::new();
        let _ = ctx.run_ui_test(input.clone(), |ui| {
            pdf_export_window(ui.ctx(), &pjm.app, &mut pjm.ui, &mut actions);
        });
        pjm.ui.pdf_export = None;
    };

    assert_eq!(open(&mut pjm, a, b), (20000, 20140));
    pjm.ui.pdf_export.as_mut().unwrap().range = (20063, 20105);
    frame(&mut pjm);

    // Le date del progetto cambiano: il periodo scelto non si tocca.
    pjm.app.projects.set_project_end_week(a, Some(WeekId(20203)));
    assert_eq!(open(&mut pjm, a, b), (20063, 20105));
    frame(&mut pjm);

    // Un altro progetto parte dalle sue date…
    assert_eq!(open(&mut pjm, b, a), (20210, 20350));
    frame(&mut pjm);
    // …e tornando al primo si ritrova il suo periodo.
    assert_eq!(open(&mut pjm, a, b), (20063, 20105));
}

/// Un dev escluso dai totali sparisce dal riquadro in basso a sinistra
/// del footer e le sue ore non entrano nel «Totale»; gli altri restano.
#[test]
fn dev_hidden_in_totals_leaves_footer_row_and_grand_total() {
    let mut app = App::new();
    let proj = app.projects.add("Prog", Some("ABC"), Some(WeekId(20100)));
    let front = app.devs.add("Frontend");
    let back = app.devs.add("Backend");
    let w = app.workers.add("Alice");
    for (dev, h) in [(front, 10), (back, 32)] {
        app.projects.add_dev(proj, dev);
        // 20100 = 12 gennaio 2025.
        app.projects.add_effort(proj, dev, WeekId(20100), w, Effort(h));
    }
    let ctx = egui::Context::default();
    let input = egui::RawInput {
        screen_rect: Some(screen(1200.0, 800.0)),
        ..Default::default()
    };
    let texts = |app: &App| -> Vec<String> {
        let mut state = UiState {
            selected_year: 2025,
            ..Default::default()
        };
        let workers = footer_workers(app, &Filter::default());
        let mut actions = Vec::new();
        let out = ctx.run_ui_test(input.clone(), |ui| {
            let (rect, _) =
                ui.allocate_exact_size(egui::vec2(LEFT_W, 400.0), Sense::hover());
            draw_left_footer(ui, rect, app, &mut state, &workers, &mut actions);
        });
        drawn_texts(&out)
    };

    let all = texts(&app);
    assert!(all.contains(&"Backend".to_string()));
    assert!(all.contains(&"42".to_string()), "totale con entrambi: {all:?}");

    assert!(app.devs.set_hidden_in_totals(back, true));
    assert!(!app.devs.set_hidden_in_totals(back, true), "già nascosto");
    let some = texts(&app);
    assert!(!some.contains(&"Backend".to_string()), "riga del dev nascosto");
    assert!(some.contains(&"Frontend".to_string()));
    assert!(!some.contains(&"42".to_string()) && !some.contains(&"32".to_string()));
    // «Totale» = solo le 10 ore del dev rimasto (riga dev + riga Totale).
    assert_eq!(some.iter().filter(|t| *t == "10").count(), 2, "{some:?}");
}

/// `hide_in_totals` finisce nel file solo quando è vero; i file vecchi
/// (senza il campo) si caricano con tutti i dev nei totali.
#[test]
fn dev_hidden_in_totals_round_trips_through_ron() {
    let mut app = App::new();
    let a = app.devs.add("Frontend");
    let b = app.devs.add("Backend");
    let plain = app.to_ron_string();
    assert!(!plain.contains("hide_in_totals"), "omesso quando falso");
    app.devs.set_hidden_in_totals(b, true);
    let ron = app.to_ron_string();
    assert!(ron.contains("hide_in_totals"));
    let back = App::from_ron_str(&ron).unwrap();
    assert!(back.devs.is_hidden_in_totals(b));
    assert!(!back.devs.is_hidden_in_totals(a));
    assert_eq!(back.devs.list_in_totals(), vec![(a, "Frontend".to_string())]);
    let old = App::from_ron_str(&plain).unwrap();
    assert_eq!(old.devs.list_in_totals().len(), 2);
}

/// La copia contiene tutto il progetto, è attiva, sta subito sotto
/// l'originale, ha le sue milestone personalizzate (le globali restano
/// condivise) ed è indipendente dall'originale.
#[test]
fn clone_project_copies_everything_and_duplicates_own_milestones() {
    let mut app = App::new();
    let a = app.projects.add("Alfa", Some("AAA"), Some(WeekId(20000)));
    app.projects.set_project_end_week(a, Some(WeekId(20070)));
    let b = app.projects.add("Beta", Some("BBB"), Some(WeekId(20000)));
    let dev = app.devs.add("Frontend");
    let w = app.workers.add("Alice");
    app.projects.add_dev(a, dev);
    app.projects.add_effort(a, dev, WeekId(20007), w, Effort(16));
    app.projects.set_dev_note(a, dev, "nota dev");
    app.projects
        .set_notes(a, HashMap::from([(WeekId(20007), "diario".to_string())]));
    let cat = app.categories.add("Cat");
    app.projects.set_category(a, Some(cat));
    let global = app.milestones.add("Globale");
    app.projects.add_project_milestone(a, global, WeekId(20014));
    let own = app.milestones.add_owned(
        "Privata",
        MilestoneKind::Trigger,
        vec![MilestoneCategory::External],
        Some(a),
    );
    app.milestones.set_color(own, 0x123456);
    app.projects.add_project_milestone(a, own, WeekId(20021));
    app.projects.set_closed(a, true);

    let c = clone_project(&mut app, a).unwrap();

    // Stesso nome, tripletta + «Copia», stato attivo, subito sotto l'originale.
    assert_eq!(app.projects.get_tripletta(c), "AAA Copia");
    assert_eq!(app.projects.get_info(c), "Alfa");
    assert_eq!(clone_tripletta(""), "Copia");
    assert!(!app.projects.is_closed(c) && app.projects.is_closed(a));
    let order: Vec<ProjectId> =
        app.projects.list_full().into_iter().map(|(id, _, _)| id).collect();
    assert_eq!(order, vec![a, c, b]);
    // Contenuto.
    assert_eq!(app.projects.get_project_start_week(c), Some(WeekId(20000)));
    assert_eq!(app.projects.get_project_end_week(c), Some(WeekId(20070)));
    assert!(app.projects.get_category(c) == Some(cat));
    assert_eq!(app.projects.get_notes(c), app.projects.get_notes(a));
    let sd = app.projects.get_single_dev(c, dev).unwrap();
    assert_eq!(sd.get_effort_by_week(WeekId(20007)), Effort(16));
    assert_eq!(sd.get_note(), "nota dev");
    // Milestone: globale condivisa, personalizzata duplicata per la copia.
    let ms: HashMap<MilestoneId, WeekId> =
        app.projects.list_project_milestones(c).into_iter().collect();
    assert_eq!(ms.get(&global), Some(&WeekId(20014)));
    assert!(!ms.contains_key(&own));
    let (&copy, &week) = ms.iter().find(|(id, _)| **id != global).unwrap();
    assert_eq!(week, WeekId(20021));
    assert_eq!(app.milestones.owner(copy), Some(c));
    assert_eq!(app.milestones.get_name(copy), Some("Privata"));
    assert_eq!(app.milestones.get_color(copy), Some(0x123456));
    assert!(app.milestones.get_kind(copy) == MilestoneKind::Trigger);
    assert_eq!(
        app.milestones.get_categories(copy),
        vec![MilestoneCategory::External]
    );
    // L'originale non cambia e la copia è indipendente.
    assert!(app.projects.list_project_milestones(a).contains(&(own, WeekId(20021))));
    app.projects.add_effort(c, dev, WeekId(20007), w, Effort(40));
    assert_eq!(
        app.projects
            .get_single_dev(a, dev)
            .unwrap()
            .get_effort_by_week(WeekId(20007)),
        Effort(16)
    );
    assert!(app.validate().is_empty(), "{:?}", app.validate());
    // Progetto inesistente: nessuna copia.
    assert!(clone_project(&mut app, ProjectId(999)).is_none());
}

#[test]
fn clone_project_window_lists_all_projects() {
    let mut app = App::new();
    let a = app.projects.add("Alfa", Some("AAA"), None);
    app.projects.add("Beta", Some("BBB"), None);
    app.projects.set_closed(a, true);
    let ctx = egui::Context::default();
    let mut state = UiState {
        clone_project: Some(CloneProject::default()),
        ..Default::default()
    };
    let mut texts = Vec::new();
    for _ in 0..2 {
        let mut actions = Vec::new();
        let out = ctx.run_ui_test(egui::RawInput::default(), |ui| {
            clone_project_window(ui.ctx(), &app, &mut state, &mut actions);
        });
        assert!(actions.is_empty());
        texts = drawn_texts(&out);
    }
    // Solo la tripletta: niente nome né stato.
    assert!(texts.iter().any(|t| t == "AAA"), "{texts:?}");
    assert!(texts.iter().any(|t| t == "BBB"));
    assert!(!texts.iter().any(|t| t.contains("Alfa") || t.contains("chiuso")));
    assert!(state.clone_project.is_some());
}

/// Progetto sull'intera griglia con un effort di Alice nella prima
/// colonna; ritorna anche la settimana (giorno) di quella colonna.
fn highlight_fixture() -> (App, WorkerId, i32) {
    let mut app = App::new();
    let week = match &columns_vec(&app, 0)[0] {
        Col::Weeks(ws) => ws[0],
        Col::YearEnd(_) => panic!("prima colonna di confine"),
    };
    let proj = app
        .projects
        .add("Prog", Some("ABC"), Some(WeekId(week as usize)));
    app.projects.set_project_end_week(proj, Some(app.end_week));
    let dev = app.devs.add("Frontend");
    app.projects.add_dev(proj, dev);
    let alice = app.workers.add("Alice");
    app.projects
        .add_effort(proj, dev, WeekId(week as usize), alice, Effort(8));
    (app, alice, week)
}

/// Col cursore sulla griglia si raccolgono la colonna e, sulle celle con
/// un worker, il worker; con una cella in modifica vince quella.
/// Griglia con un dev che lavora 8 h a settimana per 30 settimane (pianificato
/// 1000), disegnata in uno scroll spostato in orizzontale su `scroll_col`.
/// Ritorna i testi disegnati.
fn culled_grid_texts(state: &mut UiState, scroll_col: usize) -> (Vec<String>, Vec<i32>) {
    let mut app = App::new();
    let cols = columns_vec(&app, 0);
    let week_cols: Vec<(usize, i32)> = cols
        .iter()
        .enumerate()
        .filter_map(|(i, c)| match c {
            Col::Weeks(ws) => Some((i, ws[0])),
            Col::YearEnd(_) => None,
        })
        .collect();
    let proj = app
        .projects
        .add("Prog", Some("ABC"), Some(WeekId(week_cols[0].1 as usize)));
    app.projects.set_project_end_week(proj, Some(app.end_week));
    let dev = app.devs.add("Frontend");
    app.projects.add_dev_effort(proj, dev, Effort(1000));
    let alice = app.workers.add("Alice");
    for (_, w) in week_cols.iter().take(30) {
        app.projects
            .add_effort(proj, dev, WeekId(*w as usize), alice, Effort(8));
    }
    let x_off = col_x_offset(&cols, week_cols[scroll_col].0, COL_W);
    let ctx = egui::Context::default();
    let screen = screen(1000.0, 600.0);
    let mut texts = Vec::new();
    for _ in 0..3 {
        let input = egui::RawInput {
            screen_rect: Some(screen),
            ..Default::default()
        };
        let mut actions = Vec::new();
        let out = ctx.run_ui_test(input, |ui| {
            egui::ScrollArea::both()
                .scroll_offset(egui::vec2(x_off, 0.0))
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    let layout = body_layout(ui, &app, state, &Filter::default(), &None);
                    grid(ui, &app, state, &mut actions, &Filter::default(), &layout);
                });
        });
        texts = drawn_texts(&out);
    }
    let weeks = week_cols.iter().map(|(_, w)| *w).collect();
    (texts, weeks)
}

#[test]
fn grid_skips_offscreen_columns_but_keeps_the_cumulative() {
    let mut state = UiState::default();
    // ~11 colonne da 90 px in 1000 px: in vista le colonne 5..16 circa
    let (texts, _) = culled_grid_texts(&mut state, 5);
    // colonna 5 visibile: il cumulativo conta anche le 5 settimane fuori vista
    assert!(texts.iter().any(|t| t == "48 | 952"), "{texts:?}");
    // colonna 0 fuori vista: non disegnata
    assert!(!texts.iter().any(|t| t == "8 | 992"));
    // colonne oltre la destra dello schermo: nemmeno
    assert!(!texts.iter().any(|t| t == "240 | 760"), "settimana 30 fuori vista");
}

#[test]
fn grid_keeps_drawing_the_cell_being_edited_when_scrolled_away() {
    let mut state = UiState::default();
    let (_, weeks) = culled_grid_texts(&mut state, 0);
    state.editing = Some(Editing {
        proj: ProjectId(0),
        dev: DevId(0),
        week: weeks[0],
        row: 0,
        buf: "Alice|4".to_string(),
        typed: "Alice|4".to_string(),
        just_opened: true,
        had_focus: false,
        paste_note: None,
        orig_worker: "Alice".to_string(),
        orig_note: String::new(),
        orig_text: "Alice|4".to_string(),
    });
    let (texts, _) = culled_grid_texts(&mut state, 20);
    assert!(state.editing.is_some(), "l'editor fuori vista non deve chiudersi");
    assert!(texts.iter().any(|t| t == "Alice|4"), "editor disegnato: {texts:?}");
}

#[test]
fn grid_hover_records_column_and_worker_to_highlight() {
    let (app, alice, week) = highlight_fixture();
    let ctx = egui::Context::default();
    let mut state = UiState::default();
    let screen = screen(1600.0, 900.0);
    let origin = Cell::new(egui::Pos2::ZERO);
    let frame = |state: &mut UiState, pos: Option<egui::Pos2>| {
        let mut input = egui::RawInput {
            screen_rect: Some(screen),
            ..Default::default()
        };
        if let Some(p) = pos {
            input.events.push(egui::Event::PointerMoved(p));
        }
        state.grid_hl_next = GridHighlight::default();
        let mut actions = Vec::new();
        let _ = ctx.run_ui_test(input, |ui| {
            origin.set(ui.cursor().min);
            let layout = body_layout(ui, &app, state, &Filter::default(), &None);
            grid(ui, &app, state, &mut actions, &Filter::default(), &layout);
        });
    };
    frame(&mut state, None);
    let x = origin.get().x + COL_W / 2.0;

    // Sulla striscia in cima alla griglia: solo la data.
    let top = origin.get().y + 1.0;
    frame(&mut state, Some(egui::pos2(x, top)));
    frame(&mut state, Some(egui::pos2(x, top)));
    assert_eq!(state.grid_hl_next, GridHighlight { week: Some(week), worker: None });

    // Scendendo nella colonna si incontra la cella di Alice.
    let mut found = None;
    for i in 0..80 {
        let p = egui::pos2(x, origin.get().y + i as f32 * 2.0);
        frame(&mut state, Some(p));
        frame(&mut state, Some(p));
        if state.grid_hl_next.worker.is_some() {
            found = Some(state.grid_hl_next);
            break;
        }
    }
    assert_eq!(found, Some(GridHighlight { week: Some(week), worker: Some(alice) }));

    // Cursore fuori dalla griglia: niente.
    frame(&mut state, Some(egui::pos2(x, 890.0)));
    frame(&mut state, Some(egui::pos2(x, 890.0)));
    assert_eq!(state.grid_hl_next, GridHighlight::default());

    // Cella in modifica: vince sul cursore, il worker segue il nome digitato.
    let (proj, dev) = (ProjectId(0), app.devs.list()[0].0);
    state.editing = Some(Editing {
        proj,
        dev,
        week: week + 7,
        row: 0,
        buf: "Alice|4".to_string(),
        typed: "Alice|4".to_string(),
        // Appena aperta: prende il focus (con `had_focus` senza focus reale
        // l'editor si chiuderebbe confermando).
        just_opened: true,
        had_focus: false,
        paste_note: None,
        orig_worker: String::new(),
        orig_note: String::new(),
        orig_text: String::new(),
    });
    frame(&mut state, Some(egui::pos2(x, 890.0)));
    assert!(state.editing.is_some(), "l'editor deve restare aperto");
    assert_eq!(
        state.grid_hl_next,
        GridHighlight { week: Some(week + 7), worker: Some(alice) }
    );
}

/// Col cursore sul footer: riga delle date → solo la data; cella di un
/// worker → data + worker; nome del worker (parte sinistra) → solo il worker.
#[test]
fn remaining_hours_follow_the_year_and_the_null_filter() {
    let mut app = App::new();
    let alice = app.workers.add("Alice");
    let weeks = weeks_vec(&app);
    let this_week = weeks[3];
    let year = days_to_local(this_week).year();
    // anno corrente: dalla settimana corrente in poi, solo settimane dell'anno
    let cur = remaining_weeks(&app, year, this_week);
    assert!(cur.iter().all(|w| *w >= this_week && days_to_local(*w).year() == year));
    assert_eq!(cur.first(), Some(&this_week));
    // anno successivo: tutto l'anno, anche prima di `this_week` nel calendario
    let next = remaining_weeks(&app, year + 1, this_week);
    assert!(!next.is_empty() && next.iter().all(|w| days_to_local(*w).year() == year + 1));

    let full = worker_remaining_hours(&app, alice, &cur, false);
    assert_eq!(full, cur.len() as i32 * DEFAULT_MAX_HOURS as i32);
    // con «Nulli» una settimana già assegnata non conta
    app.sovra.insert((WeekId(cur[0] as usize), alice), Effort(8));
    assert_eq!(
        worker_remaining_hours(&app, alice, &cur, true),
        full - DEFAULT_MAX_HOURS as i32
    );
    assert_eq!(worker_remaining_hours(&app, alice, &cur, false), full);
    assert_eq!(remaining_counted_weeks(&app, alice, &cur, true).count(), cur.len() - 1);
}

/// Tooltip del numero verde accanto al worker nel footer: dice che sono le
/// ore disponibili (massimo effettivo per settimana), da quando, che le ore
/// assegnate non sono sottratte e, con «Nulli», che contano solo le
/// settimane vuote.
#[test]
fn remaining_hours_tooltip_explains_the_number() {
    let t = remaining_tooltip("Alice", 2026, true, false, 10, 376, Some(35));
    assert!(t.starts_with("376h: ore disponibili di Alice da questa settimana alla fine del 2026 (10 settimane"));
    assert!(t.contains("limite impostato per quella settimana"));
    assert!(t.contains("non sono sottratte"));
    assert!(t.ends_with("Già assegnate: 35h, quindi libere: 341h."), "{t}");
    assert!(!t.contains("Nulli"));
    let t = remaining_tooltip("Alice", 2026, true, false, 10, 40, Some(50));
    assert!(t.ends_with("Già assegnate: 50h, quindi 10h oltre la capacità."), "{t}");
    // Con «Nulli» niente riga delle ore libere.
    let t = remaining_tooltip("Alice", 2027, false, true, 4, 160, None);
    assert!(t.contains("in tutto il 2027 (4 settimane"));
    assert!(t.contains("Filtro «Nulli»"));
    assert!(!t.contains("Già assegnate"));
}

/// Ore già assegnate del tooltip: somma di `sovra` nelle settimane contate,
/// di tutti i progetti (le stesse delle ore pianificate delle torte senza
/// filtro).
#[test]
fn assigned_hours_sum_sovra_over_the_weeks() {
    let mut app = App::new();
    let alice = app.workers.add("Alice");
    let weeks = weeks_vec(&app);
    let cur: Vec<i32> = weeks[3..6].to_vec();
    app.sovra.insert((WeekId(cur[0] as usize), alice), Effort(8));
    app.sovra.insert((WeekId(cur[2] as usize), alice), Effort(30));
    app.sovra.insert((WeekId(weeks[0] as usize), alice), Effort(99)); // fuori
    assert_eq!(worker_assigned_hours(&app, alice, &cur), 38);
}

#[test]
fn confirm_window_reports_ok_cancel_or_still_open() {
    let ctx = egui::Context::default();
    let screen = screen(800.0, 600.0);
    // Un frame; se `click` è dato, clicca il pulsante con quel testo (trovato
    // nel frame precedente).
    let frame = |events: Vec<egui::Event>| {
        let mut answer = None;
        let out = ctx.run_ui_test(
            egui::RawInput {
                screen_rect: Some(screen),
                events,
                ..Default::default()
            },
            |ui| answer = confirm_window(ui.ctx(), "Titolo", "Sì", |ui| {
                ui.label("Testo");
            }),
        );
        (answer, out)
    };
    let pos_of = |out: &egui::FullOutput, label: &str| {
        out.shapes.iter().find_map(|cs| match &cs.shape {
            egui::epaint::Shape::Text(t) if t.galley.text() == label => {
                Some(t.visual_bounding_rect().center())
            }
            _ => None,
        })
    };
    let click = |pos: egui::Pos2| {
        let mut last = None;
        for ev in click_events(pos, egui::PointerButton::Primary) {
            last = frame(vec![ev]).0.or(last);
        }
        last
    };
    let _ = frame(vec![]);
    let (answer, out) = frame(vec![]);
    assert_eq!(answer, None, "senza scelta resta aperta");
    assert_eq!(click(pos_of(&out, "Annulla").unwrap()), Some(false));
    let (_, out) = frame(vec![]);
    assert_eq!(click(pos_of(&out, "Sì").unwrap()), Some(true));
}

#[test]
fn cell_text_parsing_matches_the_old_split_rules() {
    // vecchia regola: `split('|')` → esattamente 2 parti, nome trim, ore o 0
    fn old(t: &str) -> Option<(&str, usize)> {
        let parts: Vec<&str> = t.split('|').collect();
        (parts.len() == 2)
            .then(|| (parts[0].trim(), parts[1].trim().parse::<usize>().unwrap_or(0)))
    }
    for t in ["Alice|8", " Alice | 8 ", "Alice|", "|8", "Alice", "", "A|b|c", "Al|x", "A||"] {
        assert_eq!(parse_cell(t), old(t), "{t:?}");
        assert_eq!(cell_worker(t), t.split('|').next().unwrap_or("").trim(), "{t:?}");
    }
}

#[test]
fn grid_and_footer_share_the_same_current_week() {
    // La griglia (`current_week_id`) e footer/saturazione (`this_week`,
    // ricalcolato a ogni frame) leggono la stessa data locale.
    assert_eq!(current_week_id().0 as i32, current_monday_days());
}

#[test]
fn footer_click_toggles_and_switches_the_selected_worker() {
    let (mut app, alice, _week) = highlight_fixture();
    let bob = app.workers.add("Bob");
    let workers = footer_workers(&app, &Filter::default());
    let row_of = |w: WorkerId| workers.iter().position(|(x, _)| *x == w).unwrap();
    let ctx = egui::Context::default();
    let mut state = UiState {
        selected_year: 0,
        ..Default::default()
    };
    let screen = screen(1600.0, 900.0);
    let left = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(LEFT_W, 400.0));
    let name_x = DEV_NAME_W + DEV_TOTAL_W + 5.0 + COL_W / 2.0;
    let click = |state: &mut UiState, w: WorkerId| {
        let pos = egui::pos2(name_x, (row_of(w) as f32 + 1.5) * ROW_H);
        for ev in click_events(pos, egui::PointerButton::Primary) {
            let input = egui::RawInput {
                screen_rect: Some(screen),
                events: vec![ev],
                ..Default::default()
            };
            let mut actions = Vec::new();
            let _ = ctx.run_ui_test(input, |ui| {
                draw_left_footer(ui, left, &app, state, &workers, &mut actions);
            });
        }
    };
    click(&mut state, alice);
    assert_eq!(state.footer_selected_worker, Some(alice));
    assert!(state.popup.is_none(), "il click sinistro non apre più «Ore max»");
    click(&mut state, bob);
    assert_eq!(state.footer_selected_worker, Some(bob), "un altro worker prende il posto");
    click(&mut state, bob);
    assert_eq!(state.footer_selected_worker, None, "riclic sullo stesso: spento");
}

#[test]
fn selected_worker_is_highlighted_in_the_dev_cells() {
    let (app, alice, _week) = highlight_fixture();
    let ctx = egui::Context::default();
    let screen = screen(1600.0, 900.0);
    // Numero di testi «Alice…» disegnati in giallo evidenziazione.
    let yellow_alice = |selected: Option<WorkerId>| {
        let mut state = UiState {
            footer_selected_worker: selected,
            ..Default::default()
        };
        let mut n = 0;
        for _ in 0..2 {
            let input = egui::RawInput {
                screen_rect: Some(screen),
                ..Default::default()
            };
            let mut actions = Vec::new();
            let out = ctx.run_ui_test(input, |ui| {
                let layout = body_layout(ui, &app, &state, &Filter::default(), &None);
                grid(ui, &app, &mut state, &mut actions, &Filter::default(), &layout);
            });
            n = out
                .shapes
                .iter()
                .filter(|cs| match &cs.shape {
                    egui::epaint::Shape::Text(t) => {
                        t.galley.text().contains("Alice")
                            && t.galley.job.sections.iter().any(|s| s.format.color == hi_text())
                    }
                    _ => false,
                })
                .count();
        }
        n
    };
    assert_eq!(yellow_alice(None), 0);
    assert!(yellow_alice(Some(alice)) > 0, "la riga di Alice va evidenziata");
}

#[test]
fn bulk_week_popup_has_the_holiday_checkbox_and_sends_the_choice() {
    let (mut app, alice, week) = highlight_fixture();
    let week = week as usize;
    let ctx = egui::Context::default();
    let screen = screen(1000.0, 800.0);
    // Disegna la finestra; se `click` è dato, clicca il testo indicato.
    let run = |app: &App, state: &mut UiState, click: Option<&str>| -> Vec<Action> {
        let mut actions = Vec::new();
        let mut target = None;
        for _ in 0..2 {
            let input = egui::RawInput {
                screen_rect: Some(screen),
                ..Default::default()
            };
            let out = ctx.run_ui_test(input, |ui| {
                popup_window(ui.ctx(), app, state, &mut actions);
            });
            for cs in &out.shapes {
                if let egui::epaint::Shape::Text(t) = &cs.shape
                    && Some(t.galley.text()) == click
                {
                    target = Some(t.visual_bounding_rect().center());
                }
            }
        }
        if let Some(pos) = target {
            for ev in click_events(pos, egui::PointerButton::Primary) {
                let input = egui::RawInput {
                    screen_rect: Some(screen),
                    events: vec![ev],
                    ..Default::default()
                };
                let _ = ctx.run_ui_test(input, |ui| {
                    popup_window(ui.ctx(), app, state, &mut actions);
                });
            }
        }
        actions
    };
    let holiday_of = |state: &UiState| match &state.popup {
        Some(Popup::BulkWeekMax { holiday, .. }) => Some(*holiday),
        _ => None,
    };

    // Nessuno in ferie: spunta spenta, la si accende e OK manda Some(true).
    let mut state = UiState {
        popup: Some(bulk_week_popup(&app, week, "d".into())),
        ..Default::default()
    };
    assert_eq!(holiday_of(&state), Some(false));
    assert!(run(&app, &mut state, Some("Metti tutti i worker in ferie")).is_empty());
    assert_eq!(holiday_of(&state), Some(true));
    let acts = run(&app, &mut state, Some("OK"));
    assert!(matches!(
        acts.as_slice(),
        [Action::SetBulkWeekLimit { holiday: Some(true), .. }]
    ));

    // Spunta mai toccata: OK non cambia gli stati.
    let mut state = UiState {
        popup: Some(bulk_week_popup(&app, week, "d".into())),
        ..Default::default()
    };
    let acts = run(&app, &mut state, Some("OK"));
    assert!(matches!(
        acts.as_slice(),
        [Action::SetBulkWeekLimit { holiday: None, .. }]
    ));

    // Tutti in ferie: la spunta parte accesa; spegnendola OK manda Some(false).
    app.workers.set_week_status(alice, week, Some(WeekStatus::Ferie));
    let mut state = UiState {
        popup: Some(bulk_week_popup(&app, week, "d".into())),
        ..Default::default()
    };
    assert_eq!(holiday_of(&state), Some(true));
    run(&app, &mut state, Some("Metti tutti i worker in ferie"));
    let acts = run(&app, &mut state, Some("OK"));
    assert!(matches!(
        acts.as_slice(),
        [Action::SetBulkWeekLimit { holiday: Some(false), .. }]
    ));
}

#[test]
fn footer_date_click_opens_the_bulk_week_limit() {
    let (app, _alice, week) = highlight_fixture();
    let workers = footer_workers(&app, &Filter::default());
    let cols = columns_vec(&app, 0);
    let ctx = egui::Context::default();
    let mut state = UiState::default();
    let screen = screen(3000.0, 900.0);
    let right = egui::Rect::from_min_size(
        egui::Pos2::ZERO,
        egui::vec2(cols_width(&cols, COL_W), 400.0),
    );
    let pos = egui::pos2(COL_W / 2.0, ROW_H / 2.0);
    for ev in click_events(pos, egui::PointerButton::Primary) {
        let input = egui::RawInput {
            screen_rect: Some(screen),
            events: vec![ev],
            ..Default::default()
        };
        let mut actions = Vec::new();
        let _ = ctx.run_ui_test(input, |ui| {
            draw_right_footer(ui, right, &app, &mut state, &workers, &cols, &mut actions);
        });
    }
    assert!(
        matches!(state.popup, Some(Popup::BulkWeekMax { week: w, .. }) if w == week as usize),
        "il click sulla data del footer apre il limite per tutti i worker"
    );
}

#[test]
fn footer_hover_records_column_and_worker_to_highlight() {
    let (app, alice, week) = highlight_fixture();
    let workers = footer_workers(&app, &Filter::default());
    let row = workers.iter().position(|(w, _)| *w == alice).unwrap();
    let cols = columns_vec(&app, 0);
    let ctx = egui::Context::default();
    let mut state = UiState {
        selected_year: 0,
        ..Default::default()
    };
    let screen = screen(3000.0, 900.0);
    // Footer sinistro a (0,0), footer destro subito a destra.
    let left = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(LEFT_W, 400.0));
    let right = egui::Rect::from_min_size(
        egui::pos2(LEFT_W, 0.0),
        egui::vec2(cols_width(&cols, COL_W), 400.0),
    );
    let frame = |state: &mut UiState, p: egui::Pos2| {
        state.grid_hl_next = GridHighlight::default();
        for _ in 0..2 {
            let input = egui::RawInput {
                screen_rect: Some(screen),
                events: vec![egui::Event::PointerMoved(p)],
                ..Default::default()
            };
            state.grid_hl_next = GridHighlight::default();
            let mut actions = Vec::new();
            let _ = ctx.run_ui_test(input, |ui| {
                draw_left_footer(ui, left, &app, state, &workers, &mut actions);
                draw_right_footer(ui, right, &app, state, &workers, &cols, &mut actions);
            });
        }
        state.grid_hl_next
    };
    let x = right.left() + COL_W / 2.0;
    let y_row = (row as f32 + 1.5) * ROW_H;
    assert_eq!(
        frame(&mut state, egui::pos2(x, ROW_H / 2.0)),
        GridHighlight { week: Some(week), worker: None }
    );
    assert_eq!(
        frame(&mut state, egui::pos2(x, y_row)),
        GridHighlight { week: Some(week), worker: Some(alice) }
    );
    // Nome nel footer sinistro: colonna dei worker a `DEV_NAME_W + DEV_TOTAL_W + 5`.
    let name_x = DEV_NAME_W + DEV_TOTAL_W + 5.0 + COL_W / 2.0;
    assert_eq!(
        frame(&mut state, egui::pos2(name_x, y_row)),
        GridHighlight { week: None, worker: Some(alice) }
    );
}

/// Con un'evidenziazione attiva, intestazione e footer disegnano in giallo
/// la data della colonna, il nome del worker e la sua cella di quella
/// settimana — e nient'altro.
#[test]
fn header_and_footer_paint_the_highlighted_date_and_worker() {
    let (mut app, alice, week) = highlight_fixture();
    // Un secondo worker, che non va evidenziato.
    app.workers.add("Bob");
    let ctx = egui::Context::default();
    let mut state = UiState {
        grid_hl: GridHighlight { week: Some(week), worker: Some(alice) },
        ..Default::default()
    };
    let input = egui::RawInput {
        screen_rect: Some(screen(1600.0, 900.0)),
        ..Default::default()
    };
    let mut actions = Vec::new();
    let out = ctx.run_ui_test(input, |ui| {
        header(ui, &app, &mut state);
        footer(ui, &app, &mut state, &mut actions);
    });
    let yellow: Vec<String> = out
        .shapes
        .iter()
        .filter_map(|cs| match &cs.shape {
            egui::epaint::Shape::Text(t) if t.fallback_color == hi_text() => {
                Some(t.galley.text().to_string())
            }
            _ => None,
        })
        .collect();
    let date = primo_giorno_settimana_corrente(&days_to_local(week))
        .format("%y-%m-%d")
        .to_string();
    // Data: in alto e nel footer, due passate (grassetto) ciascuna.
    assert_eq!(yellow.iter().filter(|t| **t == date).count(), 4, "{yellow:?}");
    assert_eq!(yellow.iter().filter(|t| *t == "Alice").count(), 2, "{yellow:?}");
    assert!(!yellow.iter().any(|t| t == "Bob"));
    // La cella Alice × settimana (valore) e nient'altro.
    let others: Vec<&String> = yellow
        .iter()
        .filter(|t| **t != date && *t != "Alice")
        .collect();
    assert_eq!(others.len(), 2, "solo la cella Alice × settimana: {others:?}");
}

/// Le due visibilità del worker: footer (`hide_in_footer`) ed elenchi
/// (`show_in_find`). Cambiano solo se diverse, finiscono nel file solo
/// quando non sono il default e la riga sparisce dal footer.
#[test]
fn worker_visibility_setters_footer_and_ron() {
    let mut app = App::new();
    let alice = app.workers.add("Alice");
    let bob = app.workers.add("Bob");
    let plain = app.to_ron_string();

    assert!(app.workers.set_hidden_in_footer(bob, true));
    assert!(!app.workers.set_hidden_in_footer(bob, true), "già nascosto");
    assert!(app.workers.set_shown_in_find(bob, false));
    assert!(!app.workers.set_shown_in_find(bob, false));
    let names: Vec<String> = footer_workers(&app, &Filter::default())
        .into_iter()
        .map(|(_, n)| n)
        .collect();
    assert_eq!(names, vec!["Alice".to_string()]);

    let back = App::from_ron_str(&app.to_ron_string()).unwrap();
    assert!(back.workers.is_hidden_in_footer(bob) && !back.workers.is_shown_in_find(bob));
    assert!(!back.workers.is_hidden_in_footer(alice) && back.workers.is_shown_in_find(alice));

    // Rimesso visibile: il file torna identico a prima.
    app.workers.set_hidden_in_footer(bob, false);
    app.workers.set_shown_in_find(bob, true);
    assert_eq!(app.to_ron_string(), plain);
}

/// La finestra elenca tutti i worker (anche nascosti) con la ricerca e
/// resta aperta senza click.
#[test]
fn worker_visibility_window_lists_and_searches_workers() {
    let mut app = App::new();
    app.workers.add("Alice");
    let bob = app.workers.add("Bob");
    app.workers.set_hidden_in_footer(bob, true);
    let ctx = egui::Context::default();
    let mut state = UiState {
        show_worker_visibility: true,
        worker_visibility_just_opened: true,
        ..Default::default()
    };
    let texts = |state: &mut UiState| -> Vec<String> {
        let mut out_texts = Vec::new();
        for _ in 0..2 {
            let mut actions = Vec::new();
            let out = ctx.run_ui_test(egui::RawInput::default(), |ui| {
                worker_visibility_window(ui.ctx(), &app, state, &mut actions);
            });
            assert!(actions.is_empty());
            out_texts = drawn_texts(&out);
        }
        out_texts
    };
    let all = texts(&mut state);
    assert!(all.contains(&"Alice".to_string()) && all.contains(&"Bob".to_string()));
    for col in ["Footer", "Elenchi", "Ghost"] {
        assert!(all.contains(&col.to_string()), "colonna {col}: {all:?}");
    }
    state.worker_visibility_search = "bo".to_string();
    let some = texts(&mut state);
    assert!(some.contains(&"Bob".to_string()) && !some.contains(&"Alice".to_string()));
    assert!(state.show_worker_visibility);
}

#[test]
fn manual_version_is_dynamic_and_toc_parses() {
    // Il file usa il segnaposto, non un numero fisso, così non invecchia.
    assert!(
        MANUAL_MD.contains("{{VERSION}}"),
        "il manuale deve usare il segnaposto {{{{VERSION}}}}"
    );
    let manual = MANUAL_MD.replace("{{VERSION}}", env!("CARGO_PKG_VERSION"));
    assert!(!manual.contains("{{VERSION}}"));
    assert!(manual.contains(env!("CARGO_PKG_VERSION")));

    let sections = manual_sections(&manual);
    // Esattamente una sezione "## Indice", sostituita in-app dalla colonna.
    assert_eq!(sections.iter().filter(|s| is_index_section(s)).count(), 1);
    // I capitoli navigabili nell'indice laterale sono i 20 numerati.
    let chapters: Vec<&str> = sections.iter().filter_map(|s| chapter_title(s)).collect();
    assert_eq!(
        chapters.len(),
        20,
        "attesi 20 capitoli, trovati {}",
        chapters.len()
    );
    assert!(chapters[0].starts_with("1. "), "primo: {}", chapters[0]);
    assert!(chapters[19].starts_with("20. "), "ultimo: {}", chapters[19]);
    // "Indice" non deve comparire tra i capitoli navigabili.
    assert!(!chapters.contains(&"Indice"));
}

#[test]
fn body_projects_respects_view_mode_and_closed() {
    let mut app = App::new();
    // aperto e visibile
    let open = app.projects.add("Aperto", Some("AAA"), None);
    // aperto ma nascosto dal filtro «Progetti…» (enable = false)
    let hidden = app.projects.add("Nascosto", Some("BBB"), None);
    app.projects.set_enable(hidden, Enable(false));
    // chiuso (set_closed forza enable = false)
    let closed = app.projects.add("Chiuso", Some("CCC"), None);
    app.projects.set_closed(closed, true);
    // on-hold (anch'esso con enable = false)
    let hold = app.projects.add("Sospeso", Some("DDD"), None);
    app.projects.set_status(hold, ProjectStatus::OnHold);

    // ProjectId non implementa Debug: confronto sugli id interni.
    let ids = |view| {
        body_projects(&app, view, &ProjectFilter::default())
            .into_iter()
            .map(|p| p.0)
            .collect::<Vec<_>>()
    };

    // Ctrl+1: solo l'attivo visibile.
    assert_eq!(ids(ProjectViewMode::Open), vec![open.0]);
    // Ctrl+2: solo l'on-hold, malgrado enable = false.
    assert_eq!(ids(ProjectViewMode::OnHold), vec![hold.0]);
    // Ctrl+3: attivo visibile + on-hold, niente chiusi.
    assert_eq!(ids(ProjectViewMode::OpenOnHold), vec![open.0, hold.0]);
    // Ctrl+4: tutto tranne l'attivo nascosto dal filtro.
    assert_eq!(ids(ProjectViewMode::All), vec![open.0, closed.0, hold.0]);

    // Il filtro «Progetti…» nasconde anche on-hold e chiusi.
    app.projects.set_enable(hold, Enable(false));
    app.projects.set_enable(closed, Enable(false));
    let ids = |view| {
        body_projects(&app, view, &ProjectFilter::default())
            .into_iter()
            .map(|p| p.0)
            .collect::<Vec<_>>()
    };
    assert!(ids(ProjectViewMode::OnHold).is_empty());
    assert_eq!(ids(ProjectViewMode::OpenOnHold), vec![open.0]);
    assert_eq!(ids(ProjectViewMode::All), vec![open.0]);
}

#[test]
fn filters_project_list_follows_the_view_and_hides_any_status() {
    let mut app = App::new();
    app.projects.add("Aperto", Some("AAA"), None);
    let hold = app.projects.add("Sospeso", Some("HHH"), None);
    app.projects.set_status(hold, ProjectStatus::OnHold);
    let closed = app.projects.add("Chiuso", Some("CCC"), None);
    app.projects.set_status(closed, ProjectStatus::Closed);

    let ctx = egui::Context::default();
    let input = egui::RawInput {
        screen_rect: Some(screen(1600.0, 900.0)),
        ..Default::default()
    };
    let listed = |view| {
        let mut state = UiState {
            show_filters: true,
            project_view: view,
            ..Default::default()
        };
        let mut texts = Vec::new();
        for _ in 0..2 {
            let mut actions = Vec::new();
            let out = ctx.run_ui_test(input.clone(), |ui| {
                filters_window(ui.ctx(), &app, &mut state, &mut actions);
            });
            texts = drawn_texts(&out);
        }
        ["AAA", "HHH", "CCC"]
            .into_iter()
            .filter(|t| texts.iter().any(|x| x == t))
            .collect::<Vec<_>>()
    };
    assert_eq!(listed(ProjectViewMode::Open), vec!["AAA"]);
    assert_eq!(listed(ProjectViewMode::OnHold), vec!["HHH"]);
    assert_eq!(listed(ProjectViewMode::OpenOnHold), vec!["AAA", "HHH"]);
    assert_eq!(listed(ProjectViewMode::All), vec!["AAA", "HHH", "CCC"]);

    // Accanto alla tripletta c'è lo stato del progetto.
    let mut state = UiState {
        show_filters: true,
        project_view: ProjectViewMode::All,
        ..Default::default()
    };
    let mut out = None;
    for _ in 0..2 {
        let mut actions = Vec::new();
        out = Some(ctx.run_ui_test(input.clone(), |ui| {
            filters_window(ui.ctx(), &app, &mut state, &mut actions);
        }));
    }
    let out = out.unwrap();
    for (trip, status) in [("AAA", "Attivo"), ("HHH", "On-Hold"), ("CCC", "Closed")] {
        let t = text_pos(&out, trip).expect(trip);
        let found = out.shapes.iter().any(|cs| match &cs.shape {
            egui::epaint::Shape::Text(x) => {
                x.galley.text() == status
                    && (x.visual_bounding_rect().center().y - t.y).abs() < 4.0
                    && x.visual_bounding_rect().center().x > t.x
            }
            _ => false,
        });
        assert!(found, "{status} accanto a {trip}");
    }

    // Cambiare stato non nasconde il progetto: resta visibile nella sua vista.
    assert!(app.projects.get_enable(&hold).0 && app.projects.get_enable(&closed).0);
}

#[test]
fn ctrl_1_to_4_switch_the_project_view() {
    let mut pjm = PjmApp { app: App::new(), ui: UiState::default() };
    let ctx = egui::Context::default();
    let mut frame = eframe::Frame::_new_kittest();
    for (key, mode) in [
        (egui::Key::Num2, ProjectViewMode::OnHold),
        (egui::Key::Num3, ProjectViewMode::OpenOnHold),
        (egui::Key::Num4, ProjectViewMode::All),
        (egui::Key::Num1, ProjectViewMode::Open),
    ] {
        let input = egui::RawInput {
            screen_rect: Some(screen(1200.0, 800.0)),
            events: vec![
                egui::Event::ModifiersChanged(egui::Modifiers::COMMAND),
                egui::Event::Key {
                    key,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::COMMAND,
                },
            ],
            ..Default::default()
        };
        let _ = ctx.run_ui_test(input, |ui| eframe::App::ui(&mut pjm, ui, &mut frame));
        assert!(pjm.ui.project_view == mode, "{key:?}");
    }
}

/// Accoda gli eventi di un click "realistico" su `pos`: hover in un frame,
/// Gli ambiti spuntati nelle dialog di export diventano l'elenco passato
/// alle azioni: uno = un file, due = due file (ordine Internal, External).
#[test]
fn export_scopes_list_follows_the_checkboxes() {
    let dflt = ExportScopes::default();
    assert_eq!(
        dflt.list(),
        vec![
            MilestoneScope::Internal,
            MilestoneScope::External,
            MilestoneScope::Mix
        ]
    );
    assert!(!dflt.is_empty());

    let only_mix = ExportScopes {
        internal: false,
        external: false,
        mix: true,
    };
    assert_eq!(only_mix.list(), vec![MilestoneScope::Mix]);

    let none = ExportScopes {
        internal: false,
        external: false,
        mix: false,
    };
    assert!(none.is_empty());
    assert!(none.list().is_empty());
}

/// Un export per ambito richiesto, con il suffisso del file; gli ambiti in
/// cui non esce niente (progetto senza inizio/fine) non producono file.
#[test]
fn scoped_exports_builds_one_item_per_scope() {
    // Le opzioni di base arrivano a ogni export, con l'ambito di quel file.
    let base = crate::pdf_export::ExportOptions {
        show_pct: true,
        ..Default::default()
    };
    let items = scoped_exports(
        &base,
        &[
            MilestoneScope::Internal,
            MilestoneScope::External,
            MilestoneScope::Mix,
        ],
        |opts| {
            assert!(opts.show_pct, "le opzioni di base passano a ogni ambito");
            Some(opts.scope.label().as_bytes().to_vec())
        },
    );
    let suffixes: Vec<String> = items.iter().map(|(s, _)| s.clone()).collect();
    assert_eq!(suffixes, ["internal", "external", "mix"]);
    assert_eq!(items[1].1, b"Solo external".to_vec());

    assert!(scoped_exports(&base, &[MilestoneScope::Internal], |_| None).is_empty());
}

/// Con due ambiti si salva una volta sola e i file prendono il suffisso
/// dell'ambito, mantenendo cartella ed estensione scelte nel dialog.
#[test]
fn path_with_suffix_appends_the_scope_to_the_file_name() {
    let p = std::path::Path::new("/tmp/export/progetti_2026_07_13.pdf");
    assert_eq!(
        path_with_suffix(p, MilestoneScope::Internal.file_suffix(), "pdf"),
        std::path::PathBuf::from("/tmp/export/progetti_2026_07_13_internal.pdf")
    );
    assert_eq!(
        path_with_suffix(p, MilestoneScope::External.file_suffix(), "pdf"),
        std::path::PathBuf::from("/tmp/export/progetti_2026_07_13_external.pdf")
    );
    // Senza estensione nel percorso scelto si usa quella dell'export.
    assert_eq!(
        path_with_suffix(std::path::Path::new("/tmp/grafico"), "external", "svg"),
        std::path::PathBuf::from("/tmp/grafico_external.svg")
    );
}

/// Smoke test della dialog di export a singolo progetto con l'elenco
/// milestone: si disegna senza panic (id delle due liste distinti) e la
/// «Esporta PDF» di un solo progetto con 20 dev e 20 milestone: ciascun
/// elenco mostra al massimo `MAX_VISIBLE_EXPORT_ROWS` righe, il resto scorre.
/// La dialog di export del singolo progetto si adatta al contenuto: con un
/// solo dev non deve allungarsi fino all'altezza dello schermo (lo faceva
/// un `ui.separator()` verticale tra formato barre e opzioni).
#[test]
fn pdf_export_window_does_not_stretch_to_screen_height() {
    let ctx = egui::Context::default();
    let input = egui::RawInput {
        screen_rect: Some(screen(1536.0, 2000.0)),
        ..Default::default()
    };
    let mut app = App::new();
    let proj = app.projects.add("Prog", Some("ABC"), Some(WeekId(20000)));
    app.projects.set_project_end_week(proj, Some(WeekId(20700)));
    let dev = app.devs.add("Frontend");
    app.projects.add_dev(proj, dev);
    let mut state = UiState {
        pdf_export: Some(PdfExport {
            proj,
            entries: vec![(dev, true)],
            milestones: Vec::new(),
            formats: HashMap::new(),
            range: project_export_range(&app, proj),
        }),
        ..Default::default()
    };
    // Più frame: col difetto la finestra cresceva un po' a ogni frame.
    for _ in 0..20 {
        let mut actions = Vec::new();
        let _ = ctx.run_ui_test(input.clone(), |ui| {
            pdf_export_window(ui.ctx(), &app, &mut state, &mut actions);
        });
    }
    let rect = ctx
        .memory(|m| m.area_rect(egui::Id::unique("pdf_export_window")))
        .expect("finestra disegnata");
    assert!(rect.height() < 500.0, "finestra alta {}", rect.height());
}

#[test]
fn pdf_export_window_lists_show_at_most_10_rows() {
    let ctx = egui::Context::default();
    // Schermo alto: il limite è quello delle righe, non lo spazio.
    let input = egui::RawInput {
        screen_rect: Some(screen(1000.0, 2000.0)),
        ..Default::default()
    };
    let mut app = App::new();
    let proj = app.projects.add("Prog", Some("ABC"), Some(WeekId(20000)));
    app.projects.set_project_end_week(proj, Some(WeekId(20700)));
    let mut entries = Vec::new();
    let mut milestones = Vec::new();
    for i in 0..20 {
        let dev = app.devs.add(&format!("Dev{i:02}"));
        app.projects.add_dev(proj, dev);
        entries.push((dev, true));
        let ms = app.milestones.add(&format!("Ms{i:02}"));
        let week = WeekId(20000 + 7 * i);
        app.projects.add_project_milestone(proj, ms, week);
        milestones.push((ms, week, true));
    }
    let mut state = UiState {
        pdf_export: Some(PdfExport {
            proj,
            entries,
            milestones,
            formats: HashMap::new(),
            range: project_export_range(&app, proj),
        }),
        ..Default::default()
    };
    let (mut devs, mut mss) = (0, 0);
    for _ in 0..3 {
        let mut actions = Vec::new();
        let out = ctx.run_ui_test(input.clone(), |ui| {
            pdf_export_window(ui.ctx(), &app, &mut state, &mut actions);
        });
        // Righe interamente visibili (non tagliate dal clip dell'elenco).
        let visible = |pat: &str| {
            out.shapes
                .iter()
                .filter(|cs| match &cs.shape {
                    egui::epaint::Shape::Text(t) => {
                        t.galley.text().contains(pat)
                            && cs.clip_rect.contains_rect(t.visual_bounding_rect())
                    }
                    _ => false,
                })
                .count()
        };
        devs = visible("≡  Dev");
        mss = visible(" Ms");
    }
    assert_eq!(devs, MAX_VISIBLE_EXPORT_ROWS, "righe dev visibili");
    assert_eq!(mss, MAX_VISIBLE_EXPORT_ROWS, "righe milestone visibili");
}

/// selezione iniziale — tutte spuntate — resta tale finché non si clicca.
#[test]
fn pdf_export_window_draws_the_milestone_checklist() {
    let ctx = egui::Context::default();
    let input = egui::RawInput {
        screen_rect: Some(screen(1000.0, 800.0)),
        ..Default::default()
    };

    let mut app = App::new();
    let proj = app.projects.add("Prog", Some("ABC"), Some(WeekId(20000)));
    app.projects.set_project_end_week(proj, Some(WeekId(20070)));
    let dev = app.devs.add("Frontend");
    app.projects.add_dev(proj, dev);
    let a = app.milestones.add("Alpha");
    let b = app.milestones.add("Beta");
    app.projects.add_project_milestone(proj, a, WeekId(20021));
    app.projects.add_project_milestone(proj, b, WeekId(20035));

    let mut state = UiState {
        pdf_export: Some(PdfExport {
            proj,
            entries: vec![(dev, true)],
            milestones: vec![(a, WeekId(20021), true), (b, WeekId(20035), true)],
            formats: HashMap::new(),
            range: project_export_range(&app, proj),
        }),
        ..Default::default()
    };
    let mut actions: Vec<Action> = Vec::new();
    for _ in 0..2 {
        let _ = ctx.run_ui_test(input.clone(), |ui| {
            let ctx = ui.ctx();
            pdf_export_window(ctx, &app, &mut state, &mut actions);
        });
    }

    assert!(actions.is_empty(), "senza click non parte nessun export");
    let px = state.pdf_export.expect("la dialog resta aperta");
    assert!(px.milestones.iter().all(|(_, _, s)| *s));
}

/// Accoda un click sinistro in `pos`, un evento per frame (vedi `click_events`).
fn queue_click(pending: &mut Vec<Vec<egui::Event>>, pos: egui::Pos2) {
    pending.extend(click_events(pos, egui::PointerButton::Primary).map(|e| vec![e]));
}

/// Finestra «Stato progetti»: aprire la combo di un progetto e scegliere
/// «On-Hold» con click veri produce l'azione **senza chiudere la finestra**
/// (il menu della combo è un livello a sé, il click non è «fuori»).
#[test]
fn status_combo_sets_on_hold_and_keeps_the_window_open() {
    let ctx = egui::Context::default();
    let input = egui::RawInput {
        screen_rect: Some(screen(900.0, 600.0)),
        ..Default::default()
    };
    let mut app = App::new();
    let proj = app.projects.add("Uno", Some("P1"), None);
    let mut state = UiState {
        show_closed_filter: true,
        closed_filter_just_opened: true,
        closed_btn_pos: egui::pos2(50.0, 50.0),
        ..Default::default()
    };
    let mut got: Option<ProjectStatus> = None;
    let mut pending: Vec<Vec<egui::Event>> = Vec::new();
    let mut opened = false;
    let mut picked = false;
    for _ in 0..30 {
        let mut inp = input.clone();
        if !pending.is_empty() {
            inp.events = pending.remove(0);
        }
        let mut actions = Vec::new();
        let _ = ctx.run_ui_test(inp, |ui| {
            closed_filter_window(ui.ctx(), &app, &mut state, &mut actions);
        });
        for a in actions {
            if let Action::SetProjectStatus { proj: p, status } = a {
                assert_eq!(p, proj);
                got = Some(status);
                app.projects.set_status(p, status);
            }
        }
        if got.is_some() {
            // Qualche frame in più: la finestra deve restare aperta.
            if pending.is_empty() {
                break;
            }
            continue;
        }
        assert!(state.show_closed_filter, "la finestra si è chiusa");
        if !pending.is_empty() {
            continue;
        }
        if !opened {
            queue_click(&mut pending, state.status_combo_rects[0].center());
            opened = true;
        } else if !picked && state.status_item_rects.len() == 3 {
            // Voci: Attivo, On-Hold, Closed.
            queue_click(&mut pending, state.status_item_rects[1].center());
            picked = true;
        }
    }
    assert_eq!(got, Some(ProjectStatus::OnHold));
    assert_eq!(app.projects.status(proj), ProjectStatus::OnHold);
    assert!(app.projects.is_closed(proj));
    // Altri frame senza input: la finestra resta aperta.
    for _ in 0..3 {
        let mut actions = Vec::new();
        let _ = ctx.run_ui_test(input.clone(), |ui| {
            closed_filter_window(ui.ctx(), &app, &mut state, &mut actions);
        });
    }
    assert!(state.show_closed_filter, "scelta la voce, la finestra deve restare aperta");
}

/// La spunta delle categorie in fase di creazione segue la stessa regola
/// del modello: elenco ordinato, senza doppioni e mai vuoto.
#[test]
fn toggle_category_keeps_the_list_ordered_and_non_empty() {
    // Da "vuoto" (= Internal) si aggiunge External → entrambe.
    let both = toggle_category(
        &effective_categories(&[]),
        MilestoneCategory::External,
        true,
    );
    assert_eq!(
        both,
        vec![MilestoneCategory::Internal, MilestoneCategory::External]
    );
    // Tolta Internal resta solo External…
    let only_ext = toggle_category(&both, MilestoneCategory::Internal, false);
    assert_eq!(only_ext, vec![MilestoneCategory::External]);
    // …e togliendo anche l'ultima si torna a Internal.
    assert_eq!(
        toggle_category(&only_ext, MilestoneCategory::External, false),
        vec![MilestoneCategory::Internal]
    );
}

/// Come `queue_click`, ma col tasto destro (apertura del menù contestuale).
fn queue_right_click(pending: &mut Vec<Vec<egui::Event>>, pos: egui::Pos2) {
    pending.extend(click_events(pos, egui::PointerButton::Secondary).map(|e| vec![e]));
}

/// Il campo «Nuova milestone solo qui…» vive dentro un sottomenù del menù
/// contestuale della griglia: cliccarci dentro e scrivere **non deve
/// chiudere il menù**, altrimenti il nome non si riuscirebbe a digitare.
/// (Stessa classe di problemi del selettore del tipo, vedi sotto.)
#[test]
fn project_milestone_field_keeps_the_context_menu_open() {
    let ctx = egui::Context::default();
    let input = egui::RawInput {
        screen_rect: Some(screen(900.0, 600.0)),
        ..Default::default()
    };

    let anchor = Cell::new(egui::Rect::NOTHING);
    let sub_btn = Cell::new(egui::Rect::NOTHING);
    let field = Cell::new(egui::Rect::NOTHING);
    let menu_open = Cell::new(false);
    let sub_open = Cell::new(false);
    let focused = Cell::new(false);
    let created: Cell<Option<String>> = Cell::new(None);
    let cats_btn = Cell::new(egui::Rect::NOTHING);
    let mut cats: Vec<MilestoneCategory> = Vec::new();
    let mut opened_cats = false;
    let mut buf = String::new();
    let mut typed = false;
    let mut submitted = false;
    let mut clicked_field = false;
    let mut pending: Vec<Vec<egui::Event>> = Vec::new();

    for _ in 0..40 {
        let mut inp = input.clone();
        if !pending.is_empty() {
            inp.events = pending.remove(0);
        }
        menu_open.set(false);
        sub_open.set(false);
        let _ = ctx.run_ui_test(inp, |ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                // Stessa struttura di `draw_milestone_strip`: una zona che
                // risponde al tasto destro, un sottomenù col campo dentro.
                let (rect, resp) =
                    ui.allocate_exact_size(egui::vec2(200.0, 40.0), Sense::click());
                anchor.set(rect);
                resp.context_menu(|ui| {
                    menu_open.set(true);
                    // Stessa configurazione del sottomenù di `grid.rs`.
                    let r = egui::containers::menu::SubMenuButton::new(
                        "Aggiungi milestone qui",
                    )
                    .config(
                        egui::containers::menu::MenuConfig::new()
                            .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside),
                    )
                    .ui(ui, |ui| {
                        sub_open.set(true);
                        // Sopra il campo, il sottomenù delle categorie:
                        // aprirlo non deve chiudere il menù contestuale.
                        cats_btn.set(
                            milestone_categories_submenu(ui, &mut cats).rect,
                        );
                        let e = add_field(
                            ui,
                            "Milestone di progetto",
                            "Nuova milestone solo qui…",
                            &mut buf,
                            |name| created.set(Some(name)),
                        );
                        field.set(e.rect);
                        focused.set(e.has_focus());
                    });
                    sub_btn.set(r.0.rect);
                });
            });
        });

        if !pending.is_empty() {
            continue;
        }
        if !menu_open.get() {
            queue_right_click(&mut pending, anchor.get().center());
        } else if !sub_open.get() {
            queue_click(&mut pending, sub_btn.get().center());
        } else if !opened_cats {
            // Apre «Categorie: …»: il menù deve restare aperto.
            queue_click(&mut pending, cats_btn.get().center());
            opened_cats = true;
        } else if !clicked_field {
            queue_click(&mut pending, field.get().center());
            clicked_field = true;
        } else if !typed {
            pending.push(vec![egui::Event::Text("Rilascio".to_string())]);
            typed = true;
        } else if !submitted {
            // Invio = conferma: crea la milestone e svuota il campo.
            assert_eq!(buf, "Rilascio", "il testo digitato non è arrivato al campo");
            assert!(focused.get(), "il campo ha perso il focus mentre si scriveva");
            pending.push(vec![egui::Event::Key {
                key: egui::Key::Enter,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Default::default(),
            }]);
            submitted = true;
        }
    }

    assert!(menu_open.get(), "il menù contestuale si è chiuso");
    assert!(sub_open.get(), "il sottomenù si è chiuso");
    assert!(
        cats_btn.get().is_finite(),
        "il sottomenù «Categorie» non è stato disegnato"
    );
    assert_eq!(
        cats,
        Vec::new(),
        "aprire il sottomenù non deve cambiare le categorie"
    );
    assert_eq!(
        created.take(),
        Some("Rilascio".to_string()),
        "l'Invio non ha creato la milestone di progetto"
    );
    assert!(buf.is_empty(), "il campo va svuotato dopo la creazione");
}

/// Il selettore del tipo milestone nel menù «Aggiungi» **non deve chiudere
/// il menù**: si sceglie il tipo e si continua a scrivere il nome nello
/// stesso menù. Vale con il sottomenù «Tipo: … ⏵»; con una `ComboBox` no —
/// la sua tendina sta in un altro layer, il menù la legge come click "fuori"
/// e si chiude prima ancora che la voce venga selezionata (egui 0.34).
#[test]
fn milestone_kind_submenu_keeps_the_add_menu_open() {
    let ctx = egui::Context::default();
    let input = egui::RawInput {
        screen_rect: Some(screen(900.0, 600.0)),
        ..Default::default()
    };

    let menu_btn = Cell::new(egui::Rect::NOTHING);
    let sub_btn = Cell::new(egui::Rect::NOTHING);
    let menu_open = Cell::new(false);
    let mut kind = MilestoneKind::Goal;
    let mut pending: Vec<Vec<egui::Event>> = Vec::new();

    for step in 0..20 {
        let mut inp = input.clone();
        if !pending.is_empty() {
            inp.events = pending.remove(0);
        }
        menu_open.set(false);
        let out = ctx.run_ui_test(inp, |ui| {
            egui::Panel::top("t").show(ui, |ui| {
                egui::MenuBar::new().ui(ui, |ui| {
                    // Stessa configurazione del menù «Aggiungi» in toolbar.rs.
                    let (btn, _) = egui::containers::menu::MenuButton::new("Aggiungi")
                        .config(
                            egui::containers::menu::MenuConfig::new()
                                .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside),
                        )
                        .ui(ui, |ui| {
                            menu_open.set(true);
                            milestone_kind_submenu(ui, &mut kind);
                            sub_btn.set(ui.min_rect());
                        });
                    menu_btn.set(btn.rect);
                });
            });
        });

        // Posizione della voce "Trigger": si cerca il testo disegnato, così
        // il test non dipende da coordinate indovinate a mano.
        let trigger_item = text_pos(&out, "Trigger");

        match step {
            0 => queue_click(&mut pending, menu_btn.get().center()),
            // hover sul sottomenù (si apre al passaggio del mouse)
            5 => {
                let p = sub_btn.get().center();
                pending.push(vec![egui::Event::PointerMoved(p)]);
                pending.push(vec![egui::Event::PointerMoved(p)]);
            }
            9 => {
                let p = trigger_item.expect("il sottomenù aperto disegna la voce «Trigger»");
                queue_click(&mut pending, p);
            }
            _ => {}
        }
    }

    assert_eq!(
        kind,
        MilestoneKind::Trigger,
        "la voce del sottomenù deve cambiare il tipo"
    );
    assert!(
        menu_open.get(),
        "dopo la scelta il menù «Aggiungi» deve restare aperto per digitare il nome"
    );
}

/// Le icone del tipo milestone (bandierina / fulmine) devono essere davvero
/// disegnabili con i font caricati dall'app: `has_glyph` è false quando il
/// carattere finirebbe come tofu. Guardia contro un cambio di font.
#[test]
fn milestone_kind_icons_are_renderable() {
    let ctx = egui::Context::default();
    install_symbol_fallback(&ctx);
    let _ = ctx.run_ui_test(egui::RawInput::default(), |_| {});
    let font = egui::FontId::proportional(14.0);
    for kind in [MilestoneKind::Goal, MilestoneKind::Trigger] {
        assert!(
            ctx.fonts_mut(|f| f.has_glyphs(&font, kind.icon())),
            "l'icona di {} ({}) non è disegnabile coi font caricati",
            kind.label(),
            kind.icon()
        );
    }
    assert_ne!(
        MilestoneKind::Goal.icon(),
        MilestoneKind::Trigger.icon(),
        "traguardo e trigger devono avere icone diverse"
    );
}

#[test]
fn milestone_bands_fit_column_width() {
    // Colonna normale: ci stanno tutte le milestone presenti.
    assert_eq!(milestone_bands_shown(COL_W, 1), 1);
    assert_eq!(milestone_bands_shown(COL_W, 3), 3);
    // Non se ne inventano più di quelle collocate nella settimana.
    assert_eq!(milestone_bands_shown(COL_W, 0), 0);
    // Vista compatta (25 px): al massimo 6 bande da 4 px.
    assert_eq!(milestone_bands_shown(COMPACT_W, 4), 4);
    assert_eq!(milestone_bands_shown(COMPACT_W, 10), 6);
    // Colonna più stretta della banda minima: almeno una banda, sempre.
    assert_eq!(milestone_bands_shown(3.0, 5), 1);
}

#[test]
fn multiple_milestones_can_share_a_week() {
    // Il modello dati ammette più milestone nella stessa settimana: la
    // chiave è la milestone, non la settimana.
    let mut app = App::new();
    let pid = app.projects.add("P", Some("AAA"), None);
    let a = app.milestones.add("Alpha");
    let b = app.milestones.add("Beta");
    let w = WeekId(20000);
    app.projects.add_project_milestone(pid, a, w);
    app.projects.add_project_milestone(pid, b, w);

    // Ordinate per id → l'ordine delle bande è stabile tra un frame e l'altro.
    assert_eq!(app.projects.project_milestones_at_week(pid, w), vec![a, b]);
    // …e ciascuna ha il suo colore, quindi le bande sono distinguibili.
    assert_ne!(app.milestones.get_color(a), app.milestones.get_color(b));
}

// ── Bande dei mesi sulle righe delle date ──────────────────────────────

/// Numero di giorno (WeekId) del lunedì della settimana che contiene la data.
fn week_of(y: i32, m: u32, d: u32) -> i32 {
    let date = chrono::NaiveDate::from_ymd_opt(y, m, d).unwrap();
    local_to_days(&primo_giorno_settimana_corrente(&date))
}

/// Settimana tutta dentro un mese: una banda sola, larga tutta la cella.
#[test]
fn month_bands_of_a_single_month_week() {
    let bands = month_bands(&[week_of(2026, 8, 24)]);
    assert_eq!(bands, vec![(8, 0.0, 1.0)]);
}

/// Settimana a cavallo: la parte sinistra è larga quanti sono i giorni
/// rimasti nel mese precedente (su 5), il resto è del mese nuovo.
#[test]
fn month_bands_split_on_the_day_the_month_changes() {
    // lun 31/8 in agosto, mar–ven in settembre → 1/5 + 4/5
    assert_eq!(
        month_bands(&[week_of(2026, 8, 31)]),
        vec![(8, 0.0, 0.2), (9, 0.2, 1.0)]
    );
    // lun–mer in settembre, gio 1/10 e ven in ottobre → 3/5 + 2/5
    assert_eq!(
        month_bands(&[week_of(2026, 9, 28)]),
        vec![(9, 0.0, 0.6), (10, 0.6, 1.0)]
    );
    // il mese cambia di sabato: i 5 giorni colorati sono tutti di luglio
    assert_eq!(month_bands(&[week_of(2026, 7, 27)]), vec![(7, 0.0, 1.0)]);
    // cambio d'anno: dicembre → gennaio
    assert_eq!(
        month_bands(&[week_of(2025, 12, 29)]),
        vec![(12, 0.0, 0.6), (1, 0.6, 1.0)]
    );
}

/// Colonna mergiata (zoom): le settimane del gruppo si accodano, quindi le
/// frazioni sono sui `5 × n` giorni complessivi.
#[test]
fn month_bands_span_merged_columns() {
    let bands = month_bands(&[week_of(2026, 8, 24), week_of(2026, 8, 31)]);
    assert_eq!(bands, vec![(8, 0.0, 0.6), (9, 0.6, 1.0)]);
    // le bande sono contigue e coprono tutta la cella
    assert_eq!(bands.first().unwrap().1, 0.0);
    assert_eq!(bands.last().unwrap().2, 1.0);
    for pair in bands.windows(2) {
        assert_eq!(pair[0].2, pair[1].1);
    }
}

/// Lo sfondo restituito (quello che decide il colore della data) è quello
/// della banda che sta **sotto l'etichetta**, cioè al centro della cella.
#[test]
fn month_tint_center_is_the_band_under_the_label() {
    set_bw_mode(false);
    set_dark_theme(true);
    set_month_colors(MONTH_COLORS_DEFAULT);
    set_month_tint_pct(100);

    let ctx = egui::Context::default();
    let out = Cell::new(Color32::TRANSPARENT);
    let _ = ctx.run_ui_test(egui::RawInput::default(), |ui| {
        egui::CentralPanel::default().show(ui, |ui| {
            let cell = Rect::from_min_size(egui::pos2(0.0, 0.0), Vec2::new(COL_W, ROW_H));
            // 1 solo giorno di agosto: il centro cade nella banda di settembre
            out.set(paint_month_tint(ui, cell, &[week_of(2026, 8, 31)]));
        });
    });
    assert_eq!(out.get(), blend(bg(), month_tint(9)));

    set_month_tint_pct(MONTH_TINT_PCT_DEFAULT);
}

// ── Asse delle colonne (header + griglia + footer condividono questo) ───

/// Costruisce un'app la cui griglia va da `from` a `to` (date locali), così
/// il confine d'anno cade in un punto noto.
fn app_spanning(from: (i32, u32, u32), to: (i32, u32, u32)) -> App {
    let day = |(y, m, d): (i32, u32, u32)| {
        local_to_days(&chrono::NaiveDate::from_ymd_opt(y, m, d).unwrap()) as usize
    };
    let mut app = App::new();
    app.start_week = WeekId(day(from));
    app.end_week = WeekId(day(to));
    app
}

/// Settimane di una colonna (vuoto per la colonna di confine).
fn col_weeks(c: &Col) -> Vec<i32> {
    match c {
        Col::Weeks(ws) => ws.clone(),
        Col::YearEnd(_) => Vec::new(),
    }
}

/// Senza zoom ogni colonna è **una** settimana, nell'ordine di `weeks_vec`.
#[test]
fn columns_at_zoom_zero_are_single_weeks() {
    let app = app_spanning((2026, 1, 5), (2026, 2, 9)); // 6 lunedì
    let cols = columns_vec(&app, 0);
    assert_eq!(cols.len(), 6);
    assert!(cols.iter().all(|c| col_weeks(c).len() == 1));
    // Le settimane sono consecutive di 7 giorni.
    let first = col_weeks(&cols[0])[0];
    let last = col_weeks(&cols[5])[0];
    assert_eq!(last - first, 5 * WEEK_STEP as i32);
}

/// Lo zoom raggruppa 2 (livello 1) o 4 (livello 2) settimane per colonna.
#[test]
fn zoom_groups_weeks_per_column() {
    let app = app_spanning((2026, 1, 5), (2026, 2, 23)); // 8 settimane
    assert_eq!(columns_vec(&app, 0).len(), 8);

    let z1 = columns_vec(&app, 1);
    assert_eq!(z1.len(), 4);
    assert!(z1.iter().all(|c| col_weeks(c).len() == 2));

    let z2 = columns_vec(&app, 2);
    assert_eq!(z2.len(), 2);
    assert!(z2.iter().all(|c| col_weeks(c).len() == 4));

    // Qualunque sia lo zoom, nessuna settimana si perde o si duplica.
    for level in [0, 1, 2] {
        let all: Vec<i32> = columns_vec(&app, level)
            .iter()
            .flat_map(col_weeks)
            .collect();
        assert_eq!(all.len(), 8, "livello {level}");
    }
}

/// A cavallo di fine anno viene inserita la colonna gialla di confine, e
/// **nessun gruppo dello zoom attraversa il confine**: dicembre e gennaio
/// non finiscono mai nella stessa colonna.
#[test]
fn year_boundary_splits_groups_and_inserts_a_canary_column() {
    // Dal 30/11/2026 al 25/01/2027: il confine 2026→2027 cade in mezzo.
    let app = app_spanning((2026, 11, 30), (2027, 1, 25));
    let cols = columns_vec(&app, 2); // gruppi da 4

    let boundaries: Vec<&Col> = cols
        .iter()
        .filter(|c| matches!(c, Col::YearEnd(_)))
        .collect();
    assert_eq!(boundaries.len(), 1, "un solo confine d'anno");
    assert!(matches!(boundaries[0], Col::YearEnd(2026)));

    // Ogni gruppo sta tutto dentro un anno solo.
    for c in &cols {
        let years: HashSet<i32> = col_weeks(c)
            .iter()
            .map(|w| days_to_local(*w).year())
            .collect();
        assert!(
            years.len() <= 1,
            "un gruppo non può attraversare il confine d'anno: {years:?}"
        );
    }
}

/// La colonna di confine è più stretta di una settimana, e gli offset sono
/// coerenti con la larghezza totale: è ciò che tiene allineati header,
/// griglia e footer, che scorrono insieme.
#[test]
fn column_offsets_are_consistent_with_total_width() {
    let app = app_spanning((2026, 11, 30), (2027, 1, 25));
    let cols = columns_vec(&app, 0);
    let cw = COL_W;

    // L'offset dell'ultima colonna più la sua larghezza = larghezza totale.
    let last = cols.len() - 1;
    assert_eq!(
        col_x_offset(&cols, last, cw) + col_width(&cols[last], cw),
        cols_width(&cols, cw)
    );
    // Il primo offset è zero e gli offset crescono monotonamente.
    assert_eq!(col_x_offset(&cols, 0, cw), 0.0);
    for i in 1..cols.len() {
        assert!(col_x_offset(&cols, i, cw) > col_x_offset(&cols, i - 1, cw));
    }
    // La colonna di confine non è larga come una settimana.
    let boundary = cols.iter().find(|c| matches!(c, Col::YearEnd(_))).unwrap();
    assert_ne!(col_width(boundary, cw), cw);
}

/// Lo scroll che centra una colonna non va mai a sinistra dell'origine
/// (le prime colonne restano semplicemente attaccate al bordo).
#[test]
fn centered_scroll_never_goes_negative() {
    let app = app_spanning((2026, 1, 5), (2026, 6, 29)); // ~26 settimane
    let cols = columns_vec(&app, 0);
    let visible = 800.0;

    // Prima colonna: vorrebbe un offset negativo → resta a 0.
    assert_eq!(centered_scroll_x(&cols, 0, COL_W, visible), 0.0);
    // Colonna lontana: centrata nella vista.
    let idx = cols.len() - 1;
    let x = centered_scroll_x(&cols, idx, COL_W, visible);
    assert_eq!(
        x,
        col_x_offset(&cols, idx, COL_W) + COL_W / 2.0 - visible / 2.0
    );
    // Una vista larghissima non lascia nulla da scorrere.
    assert_eq!(centered_scroll_x(&cols, idx, COL_W, 100_000.0), 0.0);
}

// ── Altezze delle righe (colonna sinistra e griglia usano la stessa) ────

/// L'altezza del blocco di un dev dipende dalla modalità: compatta = 1 riga,
/// mergiata = 2 (cumulativo + somma), normale = una riga per worker + il
/// cumulativo. Il bordo è contato due volte (sopra e sotto).
#[test]
fn dev_block_height_per_mode() {
    // Normale: 3 worker → 3 righe + 1 cumulativo.
    assert_eq!(dev_inner_h(3, false, false), 4.0 * ROW_H);
    // Compatta: sempre una riga (la barra), quanti che siano i worker.
    assert_eq!(dev_inner_h(3, true, false), ROW_H);
    // Mergiata (zoom): cumulativo + somma.
    assert_eq!(dev_inner_h(3, false, true), 2.0 * ROW_H);
    // La compatta vince sulla mergiata.
    assert_eq!(dev_inner_h(3, true, true), ROW_H);
    // Il blocco aggiunge il bordo sopra e sotto.
    assert_eq!(
        dev_block_height(3, false, false),
        2.0 * DEV_BORDER + 4.0 * ROW_H
    );
}

/// `project_layout` è l'unica sorgente di verità delle altezze: quello che
/// restituisce deve quadrare con `total_content_h`, altrimenti la colonna
/// sinistra e la griglia scorrerebbero disallineate.
fn layout_in_headless_ui(
    app: &App,
    filter: &Filter,
    dev_filter: &DevFilter,
    compact: bool,
) -> Vec<ProjLayout> {
    let ctx = egui::Context::default();
    let input = egui::RawInput {
        screen_rect: Some(screen(1200.0, 800.0)),
        ..Default::default()
    };
    let out = std::cell::RefCell::new(Vec::new());
    let _ = ctx.run_ui_test(input, |ui| {
        let ctx = ui.ctx();
        egui::Window::new("t").show(ctx, |ui| {
            *out.borrow_mut() = project_layout(
                ui,
                app,
                filter,
                false,
                dev_filter,
                &ProjectFilter::default(),
                ProjectViewMode::Open,
                compact,
                false,
            );
        });
    });
    out.into_inner()
}

/// App di prova: due progetti aperti, il primo con due dev.
fn layout_app() -> (App, ProjectId, ProjectId, DevId, DevId) {
    let mut app = App::new();
    let alice = app.workers.add("Alice");
    let bob = app.workers.add("Bob");
    let front = app.devs.add("Frontend");
    let back = app.devs.add("Backend");
    let wk = app.start_week;

    let p1 = app.projects.add("Primo", Some("AAA"), Some(wk));
    app.projects.add_dev(p1, front);
    app.projects.add_dev(p1, back);
    app.projects.add_effort(p1, front, wk, alice, Effort(8));
    app.projects.add_effort(p1, front, wk, bob, Effort(4));
    app.projects.add_effort(p1, back, wk, bob, Effort(8));

    let p2 = app.projects.add("Secondo", Some("BBB"), Some(wk));
    app.projects.add_dev(p2, front);
    app.projects.add_effort(p2, front, wk, alice, Effort(8));

    (app, p1, p2, front, back)
}

#[test]
fn project_layout_heights_add_up_to_the_scrollable_content() {
    let (app, ..) = layout_app();
    let layout = layout_in_headless_ui(&app, &None, &None, false);
    assert_eq!(layout.len(), 2);

    let expected: f32 = DEV_BORDER + layout.iter().map(|p| p.proj_h + DEV_BORDER).sum::<f32>();
    assert_eq!(total_content_h(&layout), expected);

    // L'altezza di un progetto copre sempre i suoi blocchi dev.
    for p in &layout {
        let devs: f32 = p
            .devs
            .iter()
            .map(|(_, m)| dev_block_height(*m, false, false))
            .sum();
        assert!(
            p.proj_h >= devs,
            "il progetto deve contenere i suoi dev: {} < {devs}",
            p.proj_h
        );
    }
}

/// Con un filtro attivo l'intestazione del progetto si riduce a una riga
/// (solo tripletta), così le righe filtrate non trascinano spazio inutile.
#[test]
fn active_filter_shrinks_the_project_header() {
    let (app, ..) = layout_app();
    let unfiltered = layout_in_headless_ui(&app, &None, &None, false);

    let only_alice: Filter = Some(["Alice".to_string()].into_iter().collect());
    let filtered = layout_in_headless_ui(&app, &only_alice, &None, false);

    // Alice lavora solo su Frontend: nel primo progetto resta un dev solo.
    let p1_before = &unfiltered[0];
    let p1_after = &filtered[0];
    assert_eq!(p1_before.devs.len(), 2);
    assert_eq!(p1_after.devs.len(), 1);
    assert!(
        p1_after.proj_h < p1_before.proj_h,
        "l'intestazione compressa deve far calare l'altezza: {} !< {}",
        p1_after.proj_h,
        p1_before.proj_h
    );
}

/// Un progetto senza dev corrispondenti al filtro sparisce del tutto.
#[test]
fn projects_without_matching_workers_disappear() {
    let (app, ..) = layout_app();
    let only_bob: Filter = Some(["Bob".to_string()].into_iter().collect());
    let layout = layout_in_headless_ui(&app, &only_bob, &None, false);
    // Bob è solo nel primo progetto → il secondo non compare.
    assert_eq!(layout.len(), 1);
    assert_eq!(layout[0].name, "Primo");
}

/// La vista compatta abbassa ogni progetto: un blocco dev è una riga sola.
#[test]
fn compact_view_is_shorter_than_the_normal_one() {
    let (app, ..) = layout_app();
    let normal = layout_in_headless_ui(&app, &None, &None, false);
    let compact = layout_in_headless_ui(&app, &None, &None, true);

    assert_eq!(normal.len(), compact.len());
    assert!(
        total_content_h(&compact) < total_content_h(&normal),
        "compatta {} !< normale {}",
        total_content_h(&compact),
        total_content_h(&normal)
    );
}

// ── Notifiche in-app (toast) ────────────────────────────────────────────

#[test]
fn toast_icons_are_renderable() {
    let ctx = egui::Context::default();
    install_symbol_fallback(&ctx);
    let _ = ctx.run_ui_test(egui::RawInput::default(), |_| {});
    let font = egui::FontId::proportional(14.0);
    for level in [
        ToastLevel::Info,
        ToastLevel::Success,
        ToastLevel::Warning,
        ToastLevel::Error,
    ] {
        assert!(
            ctx.fonts_mut(|f| f.has_glyphs(&font, level.icon())),
            "l'icona di {level:?} ({}) non è disegnabile coi font caricati",
            level.icon()
        );
    }
}

#[test]
fn repeated_toasts_refresh_instead_of_stacking() {
    let mut state = UiState::default();
    state.toast_error("git: 'push' non riuscito");
    state.toast_error("git: 'push' non riuscito");
    state.toast_error("git: 'push' non riuscito");
    // Un errore che si ripete a ogni autosave non impila copie.
    assert_eq!(state.toasts.len(), 1);

    // Un testo diverso è una notifica a sé.
    state.toast_ok("File salvato");
    assert_eq!(state.toasts.len(), 2);
}

#[test]
fn toast_list_is_capped_keeping_the_newest() {
    let mut state = UiState::default();
    for i in 0..(MAX_TOASTS + 3) {
        state.toast_info(format!("messaggio {i}"));
    }
    assert_eq!(state.toasts.len(), MAX_TOASTS);
    // Le più vecchie escono dalla cima: resta in coda l'ultima arrivata.
    assert_eq!(
        state.toasts.last().map(|t| t.text.as_str()),
        Some(format!("messaggio {}", MAX_TOASTS + 2).as_str())
    );
    assert!(!state.toasts.iter().any(|t| t.text == "messaggio 0"));
}

#[test]
fn only_errors_are_sticky() {
    // Gli errori restano finché non li si chiude; gli altri livelli scadono.
    assert_eq!(ToastLevel::Error.ttl(), None);
    for level in [ToastLevel::Info, ToastLevel::Success, ToastLevel::Warning] {
        assert!(level.ttl().is_some(), "{level:?} deve scadere da sola");
    }
}

/// Il layer si disegna davvero (layout annidato ✕ + testo a capo) e le
/// notifiche scadute spariscono da sole, quelle di errore no.
#[test]
fn toasts_layer_draws_and_expires() {
    let ctx = egui::Context::default();
    let input = egui::RawInput {
        screen_rect: Some(screen(1000.0, 700.0)),
        ..Default::default()
    };

    let mut state = UiState::default();
    state.toast_error(
        "Errore lungo che deve andare a capo dentro il riquadro \
                       della notifica senza far esplodere il layout",
    );
    state.toast_ok("PDF salvato: progetto_26-08-10.pdf");
    state.toast_warn("Nessun progetto visibile: PDF non creato.");

    // Due frame: la prima registra l'area, la seconda disegna a layout stabile.
    for _ in 0..2 {
        let _ = ctx.run_ui_test(input.clone(), |ui| toasts_layer(ui.ctx(), &mut state));
    }
    assert_eq!(state.toasts.len(), 3, "nessuna deve sparire subito");

    // Invecchiamo tutto oltre la ttl più lunga: resta solo l'errore.
    let old = std::time::Instant::now() - std::time::Duration::from_secs(3600);
    for t in state.toasts.iter_mut() {
        t.born = old;
    }
    let _ = ctx.run_ui_test(input.clone(), |ui| toasts_layer(ui.ctx(), &mut state));
    assert_eq!(state.toasts.len(), 1);
    assert_eq!(state.toasts[0].level, ToastLevel::Error);
}

/// Un errore di salvataggio deve arrivare all'utente come notifica: qui il
/// percorso è una cartella inesistente, quindi la scrittura fallisce.
#[test]
fn failed_save_reports_an_error_toast() {
    let app = App::new();
    let bad = "/pjm_cartella_inesistente_per_test/workers.ron";
    let err = app.save(bad).expect_err("il salvataggio deve fallire");

    let mut state = UiState::default();
    state.toast_error(err);
    assert_eq!(state.toasts.len(), 1);
    assert_eq!(state.toasts[0].level, ToastLevel::Error);
    assert!(state.toasts[0].text.contains(bad));
}

/// Ogni carattere non ASCII nelle stringhe dell'interfaccia (simboli, icone,
/// frecce, lettere accentate) dev'essere disegnabile con i font caricati
/// dall'app; altrimenti comparirebbe un quadratino vuoto. Legge i sorgenti di
/// `src/ui`: copre anche i simboli aggiunti in futuro. Da egui 0.37 i font con
/// simboli/emoji vanno attivati (`monochrome_emoji_fonts` in `Cargo.toml`), e
/// «✕» non c'è in nessun font di egui (al suo posto «✖»).
#[test]
fn every_ui_symbol_is_renderable() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/ui");
    let mut chars = std::collections::BTreeMap::<char, String>::new();
    for entry in std::fs::read_dir(&dir).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        if !name.ends_with(".rs") || name == "tests.rs" || name == "test_util.rs" {
            continue;
        }
        for (n, line) in std::fs::read_to_string(&path).unwrap().lines().enumerate() {
            if line.trim_start().starts_with("//") {
                continue;
            }
            // Testo tra virgolette (stringhe sulla riga), commenti finali esclusi.
            let code = line.split(" // ").next().unwrap_or(line);
            for (k, part) in code.split('"').enumerate() {
                if k % 2 == 1 {
                    for c in part.chars().filter(|c| !c.is_ascii()) {
                        chars.entry(c).or_insert_with(|| format!("{name}:{}", n + 1));
                    }
                }
            }
        }
    }
    assert!(chars.len() > 10, "il sorgente dovrebbe contenere simboli: {chars:?}");
    let ctx = egui::Context::default();
    install_symbol_fallback(&ctx);
    let _ = ctx.run_ui_test(egui::RawInput::default(), |_| {});
    let font = egui::FontId::proportional(14.0);
    let missing: Vec<String> = chars
        .iter()
        .filter(|(c, _)| !ctx.fonts_mut(|f| f.has_glyphs(&font, &c.to_string())))
        .map(|(c, at)| format!("{c} (U+{:04X}) in {at}", *c as u32))
        .collect();
    assert!(missing.is_empty(), "simboli non disegnabili: {missing:?}");
}

/// Tasto destro premuto e trascinamento sulla griglia: scorre in orizzontale e
/// in verticale (è il modo in cui l'utente si muove ovunque). Da egui 0.37 il
/// trascinamento vale di default solo sui touch screen: `drag_scroll()` lo
/// riattiva col mouse.
#[test]
fn right_drag_scrolls_the_grid_both_ways() {
    let mut app = App::new();
    for i in 0..40 {
        let p = app
            .projects
            .add("P", Some(&format!("T{i:02}")), Some(app.start_week));
        app.projects.set_project_end_week(p, Some(app.end_week));
        let dev = app.devs.add("Frontend");
        app.projects.add_dev(p, dev);
    }
    let ctx = egui::Context::default();
    let mut state = UiState::default();
    let run = |state: &mut UiState, events: Vec<egui::Event>| {
        let input = egui::RawInput {
            screen_rect: Some(screen(1600.0, 900.0)),
            events,
            ..Default::default()
        };
        let mut actions = Vec::new();
        let _ = ctx.run_ui_test(input, |ui| body(ui, &app, state, &mut actions));
    };
    run(&mut state, vec![]);
    run(&mut state, vec![]);
    let start = egui::pos2(1000.0, 500.0);
    let end = egui::pos2(700.0, 300.0); // trascina a sinistra e in su
    let button = |pos, pressed| egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Secondary,
        pressed,
        modifiers: Default::default(),
    };
    run(&mut state, vec![egui::Event::PointerMoved(start)]);
    run(&mut state, vec![button(start, true)]);
    for k in 1..=10 {
        let t = k as f32 / 10.0;
        run(&mut state, vec![egui::Event::PointerMoved(start + (end - start) * t)]);
    }
    run(&mut state, vec![button(end, false)]);
    assert!(state.scroll_x > 250.0, "orizzontale: {}", state.scroll_x);
    assert!(state.scroll_y > 150.0, "verticale: {}", state.scroll_y);
}

/// L'autocompletamento della cella non propone i worker senza la spunta
/// «Elenchi»; scritto per intero, però, un nome nascosto viene accettato.
#[test]
fn cell_completion_skips_workers_hidden_from_lists() {
    let mut app = App::new();
    app.workers.add("Marco");
    let mario = app.workers.add("Mario");
    app.workers.set_shown_in_find(mario, false);
    // «Mar»: resta solo Marco, completato col separatore.
    assert_eq!(find_completion(&app, "Mar", true), "Marco|");
    // «Mari»: Mario è nascosto, nessun suggerimento (resta il digitato).
    assert_eq!(find_completion(&app, "Mari", true), "");
    let (mut typed, mut buf) = ("Mari".to_string(), String::new());
    recompute_completion(&app, &mut typed, &mut buf);
    assert_eq!(buf, "Mari");
    // Rimesso negli elenchi torna a essere proposto.
    app.workers.set_shown_in_find(mario, true);
    assert_eq!(find_completion(&app, "Mari", true), "Mario|");
    // Nome nascosto scritto per intero: la cella lo accetta.
    app.workers.set_shown_in_find(mario, false);
    let (name, effort) = parse_cell("Mario|8").unwrap();
    assert_eq!(app.workers.get_id_by_name(name), Some(mario));
    assert_eq!(effort, 8);
}

/// Navigazione da tastiera: destra/sinistra saltano le settimane bloccate e si
/// fermano ai limiti del progetto; su/giù passano al dev vicino, anche di un
/// altro progetto, saltando quelli in cui la settimana non è modificabile.
#[test]
fn nav_target_skips_locked_cells_and_crosses_devs() {
    let mut app = App::new();
    let weeks = weeks_vec(&app);
    let (w0, w1, w2) = (weeks[2], weeks[3], weeks[4]);
    // A: tre settimane (w0..w2), dev X (2 righe) e Y (1 riga)
    let a = app.projects.add("A", Some("AAA"), Some(WeekId(w0 as usize)));
    app.projects.set_project_end_week(a, Some(WeekId(w2 as usize)));
    // B: comincia dopo w0 → a w0 non è modificabile
    let b = app.projects.add("B", Some("BBB"), Some(WeekId(w1 as usize)));
    app.projects.set_project_end_week(b, Some(WeekId(w2 as usize)));
    // C: comprende w0
    let c = app.projects.add("C", Some("CCC"), Some(WeekId(w0 as usize)));
    app.projects.set_project_end_week(c, Some(WeekId(w2 as usize)));
    let (x, y, z, t) = (DevId(0), DevId(1), DevId(2), DevId(3));
    let order = vec![(a, x, 2), (a, y, 1), (b, z, 1), (c, t, 3)];
    let nav = |from, dir| nav_target(&app, &order, from, dir);

    // destra/sinistra dentro il periodo, stop ai bordi del progetto
    assert_eq!(nav((a, x, w0, 1), NavDir::Right), Some((a, x, w1, 1)));
    assert_eq!(nav((a, x, w2, 1), NavDir::Right), None, "dopo la fine: fermo");
    assert_eq!(nav((a, x, w0, 0), NavDir::Left), None, "prima dell'inizio: fermo");
    // giù: riga successiva, poi dev successivo dalla riga 0
    assert_eq!(nav((a, x, w0, 0), NavDir::Down), Some((a, x, w0, 1)));
    assert_eq!(nav((a, x, w0, 1), NavDir::Down), Some((a, y, w0, 0)));
    // da Y a w0: B non ha w0 → si salta a C
    assert_eq!(nav((a, y, w0, 0), NavDir::Down), Some((c, t, w0, 0)));
    // a w1 invece B c'è
    assert_eq!(nav((a, y, w1, 0), NavDir::Down), Some((b, z, w1, 0)));
    assert_eq!(nav((c, t, w0, 2), NavDir::Down), None, "ultima riga dell'ultimo dev");
    // su: riga precedente, poi ultima riga del dev precedente (saltando B a w0)
    assert_eq!(nav((c, t, w0, 0), NavDir::Up), Some((a, y, w0, 0)));
    assert_eq!(nav((a, y, w0, 0), NavDir::Up), Some((a, x, w0, 1)));
    assert_eq!(nav((a, x, w0, 0), NavDir::Up), None);
}

/// Tasti veri sull'interfaccia intera: freccia destra, Tab e Invio confermano
/// il valore e aprono la cella vicina (egui non deve rubare frecce/Tab al
/// campo per spostare il focus altrove).
#[test]
fn keyboard_moves_between_cells_and_commits() {
    let mut app = App::new();
    let weeks = weeks_vec(&app);
    let w = weeks[2];
    let p = app.projects.add("P", Some("PPP"), Some(WeekId(weeks[0] as usize)));
    app.projects.set_project_end_week(p, Some(WeekId(weeks[10] as usize)));
    let dev = app.devs.add("Frontend");
    app.projects.add_dev(p, dev);
    let alice = app.workers.add("Alice");
    let bob = app.workers.add("Bob");
    // due righe nel dev (due worker nella stessa settimana)
    app.projects.add_effort(p, dev, WeekId(weeks[5] as usize), alice, Effort(1));
    app.projects.add_effort(p, dev, WeekId(weeks[5] as usize), bob, Effort(1));
    app.compute_sovra();

    let ctx = egui::Context::default();
    let mut pjm = PjmApp { app, ui: UiState::default() };
    pjm.ui.editing = Some(Editing::open(p, dev, w, 0, "Alice|4", ""));
    let mut frame = eframe::Frame::_new_kittest();
    let mut run = |pjm: &mut PjmApp, events: Vec<egui::Event>| {
        let input = egui::RawInput { screen_rect: Some(screen(1600.0, 900.0)), events, ..Default::default() };
        let _ = ctx.run_ui_test(input, |ui| eframe::App::ui(pjm, ui, &mut frame));
    };
    let key = |key, shift: bool| egui::Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: if shift { egui::Modifiers::SHIFT } else { egui::Modifiers::NONE },
    };
    let at = |pjm: &PjmApp| pjm.ui.editing.as_ref().map(|e| (e.week, e.row));
    run(&mut pjm, vec![]);
    run(&mut pjm, vec![]);

    // → : conferma Alice|4 e apre la settimana dopo, stessa riga
    run(&mut pjm, vec![key(egui::Key::ArrowRight, false)]);
    run(&mut pjm, vec![]);
    let effort = |pjm: &PjmApp, week: i32, wid| {
        pjm.app.projects.get_single_dev(p, dev).unwrap().get_all(WeekId(week as usize))
            .and_then(|s| s.worker_id.get(&wid).map(|e| e.get_effort().0))
    };
    assert_eq!(effort(&pjm, w, alice), Some(4), "valore confermato");
    assert_eq!(at(&pjm), Some((weeks[3], 0)), "cella a destra");

    // Tab → ancora a destra; Shift+Tab ← torna indietro
    run(&mut pjm, vec![key(egui::Key::Tab, false)]);
    run(&mut pjm, vec![]);
    assert_eq!(at(&pjm), Some((weeks[4], 0)));
    run(&mut pjm, vec![key(egui::Key::Tab, true)]);
    run(&mut pjm, vec![]);
    assert_eq!(at(&pjm), Some((weeks[3], 0)));

    // Invio → riga sotto; Shift+Invio ↑ torna su
    run(&mut pjm, vec![key(egui::Key::Enter, false)]);
    run(&mut pjm, vec![]);
    assert_eq!(at(&pjm), Some((weeks[3], 1)));
    run(&mut pjm, vec![key(egui::Key::Enter, true)]);
    run(&mut pjm, vec![]);
    assert_eq!(at(&pjm), Some((weeks[3], 0)));

    // Esc annulla e chiude, senza spostarsi
    run(&mut pjm, vec![key(egui::Key::Escape, false)]);
    run(&mut pjm, vec![]);
    assert_eq!(at(&pjm), None);
}

/// Navigando oltre il bordo dello schermo la griglia scorre per tenere visibile
/// la cella in modifica (a destra e poi di nuovo a sinistra).
#[test]
fn keyboard_navigation_scrolls_the_cell_into_view() {
    let mut app = App::new();
    let weeks = weeks_vec(&app);
    let p = app.projects.add("P", Some("PPP"), Some(WeekId(weeks[0] as usize)));
    app.projects.set_project_end_week(p, Some(WeekId(weeks[60] as usize)));
    let dev = app.devs.add("Frontend");
    app.projects.add_dev(p, dev);
    let ctx = egui::Context::default();
    let mut pjm = PjmApp { app, ui: UiState::default() };
    pjm.ui.editing = Some(Editing::open(p, dev, weeks[0], 0, "", ""));
    let mut frame = eframe::Frame::_new_kittest();
    let mut run = |pjm: &mut PjmApp, events: Vec<egui::Event>| {
        let input = egui::RawInput { screen_rect: Some(screen(1200.0, 800.0)), events, ..Default::default() };
        let _ = ctx.run_ui_test(input, |ui| eframe::App::ui(pjm, ui, &mut frame));
    };
    let right = egui::Event::Key {
        key: egui::Key::ArrowRight,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    };
    run(&mut pjm, vec![]);
    run(&mut pjm, vec![]);
    assert_eq!(pjm.ui.scroll_x, 0.0);
    // ~10 colonne da 90 px stanno nei ~900 px della griglia: 20 passi escono
    for _ in 0..20 {
        run(&mut pjm, vec![right.clone()]);
        run(&mut pjm, vec![]);
        run(&mut pjm, vec![]);
    }
    assert_eq!(pjm.ui.editing.as_ref().map(|e| e.week), Some(weeks[20]));
    let x = pjm.ui.scroll_x;
    assert!(x > 10.0 * COL_W, "la griglia deve seguire la cella: scroll_x = {x}");
    // la cella in modifica è dentro l'area visibile della griglia
    let cell_left = (20.0 * COL_W) - x; // rispetto all'inizio delle colonne
    assert!(cell_left >= 0.0 && cell_left + COL_W <= 1200.0 - LEFT_W, "cella fuori vista: {cell_left}");
}

#[test]
fn file_safe_replaces_characters_not_allowed_in_file_names() {
    assert_eq!(file_safe(" Mario Rossi "), "Mario_Rossi");
    assert_eq!(file_safe("a/b:c*d?"), "a_b_c_d_");
    assert_eq!(file_safe("Élodie"), "Élodie");
}

/// Fixture di «Sposta efforts»: `d1` ha effort in 3 settimane consecutive da
/// `base + 7` (un lunedì), `d2` solo in `base`; milestone in `base + 14`.
/// Ritorna le `moves` come le costruisce il passo SelectDevs.
#[allow(clippy::type_complexity)]
fn move_devs_fixture() -> (App, ProjectId, usize, MilestoneId, Vec<(DevId, Vec<WeekId>)>) {
    let mut app = App::new();
    let w = app.workers.add("Alice");
    let d1 = app.devs.add("Frontend");
    let d2 = app.devs.add("Backend");
    let proj = app.projects.add("Progetto", Some("ABC"), None);
    let base = current_monday_days() as usize;
    for i in 1..4 {
        app.projects.add_effort(proj, d1, WeekId(base + i * WEEK_STEP), w, Effort(8));
    }
    app.projects.add_effort(proj, d2, WeekId(base), w, Effort(4));
    let m = app.milestones.add("Rilascio");
    app.projects.add_project_milestone(proj, m, WeekId(base + 2 * WEEK_STEP));
    let moves = vec![
        (d1, app.projects.dev_effort_weeks(proj, d1)),
        (d2, app.projects.dev_effort_weeks(proj, d2)),
    ];
    (app, proj, base, m, moves)
}

#[test]
fn move_devs_end_goes_to_the_clicked_week_and_dates_follow_the_weeks() {
    let (app, proj, base, m, moves) = move_devs_fixture();
    // Il gruppo va da `base` (d2) a `base + 21` (d1); clic su `base + 35`.
    let MoveDialog::Params {
        moves: m2,
        weeks_text,
        milestones,
        from,
        ..
    } = make_move_devs_params(&app, proj, moves.clone(), WeekId(base + 5 * WEEK_STEP))
    else {
        panic!("atteso il passo Params");
    };
    assert_eq!(m2, moves, "si sposta tutto il dev");
    assert_eq!(milestones.iter().map(|(id, _, s)| (*id, *s)).collect::<Vec<_>>(), vec![(m, true)]);
    let f = from.expect("Sposta efforts porta le date");
    assert_eq!((f.first, f.last, f.delta), (base, base + 3 * WEEK_STEP, 2));
    assert_eq!(weeks_text, "2", "la fine del gruppo va nella settimana cliccata");

    // Un giorno qualsiasi vale la sua settimana; avanti e indietro.
    assert_eq!(weeks_to_date(base, (base + 2 * WEEK_STEP + 3) as i32), 2);
    assert_eq!(weeks_to_date(base, (base - 3 * WEEK_STEP + 6) as i32), -3);
    assert_eq!(weeks_to_date(base, base as i32), 0);

    // «Sposta blocco» non ha le date.
    let MoveDialog::Params { from, .. } = make_move_params(&app, proj, moves) else {
        panic!("atteso il passo Params");
    };
    assert!(from.is_none());
}

#[test]
fn move_devs_window_shows_start_and_end_and_typing_replaces_the_weeks() {
    let (app, proj, base, _m, moves) = move_devs_fixture();
    let ctx = egui::Context::default();
    let input = egui::RawInput {
        screen_rect: Some(screen(1000.0, 800.0)),
        ..Default::default()
    };
    let mut state = UiState {
        move_dialog: Some(make_move_devs_params(
            &app,
            proj,
            moves.clone(),
            WeekId(base + 5 * WEEK_STEP),
        )),
        ..Default::default()
    };
    let date = |w: usize| days_to_local(w as i32).format("%Y-%m-%d").to_string();
    let mut actions = Vec::new();
    let mut button = None;
    let mut first_texts = Vec::new();
    // Primo frame: il campo settimane («2») prende il focus tutto selezionato;
    // poi si scrive «-1», che lo sostituisce.
    for (k, ev) in [None, None, Some(egui::Event::Text("-1".into())), None]
        .into_iter()
        .enumerate()
    {
        let mut i = input.clone();
        i.events = ev.into_iter().collect();
        let out = ctx.run_ui_test(i, |ui| {
            move_dialog_window(ui.ctx(), &app, &mut state, &mut actions);
        });
        if k == 1 {
            first_texts = drawn_texts(&out);
        }
        button = text_pos(&out, "Sposta");
    }
    for t in ["Sposta inizio qua", "Sposta fine qua"] {
        assert!(first_texts.iter().any(|x| x == t), "{t}: {first_texts:?}");
    }
    // Spostamento 2: inizio base+14, fine base+35 (la settimana cliccata).
    for d in [date(base + 2 * WEEK_STEP), date(base + 5 * WEEK_STEP)] {
        assert!(first_texts.iter().any(|x| x.starts_with(&d)), "{d}: {first_texts:?}");
    }
    let Some(MoveDialog::Params { from: Some(f), weeks_text, .. }) = &state.move_dialog else {
        panic!("atteso il passo Params con le date");
    };
    assert_eq!(weeks_text, "-1", "digitando si sostituisce il valore proposto");
    assert_eq!(f.delta, -1, "le date seguono le settimane");

    for ev in click_events(button.expect("bottone «Sposta»"), egui::PointerButton::Primary) {
        let mut i = input.clone();
        i.events = vec![ev];
        let _ = ctx.run_ui_test(i, |ui| {
            move_dialog_window(ui.ctx(), &app, &mut state, &mut actions);
        });
    }
    match actions.as_slice() {
        [Action::MoveEffort { moves: m, delta: -1, .. }] => assert_eq!(m, &moves),
        other => panic!("attesa una MoveEffort, trovate {} azioni", other.len()),
    }
}

/// Settimana corrente: niente più velatura, un bordo verde acceso sulla sua
/// colonna in intestazione (chiuso in alto), griglia e footer (chiuso in
/// fondo); nessun bordo sulle altre colonne.
#[test]
fn current_week_has_a_green_border_in_header_grid_and_footer() {
    let (app, _, week) = highlight_fixture();
    let ctx = egui::Context::default();
    let mut state = UiState {
        this_week: week,
        ..Default::default()
    };
    let input = egui::RawInput {
        screen_rect: Some(screen(1600.0, 900.0)),
        ..Default::default()
    };
    let mut actions = Vec::new();
    let out = ctx.run_ui_test(input, |ui| {
        header(ui, &app, &mut state);
        footer(ui, &app, &mut state, &mut actions);
        let layout = body_layout(ui, &app, &state, &Filter::default(), &None);
        grid(ui, &app, &mut state, &mut actions, &Filter::default(), &layout);
    });
    let green: Vec<Rect> = out
        .shapes
        .iter()
        .filter_map(|cs| match &cs.shape {
            egui::epaint::Shape::Rect(r) if r.fill == this_week_border() => Some(r.rect),
            _ => None,
        })
        .collect();
    // Lati in tre punti (2 + 2 + 2) + chiusura in alto e in fondo.
    assert_eq!(green.len(), 8, "{green:?}");
    let verticals = green.iter().filter(|r| r.height() > r.width()).count();
    assert_eq!(verticals, 6);
    // Le chiusure in alto e in fondo sono larghe quanto la colonna (qui i tre
    // pezzi hanno origini diverse: l'intestazione riserva la colonna sinistra).
    for r in green.iter().filter(|r| r.width() > r.height()) {
        assert!((r.width() - COL_W).abs() < 0.5, "chiusura larga {}", r.width());
    }
    // La vecchia velatura giallo neon non c'è più.
    let neon = Color32::from_rgb(0xCC, 0xFF, 0x00);
    assert!(!out.shapes.iter().any(|cs| matches!(&cs.shape,
        egui::epaint::Shape::Rect(r) if r.fill.r() == neon.r() && r.fill.g() == neon.g())));
}

/// Finestra «Worker, dev e categorie»: per ogni worker l'uso; il cestino
/// funziona solo su chi non è usato (Anna), su Mario (con celle) no.
#[test]
fn registry_window_shows_usage_and_deletes_only_unused() {
    let mut app = App::new();
    let mario = app.workers.add("Mario");
    let anna = app.workers.add("Anna");
    let fe = app.devs.add("Frontend");
    let p = app.projects.add("x", Some("P1"), None);
    app.projects.add_dev(p, fe);
    app.projects.add_effort(p, fe, WeekId(20000), mario, Effort(8));
    let ctx = egui::Context::default();
    let mut state = UiState {
        show_registry: true,
        ..Default::default()
    };
    let input = egui::RawInput {
        screen_rect: Some(screen(1200.0, 800.0)),
        ..Default::default()
    };
    let frame = |state: &mut UiState, events: Vec<egui::Event>| {
        let mut actions = Vec::new();
        let input = egui::RawInput { events, ..input.clone() };
        let out = ctx.run_ui_test(input, |ui| registry_window(ui.ctx(), &app, state, &mut actions));
        (out, actions)
    };
    frame(&mut state, vec![]);
    let (out, _) = frame(&mut state, vec![]);
    let texts = drawn_texts(&out);
    for t in ["Mario", "Anna", "1 cella in 1 progetto", "non usato", "Nascosto"] {
        assert!(texts.iter().any(|x| x == t), "manca «{t}»: {texts:?}");
    }
    // Cestini in ordine dall'alto: Anna, poi Mario (elenco per nome).
    let mut bins: Vec<egui::Pos2> = out
        .shapes
        .iter()
        .filter_map(|cs| match &cs.shape {
            egui::epaint::Shape::Text(t) if t.galley.text() == "🗑" => {
                Some(Rect::from_min_size(t.pos, t.galley.size()).center())
            }
            _ => None,
        })
        .collect();
    bins.sort_by(|a, b| a.y.total_cmp(&b.y));
    assert_eq!(bins.len(), 2);
    let click = |state: &mut UiState, pos: egui::Pos2| {
        let mut all = Vec::new();
        for e in click_events(pos, egui::PointerButton::Primary) {
            let (_, a) = frame(state, vec![e]);
            all.extend(a);
        }
        all
    };
    let acts = click(&mut state, bins[1]);
    assert!(acts.is_empty(), "Mario è usato: cestino spento");
    let acts = click(&mut state, bins[0]);
    assert!(matches!(acts.as_slice(), [Action::DeleteWorker(w)] if *w == anna), "{}", acts.len());
}

/// Le azioni della finestra: rinomina (il filtro worker, per nome, segue;
/// un doppione è rifiutato con un avviso), nascondi, elimina (via anche dalle
/// scelte della sessione).
#[test]
fn registry_actions_rename_hide_delete_and_clean_the_session() {
    let mut app = App::new();
    let mario = app.workers.add("Mario");
    let luigi = app.workers.add("Luigi");
    let fe = app.devs.add("Frontend");
    let ca = app.categories.add("Cliente A");
    let mut pjm = PjmApp {
        app,
        ui: UiState {
            worker_filter: Some(["Mario".to_string()].into()),
            pies_devs: [fe].into(),
            dev_filter: Some([fe].into()),
            selected_category: Some(ca),
            pies_workers: [luigi].into(),
            ..Default::default()
        },
    };
    pjm.apply(Action::RenameWorker { worker: mario, name: "Mario Rossi".into() });
    assert_eq!(pjm.app.workers.get_name_by_id(mario), "Mario Rossi");
    assert!(pjm.ui.worker_filter.as_ref().unwrap().contains("Mario Rossi"), "il filtro segue");
    pjm.apply(Action::RenameWorker { worker: mario, name: "luigi".into() });
    assert_eq!(pjm.app.workers.get_name_by_id(mario), "Mario Rossi", "doppione rifiutato");
    assert!(pjm.ui.toasts.iter().any(|t| t.text.contains("luigi")));

    pjm.apply(Action::SetWorkerArchived { worker: luigi, archived: true });
    assert!(pjm.app.workers.is_archived(luigi));
    pjm.apply(Action::SetDevHidden { dev: fe, hidden: true });
    pjm.apply(Action::SetCategoryHidden { cat: ca, hidden: true });
    assert!(pjm.app.devs.is_hidden(fe) && pjm.app.categories.is_hidden(ca));

    pjm.apply(Action::DeleteWorker(luigi));
    pjm.apply(Action::DeleteDev(fe));
    pjm.apply(Action::DeleteCategory(ca));
    assert!(pjm.app.workers.get_id_by_name("Luigi").is_none());
    assert!(pjm.app.devs.get(fe).is_none() && pjm.app.categories.get_name(ca).is_none());
    assert!(pjm.ui.pies_workers.is_empty() && pjm.ui.pies_devs.is_empty());
    assert!(pjm.ui.dev_filter.as_ref().unwrap().is_empty());
    assert_eq!(pjm.ui.selected_category, None);
}

/// Un dev nascosto non è più proposto in «Dev del progetto», tranne in un
/// progetto che lo ha già (così si può ancora togliere).
#[test]
fn hidden_dev_is_not_offered_to_projects_that_lack_it() {
    let mut app = App::new();
    let fe = app.devs.add("Frontend");
    app.devs.add("Backend");
    let p = app.projects.add("x", Some("P1"), None);
    let q = app.projects.add("y", Some("P2"), None);
    app.projects.add_dev(q, fe);
    app.devs.set_hidden(fe, true);
    let texts_for = |proj: ProjectId| {
        let ctx = egui::Context::default();
        let mut state = UiState { dev_manage: Some(proj), ..Default::default() };
        let mut actions = Vec::new();
        let input = egui::RawInput {
            screen_rect: Some(screen(1200.0, 800.0)),
            ..Default::default()
        };
        // Il primo frame misura la finestra (ancorata al centro) senza testi.
        let _ = ctx.run_ui_test(input.clone(), |ui| {
            dev_manage_window(ui.ctx(), &app, &mut state, &mut actions)
        });
        drawn_texts(&ctx.run_ui_test(input, |ui| {
            dev_manage_window(ui.ctx(), &app, &mut state, &mut actions)
        }))
    };
    let t = texts_for(p);
    assert!(t.iter().any(|x| x == "Backend") && !t.iter().any(|x| x == "Frontend"), "{t:?}");
    let t = texts_for(q);
    assert!(t.iter().any(|x| x == "Frontend"), "già nel progetto: resta {t:?}");
}

/// Finestra «Worker, dev e categorie»: con un elenco lungo, scorrendo in giù
/// l'intestazione (Nome / Uso / Nascosto) resta visibile sopra le righe, e la
/// colonna Nome è larga.
#[test]
fn registry_header_stays_visible_when_scrolled() {
    let mut app = App::new();
    for i in 0..60 {
        app.workers.add(&format!("W{i:02}"));
    }
    let ctx = egui::Context::default();
    let mut state = UiState { show_registry: true, ..Default::default() };
    let base = egui::RawInput {
        screen_rect: Some(screen(1200.0, 800.0)),
        ..Default::default()
    };
    let mut frame = |events: Vec<egui::Event>| {
        let mut actions = Vec::new();
        ctx.run_ui_test(egui::RawInput { events, ..base.clone() }, |ui| {
            registry_window(ui.ctx(), &app, &mut state, &mut actions)
        })
    };
    frame(vec![]);
    let out = frame(vec![]);
    // Visibile = dentro il proprio ritaglio.
    let visible = |out: &egui::FullOutput, s: &str| -> Option<Rect> {
        out.shapes.iter().find_map(|cs| match &cs.shape {
            egui::epaint::Shape::Text(t) if t.galley.text() == s => {
                let r = Rect::from_min_size(t.pos, t.galley.size());
                cs.clip_rect.intersects(r).then_some(r)
            }
            _ => None,
        })
    };
    let first = visible(&out, "W00").expect("prima riga visibile all'inizio");
    let nome = visible(&out, "Nome").expect("intestazione");
    assert!(nome.bottom() <= first.top());
    // Rotella sopra l'elenco: scorre in giù.
    let over = first.center();
    frame(vec![egui::Event::PointerMoved(over)]);
    for _ in 0..5 {
        frame(vec![
            egui::Event::PointerMoved(over),
            egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: egui::vec2(0.0, -400.0),
                modifiers: Default::default(),
                phase: egui::TouchPhase::Move,
                source: egui::MouseWheelSource::Wheel,
            },
        ]);
    }
    let out = frame(vec![]);
    assert!(visible(&out, "W00").is_none(), "l'elenco è scorso");
    let nome2 = visible(&out, "Nome").expect("intestazione ancora visibile");
    assert!((nome2.top() - nome.top()).abs() < 1.0, "ferma: {nome:?} → {nome2:?}");
    // Colonna Nome larga: «Uso» parte dopo i 340 px del nome.
    let uso = visible(&out, "Uso").unwrap();
    assert!(uso.left() - nome2.left() >= 340.0, "Nome largo {}", uso.left() - nome2.left());
    // Le righe restano allineate all'intestazione.
    let usage = visible(&out, "non usato").expect("una riga visibile");
    assert!((usage.left() - uso.left()).abs() < 1.0, "Uso {uso:?} / riga {usage:?}");
}

/// Scheda Dev: colonna «Nei totali» (la spunta di Filtri ▸ Dev nei totali…),
/// indipendente da «Nascosto»; un dev tolto dai totali ha la casella vuota e
/// spuntarla lo rimette. Nelle altre schede la colonna non c'è.
#[test]
fn registry_dev_tab_has_the_in_totals_column() {
    let mut app = App::new();
    let dms = app.devs.add("DMS");
    app.devs.add("Frontend");
    app.devs.set_hidden_in_totals(dms, true);
    let ctx = egui::Context::default();
    let mut state = UiState {
        show_registry: true,
        registry_tab: RegistryTab::Devs,
        ..Default::default()
    };
    let base = egui::RawInput {
        screen_rect: Some(screen(1200.0, 800.0)),
        ..Default::default()
    };
    let frame = |state: &mut UiState, events: Vec<egui::Event>| {
        let mut actions = Vec::new();
        let out = ctx.run_ui_test(egui::RawInput { events, ..base.clone() }, |ui| {
            registry_window(ui.ctx(), &app, state, &mut actions)
        });
        (out, actions)
    };
    frame(&mut state, vec![]);
    let (out, _) = frame(&mut state, vec![]);
    let head = text_pos(&out, "Nei totali").expect("colonna nella scheda Dev");
    let dms_y = text_pos(&out, "DMS").unwrap().y;
    // La casella della riga DMS sotto «Nei totali»: un clic la spunta.
    let at = egui::pos2(head.x - 20.0, dms_y);
    let mut acts = Vec::new();
    for e in click_events(at, egui::PointerButton::Primary) {
        acts.extend(frame(&mut state, vec![e]).1);
    }
    assert!(
        matches!(acts.as_slice(), [Action::SetDevHiddenInTotals { dev, hidden: false }] if *dev == dms),
        "{} azioni",
        acts.len()
    );
    state.registry_tab = RegistryTab::Workers;
    frame(&mut state, vec![]);
    let (out, _) = frame(&mut state, vec![]);
    assert!(text_pos(&out, "Nei totali").is_none(), "solo nella scheda Dev");
}

/// Cronologia di Annulla/Ripeti: uno stato uguale non conta, annulla e
/// ripete in ordine, una modifica nuova cancella i «ripeti», al massimo
/// `UNDO_DEPTH` passi, `reset` riparte da zero.
#[test]
fn undo_history_records_undoes_and_redoes() {
    let mut h = UndoHistory::default();
    h.reset("A".into());
    assert!(!h.record("A".into()), "stato uguale: niente da annullare");
    assert!(h.record("B".into()) && h.record("C".into()));
    assert_eq!(h.undo().as_deref(), Some("B"));
    assert_eq!(h.undo().as_deref(), Some("A"));
    assert_eq!(h.undo(), None, "niente prima di A");
    assert_eq!(h.redo().as_deref(), Some("B"));
    assert!(h.can_redo());
    assert!(h.record("D".into()), "modifica nuova dopo un annulla");
    assert!(!h.can_redo(), "i «ripeti» si perdono");
    assert_eq!(h.undo().as_deref(), Some("B"));
    for i in 0..UNDO_DEPTH + 10 {
        h.record(format!("S{i}"));
    }
    let mut steps = 0;
    while h.undo().is_some() {
        steps += 1;
    }
    assert_eq!(steps, UNDO_DEPTH, "al massimo {UNDO_DEPTH} passi");
    h.reset("Z".into());
    assert!(!h.can_undo() && !h.can_redo());
}

/// Annulla/Ripeti sul programma: tornano i dati di prima (e poi quelli
/// dopo), il file risulta modificato, la visibilità dei progetti (Filtri ▸
/// Progetti, non è una modifica) resta quella attuale; con più modifiche nello
/// stesso frame si annulla tutto il frame.
#[test]
fn undo_and_redo_restore_the_data_but_keep_project_visibility() {
    let mut app = App::new();
    let mario = app.workers.add("Mario");
    let p = app.projects.add("x", Some("P1"), None);
    let mut pjm = PjmApp { app, ui: UiState::default() };
    pjm.reset_undo();
    let frame = |pjm: &mut PjmApp, acts: Vec<Action>| {
        for a in acts {
            pjm.apply(a);
        }
        pjm.record_undo();
    };
    frame(&mut pjm, vec![Action::RenameWorker { worker: mario, name: "Mario Rossi".into() }]);
    frame(
        &mut pjm,
        vec![
            Action::SetProjectStatus { proj: p, status: ProjectStatus::OnHold },
            Action::RenameWorker { worker: mario, name: "M. Rossi".into() },
        ],
    );
    // Filtri ▸ Progetti: non è una modifica, resta com'è.
    pjm.apply(Action::SetProjectEnabled { proj: p, enabled: false });
    pjm.ui.changed = false;

    frame(&mut pjm, vec![Action::Undo]);
    assert_eq!(pjm.app.workers.get_name_by_id(mario), "Mario Rossi");
    assert_eq!(pjm.app.projects.status(p), ProjectStatus::Active, "tutto il frame annullato");
    assert!(pjm.ui.changed, "da salvare");
    assert!(!pjm.app.projects.get_enable(&p).0, "visibilità attuale tenuta");
    frame(&mut pjm, vec![Action::Undo]);
    assert_eq!(pjm.app.workers.get_name_by_id(mario), "Mario");
    frame(&mut pjm, vec![Action::Undo]);
    assert_eq!(pjm.app.workers.get_name_by_id(mario), "Mario", "niente prima del caricamento");
    frame(&mut pjm, vec![Action::Redo, Action::Redo]);
    assert_eq!(pjm.app.workers.get_name_by_id(mario), "M. Rossi");
    assert_eq!(pjm.app.projects.status(p), ProjectStatus::OnHold);
    assert!(!pjm.ui.undo.can_redo());
}

/// Bagliore di Annulla/Ripeti: sale, resta, svanisce entro `UNDO_FLASH_SECS`.
#[test]
fn undo_flash_fades_in_and_out() {
    assert_eq!(UndoFlash::strength(0.0), 0.0);
    assert!(UndoFlash::strength(0.1) > 0.4 && UndoFlash::strength(0.1) < 0.6);
    assert_eq!(UndoFlash::strength(0.5), 1.0);
    assert!(UndoFlash::strength(1.5) < 1.0 && UndoFlash::strength(1.5) > 0.0);
    assert_eq!(UndoFlash::strength(UNDO_FLASH_SECS), 0.0);
}

/// Annulla una cella lontana (ultima settimana della griglia): la griglia ci
/// scorre, la colonna di quel dev brilla nel colore del bagliore e la
/// notifica dice dove; con il progetto nascosto dai filtri invece solo un
/// avviso, senza scroll né bagliore.
#[test]
fn undo_scrolls_to_the_change_and_makes_it_glow() {
    let mut app = App::new();
    let cols = columns_vec(&app, 0);
    let far = cols
        .iter()
        .rev()
        .find_map(|c| match c {
            Col::Weeks(ws) => Some(ws[0]),
            Col::YearEnd(_) => None,
        })
        .unwrap();
    let mario = app.workers.add("Mario");
    let fe = app.devs.add("Frontend");
    // Altri progetti prima, così la griglia deve scorrere anche in verticale.
    for i in 0..15 {
        let q = app.projects.add("y", Some(&format!("Q{i:02}")), None);
        app.projects.add_dev(q, fe);
        app.projects.add_effort(q, fe, WeekId(far as usize), mario, Effort(1));
    }
    let p = app.projects.add("z", Some("ABC"), None);
    app.projects.add_dev(p, fe);
    app.projects.add_effort(p, fe, WeekId(far as usize), mario, Effort(8));
    let mut pjm = PjmApp { app, ui: UiState::default() };
    pjm.reset_undo();
    pjm.app.projects.add_effort(p, fe, WeekId(far as usize), mario, Effort(4));
    pjm.mark_changed();
    pjm.record_undo();
    pjm.apply(Action::Undo);
    let cell = |pjm: &PjmApp| {
        pjm.app.projects.get_single_dev(p, fe).unwrap().get_all(WeekId(far as usize)).unwrap().worker_id[&mario].get_effort().0
    };
    assert_eq!(cell(&pjm), 8, "tornata a 8");

    let ctx = egui::Context::default();
    let input = egui::RawInput {
        screen_rect: Some(screen(1400.0, 800.0)),
        ..Default::default()
    };
    let mut out = None;
    // Il tempo deve scorrere: il bagliore parte da 0 e sale in 0,2 s.
    for i in 0..3 {
        let PjmApp { app, ui: st } = &mut pjm;
        let mut actions = Vec::new();
        let input = egui::RawInput { time: Some(i as f64 * 0.25), ..input.clone() };
        out = Some(ctx.run_ui_test(input, |ui| body(ui, app, st, &mut actions)));
    }
    let out = out.unwrap();
    assert!(pjm.ui.scroll_x > 0.0, "la griglia è scorsa verso la settimana lontana");
    assert!(pjm.ui.scroll_y > 0.0, "e in giù, verso il progetto in fondo");
    let glow: Vec<Rect> = out
        .shapes
        .iter()
        .filter_map(|cs| match &cs.shape {
            egui::epaint::Shape::Rect(r) => {
                let [rr, gg, bb, a] = r.stroke.color.to_srgba_unmultiplied();
                (a > 0 && [rr, gg, bb] == [0xFF, 0x2B, 0xD6]).then_some(r.rect)
            }
            _ => None,
        })
        .collect();
    assert_eq!(glow.len(), 1, "brilla solo la cella cambiata: {glow:?}");
    assert!((glow[0].height() - ROW_H).abs() < 0.5, "una cella: {:?}", glow[0]);
    let label = format!("ABC · Frontend · {}", week_label(far));
    assert!(
        pjm.ui.toasts.iter().any(|t| t.text == format!("Modifica annullata: {label}")),
        "{:?}",
        pjm.ui.toasts.iter().map(|t| &t.text).collect::<Vec<_>>()
    );

    // Ripeti con il progetto nascosto (Filtri ▸ Progetti): solo l'avviso.
    pjm.apply(Action::SetProjectEnabled { proj: p, enabled: false });
    let scroll_before = pjm.ui.scroll_x;
    pjm.ui.undo_flash = None;
    pjm.apply(Action::Redo);
    let PjmApp { app, ui: st } = &mut pjm;
    let mut actions = Vec::new();
    let _ = ctx.run_ui_test(input.clone(), |ui| body(ui, app, st, &mut actions));
    assert!(pjm.ui.undo_flash.is_none(), "niente da far brillare");
    assert!(pjm.ui.toasts.iter().any(|t| t.text.starts_with("Modifica ripetuta: ABC")
        && t.text.contains("non visibile")));
    assert_eq!(pjm.ui.scroll_x, scroll_before, "nessuno scroll");
}

/// Cmd/Ctrl+Z con una cella aperta: se non ci si è scritto nulla è
/// l'Annulla dei dati (la cella si chiude), se si è scritto annulla la
/// digitazione; senza cella, non mentre si scrive in un altro campo.
#[test]
fn undo_shortcut_works_on_an_untouched_open_cell() {
    let mut e = Editing::open(ProjectId(0), DevId(0), 20000, 0, "Alice|4", "");
    assert!(undo_shortcut_applies(Some(&e), true), "cella appena aperta (ha il focus)");
    e.buf = "Alice|45".to_string();
    assert!(!undo_shortcut_applies(Some(&e), true), "ci si è scritto: annulla la digitazione");
    e.buf = "Alice|4".to_string();
    assert!(undo_shortcut_applies(Some(&e), true), "tornata com'era");
    let empty = Editing::open(ProjectId(0), DevId(0), 20000, 1, "", "");
    assert!(undo_shortcut_applies(Some(&empty), true), "cella vuota appena aperta");
    assert!(undo_shortcut_applies(None, false));
    assert!(!undo_shortcut_applies(None, true), "si scrive in un altro campo");
}

/// La priorità del progetto sta sulla riga della categoria, a destra: «prio —»
/// finché non c'è; il clic apre la scelta, e un valore manda
/// `SetProjectPriority`.
#[test]
fn priority_row_opens_the_choice_and_sets_the_priority() {
    let mut app = App::new();
    let dev = app.devs.add("Frontend");
    let proj = app.projects.add("Progetto", Some("ABC"), None);
    app.projects.add_dev(proj, dev);
    let must = app.priorities.list()[0].0;
    let ctx = egui::Context::default();
    let mut state = UiState::default();
    let mut actions = Vec::new();
    let frame = |app: &App, state: &mut UiState, actions: &mut Vec<Action>, ev: Option<egui::Event>| {
        let mut input = egui::RawInput {
            screen_rect: Some(screen(1200.0, 800.0)),
            ..Default::default()
        };
        input.events.extend(ev);
        ctx.run_ui_test(input, |ui| {
            let layout = body_layout(ui, app, state, &Filter::default(), &None);
            left_column(ui, app, state, actions, &Filter::default(), &None, &layout);
            popup_window(ui.ctx(), app, state, actions);
        })
    };
    let out = frame(&app, &mut state, &mut actions, None);
    let row = text_pos(&out, "prio —").expect("priorità sulla riga della categoria");
    for ev in click_events(row, egui::PointerButton::Primary) {
        let _ = frame(&app, &mut state, &mut actions, Some(ev));
    }
    assert!(matches!(state.popup, Some(Popup::Priority { proj: p }) if p == proj));
    let out = frame(&app, &mut state, &mut actions, None);
    let item = text_pos(&out, "Must").expect("voce «Must»");
    actions.clear();
    for ev in click_events(item, egui::PointerButton::Primary) {
        let _ = frame(&app, &mut state, &mut actions, Some(ev));
    }
    assert!(
        actions
            .iter()
            .any(|a| matches!(a, Action::SetProjectPriority { proj: p, pri } if *p == proj && *pri == Some(must))),
        "SetProjectPriority con Must"
    );

    // Con la priorità impostata la riga la mostra.
    app.projects.set_priority(proj, Some(must));
    state.popup = None;
    let out = frame(&app, &mut state, &mut actions, None);
    let pri = text_pos(&out, "Must").expect("priorità nell'intestazione");
    let cat = text_pos(&out, "—").expect("categoria assente");
    assert!((pri.y - cat.y).abs() < 1.0 && pri.x > cat.x, "stessa riga, a destra");
}

/// In fondo all'area di ogni progetto, nella colonna sinistra, su una riga
/// «previste / spese» (pianificato dei dev / ore fino alla settimana corrente), anche quando
/// i dev rendono il blocco più alto dell'intestazione.
#[test]
fn project_area_ends_with_planned_and_spent_hours() {
    let mut app = App::new();
    let w = app.workers.add("Mario");
    let proj = app.projects.add("Progetto", Some("ABC"), None);
    let now = current_week_id();
    for i in 0..12 {
        let dev = app.devs.add(&format!("Dev{i}"));
        app.projects.add_dev(proj, dev);
        app.projects.add_dev_effort(proj, dev, Effort(10));
        if i < 3 {
            app.projects.add_effort(proj, dev, now, w, Effort(4));
        }
    }
    let ctx = egui::Context::default();
    let mut state = UiState::default();
    let input = egui::RawInput {
        screen_rect: Some(screen(1200.0, 2000.0)),
        ..Default::default()
    };
    let mut proj_bottom = 0.0;
    let out = ctx.run_ui_test(input, |ui| {
        let top = ui.cursor().min.y;
        let layout = body_layout(ui, &app, &state, &Filter::default(), &None);
        proj_bottom = top + DEV_BORDER + layout[0].proj_h;
        let mut actions = Vec::new();
        left_column(ui, &app, &mut state, &mut actions, &Filter::default(), &None, &layout);
    });
    let row = text_pos(&out, "120h / 12h").expect("previste / spese su una riga");
    assert!((proj_bottom - row.y - ROW_H / 2.0).abs() < 1.0, "in fondo all'area del progetto");
}

/// Dialog «Esporta CSV»: con il periodo attivo e le 0 ore, «Esporta CSV…»
/// manda i progetti spuntati e le opzioni, e le opzioni restano per la
/// prossima apertura.
#[test]
fn csv_export_window_sends_projects_and_options() {
    let mut app = App::new();
    let p = app.projects.add("Uno", Some("AAA"), None);
    let q = app.projects.add("Due", Some("BBB"), None);
    let choices = CsvChoices {
        limit: true,
        zero: true,
        ..CsvChoices::initial()
    };
    let mut state = UiState {
        csv_export: Some(CsvExportState {
            entries: vec![(p, true), (q, false)],
            choices,
        }),
        ..Default::default()
    };
    let ctx = egui::Context::default();
    let mut actions = Vec::new();
    let frame = |state: &mut UiState, actions: &mut Vec<Action>, ev: Option<egui::Event>| {
        let mut input = egui::RawInput {
            screen_rect: Some(screen(1000.0, 800.0)),
            ..Default::default()
        };
        input.events.extend(ev);
        ctx.run_ui_test(input, |ui| csv_export_window(ui.ctx(), &app, state, actions))
    };
    let mut out = frame(&mut state, &mut actions, None);
    for _ in 0..3 {
        out = frame(&mut state, &mut actions, None);
    }
    let btn = text_pos(&out, "Esporta CSV…").expect("bottone");
    for ev in click_events(btn, egui::PointerButton::Primary) {
        let _ = frame(&mut state, &mut actions, Some(ev));
    }
    let sent = actions.iter().find_map(|a| match a {
        Action::ExportEffortCsv { projects, opts } => Some((projects.clone(), *opts)),
        _ => None,
    });
    let (projects, opts) = sent.expect("ExportEffortCsv");
    assert_eq!(projects, [p]);
    assert_eq!(opts.range, Some(choices.range));
    assert!(opts.zero);
    assert!(state.csv_export.is_none(), "dialog chiusa");
    assert_eq!(state.csv_choices, Some(choices), "opzioni ricordate");
}

/// Regole di automazione nel programma: la regola agisce appena attivata,
/// segue le modifiche (anche alle regole), si annulla con Annulla insieme al
/// suo effetto; eliminandola la milestone resta dov'è e torna libera.
#[test]
fn rules_apply_on_every_change_and_undo_with_it() {
    use crate::rules::{Anchor, Rule, RuleScope};
    let mut app = App::new();
    let w = app.workers.add("Mario");
    let be = app.devs.add("Backend");
    let p = app.projects.add("x", Some("ABC"), None);
    app.projects.add_dev(p, be);
    let base = 20004;
    app.projects.add_effort(p, be, WeekId(base), w, Effort(8));
    let collaudo = app.milestones.add("Collaudo");
    let mut pjm = PjmApp { app, ui: UiState::default() };
    pjm.reset_undo();
    let at = |pjm: &PjmApp| {
        pjm.app.projects.list_project_milestones(p).into_iter().find(|(m, _)| *m == collaudo).map(|(_, w)| w)
    };
    let mut rule = Rule {
        name: "Collaudo".into(),
        enabled: false,
        scope: RuleScope::All,
        target: crate::rules::Target::Milestone(collaudo),
        anchor: Anchor::DevEnd(be),
        offset_weeks: 2,
        inactive: Default::default(),
        check: Default::default(),
    };
    pjm.apply(Action::AddRule(rule.clone()));
    pjm.record_undo();
    assert_eq!(at(&pjm), None, "regola nuova spenta: niente");
    let id = pjm.app.rules.list()[0].0;
    rule.enabled = true;
    pjm.apply(Action::UpdateRule(id, rule.clone()));
    pjm.record_undo();
    assert_eq!(at(&pjm), Some(WeekId(base + 14)));
    // Una modifica qualunque (un effort in più) la fa seguire.
    pjm.apply(Action::BulkFillEffort { proj: p, dev: be, start: WeekId(base + 7), workers: vec![w], effort: Effort(4), weeks: 1, until: None });
    pjm.record_undo();
    assert_eq!(at(&pjm), Some(WeekId(base + 21)));
    // Annulla: torna l'effort di prima e con lui la milestone.
    pjm.apply(Action::Undo);
    assert_eq!(at(&pjm), Some(WeekId(base + 14)));
    pjm.apply(Action::Redo);
    assert_eq!(at(&pjm), Some(WeekId(base + 21)));
    // Eliminata la regola, la milestone resta dov'è.
    pjm.apply(Action::DeleteRule(id));
    assert!(pjm.app.rules.is_empty());
    assert_eq!(at(&pjm), Some(WeekId(base + 21)));
}

/// All'apertura (o all'arrivo del file di un collega) il file si allinea
/// alle regole e, se cambia qualcosa, risulta da salvare.
#[test]
fn opening_a_file_aligns_it_to_the_rules() {
    use crate::rules::{Anchor, Rule, RuleScope};
    let mut app = App::new();
    let p = app.projects.add("x", Some("ABC"), None);
    app.projects.set_project_end_week(p, Some(WeekId(20004 + 70)));
    let rilascio = app.milestones.add("Rilascio");
    app.rules.add(Rule {
        name: String::new(),
        enabled: true,
        scope: RuleScope::Project(p),
        target: crate::rules::Target::Milestone(rilascio),
        anchor: Anchor::ProjectEnd,
        offset_weeks: -1,
        inactive: Default::default(),
        check: Default::default(),
    });
    let ron = app.to_ron_string();
    let back = App::from_ron_str(&ron).expect("regole nel .ron");
    assert_eq!(back.rules, app.rules);
    let mut pjm = PjmApp { app: back, ui: UiState::default() };
    pjm.reset_undo();
    assert!(pjm.ui.changed, "allineato: da salvare");
    assert_eq!(pjm.app.projects.list_project_milestones(p), [(rilascio, WeekId(20004 + 63))]);
    assert!(pjm.app.validate().is_empty());
}

/// Finestra «Regole di automazione»: «+ Nuova regola» crea una regola
/// spenta; ogni regola mostra il suo stato; una milestone eliminata è
/// segnalata (anche da «Problemi nel file»).
#[test]
fn rules_window_adds_rules_and_shows_their_status() {
    use crate::rules::{Anchor, Rule, RuleScope};
    let mut app = App::new();
    let be = app.devs.add("Backend");
    let collaudo = app.milestones.add("Collaudo");
    let mut state = UiState { show_rules: true, ..Default::default() };
    let ctx = egui::Context::default();
    let input = egui::RawInput { screen_rect: Some(screen(1400.0, 800.0)), ..Default::default() };
    let mut actions = Vec::new();
    let mut out = None;
    for _ in 0..2 {
        out = Some(ctx.run_ui_test(input.clone(), |ui| rules_window(ui.ctx(), &app, &mut state, &mut actions)));
    }
    let pos = text_pos(&out.unwrap(), "+ Nuova regola").expect("pulsante");
    for ev in click_events(pos, egui::PointerButton::Primary) {
        let mut i = input.clone();
        i.events = vec![ev];
        let _ = ctx.run_ui_test(i, |ui| rules_window(ui.ctx(), &app, &mut state, &mut actions));
    }
    match actions.as_slice() {
        [Action::AddRule(r)] => {
            assert!(!r.enabled, "nasce spenta");
            assert_eq!((r.target, r.anchor, r.scope.clone()), (crate::rules::Target::Milestone(collaudo), Anchor::DevEnd(be), RuleScope::All));
        }
        _ => panic!("attesa una AddRule"),
    }
    app.rules.add(Rule {
        name: "Collaudo dopo il Backend".into(),
        enabled: true,
        scope: RuleScope::All,
        target: crate::rules::Target::Milestone(collaudo),
        anchor: Anchor::DevEnd(be),
        offset_weeks: 2,
        inactive: Default::default(),
        check: Default::default(),
    });
    let texts = drawn_texts(&ctx.run_ui_test(input.clone(), |ui| rules_window(ui.ctx(), &app, &mut state, &mut Vec::new())));
    for t in ["Fine dev Backend", "Milestone Collaudo", "Tutti i progetti", "in 0 progetti"] {
        assert!(texts.iter().any(|s| s == t), "manca «{t}»: {texts:?}");
    }
    app.milestones.del(collaudo);
    let texts = drawn_texts(&ctx.run_ui_test(input, |ui| rules_window(ui.ctx(), &app, &mut state, &mut Vec::new())));
    assert!(texts.iter().any(|s| s == "⚠ milestone inesistente"), "{texts:?}");
    assert!(app.validate().iter().any(|i| i.contains("Collaudo dopo il Backend")));
}

/// Regola su una milestone di un solo progetto: l'ambito diventa da solo
/// quel progetto (anche per una regola già esistente con un altro ambito) e
/// la combo dell'ambito è bloccata.
#[test]
fn project_milestone_rules_get_the_project_as_scope() {
    use crate::rules::{Anchor, Rule, RuleScope};
    let mut app = App::new();
    let p = app.projects.add("x", Some("ABC"), None);
    let own = app.milestones.add_owned("Demo interna", Default::default(), Vec::new(), Some(p));
    let id = app.rules.add(Rule {
        name: String::new(),
        enabled: true,
        scope: RuleScope::All,
        target: crate::rules::Target::Milestone(own),
        anchor: Anchor::ProjectEnd,
        offset_weeks: 0,
        inactive: Default::default(),
        check: Default::default(),
    });
    let mut state = UiState { show_rules: true, ..Default::default() };
    let ctx = egui::Context::default();
    let input = egui::RawInput { screen_rect: Some(screen(1400.0, 800.0)), ..Default::default() };
    let mut actions = Vec::new();
    // Due frame: al primo la griglia misura soltanto.
    let _ = ctx.run_ui_test(input.clone(), |ui| rules_window(ui.ctx(), &app, &mut state, &mut actions));
    let out = ctx.run_ui_test(input, |ui| rules_window(ui.ctx(), &app, &mut state, &mut Vec::new()));
    match actions.as_slice() {
        [Action::UpdateRule(rid, r)] => {
            assert_eq!(*rid, id);
            assert_eq!(r.scope, RuleScope::Projects(vec![p]));
        }
        _ => panic!("attesa una UpdateRule con l'ambito del progetto"),
    }
    assert!(drawn_texts(&out).iter().any(|t| t == "Milestone Demo interna · ABC"));
    // Una regola nuova sulla milestone del progetto nasce già con quell'ambito.
    let mut only = App::new();
    let q = only.projects.add("y", Some("XYZ"), None);
    only.milestones.add_owned("Solo XYZ", Default::default(), Vec::new(), Some(q));
    assert_eq!(new_rule(&only).scope, RuleScope::Projects(vec![q]));
}

/// La finestra delle regole non esplode in orizzontale: con un avviso a più
/// punti e ancore (la riga più lunga di prima) tutto quello che disegna sta
/// in uno schermo da 1536 px (1920 al 125%), dentro la finestra.
#[test]
fn rules_window_fits_a_1536_px_screen() {
    use crate::rules::{Anchor, CheckOpts, Cmp, Rule, RuleScope, Target};
    let mut app = App::new();
    let devs: Vec<_> = ["Backend lungo nome", "Frontend lungo nome", "Test lungo nome"].iter().map(|n| app.devs.add(n)).collect();
    let m = app.milestones.add("Milestone dal nome molto lungo");
    let id = app.rules.add(Rule {
        name: "Una regola d'avviso con un nome lungo".into(),
        enabled: true,
        scope: RuleScope::All,
        target: Target::Check(Anchor::DevStart(devs[2]), Cmp::NotBefore),
        anchor: Anchor::DevEnd(devs[0]),
        offset_weeks: 1,
        inactive: Default::default(),
        check: CheckOpts { points: vec![Anchor::DevEnd(devs[2])], anchors: vec![Anchor::DevEnd(devs[1]), Anchor::Milestone(m)], block: true },
    });
    let mut state = UiState { show_rules: true, rule_selected: Some(id), ..Default::default() };
    let ctx = egui::Context::default();
    let input = egui::RawInput { screen_rect: Some(screen(1536.0, 864.0)), ..Default::default() };
    let mut out = None;
    for _ in 0..3 {
        out = Some(ctx.run_ui_test(input.clone(), |ui| rules_window(ui.ctx(), &app, &mut state, &mut Vec::new())));
    }
    let out = out.unwrap();
    let right = out
        .shapes
        .iter()
        .filter_map(|cs| match &cs.shape {
            egui::epaint::Shape::Text(t) => Some(t.pos.x + t.galley.size().x),
            _ => None,
        })
        .fold(0.0_f32, f32::max);
    assert!(right <= 60.0 + RULES_WINDOW_W, "testo fino a x = {right}");
    let texts = drawn_texts(&out);
    for t in ["Controlla", "Ancora", "Blocca", "Settimane", "Stato", "Clona"] {
        assert!(texts.iter().any(|x| x == t), "manca «{t}» nel dettaglio: {texts:?}");
    }
}

/// «Applica ora…» di una regola «Ore del dev»: l'anteprima elenca lo
/// spostamento e «Sposta le ore» manda `ApplyRuleNow`, che sposta le ore e si
/// annulla con Annulla.
#[test]
fn rule_preview_lists_the_moves_and_applies_them() {
    use crate::rules::{Anchor, Rule, RuleScope, Target};
    let mut app = App::new();
    let w = app.workers.add("Mario");
    let be = app.devs.add("Backend");
    let test = app.devs.add("Test");
    let p = app.projects.add("x", Some("ABC"), None);
    app.projects.add_dev(p, be);
    app.projects.add_dev(p, test);
    let base = 20004;
    app.projects.add_effort(p, be, WeekId(base + 7), w, Effort(8));
    app.projects.add_effort(p, test, WeekId(base), w, Effort(4));
    let id = app.rules.add(Rule {
        name: "Test dopo il Backend".into(),
        enabled: true,
        scope: RuleScope::All,
        target: Target::DevStart(test),
        anchor: Anchor::DevEnd(be),
        offset_weeks: 1,
        inactive: Default::default(),
        check: Default::default(),
    });
    let mut state = UiState { rule_preview: Some(id), ..Default::default() };
    let ctx = egui::Context::default();
    let input = egui::RawInput { screen_rect: Some(screen(1200.0, 800.0)), ..Default::default() };
    let mut actions = Vec::new();
    let mut out = None;
    for _ in 0..2 {
        out = Some(ctx.run_ui_test(input.clone(), |ui| rule_preview_window(ui.ctx(), &app, &mut state, &mut actions)));
    }
    let out = out.unwrap();
    let texts = drawn_texts(&out);
    assert!(texts.iter().any(|t| t == "+2 sett."), "{texts:?}");
    assert!(texts.iter().any(|t| t == "ABC"), "{texts:?}");
    let pos = text_pos(&out, "Sposta le ore").expect("pulsante");
    for ev in click_events(pos, egui::PointerButton::Primary) {
        let mut i = input.clone();
        i.events = vec![ev];
        let _ = ctx.run_ui_test(i, |ui| rule_preview_window(ui.ctx(), &app, &mut state, &mut actions));
    }
    assert!(matches!(actions.as_slice(), [Action::ApplyRuleNow(r)] if *r == id));
    assert!(state.rule_preview.is_none(), "anteprima chiusa");
    let mut pjm = PjmApp { app, ui: UiState::default() };
    pjm.reset_undo();
    let first = |pjm: &PjmApp| pjm.app.projects.get_single_dev(p, test).unwrap().effort_span().map(|s| s.0);
    pjm.apply(Action::ApplyRuleNow(id));
    pjm.record_undo();
    assert_eq!(first(&pjm), Some(WeekId(base + 14)));
    pjm.apply(Action::Undo);
    assert_eq!(first(&pjm), Some(WeekId(base)));
}

/// Un dev richiesto da una regola «Aggiungi dev» non si toglie dalla
/// finestra dei dev del progetto (il pulsante è spento).
#[test]
fn a_dev_required_by_a_rule_cannot_be_removed() {
    use crate::rules::{Anchor, Rule, RuleScope, Target};
    let mut app = App::new();
    let be = app.devs.add("Backend");
    let test = app.devs.add("Test");
    let p = app.projects.add("x", Some("ABC"), None);
    app.projects.add_dev(p, be);
    app.rules.add(Rule {
        name: String::new(),
        enabled: true,
        scope: RuleScope::All,
        target: Target::AddDev(test),
        anchor: Anchor::DevPresent(be),
        offset_weeks: 0,
        inactive: Default::default(),
        check: Default::default(),
    });
    crate::rules::apply(&mut app);
    assert!(app.projects.list_devs(p).contains(&test));
    let mut state = UiState { dev_manage: Some(p), ..Default::default() };
    let ctx = egui::Context::default();
    let input = egui::RawInput { screen_rect: Some(screen(900.0, 700.0)), ..Default::default() };
    let mut actions = Vec::new();
    let mut out = None;
    for _ in 0..2 {
        out = Some(ctx.run_ui_test(input.clone(), |ui| dev_manage_window(ui.ctx(), &app, &mut state, &mut actions)));
    }
    let pos = text_pos(&out.unwrap(), "Test").expect("dev Test");
    for ev in click_events(pos, egui::PointerButton::Primary) {
        let mut i = input.clone();
        i.events = vec![ev];
        let _ = ctx.run_ui_test(i, |ui| dev_manage_window(ui.ctx(), &app, &mut state, &mut actions));
    }
    assert!(actions.is_empty(), "nessuna rimozione: {}", actions.len());
}

/// Avvisi delle regole nel programma: notifica solo quando nasce un avviso
/// nuovo (non per quelli già nel file aperto), «⚠» accanto alla tripletta,
/// elenco nella finestra «Avvisi delle regole».
#[test]
fn rule_warnings_notify_new_ones_and_show_in_grid_and_list() {
    use crate::rules::{Anchor, Cmp, Rule, RuleScope, Target};
    let mut app = App::new();
    let w = app.workers.add("Mario");
    let be = app.devs.add("Backend");
    let p = app.projects.add("x", Some("ABC"), None);
    app.projects.add_dev(p, be);
    let base = 20004;
    app.projects.add_effort(p, be, WeekId(base + 14), w, Effort(8));
    let rilascio = app.milestones.add("Rilascio");
    app.rules.add(Rule {
        name: "Rilascio dopo il Backend".into(),
        enabled: true,
        scope: RuleScope::All,
        target: Target::Check(Anchor::Milestone(rilascio), Cmp::NotBefore),
        anchor: Anchor::DevEnd(be),
        offset_weeks: 0,
        inactive: Default::default(),
        check: Default::default(),
    });
    // Avviso già presente all'apertura: niente notifica.
    app.projects.add_project_milestone(p, rilascio, WeekId(base));
    let mut pjm = PjmApp { app, ui: UiState::default() };
    pjm.reset_undo();
    assert!(pjm.ui.toasts.is_empty(), "avviso già noto");
    // Rispettata, poi di nuovo violata: notifica.
    pjm.apply(Action::AddProjectMilestone { proj: p, milestone: rilascio, week: WeekId(base + 21) });
    assert!(pjm.ui.toasts.is_empty());
    pjm.apply(Action::AddProjectMilestone { proj: p, milestone: rilascio, week: WeekId(base + 7) });
    assert_eq!(pjm.ui.toasts.len(), 1);
    assert!(pjm.ui.toasts[0].text.starts_with("Avviso «Rilascio dopo il Backend» in ABC:"), "{}", pjm.ui.toasts[0].text);
    // Un'altra modifica che lascia l'avviso: nessuna nuova notifica.
    pjm.apply(Action::AddProjectMilestone { proj: p, milestone: rilascio, week: WeekId(base) });
    assert_eq!(pjm.ui.toasts.len(), 1);

    // «⚠» nella colonna di sinistra.
    let ctx = egui::Context::default();
    let input = egui::RawInput { screen_rect: Some(screen(1200.0, 900.0)), ..Default::default() };
    let PjmApp { app, ui: state } = &mut pjm;
    let out = ctx.run_ui_test(input.clone(), |ui| {
        let layout = body_layout(ui, app, state, &Filter::default(), &None);
        left_column(ui, app, state, &mut Vec::new(), &Filter::default(), &None, &layout);
    });
    assert!(drawn_texts(&out).iter().any(|t| t == "⚠"), "segno sul progetto");
    // Elenco.
    state.show_rule_warnings = true;
    let mut out = None;
    for _ in 0..2 {
        out = Some(ctx.run_ui_test(input.clone(), |ui| rule_warnings_window(ui.ctx(), app, state)));
    }
    let texts = drawn_texts(&out.unwrap());
    assert!(texts.iter().any(|t| t == "Rilascio dopo il Backend"), "{texts:?}");
    assert!(texts.iter().any(|t| t.starts_with("Milestone Rilascio (")), "{texts:?}");
}

/// Riga di una regola d'avviso: «Avviso» con accanto punto e confronto, lo
/// stato con il numero di avvisi e il pulsante «⚠ Avvisi (1)…».
#[test]
fn warning_rule_row_shows_point_comparison_and_count() {
    use crate::rules::{Anchor, Cmp, Rule, RuleScope, Target};
    let mut app = App::new();
    let w = app.workers.add("Mario");
    let be = app.devs.add("Backend");
    let p = app.projects.add("x", Some("ABC"), None);
    app.projects.add_dev(p, be);
    app.projects.add_effort(p, be, WeekId(20004 + 14), w, Effort(8));
    let rilascio = app.milestones.add("Rilascio");
    app.projects.add_project_milestone(p, rilascio, WeekId(20004));
    app.rules.add(Rule {
        name: "Controllo".into(),
        enabled: true,
        scope: RuleScope::All,
        target: Target::Check(Anchor::Milestone(rilascio), Cmp::NotBefore),
        anchor: Anchor::DevEnd(be),
        offset_weeks: 0,
        inactive: Default::default(),
        check: Default::default(),
    });
    let mut state = UiState { show_rules: true, ..Default::default() };
    let ctx = egui::Context::default();
    let input = egui::RawInput { screen_rect: Some(screen(1700.0, 800.0)), ..Default::default() };
    let mut actions = Vec::new();
    let mut out = None;
    for _ in 0..2 {
        out = Some(ctx.run_ui_test(input.clone(), |ui| rules_window(ui.ctx(), &app, &mut state, &mut actions)));
    }
    let texts = drawn_texts(&out.unwrap());
    for t in ["Avviso", "Milestone Rilascio", "non prima di", "Fine dev Backend", "⚠ 1 avvisi", "⚠ Avvisi (1)…"] {
        assert!(texts.iter().any(|s| s == t), "manca «{t}»: {texts:?}");
    }
    assert!(actions.is_empty(), "disegnare non modifica la regola");
}

/// «Importa da testo…»: le regole buone si aggiungono (e si annullano con
/// Annulla), le scartate finiscono nel resoconto; i pulsanti di testo e log
/// mandano le loro azioni.
#[test]
fn rules_text_import_reports_and_buttons_send_actions() {
    let mut app = App::new();
    app.devs.add("Backend");
    app.milestones.add("Collaudo");
    let mut pjm = PjmApp { app, ui: UiState::default() };
    pjm.reset_undo();
    let text = "[regola]\nnome = Buona\ncosa = milestone \"Collaudo\"\nancora = fine dev \"Backend\"\n\n[regola]\ncosa = milestone \"Altro\"\nancora = fine progetto\n";
    pjm.import_rules_text(text);
    pjm.record_undo();
    assert_eq!(pjm.app.rules.list().len(), 1);
    assert!(pjm.ui.toasts[0].text.starts_with("Regole importate: 1 nuove, 0 già presenti, 1 scartate."));
    let errors = pjm.ui.rules_import_errors.clone().expect("resoconto");
    assert!(errors[0].contains("milestone «Altro» inesistente"), "{errors:?}");
    pjm.apply(Action::Undo);
    assert!(pjm.app.rules.is_empty(), "Annulla toglie l'importazione");
    pjm.apply(Action::Redo);

    let ctx = egui::Context::default();
    let input = egui::RawInput { screen_rect: Some(screen(1700.0, 800.0)), ..Default::default() };
    let PjmApp { app, ui: state } = &mut pjm;
    state.show_rules = true;
    for label in ["Esporta in testo…", "Importa da testo…", "Log…"] {
        let mut actions = Vec::new();
        let mut out = None;
        for _ in 0..2 {
            out = Some(ctx.run_ui_test(input.clone(), |ui| rules_window(ui.ctx(), app, state, &mut actions)));
        }
        let pos = text_pos(&out.unwrap(), label).unwrap_or_else(|| panic!("manca «{label}»"));
        for ev in click_events(pos, egui::PointerButton::Primary) {
            let mut i = input.clone();
            i.events = vec![ev];
            let _ = ctx.run_ui_test(i, |ui| rules_window(ui.ctx(), app, state, &mut actions));
        }
        let ok = match label {
            "Esporta in testo…" => matches!(actions.as_slice(), [Action::ExportRulesText]),
            "Importa da testo…" => matches!(actions.as_slice(), [Action::ImportRulesText]),
            _ => matches!(actions.as_slice(), [Action::ExportRulesLog]),
        };
        assert!(ok, "«{label}»: {} azioni", actions.len());
    }
}

/// Finestra delle regole: nella combo «Progetti» si spuntano più progetti
/// (l'ambito diventa quei progetti); «Clona» aggiunge una copia spenta.
#[test]
fn rules_window_picks_several_projects_and_clones_a_rule() {
    use crate::rules::{Anchor, Rule, RuleScope, Target};
    let mut app = App::new();
    let p = app.projects.add("x", Some("ABC"), None);
    let q = app.projects.add("y", Some("XYZ"), None);
    let id = app.rules.add(Rule {
        name: "Fine".into(),
        enabled: true,
        scope: RuleScope::Projects(vec![p]),
        target: Target::ProjectEnd,
        anchor: Anchor::LastEffort,
        offset_weeks: 1,
        inactive: Default::default(),
        check: Default::default(),
    });
    let mut state = UiState { show_rules: true, ..Default::default() };
    let ctx = egui::Context::default();
    let input = egui::RawInput { screen_rect: Some(screen(1700.0, 900.0)), ..Default::default() };
    let mut actions = Vec::new();
    let mut frame = |state: &mut UiState, actions: &mut Vec<Action>, events: Vec<egui::Event>| {
        let mut i = input.clone();
        i.events = events;
        ctx.run_ui_test(i, |ui| rules_window(ui.ctx(), &app, state, actions))
    };
    type Frame<'a> = dyn FnMut(&mut UiState, &mut Vec<Action>, Vec<egui::Event>) -> egui::FullOutput + 'a;
    let click = |state: &mut UiState, actions: &mut Vec<Action>, frame: &mut Frame, label: &str| {
        frame(state, actions, vec![]);
        let out = frame(state, actions, vec![]);
        let pos = text_pos(&out, label).unwrap_or_else(|| panic!("manca «{label}»: {:?}", drawn_texts(&out)));
        for ev in click_events(pos, egui::PointerButton::Primary) {
            frame(state, actions, vec![ev]);
        }
    };
    click(&mut state, &mut actions, &mut frame, "Progetto: ABC");
    actions.clear();
    click(&mut state, &mut actions, &mut frame, "XYZ");
    let scopes: Vec<RuleScope> = actions
        .iter()
        .filter_map(|a| match a {
            Action::UpdateRule(r, rule) if *r == id => Some(rule.scope.clone()),
            _ => None,
        })
        .collect();
    assert!(scopes.contains(&RuleScope::Projects(vec![p, q])), "{scopes:?}");
    // Chiusa la combo, «Clona».
    frame(&mut state, &mut actions, vec![egui::Event::Key {
        key: egui::Key::Escape,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    }]);
    actions.clear();
    click(&mut state, &mut actions, &mut frame, "Clona");
    match actions.as_slice() {
        [Action::AddRule(r)] => {
            assert_eq!(r.name, "Fine (copia)");
            assert!(!r.enabled, "la copia nasce spenta");
            assert_eq!((r.target, r.anchor, r.offset_weeks), (Target::ProjectEnd, Anchor::LastEffort, 1));
            assert_eq!(r.scope, RuleScope::Projects(vec![p]));
        }
        _ => panic!("attesa una AddRule, ricevute {}", actions.len()),
    }
}

/// Tendina «Progetti»: «Tutti i progetti tranne…» e poi la spunta di un
/// progetto lo **esclude**.
#[test]
fn rules_window_all_except_excludes_the_ticked_project() {
    use crate::rules::{Anchor, Rule, RuleScope, Target};
    let mut app = App::new();
    app.projects.add("x", Some("ABC"), None);
    let q = app.projects.add("y", Some("XYZ"), None);
    let id = app.rules.add(Rule {
        name: "Fine".into(),
        enabled: true,
        scope: RuleScope::All,
        target: Target::ProjectEnd,
        anchor: Anchor::LastEffort,
        offset_weeks: 0,
        inactive: Default::default(),
        check: Default::default(),
    });
    let ctx = egui::Context::default();
    let input = egui::RawInput { screen_rect: Some(screen(1700.0, 900.0)), ..Default::default() };
    let mut state = UiState { show_rules: true, ..Default::default() };
    let click = |app: &App, state: &mut UiState, label: &str| -> Vec<Action> {
        let mut actions = Vec::new();
        let frame = |state: &mut UiState, actions: &mut Vec<Action>, events: Vec<egui::Event>| {
            let mut i = input.clone();
            i.events = events;
            ctx.run_ui_test(i, |ui| rules_window(ui.ctx(), app, state, actions))
        };
        frame(state, &mut actions, vec![]);
        let out = frame(state, &mut actions, vec![]);
        let pos = text_pos(&out, label).unwrap_or_else(|| panic!("manca «{label}»: {:?}", drawn_texts(&out)));
        for ev in click_events(pos, egui::PointerButton::Primary) {
            frame(state, &mut actions, vec![ev]);
        }
        actions
    };
    let scope_of = |actions: &[Action]| {
        actions.iter().rev().find_map(|a| match a {
            Action::UpdateRule(r, rule) if *r == id => Some(rule.scope.clone()),
            _ => None,
        })
    };
    click(&app, &mut state, "Tutti i progetti");
    let a = click(&app, &mut state, "Tutti i progetti tranne…");
    assert_eq!(scope_of(&a), Some(RuleScope::AllExcept(vec![])));
    // La regola cambia davvero, poi si spunta XYZ fra gli esclusi.
    let mut rule = app.rules.get(id).unwrap().clone();
    rule.scope = RuleScope::AllExcept(vec![]);
    app.rules.update(id, rule);
    click(&app, &mut state, "Tutti tranne: —");
    let a = click(&app, &mut state, "XYZ");
    assert_eq!(scope_of(&a), Some(RuleScope::AllExcept(vec![q])));
}

/// Spuntare «Attiva» su una regola che cambierebbe dei progetti non la
/// salva subito: apre l'anteprima con le modifiche (e lo stato dei progetti
/// non attivi); «Applica» manda l'`UpdateRule`, «Annulla» no. La combo degli
/// stati parte da «Solo attivi».
#[test]
fn activating_a_rule_shows_the_changes_before_applying() {
    use crate::rules::{Anchor, Inactive, Rule, RuleScope, Target};
    let mut app = App::new();
    let w = app.workers.add("Mario");
    let be = app.devs.add("Backend");
    let p = app.projects.add("x", Some("ABC"), None);
    app.projects.add_dev(p, be);
    let base = 20004;
    app.projects.add_effort(p, be, WeekId(base), w, Effort(8));
    let collaudo = app.milestones.add("Collaudo");
    let rule = Rule {
        name: "Collaudo".into(),
        enabled: false,
        scope: RuleScope::All,
        target: Target::Milestone(collaudo),
        anchor: Anchor::DevEnd(be),
        offset_weeks: 2,
        inactive: Inactive { on_hold: false, closed: true },
        check: Default::default(),
    };
    let id = app.rules.add(rule.clone());
    let q = app.projects.add("y", Some("XYZ"), None);
    app.projects.add_dev(q, be);
    app.projects.add_effort(q, be, WeekId(base), w, Effort(8));
    app.projects.set_status(q, crate::project_utils::project::ProjectStatus::Closed);
    let mut state = UiState { show_rules: true, ..Default::default() };
    let ctx = egui::Context::default();
    let input = egui::RawInput { screen_rect: Some(screen(1900.0, 900.0)), ..Default::default() };
    let mut actions = Vec::new();
    let frame = |state: &mut UiState, actions: &mut Vec<Action>, events: Vec<egui::Event>| {
        let mut i = input.clone();
        i.events = events;
        ctx.run_ui_test(i, |ui| {
            rules_window(ui.ctx(), &app, state, actions);
            rule_activation_window(ui.ctx(), &app, state, actions);
        })
    };
    frame(&mut state, &mut actions, vec![]);
    let out = frame(&mut state, &mut actions, vec![]);
    assert!(text_pos(&out, "Attivi + closed").is_some(), "{:?}", drawn_texts(&out));
    // La spunta «Attiva» dell'elenco: a sinistra del nome della regola.
    let name = text_rect(&out, "Collaudo").expect("nome della regola");
    let tick = egui::pos2(name.left() - 14.0, name.center().y);
    for ev in click_events(tick, egui::PointerButton::Primary) {
        frame(&mut state, &mut actions, vec![ev]);
    }
    assert!(actions.is_empty(), "niente salvataggio prima della conferma");
    let act = state.rule_activation.as_ref().expect("anteprima aperta");
    assert!(matches!(&act.pending, RulePending::Update { rule, was_enabled: false, .. } if rule.enabled));
    assert_eq!(act.changes.len(), 2, "ABC e XYZ (closed ammesso)");
    frame(&mut state, &mut actions, vec![]);
    let out = frame(&mut state, &mut actions, vec![]);
    let texts = drawn_texts(&out);
    assert!(texts.iter().any(|t| t == "Attivare la regola?"), "{texts:?}");
    assert!(texts.iter().any(|t| t == "XYZ") && texts.iter().any(|t| t == "Closed"), "{texts:?}");
    let pos = text_pos(&out, "Applica").expect("Applica");
    for ev in click_events(pos, egui::PointerButton::Primary) {
        frame(&mut state, &mut actions, vec![ev]);
    }
    assert!(state.rule_activation.is_none());
    match actions.as_slice() {
        [Action::UpdateRule(r, x)] => assert!(*r == id && x.enabled && x.inactive == rule.inactive),
        _ => panic!("attesa una UpdateRule, ricevute {}", actions.len()),
    }
}

/// «Importa da testo…» con regole che cambierebbero dei progetti: prima
/// l'anteprima («Importare le regole?»), nulla aggiunto finché non si
/// conferma; «Applica» → `ImportRules` aggiunge e dà il resoconto,
/// «Annulla» non aggiunge nulla e lo dice.
#[test]
fn importing_rules_that_change_projects_asks_first() {
    let mut app = App::new();
    let w = app.workers.add("Mario");
    let be = app.devs.add("Backend");
    let p = app.projects.add("x", Some("ABC"), None);
    app.projects.add_dev(p, be);
    app.projects.add_effort(p, be, WeekId(20004), w, Effort(8));
    let collaudo = app.milestones.add("Collaudo");
    let mut pjm = PjmApp { app, ui: UiState::default() };
    pjm.reset_undo();
    let text = "[regola]\nnome = Buona\nattiva = sì\ncosa = milestone \"Collaudo\"\nancora = fine dev \"Backend\"\nsettimane = +2\n\n[regola]\ncosa = milestone \"Altro\"\nancora = fine progetto\n";
    pjm.import_rules_text(text);
    assert!(pjm.app.rules.is_empty(), "niente prima della conferma");
    let act = pjm.ui.rule_activation.as_ref().expect("anteprima");
    let RulePending::Import(plan) = &act.pending else { panic!("attesa un'importazione") };
    assert_eq!((plan.rules.len(), plan.errors.len()), (1, 1));
    assert_eq!(act.changes.len(), 1);

    // La finestra: titolo e progetto, poi «Applica».
    let ctx = egui::Context::default();
    let input = egui::RawInput { screen_rect: Some(screen(1200.0, 800.0)), ..Default::default() };
    let PjmApp { app, ui: state } = &mut pjm;
    let mut actions = Vec::new();
    let mut out = None;
    for _ in 0..2 {
        out = Some(ctx.run_ui_test(input.clone(), |ui| rule_activation_window(ui.ctx(), app, state, &mut actions)));
    }
    let out = out.unwrap();
    let texts = drawn_texts(&out);
    assert!(texts.iter().any(|t| t == "Importare le regole?") && texts.iter().any(|t| t == "ABC"), "{texts:?}");
    let pos = text_pos(&out, "Applica").expect("Applica");
    for ev in click_events(pos, egui::PointerButton::Primary) {
        let mut i = input.clone();
        i.events = vec![ev];
        let _ = ctx.run_ui_test(i, |ui| rule_activation_window(ui.ctx(), app, state, &mut actions));
    }
    assert!(state.rule_activation.is_none());
    let [Action::ImportRules(_)] = actions.as_slice() else { panic!("attesa ImportRules: {}", actions.len()) };
    for a in actions {
        pjm.apply(a);
    }
    assert_eq!(pjm.app.rules.list().len(), 1);
    assert!(pjm.app.projects.list_project_milestones(p).iter().any(|(m, _)| *m == collaudo), "regola applicata");
    assert!(pjm.ui.toasts.iter().any(|t| t.text.starts_with("Regole importate: 1 nuove, 0 già presenti, 1 scartate.")));
    assert!(pjm.ui.rules_import_errors.is_some());

    // Di nuovo: la regola è già presente, quindi nulla da chiedere. Una
    // a pari ambito non cambierebbe nulla (vince la più vecchia); una più
    // specifica sì: si chiede e si annulla.
    pjm.import_rules_text(text);
    assert!(pjm.ui.rule_activation.is_none(), "identica: nessuna anteprima");
    pjm.import_rules_text(&text.replace("+2", "+3"));
    assert!(pjm.ui.rule_activation.is_none(), "perde contro la più vecchia: nessuna anteprima");
    assert_eq!(pjm.app.rules.list().len(), 2);
    let other = text.replace("+2", "+4").replace("nome = Buona\n", "nome = Buona\nprogetti = progetto \"ABC\"\n");
    pjm.import_rules_text(&other);
    assert!(pjm.ui.rule_activation.is_some());
    let PjmApp { app, ui: state } = &mut pjm;
    let mut actions = Vec::new();
    let mut out = None;
    for _ in 0..2 {
        out = Some(ctx.run_ui_test(input.clone(), |ui| rule_activation_window(ui.ctx(), app, state, &mut actions)));
    }
    let pos = text_pos(&out.unwrap(), "Annulla").expect("Annulla");
    for ev in click_events(pos, egui::PointerButton::Primary) {
        let mut i = input.clone();
        i.events = vec![ev];
        let _ = ctx.run_ui_test(i, |ui| rule_activation_window(ui.ctx(), app, state, &mut actions));
    }
    assert!(actions.is_empty() && state.rule_activation.is_none());
    assert_eq!(pjm.app.rules.list().len(), 2, "nulla aggiunto");
    assert!(pjm.ui.toasts.iter().any(|t| t.text.starts_with("Importazione delle regole annullata")));
}

/// 🗑 su una regola che ha collocato delle milestone: prima la conferma con
/// l'elenco e la spunta «Toglile dai progetti»; con la spunta, «Applica»
/// elimina la regola **e** toglie la milestone (che nessuna regola rimette).
/// Spegnere una regola senza milestone collocate passa diretto.
#[test]
fn deleting_a_rule_asks_whether_to_remove_its_milestones() {
    use crate::rules::{Anchor, Inactive, Rule, RuleScope, Target};
    let mut app = App::new();
    let w = app.workers.add("Mario");
    let be = app.devs.add("Backend");
    let p = app.projects.add("x", Some("ABC"), None);
    app.projects.add_dev(p, be);
    app.projects.add_effort(p, be, WeekId(20004), w, Effort(8));
    let collaudo = app.milestones.add("Collaudo");
    let rilascio = app.milestones.add("Rilascio");
    let rule = |target, anchor| Rule {
        name: String::new(),
        enabled: true,
        scope: RuleScope::All,
        target,
        anchor,
        offset_weeks: 2,
        inactive: Inactive::default(),
        check: Default::default(),
    };
    let id = app.rules.add(rule(Target::Milestone(collaudo), Anchor::DevEnd(be)));
    // Rilascio non ha ancora (nessuna milestone Demo): nulla di collocato.
    let idle = app.rules.add(rule(Target::Milestone(rilascio), Anchor::ProjectEnd));
    crate::rules::apply(&mut app);
    let placed = |app: &App| app.projects.list_project_milestones(p).iter().any(|(m, _)| *m == collaudo);
    assert!(placed(&app));
    let mut pjm = PjmApp { app, ui: UiState::default() };
    pjm.reset_undo();

    // Spegnere la regola che non ha collocato nulla: subito.
    let mut actions = Vec::new();
    let before = pjm.app.rules.get(idle).unwrap().clone();
    let off = Rule { enabled: false, ..before.clone() };
    update_or_confirm(&pjm.app, &mut pjm.ui, &mut actions, idle, &before, off);
    assert!(matches!(actions.as_slice(), [Action::UpdateRule(..)]) && pjm.ui.rule_activation.is_none());

    // 🗑 sulla regola del Collaudo: conferma con l'elenco.
    let mut actions = Vec::new();
    delete_or_confirm(&pjm.app, &mut pjm.ui, &mut actions, id);
    assert!(actions.is_empty());
    let act = pjm.ui.rule_activation.as_ref().expect("conferma");
    assert!(matches!(act.pending, RulePending::Delete(x) if x == id));
    assert_eq!(act.orphans.len(), 1);

    let ctx = egui::Context::default();
    let input = egui::RawInput { screen_rect: Some(screen(1200.0, 800.0)), ..Default::default() };
    let PjmApp { app, ui: state } = &mut pjm;
    let frame = |state: &mut UiState, actions: &mut Vec<Action>, events: Vec<egui::Event>| {
        let mut i = input.clone();
        i.events = events;
        ctx.run_ui_test(i, |ui| rule_activation_window(ui.ctx(), app, state, actions))
    };
    frame(state, &mut actions, vec![]);
    let out = frame(state, &mut actions, vec![]);
    let texts = drawn_texts(&out);
    assert!(texts.iter().any(|t| t == "Eliminare la regola?"), "{texts:?}");
    assert!(texts.iter().any(|t| t.contains("Collaudo (")), "{texts:?}");
    for label in ["Toglile dai progetti", "Applica"] {
        let out = frame(state, &mut actions, vec![]);
        let pos = text_pos(&out, label).unwrap_or_else(|| panic!("manca «{label}»"));
        for ev in click_events(pos, egui::PointerButton::Primary) {
            frame(state, &mut actions, vec![ev]);
        }
    }
    assert!(state.rule_activation.is_none());
    assert!(
        matches!(actions.as_slice(), [Action::DeleteRule(x), Action::RemoveProjectMilestone { milestone, .. }] if *x == id && *milestone == collaudo),
        "{} azioni",
        actions.len()
    );
    for a in actions {
        pjm.apply(a);
    }
    assert!(pjm.app.rules.get(id).is_none());
    assert!(!placed(&pjm.app), "milestone tolta");
    pjm.record_undo();
    pjm.apply(Action::Undo);
    assert!(pjm.app.rules.get(id).is_some() && placed(&pjm.app), "un Annulla rimette regola e milestone");
}

/// Ciò che una regola governa si riconosce in griglia: «⚙» nella fascia alta
/// della colonna della milestone governata e della data di inizio governata
/// (non su una milestone messa a mano), e davanti a «Inizio:» nella colonna
/// di sinistra.
#[test]
fn governed_milestones_and_dates_are_marked_in_the_grid() {
    use crate::rules::{Anchor, Inactive, Rule, RuleScope, Target};
    let (mut app, _alice, week) = highlight_fixture();
    let (proj, _) = app.projects.list()[0];
    let dev = app.devs.list()[0].0;
    let collaudo = app.milestones.add("Collaudo");
    let rilascio = app.milestones.add("Rilascio");
    app.projects.add_project_milestone(proj, rilascio, WeekId(week as usize + 14));
    let rule = |target, anchor, offset_weeks| Rule {
        name: "R".into(),
        enabled: true,
        scope: RuleScope::All,
        target,
        anchor,
        offset_weeks,
        inactive: Inactive::default(),
        check: Default::default(),
    };
    app.rules.add(rule(Target::Milestone(collaudo), Anchor::DevEnd(dev), 1));
    app.rules.add(rule(Target::ProjectStart, Anchor::FirstEffort, 0));
    crate::rules::apply(&mut app);
    let ctx = egui::Context::default();
    let mut state = UiState::default();
    let input = egui::RawInput { screen_rect: Some(screen(1600.0, 900.0)), ..Default::default() };
    let draw = |state: &mut UiState, app: &App, left: bool| {
        ctx.run_ui_test(input.clone(), |ui| {
            let layout = body_layout(ui, app, state, &Filter::default(), &None);
            let mut actions = Vec::new();
            if left {
                left_column(ui, app, state, &mut actions, &Filter::default(), &None, &layout);
            } else {
                grid(ui, app, state, &mut actions, &Filter::default(), &layout);
            }
        })
    };
    let marks = |out: &egui::FullOutput| drawn_texts(out).iter().filter(|t| *t == RULE_MARK).count();
    assert_eq!(marks(&draw(&mut state, &app, false)), 2, "inizio e Collaudo, non il Rilascio");
    let texts = drawn_texts(&draw(&mut state, &app, true));
    assert!(texts.iter().any(|t| t.starts_with("⚙ Inizio: ")), "{texts:?}");
    assert!(texts.iter().any(|t| t.starts_with("Fine: ")), "fine non governata: {texts:?}");
    // Regola spenta: nessun segno.
    let id = app.rules.list()[0].0;
    let mut off = app.rules.get(id).unwrap().clone();
    off.enabled = false;
    app.rules.update(id, off);
    assert_eq!(marks(&draw(&mut state, &app, false)), 1, "resta solo l'inizio");
}

/// Il «⚙» di una regola sta sul lato destro della fascia alta della colonna.
#[test]
fn rule_mark_sits_on_the_right_of_the_column() {
    let ctx = egui::Context::default();
    let input = egui::RawInput { screen_rect: Some(screen(400.0, 300.0)), ..Default::default() };
    let col = Rect::from_min_size(egui::pos2(100.0, 50.0), Vec2::new(COL_W, 200.0));
    let out = ctx.run_ui_test(input, |ui| paint_rule_mark(ui, col));
    let pos = text_pos(&out, RULE_MARK).expect("segno disegnato");
    assert!(pos.x > col.center().x && pos.x < col.right(), "a destra: {pos:?} in {col:?}");
    assert!((pos.y - (col.top() + ROW_H / 2.0)).abs() < 2.0, "nella fascia alta: {pos:?}");
}

/// «⚙» accanto alla tripletta (solo se il progetto ha qualcosa governato):
/// il clic apre «Regole — ABC», con le regole che agiscono qui (nome,
/// riassunto, cosa fanno) e, a parte, quelle che non agiscono.
#[test]
fn project_rules_window_opens_from_the_tripletta_mark() {
    use crate::rules::{Anchor, Inactive, Rule, RuleScope, Target};
    let (mut app, _alice, _week) = highlight_fixture();
    let (proj, _) = app.projects.list()[0];
    let dev = app.devs.list()[0].0;
    let collaudo = app.milestones.add("Collaudo");
    let rule = |name: &str, enabled| Rule {
        name: name.into(),
        enabled,
        scope: RuleScope::All,
        target: Target::Milestone(collaudo),
        anchor: Anchor::DevEnd(dev),
        offset_weeks: 1,
        inactive: Inactive::default(),
        check: Default::default(),
    };
    let ctx = egui::Context::default();
    let input = egui::RawInput { screen_rect: Some(screen(1600.0, 900.0)), ..Default::default() };
    let left = |state: &mut UiState, app: &App, events: Vec<egui::Event>| {
        let mut i = input.clone();
        i.events = events;
        ctx.run_ui_test(i, |ui| {
            let layout = body_layout(ui, app, state, &Filter::default(), &None);
            let mut actions = Vec::new();
            left_column(ui, app, state, &mut actions, &Filter::default(), &None, &layout);
        })
    };
    let mut state = UiState::default();
    // Solo una regola spenta: nessun segno.
    app.rules.add(rule("Spenta", false));
    crate::rules::apply(&mut app);
    assert!(text_pos(&left(&mut state, &app, vec![]), RULE_MARK).is_none());
    app.rules.add(rule("Collaudo dopo il Frontend", true));
    crate::rules::apply(&mut app);
    left(&mut state, &app, vec![]);
    let pos = text_pos(&left(&mut state, &app, vec![]), RULE_MARK).expect("⚙ accanto alla tripletta");
    for ev in click_events(pos, egui::PointerButton::Primary) {
        left(&mut state, &app, vec![ev]);
    }
    assert_eq!(state.project_rules, Some(proj));
    let mut out = None;
    for _ in 0..2 {
        out = Some(ctx.run_ui_test(input.clone(), |ui| project_rules_window(ui.ctx(), &app, &mut state)));
    }
    let texts = drawn_texts(&out.unwrap());
    for t in ["Regole — ABC", "Agiscono qui (1)", "Collaudo dopo il Frontend", "Non agiscono qui (1)"] {
        assert!(texts.iter().any(|s| s == t), "manca «{t}»: {texts:?}");
    }
    assert!(texts.iter().any(|s| s == "milestone \"Collaudo\" = fine dev \"Frontend\" +1 sett."), "{texts:?}");
    assert!(texts.iter().any(|s| s.starts_with("→ milestone il ")), "{texts:?}");
}

/// Filtri della finestra delle regole: la ricerca guarda nome e riassunto
/// (es. il nome della milestone), il tipo e attive / spente si combinano;
/// la finestra mostra solo le regole che passano e «N di M regole».
#[test]
fn rules_window_filters_by_search_type_and_enabled() {
    use crate::rules::{Anchor, Cmp, Inactive, Rule, RuleScope, Target};
    let mut app = App::new();
    let be = app.devs.add("Backend");
    let collaudo = app.milestones.add("Collaudo");
    let rule = |name: &str, target, anchor, enabled| Rule {
        name: name.into(),
        enabled,
        scope: RuleScope::All,
        target,
        anchor,
        offset_weeks: 0,
        inactive: Inactive::default(),
        check: Default::default(),
    };
    let a = app.rules.add(rule("Primo", Target::Milestone(collaudo), Anchor::DevEnd(be), true));
    let b = app.rules.add(rule("Secondo", Target::ProjectEnd, Anchor::LastEffort, false));
    let c = app.rules.add(rule(
        "Terzo",
        Target::Check(Anchor::Milestone(collaudo), Cmp::NotAfter),
        Anchor::ProjectEnd,
        true,
    ));
    let pass = |id, search: &str, t, e| {
        rule_passes_filters(&app, id, app.rules.get(id).unwrap(), search, t, e)
    };
    use {RuleEnabledFilter as E, RuleTypeFilter as T};
    assert!(pass(a, "collAUDO", T::All, E::All), "dal riassunto, senza maiuscole");
    assert!(pass(c, "collaudo", T::All, E::All), "anche il punto dell'avviso");
    assert!(!pass(b, "collaudo", T::All, E::All));
    assert!(pass(b, "", T::Date, E::Disabled) && !pass(b, "", T::Date, E::Enabled));
    assert!(pass(c, "", T::Warning, E::All) && !pass(a, "", T::Warning, E::All));
    assert!(pass(a, "", T::Milestone, E::Enabled) && !pass(a, "", T::Dev, E::All));

    let mut state = UiState { show_rules: true, rules_search: "terzo".into(), ..Default::default() };
    let ctx = egui::Context::default();
    let input = egui::RawInput { screen_rect: Some(screen(1700.0, 800.0)), ..Default::default() };
    let mut out = None;
    for _ in 0..2 {
        out = Some(ctx.run_ui_test(input.clone(), |ui| rules_window(ui.ctx(), &app, &mut state, &mut Vec::new())));
    }
    let texts = drawn_texts(&out.unwrap());
    assert!(texts.iter().any(|t| t == "1 di 3 regole"), "{texts:?}");
    // I nomi stanno nei campi di testo delle righe: solo «Terzo».
    assert!(texts.iter().any(|t| t == "Terzo") && !texts.iter().any(|t| t == "Primo" || t == "Secondo"), "{texts:?}");
}

/// «Aggiungi efforts»: la settimana in cui il worker è in **Ferie** si salta
/// e l'effort va alla successiva, così le settimane di effort restano N (la
/// fine scivola avanti); la Malattia non si salta; la settimana spostata si
/// sovrascrive; ogni worker segue le sue ferie; ci si ferma a fine griglia.
#[test]
fn bulk_fill_skips_vacation_weeks_to_the_next_free_one() {
    let mut app = App::new();
    let alice = app.workers.add("Alice");
    let bob = app.workers.add("Bob");
    let dev = app.devs.add("Frontend");
    let proj = app.projects.add("Progetto", Some("ABC"), None);
    let start = app.start_week;
    let wk = |i: usize| WeekId(start.0 + i * WEEK_STEP);
    app.workers.set_week_status(alice, wk(1).0, Some(WeekStatus::Ferie));
    app.workers.set_week_status(alice, wk(2).0, Some(WeekStatus::Ferie));
    app.workers.set_week_status(bob, wk(1).0, Some(WeekStatus::Malattia));
    // Valore già presente nella settimana dove scivola Alice: si sovrascrive.
    app.projects.add_effort(proj, dev, wk(4), alice, Effort(2));

    assert_eq!(bulk_fill_weeks(&app, proj, alice, start, 3, None), vec![wk(0), wk(3), wk(4)]);
    assert_eq!(bulk_fill_weeks(&app, proj, bob, start, 3, None), vec![wk(0), wk(1), wk(2)], "malattia: non si salta");
    assert!(bulk_fill_effort(&mut app, proj, dev, start, &[alice, bob], Effort(8), 3, None));
    let at = |app: &App, w: WeekId, who| {
        app.projects
            .get_single_dev(proj, dev)
            .and_then(|sd| sd.get_all(w))
            .and_then(|sew| sew.worker_id.get(&who).map(|e| e.get_effort().0))
    };
    for (i, want) in [Some(8), None, None, Some(8), Some(8)].into_iter().enumerate() {
        assert_eq!(at(&app, wk(i), alice), want, "Alice, settimana {i}");
    }
    for (i, want) in [Some(8), Some(8), Some(8), None].into_iter().enumerate() {
        assert_eq!(at(&app, wk(i), bob), want, "Bob, settimana {i}");
    }
    // A fine griglia: le ferie nell'ultima settimana accorciano.
    let last = app.end_week;
    let prev = WeekId(last.0 - WEEK_STEP);
    app.workers.set_week_status(alice, last.0, Some(WeekStatus::Ferie));
    assert_eq!(bulk_fill_weeks(&app, proj, alice, prev, 2, None), vec![prev]);
}

/// La dialog dice, per worker, quali settimane di ferie salta e fin dove
/// arriva; «Inserisci» manda il numero di settimane chiesto.
#[test]
fn bulk_fill_window_reports_skipped_vacation_weeks() {
    let mut app = App::new();
    let alice = app.workers.add("Alice");
    let dev = app.devs.add("Frontend");
    let proj = app.projects.add("Progetto", Some("ABC"), None);
    let start = app.start_week.0;
    app.workers.set_week_status(alice, start + WEEK_STEP, Some(WeekStatus::Ferie));
    let mut bf = BulkFill { weeks_text: "3".into(), effort_text: "8".into(), ..BulkFill::new(proj, dev, start as i32) };
    bf.selected.insert(alice);
    let mut state = UiState { bulk_fill: Some(bf), ..Default::default() };
    let ctx = egui::Context::default();
    let input = egui::RawInput { screen_rect: Some(screen(1000.0, 800.0)), ..Default::default() };
    let mut out = None;
    for _ in 0..2 {
        out = Some(ctx.run_ui_test(input.clone(), |ui| bulk_fill_window(ui.ctx(), &app, &mut state, &mut Vec::new())));
    }
    let out = out.unwrap();
    let texts = drawn_texts(&out);
    let skipped = format!(
        "Alice: in ferie il {} (saltate) → fino al {}",
        week_label((start + WEEK_STEP) as i32),
        week_label((start + 3 * WEEK_STEP) as i32)
    );
    assert!(texts.contains(&skipped), "{skipped}: {texts:?}");
    let summary = format!("1 worker × 8 h × 3 settimane ({} → {})", week_label(start as i32), week_label((start + 3 * WEEK_STEP) as i32));
    assert!(texts.contains(&summary), "{summary}: {texts:?}");
    let mut actions = Vec::new();
    let pos = text_pos(&out, "Inserisci").expect("Inserisci");
    for ev in click_events(pos, egui::PointerButton::Primary) {
        let mut i = input.clone();
        i.events = vec![ev];
        let _ = ctx.run_ui_test(i, |ui| bulk_fill_window(ui.ctx(), &app, &mut state, &mut actions));
    }
    assert!(
        matches!(actions.as_slice(), [Action::BulkFillEffort { weeks: 3, workers, .. }] if workers == &vec![alice]),
        "{} azioni",
        actions.len()
    );
}

/// Regola d'avviso con «Blocca» (Test non prima della fine del Backend + 1):
/// un inserimento multiplo che anticiperebbe il Test viene annullato **per
/// intero** con una notifica, il file non risulta modificato e non nasce un
/// passo di Annulla; uno che rispetta la regola passa.
#[test]
fn blocking_rule_reverts_the_whole_action_that_violates_it() {
    use crate::rules::{Anchor, CheckOpts, Cmp, Inactive, Rule, RuleScope, Target};
    let mut app = App::new();
    let w = app.workers.add("Mario");
    let be = app.devs.add("Backend");
    let test = app.devs.add("Test");
    let p = app.projects.add("x", Some("ABC"), None);
    app.projects.add_dev(p, be);
    app.projects.add_dev(p, test);
    let base = app.start_week.0;
    let wk = |i: usize| WeekId(base + i * WEEK_STEP);
    for i in 0..3 {
        app.projects.add_effort(p, be, wk(i), w, Effort(8));
    }
    app.rules.add(Rule {
        name: "Test dopo il Backend".into(),
        enabled: true,
        scope: RuleScope::All,
        target: Target::Check(Anchor::DevStart(test), Cmp::NotBefore),
        anchor: Anchor::DevEnd(be),
        offset_weeks: 1,
        inactive: Inactive::default(),
        check: CheckOpts { block: true, ..Default::default() },
    });
    let mut pjm = PjmApp { app, ui: UiState::default() };
    pjm.reset_undo();
    pjm.ui.changed = false;
    let fill = |start: usize, weeks| Action::BulkFillEffort {
        proj: p,
        dev: test,
        start: wk(start),
        workers: vec![w],
        effort: Effort(8),
        weeks,
        until: None,
    };
    let cells = |pjm: &PjmApp| {
        pjm.app.projects.get_single_dev(p, test).map(|sd| sd.get_weeks().len()).unwrap_or(0)
    };
    // Dalla settimana 2 per 4 settimane: la prima (2) è troppo presto
    // (Backend finisce alla 2, quindi Test dalla 3): tutto annullato.
    pjm.apply_guarded(fill(2, 4));
    pjm.record_undo();
    assert_eq!(cells(&pjm), 0, "nessuna settimana scritta");
    assert!(!pjm.ui.changed, "il file non risulta modificato");
    assert!(pjm.ui.undo.past.is_empty(), "nessun passo di Annulla");
    let toast = pjm.ui.toasts.last().expect("notifica").text.clone();
    assert!(toast.starts_with("Modifica annullata: la regola «Test dopo il Backend» la blocca — ABC: Inizio dev Test"), "{toast}");
    // Dalla settimana 3: rispetta la regola.
    pjm.apply_guarded(fill(3, 2));
    assert_eq!(cells(&pjm), 2);
    assert!(pjm.ui.changed);
    // Ora allungare il Backend sposterebbe la soglia oltre l'inizio del Test:
    // violazione nuova, bloccata anche se si tocca un altro dev.
    let more_be = Action::BulkFillEffort { proj: p, dev: be, start: wk(3), workers: vec![w], effort: Effort(8), weeks: 1, until: None };
    pjm.apply_guarded(more_be);
    assert!(pjm.app.projects.get_single_dev(p, be).unwrap().get_all(wk(3)).is_none(), "Backend non allungato");
}

/// Finestra delle regole: il dettaglio di un avviso mostra la spunta
/// «Blocca»; spuntarla manda l'`UpdateRule` con il blocco acceso.
#[test]
fn rules_window_has_the_block_checkbox_for_warnings() {
    use crate::rules::{Anchor, Cmp, Inactive, Rule, RuleScope, Target};
    let mut app = App::new();
    let be = app.devs.add("Backend");
    let test = app.devs.add("Test");
    let id = app.rules.add(Rule {
        name: "Ordine".into(),
        enabled: false,
        scope: RuleScope::All,
        target: Target::Check(Anchor::DevStart(test), Cmp::NotBefore),
        anchor: Anchor::DevEnd(be),
        offset_weeks: 0,
        inactive: Inactive::default(),
        check: Default::default(),
    });
    let mut state = UiState { show_rules: true, ..Default::default() };
    let ctx = egui::Context::default();
    // Schermo 1536 px (1920 al 125%): la spunta deve restare in vista.
    let input = egui::RawInput { screen_rect: Some(screen(1536.0, 800.0)), ..Default::default() };
    let mut actions = Vec::new();
    let mut out = None;
    for _ in 0..2 {
        out = Some(ctx.run_ui_test(input.clone(), |ui| rules_window(ui.ctx(), &app, &mut state, &mut actions)));
    }
    let out = out.unwrap();
    let pos = text_pos(&out, "impedisci le modifiche che la violano").expect("spunta Blocca");
    assert!(pos.x < 1536.0, "in vista: {pos:?}");
    for ev in click_events(pos, egui::PointerButton::Primary) {
        let mut i = input.clone();
        i.events = vec![ev];
        let _ = ctx.run_ui_test(i, |ui| rules_window(ui.ctx(), &app, &mut state, &mut actions));
    }
    assert!(
        matches!(actions.as_slice(), [Action::UpdateRule(r, x)] if *r == id && x.check.block),
        "{} azioni",
        actions.len()
    );
}

/// Elenco + dettaglio: di partenza è selezionata la prima regola; un clic su
/// un'altra nell'elenco la mostra nel dettaglio (il campo Nome ha il suo
/// nome); una regola aggiunta (nuova o copia) diventa la selezionata.
#[test]
fn rules_window_selects_a_rule_from_the_list() {
    use crate::rules::{Anchor, Rule, RuleScope, Target};
    let mut app = App::new();
    let rule = |name: &str| Rule {
        name: name.into(),
        enabled: false,
        scope: RuleScope::All,
        target: Target::ProjectEnd,
        anchor: Anchor::LastEffort,
        offset_weeks: 0,
        inactive: Default::default(),
        check: Default::default(),
    };
    let a = app.rules.add(rule("Prima"));
    let b = app.rules.add(rule("Seconda"));
    let mut pjm = PjmApp { app, ui: UiState { show_rules: true, ..Default::default() } };
    let ctx = egui::Context::default();
    let input = egui::RawInput { screen_rect: Some(screen(1536.0, 864.0)), ..Default::default() };
    let frame = |pjm: &mut PjmApp, events: Vec<egui::Event>| {
        let mut i = input.clone();
        i.events = events;
        let mut actions = Vec::new();
        let out = ctx.run_ui_test(i, |ui| rules_window(ui.ctx(), &pjm.app, &mut pjm.ui, &mut actions));
        (out, actions)
    };
    frame(&mut pjm, vec![]);
    let (out, _) = frame(&mut pjm, vec![]);
    assert_eq!(pjm.ui.rule_selected, Some(a));
    // Il nome compare due volte (elenco e campo del dettaglio) solo per la
    // selezionata.
    let count = |out: &egui::FullOutput, t: &str| drawn_texts(out).iter().filter(|x| *x == t).count();
    assert_eq!((count(&out, "Prima"), count(&out, "Seconda")), (2, 1));
    let pos = text_pos(&out, "Seconda").expect("Seconda nell'elenco");
    for ev in click_events(pos, egui::PointerButton::Primary) {
        frame(&mut pjm, vec![ev]);
    }
    assert_eq!(pjm.ui.rule_selected, Some(b));
    let (out, _) = frame(&mut pjm, vec![]);
    assert_eq!((count(&out, "Prima"), count(&out, "Seconda")), (1, 2));
    // «Clona» nel dettaglio: la copia diventa la selezionata.
    let pos = text_pos(&out, "Clona").expect("Clona");
    let mut sent = Vec::new();
    for ev in click_events(pos, egui::PointerButton::Primary) {
        sent.extend(frame(&mut pjm, vec![ev]).1);
    }
    for a in sent {
        pjm.apply(a);
    }
    let copy = pjm.app.rules.list().last().unwrap().0;
    assert_eq!(pjm.ui.rule_selected, Some(copy));
    assert_eq!(crate::rules::rule_label(&pjm.app, copy), "Seconda (copia)");
}

/// Nei testi delle regole i nomi di dev e milestone (dopo «dev» /
/// «milestone», anche fra virgolette) hanno il colore del dev (il suo
/// sfondo) o della milestone su due sfondi scuri diversi; il resto no; il
/// nome più lungo vince («Backend 2» su «Backend»); «tutti i dev)» non è
/// un nome.
#[test]
fn rule_texts_color_dev_and_milestone_names() {
    let mut app = App::new();
    app.devs.add("Backend");
    app.devs.add("Backend 2");
    let m = app.milestones.add("Collaudo");
    let font = egui::FontId::proportional(12.0);
    let job = ref_job(
        &app,
        "milestone \"Collaudo\" = fine dev \"Backend 2\" e Fine dev Backend (tutti i dev)",
        Color32::GRAY,
        font,
    );
    let parts: Vec<(&str, Color32, Color32)> = job
        .sections
        .iter()
        .map(|s| (&job.text[s.byte_range.start.0..s.byte_range.end.0], s.format.color, s.format.background))
        .collect();
    let colored: Vec<&str> = parts.iter().filter(|(_, _, bg)| *bg != Color32::TRANSPARENT).map(|(t, ..)| *t).collect();
    assert_eq!(colored, ["Collaudo", "Backend 2", "Backend"], "{parts:?}");
    let get = |name: &str| parts.iter().find(|(t, ..)| *t == name).unwrap();
    assert_eq!(get("Collaudo").2, rule_ref_bg(false));
    assert_eq!(get("Backend 2").2, rule_ref_bg(true));
    assert_ne!(get("Collaudo").2, get("Backend").2, "sfondi diversi per dev e milestone");
    let ms_color = from_hex(app.milestones.get_color(m).unwrap());
    assert_eq!(get("Collaudo").1, rule_ref_text(ms_color), "il colore della milestone");
    assert!(parts.iter().all(|(t, c, bg)| *bg != Color32::TRANSPARENT || *c == Color32::GRAY || t.is_empty()));
    assert_eq!(job.text, "milestone \"Collaudo\" = fine dev \"Backend 2\" e Fine dev Backend (tutti i dev)");
}

/// Gli sfondi dei nomi seguono il tema (chiari nel chiaro, scuri nello
/// scuro) e il colore del nome si adatta: un giallo chiaro si scurisce nel
/// tema chiaro, un blu scuro si schiarisce nel tema scuro; il contrasto col
/// suo sfondo resta sufficiente.
#[test]
fn rule_name_chips_follow_the_theme() {
    let lum = |c: Color32| 0.299 * c.r() as f32 + 0.587 * c.g() as f32 + 0.114 * c.b() as f32;
    let yellow = Color32::from_rgb(0xff, 0xee, 0x58);
    let navy = Color32::from_rgb(0x10, 0x20, 0x60);
    let was = is_dark();
    for dark in [false, true] {
        set_dark_theme(dark);
        for dev in [true, false] {
            let bg = rule_ref_bg(dev);
            assert_eq!(lum(bg) > 128.0, !dark, "sfondo {} col tema {}", if dev { "dev" } else { "milestone" }, dark);
            for c in [yellow, navy] {
                let t = rule_ref_text(c);
                assert!((lum(t) - lum(bg)).abs() >= 60.0, "{c:?} → {t:?} su {bg:?} (tema scuro: {dark})");
            }
        }
        assert_ne!(rule_ref_bg(true), rule_ref_bg(false));
    }
    set_dark_theme(was);
}

/// La regola in una riga, sotto il nome nell'elenco, è un po' più grande
/// del carattere piccolo.
#[test]
fn rule_summary_is_a_bit_larger_than_small_text() {
    use crate::rules::{Anchor, Rule, RuleScope, Target};
    let mut app = App::new();
    app.rules.add(Rule {
        name: "Fine".into(),
        enabled: false,
        scope: RuleScope::All,
        target: Target::ProjectEnd,
        anchor: Anchor::LastEffort,
        offset_weeks: 1,
        inactive: Default::default(),
        check: Default::default(),
    });
    let mut state = UiState { show_rules: true, ..Default::default() };
    let ctx = egui::Context::default();
    let input = egui::RawInput { screen_rect: Some(screen(1536.0, 864.0)), ..Default::default() };
    let mut out = None;
    for _ in 0..2 {
        out = Some(ctx.run_ui_test(input.clone(), |ui| rules_window(ui.ctx(), &app, &mut state, &mut Vec::new())));
    }
    let out = out.unwrap();
    let size = out.shapes.iter().find_map(|cs| match &cs.shape {
        egui::epaint::Shape::Text(t) if t.galley.job.text == "fine progetto = ultimo effort +1 sett." => {
            t.galley.job.sections.first().map(|s| s.format.font_id.size)
        }
        _ => None,
    });
    let small = egui::Style::default().text_styles[&egui::TextStyle::Small].size;
    assert_eq!(size, Some(small + RULE_SUMMARY_FONT_DELTA));
}

/// Piano di «Aggiungi automatic effort»: da distribuire = pianificato −
/// già inserite; ogni worker al massimo le ore indicate, solo interi;
/// l'ultima settimana in ordine (il primo fino al massimo, poi il resto);
/// mai oltre la capacità della settimana (massimo effettivo meno le ore su
/// tutti i progetti); saltate per ogni worker le Ferie e le settimane in cui
/// ha già ore sul dev; ci si ferma a fine griglia.
#[test]
fn auto_fill_plan_spreads_what_is_left_of_the_planned_effort() {
    let mut app = App::new();
    let alice = app.workers.add("Alice");
    let bob = app.workers.add("Bob");
    let dev = app.devs.add("Frontend");
    let p = app.projects.add("x", Some("ABC"), None);
    app.projects.add_dev(p, dev);
    app.projects.add_dev_effort(p, dev, Effort(40));
    let start = app.start_week;
    let wk = |i: usize| WeekId(start.0 + i * WEEK_STEP);
    app.projects.add_effort(p, dev, wk(0), alice, Effort(6)); // già occupata
    app.workers.set_week_status(alice, wk(1).0, Some(WeekStatus::Ferie));
    app.compute_sovra();
    let plan = auto_fill_plan(&app, p, dev, start, &[alice, bob], 8);
    assert_eq!((plan.planned, plan.allocated, plan.to_place(), plan.left), (40, 6, 34, 0));
    assert_eq!(
        plan.cells,
        vec![(wk(0), bob, 8), (wk(1), bob, 8), (wk(2), alice, 8), (wk(2), bob, 8), (wk(3), alice, 2)]
    );
    assert_eq!(plan.weeks(), 4);
    // Capacità: Bob ha già 36h su un altro progetto nella settimana 1 (resta
    // 4h) e un limite di 0h nella 2 (piena: saltata).
    let q = app.projects.add("y", Some("XYZ"), None);
    app.projects.add_dev(q, dev);
    app.projects.add_effort(q, dev, wk(1), bob, Effort(36));
    app.workers.set_week_override(bob, wk(2).0, 0);
    app.compute_sovra();
    let plan = auto_fill_plan(&app, p, dev, start, &[bob], 8);
    assert_eq!(
        plan.cells,
        vec![(wk(0), bob, 8), (wk(1), bob, 4), (wk(3), bob, 8), (wk(4), bob, 8), (wk(5), bob, 6)],
        "mai oltre la capacità della settimana"
    );
    // Ore che non stanno nella griglia: restano fuori.
    let last = app.end_week;
    let plan = auto_fill_plan(&app, p, dev, WeekId(last.0 - WEEK_STEP), &[bob], 8);
    assert_eq!((plan.cells.len(), plan.left), (2, 34 - 16));
    // Niente da distribuire, nessun worker o 0 ore: nessuna cella.
    app.projects.add_effort(p, dev, wk(6), bob, Effort(34));
    app.compute_sovra();
    assert!(auto_fill_plan(&app, p, dev, start, &[alice], 8).cells.is_empty());
    assert!(auto_fill_plan(&app, p, dev, start, &[], 8).cells.is_empty());
}

/// Tasto destro sulla fascia milestone: «Aggiungi automatic effort…» sta
/// sotto «Aggiungi efforts…», disattivato senza pianificato; con il
/// pianificato apre la dialog, che mostra cosa resta da distribuire, e
/// «Inserisci» scrive fino a esaurirlo.
#[test]
fn auto_fill_menu_item_and_dialog_fill_up_to_the_planned_effort() {
    let (mut app, alice, week) = highlight_fixture(); // Alice 8h nella prima settimana
    let dev = app.devs.list()[0].0;
    let (p, _) = app.projects.list()[0];
    let ctx = egui::Context::default();
    let origin = Cell::new(egui::Pos2::ZERO);
    let open_menu = |app: &App, state: &mut UiState| {
        let frame = |state: &mut UiState, ev: Option<egui::Event>| {
            let mut input = egui::RawInput { screen_rect: Some(screen(1600.0, 900.0)), ..Default::default() };
            input.events.extend(ev);
            let mut actions = Vec::new();
            ctx.run_ui_test(input, |ui| {
                origin.set(ui.cursor().min);
                let layout = body_layout(ui, app, state, &Filter::default(), &None);
                grid(ui, app, state, &mut actions, &Filter::default(), &layout);
            })
        };
        let _ = frame(state, None);
        let strip = egui::pos2(origin.get().x + COL_W * 1.5, origin.get().y + 1.0);
        for ev in click_events(strip, egui::PointerButton::Secondary) {
            let _ = frame(state, Some(ev));
        }
        let out = frame(state, None);
        let below = text_pos(&out, "Aggiungi automatic effort…").expect("voce nuova");
        let above = text_pos(&out, "Aggiungi efforts…").expect("voce esistente");
        assert!(below.y > above.y, "sotto «Aggiungi efforts…»");
        for ev in click_events(below, egui::PointerButton::Primary) {
            let _ = frame(state, Some(ev));
        }
    };
    // Senza pianificato: voce disattivata, nessuna dialog.
    let mut state = UiState::default();
    open_menu(&app, &mut state);
    assert!(state.auto_fill.is_none(), "senza pianificato non si apre");
    // Con 30h pianificate (8 già inserite): si apre sulla settimana cliccata.
    app.projects.add_dev_effort(p, dev, Effort(30));
    let mut state = UiState::default();
    open_menu(&app, &mut state);
    let bf = state.auto_fill.as_mut().expect("dialog automatica aperta");
    assert_eq!(bf.week, week + WEEK_STEP as i32);
    bf.selected.insert(alice);
    bf.effort_text = "8".into();
    let input = egui::RawInput { screen_rect: Some(screen(1000.0, 800.0)), ..Default::default() };
    let mut out = None;
    for _ in 0..2 {
        out = Some(ctx.run_ui_test(input.clone(), |ui| auto_fill_window(ui.ctx(), &app, &mut state, &mut Vec::new())));
    }
    let out = out.unwrap();
    let texts = drawn_texts(&out);
    assert!(texts.iter().any(|t| t == "Effort pianificato 30h, già inserite 8h: da distribuire 22h."), "{texts:?}");
    assert!(texts.iter().any(|t| t == "Alice: 22h in 3 settimane"), "{texts:?}");
    let pos = text_pos(&out, "Inserisci").expect("Inserisci");
    let mut actions = Vec::new();
    for ev in click_events(pos, egui::PointerButton::Primary) {
        let mut i = input.clone();
        i.events = vec![ev];
        let _ = ctx.run_ui_test(i, |ui| auto_fill_window(ui.ctx(), &app, &mut state, &mut actions));
    }
    assert!(state.auto_fill.is_none());
    let mut pjm = PjmApp { app, ui: UiState::default() };
    for a in actions {
        pjm.apply(a);
    }
    let sd = pjm.app.projects.get_single_dev(p, dev).unwrap();
    assert_eq!(sd.effort_up_to(WeekId(usize::MAX)).0, 30, "esaurito il pianificato");
    let w = |i: usize| sd.get_all(WeekId(week as usize + i * WEEK_STEP)).and_then(|s| s.worker_id.get(&alice)).map(|e| e.get_effort().0);
    assert_eq!([w(1), w(2), w(3), w(4)], [Some(8), Some(8), Some(6), None]);
}

/// «Aggiungi efforts» e «Aggiungi automatic effort» non vanno oltre la fine
/// del progetto (la sua settimana è l'ultima, inclusa): l'inserimento si
/// ferma lì e la dialog lo dice; senza data di fine vale la fine della
/// griglia.
#[test]
fn fills_stop_at_the_project_end() {
    let mut app = App::new();
    let alice = app.workers.add("Alice");
    let dev = app.devs.add("Frontend");
    let proj = app.projects.add("x", Some("ABC"), None);
    app.projects.add_dev(proj, dev);
    app.projects.add_dev_effort(proj, dev, Effort(100));
    let start = app.start_week;
    let wk = |i: usize| WeekId(start.0 + i * WEEK_STEP);
    // Senza fine: la griglia.
    assert_eq!(fill_limit(&app, proj), (app.end_week, false));
    app.projects.set_project_end_week(proj, Some(wk(2)));
    assert_eq!(fill_limit(&app, proj), (wk(2), true));
    // Aggiungi efforts per 5 settimane: solo 3 (0, 1, 2).
    assert_eq!(bulk_fill_weeks(&app, proj, alice, start, 5, None), vec![wk(0), wk(1), wk(2)]);
    // Automatico: 100h da 8h → solo 3 settimane, 76h restano fuori.
    let plan = auto_fill_plan(&app, proj, dev, start, &[alice], 8);
    assert_eq!((plan.cells.len(), plan.left), (3, 76));
    assert!(plan.cells.iter().all(|(w, ..)| *w <= wk(2)));
    // La dialog dell'inserimento multiplo lo dice.
    let mut bf = BulkFill { weeks_text: "5".into(), effort_text: "8".into(), ..BulkFill::new(proj, dev, start.0 as i32) };
    bf.selected.insert(alice);
    let mut state = UiState { bulk_fill: Some(bf), ..Default::default() };
    let ctx = egui::Context::default();
    let input = egui::RawInput { screen_rect: Some(screen(1000.0, 800.0)), ..Default::default() };
    let mut out = None;
    for _ in 0..2 {
        out = Some(ctx.run_ui_test(input.clone(), |ui| bulk_fill_window(ui.ctx(), &app, &mut state, &mut Vec::new())));
    }
    let texts = drawn_texts(&out.unwrap());
    assert!(texts.iter().any(|t| t == "Alice: solo 3 settimane disponibili fino alla fine del progetto."), "{texts:?}");
    // E l'automatica.
    let mut bf = BulkFill { effort_text: "8".into(), ..BulkFill::new(proj, dev, start.0 as i32) };
    bf.selected.insert(alice);
    let mut state = UiState { auto_fill: Some(bf), ..Default::default() };
    let mut out = None;
    for _ in 0..2 {
        out = Some(ctx.run_ui_test(input.clone(), |ui| auto_fill_window(ui.ctx(), &app, &mut state, &mut Vec::new())));
    }
    let texts = drawn_texts(&out.unwrap());
    assert!(texts.iter().any(|t| t == "Arrivati alla fine del progetto: restano 76h non inserite."), "{texts:?}");
}

/// «Aggiungi efforts» ▸ «Fino a fine progetto»: spuntata, l'effort va a ogni
/// worker scelto in tutte le settimane fino alla fine del progetto (inclusa),
/// saltando le sue ferie e senza l'avviso «solo N settimane»; senza data di
/// fine la spunta è disattivata.
#[test]
fn bulk_fill_until_the_project_end() {
    let mut app = App::new();
    let alice = app.workers.add("Alice");
    let dev = app.devs.add("Frontend");
    let proj = app.projects.add("x", Some("ABC"), None);
    app.projects.add_dev(proj, dev);
    let start = app.start_week;
    let wk = |i: usize| WeekId(start.0 + i * WEEK_STEP);
    app.workers.set_week_status(alice, wk(1).0, Some(WeekStatus::Ferie));
    let ctx = egui::Context::default();
    let input = egui::RawInput { screen_rect: Some(screen(1000.0, 800.0)), ..Default::default() };
    let open = || {
        let mut bf = BulkFill { effort_text: "8".into(), ..BulkFill::new(proj, dev, start.0 as i32) };
        bf.selected.insert(alice);
        UiState { bulk_fill: Some(bf), ..Default::default() }
    };
    let frames = |app: &App, state: &mut UiState, actions: &mut Vec<Action>, ev: Option<egui::Event>| {
        let mut i = input.clone();
        i.events.extend(ev);
        ctx.run_ui_test(i, |ui| bulk_fill_window(ui.ctx(), app, state, actions))
    };
    let tick = |app: &App, state: &mut UiState| {
        let mut actions = Vec::new();
        frames(app, state, &mut actions, None);
        let out = frames(app, state, &mut actions, None);
        let pos = text_pos(&out, "Fino a fine progetto").expect("spunta");
        for ev in click_events(pos, egui::PointerButton::Primary) {
            frames(app, state, &mut actions, Some(ev));
        }
    };
    // Senza data di fine: disattivata.
    let mut state = open();
    tick(&app, &mut state);
    assert!(!state.bulk_fill.as_ref().unwrap().to_end);
    // Con la fine alla settimana 3: spuntata, 4 settimane di calendario.
    app.projects.set_project_end_week(proj, Some(wk(3)));
    let mut state = open();
    tick(&app, &mut state);
    assert!(state.bulk_fill.as_ref().unwrap().to_end);
    let mut actions = Vec::new();
    let out = frames(&app, &mut state, &mut actions, None);
    let texts = drawn_texts(&out);
    let summary = format!("1 worker × 8 h × 4 settimane ({} → {})", week_label(wk(0).0 as i32), week_label(wk(3).0 as i32));
    assert!(texts.contains(&summary), "{summary}: {texts:?}");
    assert!(!texts.iter().any(|t| t.contains("solo")), "nessun avviso: {texts:?}");
    let pos = text_pos(&out, "Inserisci").expect("Inserisci");
    for ev in click_events(pos, egui::PointerButton::Primary) {
        frames(&app, &mut state, &mut actions, Some(ev));
    }
    let mut pjm = PjmApp { app, ui: UiState::default() };
    for a in actions {
        pjm.apply(a);
    }
    let sd = pjm.app.projects.get_single_dev(proj, dev).unwrap();
    assert_eq!(sd.get_weeks(), vec![wk(0), wk(2), wk(3)], "ferie saltate, niente oltre la fine");
}

#[test]
fn bulk_fill_until_a_milestone() {
    let mut app = App::new();
    let alice = app.workers.add("Alice");
    let dev = app.devs.add("Frontend");
    let proj = app.projects.add("x", Some("ABC"), None);
    app.projects.add_dev(proj, dev);
    let start = app.start_week;
    let wk = |i: usize| WeekId(start.0 + i * WEEK_STEP);
    app.projects.set_project_end_week(proj, Some(wk(6)));
    app.workers.set_week_status(alice, wk(1).0, Some(WeekStatus::Ferie));
    let collaudo = app.milestones.add("Collaudo");
    let kickoff = app.milestones.add("Kickoff");
    let oltre = app.milestones.add("Oltre");
    app.projects.add_project_milestone(proj, collaudo, wk(3));
    app.projects.add_project_milestone(proj, kickoff, wk(1));
    app.projects.add_project_milestone(proj, oltre, wk(9));
    // Solo da qui in avanti e non oltre la fine del progetto, per data.
    assert_eq!(bulk_fill_milestones(&app, proj, wk(0)), vec![(kickoff, wk(1)), (collaudo, wk(3))]);
    assert_eq!(bulk_fill_milestones(&app, proj, wk(2)), vec![(collaudo, wk(3))]);
    // La milestone ferma l'inserimento anche quando le ferie lo allungherebbero.
    assert_eq!(bulk_fill_weeks(&app, proj, alice, wk(0), 4, Some(wk(3))), vec![wk(0), wk(2), wk(3)]);

    let ctx = egui::Context::default();
    let input = egui::RawInput { screen_rect: Some(screen(1000.0, 800.0)), ..Default::default() };
    let mut bf = BulkFill { effort_text: "8".into(), ..BulkFill::new(proj, dev, start.0 as i32) };
    bf.selected.insert(alice);
    let mut state = UiState { bulk_fill: Some(bf), ..Default::default() };
    let mut actions = Vec::new();
    let frame = |state: &mut UiState, actions: &mut Vec<Action>, ev: Option<egui::Event>| {
        let mut i = input.clone();
        i.events.extend(ev);
        ctx.run_ui_test(i, |ui| bulk_fill_window(ui.ctx(), &app, state, actions))
    };
    frame(&mut state, &mut actions, None);
    let out = frame(&mut state, &mut actions, None);
    // Apre la scelta e prende «Collaudo».
    let pos = text_pos(&out, "—").expect("combo milestone");
    for ev in click_events(pos, egui::PointerButton::Primary) {
        frame(&mut state, &mut actions, Some(ev));
    }
    let out = frame(&mut state, &mut actions, None);
    let item = format!("Collaudo — {}", week_label(wk(3).0 as i32));
    let pos = text_pos(&out, &item).expect("voce Collaudo");
    for ev in click_events(pos, egui::PointerButton::Primary) {
        frame(&mut state, &mut actions, Some(ev));
    }
    assert_eq!(state.bulk_fill.as_ref().unwrap().to_milestone, Some(collaudo));
    let out = frame(&mut state, &mut actions, None);
    let texts = drawn_texts(&out);
    let summary = format!("1 worker × 8 h × 4 settimane ({} → {})", week_label(wk(0).0 as i32), week_label(wk(3).0 as i32));
    assert!(texts.contains(&summary), "{summary}: {texts:?}");
    assert!(!texts.iter().any(|t| t.contains("solo")), "nessun avviso: {texts:?}");
    let pos = text_pos(&out, "Inserisci").expect("Inserisci");
    for ev in click_events(pos, egui::PointerButton::Primary) {
        frame(&mut state, &mut actions, Some(ev));
    }
    assert!(
        matches!(actions.as_slice(), [Action::BulkFillEffort { weeks: 4, until: Some(u), .. }] if *u == wk(3)),
        "una sola BulkFillEffort fino alla milestone"
    );
    let mut pjm = PjmApp { app: app.clone(), ui: UiState::default() };
    for a in actions {
        pjm.apply(a);
    }
    let sd = pjm.app.projects.get_single_dev(proj, dev).unwrap();
    assert_eq!(sd.get_weeks(), vec![wk(0), wk(2), wk(3)], "ferie saltate, niente oltre la milestone");
}
