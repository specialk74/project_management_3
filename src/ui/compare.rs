//! Finestra "Confronta / Importa progetto": confronta i progetti del file
//! ufficiale (aperto nell'app) con quelli di un file `.ron` parallelo (una copia
//! su cui si è sperimentato) e permette di importare i dev scelti.
//!
//! Due pannelli affiancati (sinistra = ufficiale, destra = parallelo) con scroll
//! sincronizzato; tra i due, per ogni dev, due frecce per copiare il dev da un
//! lato all'altro. Vedi `project_diff` per la logica di confronto.

use super::*;

use crate::single_dev_utils::single_dev::SingleDev;

/// Individua i progetti presenti in **entrambi** i file (accoppiati per tripletta
/// non vuota) che **differiscono** tra ufficiale e parallelo. I progetti identici
/// sono esclusi: la finestra mostra solo quelli diversi. Coppie deselezionate.
pub(crate) fn common_project_pairs(off: &App, par: &App) -> Vec<ComparePair> {
    // tripletta → id nel file parallelo
    let mut par_by_trip: HashMap<String, ProjectId> = HashMap::new();
    for id in par.projects.ids() {
        let t = par.projects.get_tripletta(id);
        if !t.is_empty() {
            par_by_trip.insert(t, id);
        }
    }
    let mut pairs = Vec::new();
    for off_id in off.projects.ids() {
        let t = off.projects.get_tripletta(off_id);
        if t.is_empty() {
            continue;
        }
        if let Some(&par_id) = par_by_trip.get(&t) {
            // Includi solo se c'è una differenza (intestazione o dev).
            let differs = match (off.projects.get(off_id), par.projects.get(par_id)) {
                (Some(a), Some(b)) => !crate::project_diff::diff_projects(a, b).is_empty(),
                _ => false,
            };
            if differs {
                pairs.push(ComparePair {
                    tripletta: t,
                    off: off_id,
                    par: par_id,
                    selected: false,
                });
            }
        }
    }
    pairs
}

/// Copia (sovrascrive) l'intero progetto `src_pid` di `src` nel progetto
/// `dst_pid` di `dst`, mantenendo però `order` ed `enable` della destinazione
/// (stato di sola visualizzazione, da non ereditare dal parallelo).
pub(crate) fn copy_project_into(dst: &mut App, dst_pid: ProjectId, src: &App, src_pid: ProjectId) {
    let Some(srcp) = src.projects.get(src_pid) else {
        return;
    };
    let mut newp = srcp.clone();
    if let Some(dstp) = dst.projects.get(dst_pid) {
        newp.set_order(dstp.get_order());
        newp.set_enable(dstp.get_enable());
    }
    dst.projects.set_project(dst_pid, newp);
}

/// Inserisce (o rimuove) un `SingleDev` per `dev` nel progetto `proj` di `app`.
/// `src = Some` copia/sovrascrive; `src = None` rimuove il dev dal progetto.
pub(crate) fn copy_dev_into(
    app: &mut App,
    proj: ProjectId,
    dev: DevId,
    src: Option<SingleDev>,
) {
    let Some(mut p) = app.projects.get(proj).cloned() else {
        return;
    };
    let mut devs = p.devs().clone();
    match src {
        Some(sd) => {
            devs.insert(dev, sd);
        }
        None => {
            devs.remove(&dev);
        }
    }
    p.set_devs(devs);
    app.projects.set_project(proj, p);
}

/// Finestra di confronto. Presente solo quando `state.compare` è `Some`.
pub(crate) fn compare_window(
    ctx: &egui::Context,
    app: &App,
    state: &mut UiState,
    actions: &mut Vec<Action>,
) {
    if state.compare.is_none() {
        return;
    }
    let mut close = false;

    egui::Window::new("Confronta / Importa progetto")
        .collapsible(false)
        .resizable(true)
        .default_size([960.0, 640.0])
        .show(ctx, |ui| {
            let cmp = state.compare.as_mut().expect("compare presente");

            ui.horizontal(|ui| {
                ui.label("File parallelo:");
                ui.monospace(&cmp.other_path);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Chiudi").clicked() {
                        close = true;
                    }
                });
            });
            ui.separator();

            match cmp.stage {
                CompareStage::Select => compare_select_stage(ui, cmp),
                CompareStage::View => compare_view_stage(ui, app, cmp, actions),
            }
        });

    if close {
        // Chiudendo si riabilita l'autosave (guardia in `maybe_autosave`).
        state.compare = None;
    }
}

/// Fase di selezione: elenco dei progetti in comune con spunte; l'utente sceglie
/// quali confrontare, poi passa alla vista affiancata.
fn compare_select_stage(ui: &mut egui::Ui, cmp: &mut CompareState) {
    ui.label("Progetti diversi tra i due file (per tripletta). Scegli quali confrontare:");
    ui.add_space(4.0);

    ui.horizontal(|ui| {
        if ui.button("Seleziona tutto").clicked() {
            for p in cmp.pairs.iter_mut() {
                p.selected = true;
            }
        }
        if ui.button("Deseleziona tutto").clicked() {
            for p in cmp.pairs.iter_mut() {
                p.selected = false;
            }
        }
    });
    ui.add_space(4.0);

    egui::ScrollArea::vertical()
        .max_height(360.0)
        .show(ui, |ui| {
            for p in cmp.pairs.iter_mut() {
                ui.checkbox(&mut p.selected, &p.tripletta);
            }
        });

    ui.add_space(6.0);
    ui.separator();
    let any = cmp.pairs.iter().any(|p| p.selected);
    ui.horizontal(|ui| {
        if ui
            .add_enabled(any, egui::Button::new("Confronta »"))
            .clicked()
        {
            cmp.stage = CompareStage::View;
        }
        if !any {
            ui.label("Seleziona almeno un progetto.");
        }
    });
}

// Dimensioni della vista di confronto.
const CW: f32 = 96.0; // larghezza cella settimana ("Nome|effort", come l'originale)
const NAME_W: f32 = 140.0; // colonna nome dev dentro ciascun pannello
const GUTTER_W: f32 = 66.0; // colonna centrale con le frecce
const PROJ_H: f32 = 28.0; // intestazione progetto
const ROW_H: f32 = 20.0; // riga worker (sotto-riga del blocco dev)
const MS_H: f32 = 36.0; // riga milestone (alta il doppio: nome su due righe)
const HEADER_H: f32 = 20.0; // banda intestazione (date settimane)
const SB: f32 = 12.0; // spessore scrollbar

/// Effort di un worker per un dev in una settimana (0 se assente).
fn eff_at(app: &App, proj: ProjectId, dev: DevId, worker: WorkerId, day: i32) -> usize {
    app.projects
        .get_single_dev(proj, dev)
        .and_then(|sd| sd.get_all(WeekId(day as usize)))
        .and_then(|wk| wk.worker_id.get(&worker))
        .map(|se| se.get_effort().0)
        .unwrap_or(0)
}

/// Unione (ordinata per id) dei worker con effort > 0 per un dev, considerando
/// entrambe le versioni del progetto.
fn dev_workers(
    off: &App,
    off_p: ProjectId,
    par: &App,
    par_p: ProjectId,
    dev: DevId,
    weeks: &[i32],
) -> Vec<WorkerId> {
    let mut ids: Vec<WorkerId> = Vec::new();
    for (a, p) in [(off, off_p), (par, par_p)] {
        if let Some(sd) = a.projects.get_single_dev(p, dev) {
            for w in weeks {
                if let Some(wk) = sd.get_all(WeekId(*w as usize)) {
                    for (id, se) in wk.worker_id.iter() {
                        if *id != WORKER_ID_ZERO && se.get_effort().0 > 0 {
                            ids.push(*id);
                        }
                    }
                }
            }
        }
    }
    ids.sort_by_key(|w| w.0);
    ids.dedup();
    ids
}

/// Barra di scorrimento trascinabile disegnata a mano. Ritorna l'offset
/// aggiornato. Se il contenuto entra tutto (`content <= view`) disegna solo il
/// binario e ritorna 0.
fn scrollbar(
    ui: &mut egui::Ui,
    track: Rect,
    vertical: bool,
    content: f32,
    view: f32,
    mut off: f32,
    id: egui::Id,
) -> f32 {
    ui.painter().rect_filled(track, 2.0, Color32::from_gray(60));
    let track_len = if vertical { track.height() } else { track.width() };
    if content <= view || track_len <= 1.0 {
        return 0.0;
    }
    let max_off = content - view;
    off = off.clamp(0.0, max_off);
    let thumb = (view / content * track_len).max(24.0);
    let start = off / max_off * (track_len - thumb);
    let thumb_rect = if vertical {
        Rect::from_min_size(
            egui::pos2(track.left() + 1.0, track.top() + start),
            egui::vec2(track.width() - 2.0, thumb),
        )
    } else {
        Rect::from_min_size(
            egui::pos2(track.left() + start, track.top() + 1.0),
            egui::vec2(thumb, track.height() - 2.0),
        )
    };
    let r = ui.interact(thumb_rect, id, Sense::drag());
    let col = if r.dragged() || r.hovered() {
        Color32::from_gray(150)
    } else {
        Color32::from_gray(110)
    };
    ui.painter().rect_filled(thumb_rect, 2.0, col);
    if r.dragged() {
        let d = if vertical { r.drag_delta().y } else { r.drag_delta().x };
        off = (off + d / (track_len - thumb).max(1.0) * max_off).clamp(0.0, max_off);
    }
    off
}

/// Tronca `raw` (formato "Nome|effort") perché stia in `max_w`, accorciando il
/// nome e mantenendo il suffisso "|effort" (come `paint_person_cell`).
fn fit_cell(ui: &egui::Ui, raw: &str, font: &egui::FontId, max_w: f32) -> String {
    let m = |s: &str| ui.painter().layout_no_wrap(s.to_string(), font.clone(), Color32::WHITE).size().x;
    if m(raw) <= max_w {
        return raw.to_string();
    }
    if let Some(p) = raw.rfind('|') {
        let (name, suffix) = (&raw[..p], &raw[p..]);
        let mut keep = name.chars().count();
        while keep > 0 {
            let cand = format!("{}…{}", name.chars().take(keep).collect::<String>(), suffix);
            if m(&cand) <= max_w {
                return cand;
            }
            keep -= 1;
        }
        return format!("…{suffix}");
    }
    raw.to_string()
}

/// Effort **stimato** (pianificato) di un dev in un progetto, 0 se assente.
fn planned_of(app: &App, proj: ProjectId, dev: DevId) -> usize {
    app.projects
        .get_single_dev(proj, dev)
        .map(|s| s.planned_effort().0)
        .unwrap_or(0)
}

/// % **dichiarata** corrente di un dev in un progetto, 0 se assente.
fn decl_of(app: &App, proj: ProjectId, dev: DevId) -> u8 {
    app.projects
        .get_single_dev(proj, dev)
        .map(|s| s.declared_pct())
        .unwrap_or(0)
}

/// Milestone collocate in una data settimana per un progetto (ordinate per id,
/// così i due lati confrontano insiemi stabili).
fn ms_at(app: &App, proj: ProjectId, day: i32) -> Vec<MilestoneId> {
    let mut v = app
        .projects
        .project_milestones_at_week(proj, WeekId(day as usize));
    v.sort_by_key(|m| m.0);
    v
}

/// Etichetta italiana di un campo dell'intestazione che differisce.
fn header_label(f: crate::project_diff::HeaderField) -> &'static str {
    use crate::project_diff::HeaderField as H;
    match f {
        H::Info => "descrizione",
        H::StartWeek => "inizio",
        H::EndWeek => "fine",
        H::Tripletta => "tripletta",
        H::Category => "categoria",
        H::Closed => "chiuso",
        H::Milestones => "milestone",
        H::Notes => "note",
    }
}

/// Riga logica della vista di confronto (coordinate di contenuto, 0 = alto).
enum RowKind {
    /// Intestazione di progetto. `header` = campi dell'intestazione che
    /// differiscono (mostrati in rosso; vuoto se differiscono solo i dev).
    Proj { off: ProjectId, par: ProjectId, trip: String, header: Vec<crate::project_diff::HeaderField> },
    /// Riga milestone del progetto: per ogni settimana una cella colorata col
    /// colore della milestone (nome dentro) su entrambi i lati; le settimane in
    /// cui l'insieme di milestone differisce sono bordate di rosso.
    Milestones { off: ProjectId, par: ProjectId },
    /// Blocco di un dev: nome a sinistra/destra, frecce al centro, e una
    /// sotto-riga per ogni worker (`workers`) con celle "Nome|effort" per settimana.
    /// `off_planned`/`par_planned` = effort stimato del dev nei due file (rosso in
    /// vista se differiscono).
    DevBlock {
        off: ProjectId,
        par: ProjectId,
        dev: DevId,
        name: String,
        workers: Vec<WorkerId>,
        off_planned: usize,
        par_planned: usize,
        off_decl: u8,
        par_decl: u8,
    },
}
struct LRow {
    y: f32,
    h: f32,
    kind: RowKind,
}

/// Fase di confronto: due pannelli affiancati (sinistra = ufficiale, destra =
/// parallelo) con scroll sincronizzato e, per ogni dev, due frecce di copia.
fn compare_view_stage(
    ui: &mut egui::Ui,
    app: &App,
    cmp: &mut CompareState,
    actions: &mut Vec<Action>,
) {
    let mut back = false;
    ui.horizontal(|ui| {
        if ui.button("« Torna alla selezione").clicked() {
            back = true;
        }
        ui.separator();
        ui.label("← importa nel file ufficiale");
    });
    if back {
        cmp.stage = CompareStage::Select;
        return;
    }
    ui.add_space(4.0);

    // Coppie selezionate (dati posseduti: nessun borrow di `cmp` trattenuto).
    let sel: Vec<(String, ProjectId, ProjectId)> = cmp
        .pairs
        .iter()
        .filter(|p| p.selected)
        .map(|p| (p.tripletta.clone(), p.off, p.par))
        .collect();

    // Asse settimane condiviso = **unione dei giorni reali** con effort in una
    // qualsiasi delle versioni scelte. Così ogni settimana con effort ha la sua
    // colonna e le due griglie restano allineate (stesso giorno = stessa colonna),
    // senza il rischio di disallineamento di un asse sintetico a passo 7.
    let mut days: std::collections::BTreeSet<i32> = std::collections::BTreeSet::new();
    for (_t, off, par) in &sel {
        if let (Some(a), Some(b)) = (app.projects.get(*off), cmp.other_app.projects.get(*par)) {
            for dd in crate::project_diff::diff_projects(a, b).devs {
                // Solo i dev diversi contribuiscono all'asse dei tempi.
                if dd.status == crate::project_diff::DiffStatus::Identical {
                    continue;
                }
                for (appref, pid) in [(app, *off), (&cmp.other_app, *par)] {
                    if let Some(sd) = appref.projects.get_single_dev(pid, dd.dev) {
                        for w in sd.get_weeks() {
                            days.insert(w.0 as i32);
                        }
                    }
                }
            }
        }
        // Le settimane delle milestone (in entrambe le versioni) entrano
        // nell'asse anche se nessun dev vi ha effort, così la riga milestone ha
        // sempre una colonna in cui disegnarle.
        for (appref, pid) in [(app, *off), (&cmp.other_app, *par)] {
            for (_m, w) in appref.projects.list_project_milestones(pid) {
                days.insert(w.0 as i32);
            }
        }
    }
    // Nota: `weeks` può essere vuoto se i progetti selezionati differiscono solo
    // nell'intestazione (nessun dev diverso): in tal caso non c'è griglia da
    // disegnare ma mostriamo comunque le intestazioni di progetto (col pulsante «).
    let weeks: Vec<i32> = days.into_iter().collect();

    // Layout righe (condiviso tra i due pannelli e la colonna delle frecce).
    let mut rows: Vec<LRow> = Vec::new();
    let mut y = 0.0f32;
    for (trip, off, par) in &sel {
        let (header, devdiffs) = match (app.projects.get(*off), cmp.other_app.projects.get(*par)) {
            (Some(a), Some(b)) => {
                let d = crate::project_diff::diff_projects(a, b);
                (d.header, d.devs)
            }
            _ => (Vec::new(), Vec::new()),
        };
        rows.push(LRow { y, h: PROJ_H, kind: RowKind::Proj { off: *off, par: *par, trip: trip.clone(), header } });
        y += PROJ_H;
        // Riga milestone: presente se una qualsiasi delle due versioni ne ha.
        let has_ms = !app.projects.list_project_milestones(*off).is_empty()
            || !cmp.other_app.projects.list_project_milestones(*par).is_empty();
        if has_ms {
            rows.push(LRow { y, h: MS_H, kind: RowKind::Milestones { off: *off, par: *par } });
            y += MS_H;
        }
        for dd in devdiffs {
            let changed = dd.status != crate::project_diff::DiffStatus::Identical;
            // Mostra solo i dev diversi: quelli identici non compaiono.
            if !changed {
                continue;
            }
            let workers = dev_workers(app, *off, &cmp.other_app, *par, dd.dev, &weeks);
            let off_planned = planned_of(app, *off, dd.dev);
            let par_planned = planned_of(&cmp.other_app, *par, dd.dev);
            let off_decl = decl_of(app, *off, dd.dev);
            let par_decl = decl_of(&cmp.other_app, *par, dd.dev);
            // Altezza del blocco: una sotto-riga per worker (almeno una, così il
            // nome dev e le frecce restano visibili anche senza effort).
            let block_h = (workers.len().max(1) as f32) * ROW_H;
            rows.push(LRow {
                y,
                h: block_h,
                kind: RowKind::DevBlock {
                    off: *off,
                    par: *par,
                    dev: dd.dev,
                    name: dev_name(app, dd.dev),
                    workers,
                    off_planned,
                    par_planned,
                    off_decl,
                    par_decl,
                },
            });
            y += block_h;
        }
    }
    let content_h = y;

    // Area disponibile.
    let full = ui.available_rect_before_wrap();
    let (r, _) = ui.allocate_exact_size(full.size(), Sense::hover());

    let inner_w = r.width() - SB;
    let inner_h = r.height() - SB;
    let panel_w = ((inner_w - GUTTER_W) / 2.0).max(NAME_W + CW);
    let left_x = r.left();
    let gutter_x = left_x + panel_w;
    let right_x = gutter_x + GUTTER_W;
    let l_week_x = left_x + NAME_W;
    let r_week_x = right_x + NAME_W;
    let week_area_w = panel_w - NAME_W;
    let rows_top = r.top() + HEADER_H;
    let viewport_h = inner_h - HEADER_H;
    let content_w = weeks.len() as f32 * CW;
    let max_x = (content_w - week_area_w).max(0.0);
    let max_y = (content_h - viewport_h).max(0.0);

    let (mut sx, mut sy) = (cmp.scroll_x, cmp.scroll_y);

    // Scorrimento: rotella (entrambi gli assi; con Shift la rotella verticale
    // diventa orizzontale) quando il puntatore è sulla vista, più trascinamento
    // dello sfondo per spostarsi anche nel tempo con un mouse senza rotella
    // orizzontale. Le frecce/scrollbar (disegnate dopo) hanno la priorità sui
    // rispettivi rettangoli, quindi il drag panning parte solo dallo sfondo.
    let bg_resp = ui.interact(r, ui.id().with("cmp_view_bg"), Sense::click_and_drag());
    if bg_resp.hovered() {
        let (dx, dy) = ui.input(|i| {
            let d = i.smooth_scroll_delta;
            if i.modifiers.shift && d.x == 0.0 {
                (d.y, 0.0)
            } else {
                (d.x, d.y)
            }
        });
        sx -= dx;
        sy -= dy;
    }
    if bg_resp.dragged() {
        // Non fare panning se si trascina sopra la colonna centrale (dove stanno
        // i bottoni di copia), così il click sulle frecce non viene disturbato.
        let over_gutter = ui
            .input(|i| i.pointer.hover_pos())
            .is_some_and(|pp| pp.x >= gutter_x && pp.x < right_x);
        if !over_gutter {
            sx -= bg_resp.drag_delta().x;
            sy -= bg_resp.drag_delta().y;
        }
    }
    sx = sx.clamp(0.0, max_x);
    sy = sy.clamp(0.0, max_y);

    // Scrollbar (aggiornano sx/sy prima di disegnare le righe).
    let vtrack = Rect::from_min_max(egui::pos2(r.right() - SB, rows_top), egui::pos2(r.right(), r.top() + inner_h));
    let htrack = Rect::from_min_max(egui::pos2(r.left(), r.bottom() - SB), egui::pos2(r.left() + inner_w, r.bottom()));
    sy = scrollbar(ui, vtrack, true, content_h, viewport_h, sy, ui.id().with("cmp_vsb"));
    sx = scrollbar(ui, htrack, false, content_w, week_area_w, sx, ui.id().with("cmp_hsb"));

    // Fonts e colori (più grandi per leggibilità; celle monospaziate come griglia).
    let capf = egui::FontId::proportional(15.0);
    let namef = egui::FontId::proportional(14.0);
    let cellf = egui::FontId::monospace(13.0);
    let smallf = egui::FontId::proportional(13.0);
    let red = Color32::from_rgb(200, 45, 45);
    let sep = Color32::from_gray(130);

    let p = ui.painter_at(r);
    p.rect_filled(r, 0.0, bg());

    // Intestazione: didascalie pannelli + date settimane (scorrono in orizzontale).
    p.text(egui::pos2(left_x + 4.0, r.top() + 2.0), Align2::LEFT_TOP, "Ufficiale", capf.clone(), text());
    p.text(egui::pos2(right_x + 4.0, r.top() + 2.0), Align2::LEFT_TOP, "Parallelo", capf.clone(), text());
    for wx in [l_week_x, r_week_x] {
        let hp = ui.painter_at(Rect::from_min_max(egui::pos2(wx, r.top()), egui::pos2(wx + week_area_w, rows_top)));
        for (i, w) in weeks.iter().enumerate() {
            let cx = wx + i as f32 * CW - sx;
            if cx + CW < wx || cx > wx + week_area_w {
                continue;
            }
            // Etichetta identica alla griglia originale: primo giorno settimana %y-%m-%d.
            let lbl = days_to_local(*w).format("%y-%m-%d").to_string();
            hp.text(egui::pos2(cx + CW / 2.0, r.top() + HEADER_H / 2.0), Align2::CENTER_CENTER, lbl, smallf.clone(), text_dim());
        }
    }

    // Painter clippati per nomi (per pannello) e celle (area settimane).
    let lname = ui.painter_at(Rect::from_min_max(egui::pos2(left_x, rows_top), egui::pos2(left_x + NAME_W, r.top() + inner_h)));
    let rname = ui.painter_at(Rect::from_min_max(egui::pos2(right_x, rows_top), egui::pos2(right_x + NAME_W, r.top() + inner_h)));
    let lcell = ui.painter_at(Rect::from_min_max(egui::pos2(l_week_x, rows_top), egui::pos2(l_week_x + week_area_w, r.top() + inner_h)));
    let rcell = ui.painter_at(Rect::from_min_max(egui::pos2(r_week_x, rows_top), egui::pos2(r_week_x + week_area_w, r.top() + inner_h)));

    // Separatori verticali.
    for x in [l_week_x, gutter_x, right_x, r_week_x] {
        p.vline(x, rows_top..=(r.top() + inner_h), Stroke::new(1.0, sep));
    }

    // Righe.
    for row in &rows {
        let screen_y = rows_top + row.y - sy;
        if screen_y + row.h < rows_top || screen_y > r.top() + inner_h {
            continue;
        }
        match &row.kind {
            RowKind::Proj { off, par, trip, header } => {
                let band = Rect::from_min_max(egui::pos2(r.left(), screen_y), egui::pos2(r.left() + inner_w, screen_y + PROJ_H));
                p.rect_filled(band, 0.0, strip_bg(false));
                lname.text(egui::pos2(left_x + 4.0, screen_y + PROJ_H / 2.0), Align2::LEFT_CENTER, trip, capf.clone(), text());
                rname.text(egui::pos2(right_x + 4.0, screen_y + PROJ_H / 2.0), Align2::LEFT_CENTER, trip, capf.clone(), text());
                // Campi dell'intestazione che differiscono (in rosso), nell'area
                // settimane della riga di progetto.
                if !header.is_empty() {
                    let labels: Vec<&str> = header.iter().map(|f| header_label(*f)).collect();
                    lcell.text(
                        egui::pos2(l_week_x + 6.0, screen_y + PROJ_H / 2.0),
                        Align2::LEFT_CENTER,
                        format!("≠ intestazione: {}", labels.join(", ")),
                        namef.clone(),
                        red,
                    );
                }
                // Bottone "importa l'intero progetto nel file ufficiale" (« =
                // parallelo→ufficiale). Nessuna direzione opposta: il parallelo
                // non viene mai salvato.
                let cy = screen_y + PROJ_H / 2.0;
                let bh = (PROJ_H - 6.0).min(18.0);
                let lb = Rect::from_center_size(egui::pos2(gutter_x + GUTTER_W / 2.0, cy), egui::vec2((GUTTER_W - 14.0).min(42.0), bh));
                let lr = ui
                    .interact(lb, ui.id().with(("cmp_pl", off.0)), Sense::click())
                    .on_hover_text("Importa l'intero progetto nel file ufficiale");
                let fill = if lr.hovered() { Color32::from_gray(120) } else { Color32::from_gray(70) };
                p.rect_filled(lb, 3.0, fill);
                p.text(lb.center(), Align2::CENTER_CENTER, "«", capf.clone(), Color32::WHITE);
                if lr.clicked() {
                    actions.push(Action::CompareCopyProject { off: *off, par: *par, to_official: true });
                }
            }
            RowKind::Milestones { off, par } => {
                // Etichetta di riga in entrambe le colonne nome.
                lname.text(egui::pos2(left_x + 4.0, screen_y + MS_H / 2.0), Align2::LEFT_CENTER, "Milestone", smallf.clone(), text_dim());
                rname.text(egui::pos2(right_x + 4.0, screen_y + MS_H / 2.0), Align2::LEFT_CENTER, "Milestone", smallf.clone(), text_dim());
                for (i, w) in weeks.iter().enumerate() {
                    let cx_l = l_week_x + i as f32 * CW - sx;
                    if cx_l + CW < l_week_x || cx_l > l_week_x + week_area_w {
                        continue;
                    }
                    let cx_r = r_week_x + i as f32 * CW - sx;
                    let off_ms = ms_at(app, *off, *w);
                    let par_ms = ms_at(&cmp.other_app, *par, *w);
                    if off_ms.is_empty() && par_ms.is_empty() {
                        continue;
                    }
                    let diff = off_ms != par_ms;
                    // Cella milestone per un lato: tinta col colore della prima
                    // milestone e nomi al centro, su **due righe** (con «…» se
                    // troppo lunghi) così non sconfinano nelle settimane vicine.
                    // Il testo è nero o bianco a contrasto col colore milestone.
                    let draw_side = |painter: &egui::Painter, cx: f32, ids: &[MilestoneId]| {
                        if ids.is_empty() {
                            return;
                        }
                        let cell = Rect::from_min_size(egui::pos2(cx, screen_y), egui::vec2(CW, MS_H));
                        let fill = ids
                            .first()
                            .and_then(|m| app.milestones.get_color(*m))
                            .map(from_hex)
                            .unwrap_or_else(|| strip_bg(false));
                        painter.rect_filled(cell, 0.0, fill);
                        let names: Vec<String> = ids
                            .iter()
                            .filter_map(|m| app.milestones.get_name(*m).map(|s| s.to_string()))
                            .collect();
                        let txt = contrast_text(fill);
                        let mut job = egui::text::LayoutJob::default();
                        job.wrap = egui::text::TextWrapping {
                            max_width: CW - 6.0,
                            max_rows: 2,
                            break_anywhere: true,
                            overflow_character: Some('…'),
                        };
                        job.halign = egui::Align::Center;
                        job.append(
                            &names.join(", "),
                            0.0,
                            egui::TextFormat { font_id: cellf.clone(), color: txt, ..Default::default() },
                        );
                        let galley = painter.layout_job(job);
                        let pos = egui::pos2(cell.center().x, cell.center().y - galley.size().y / 2.0);
                        painter.galley(pos, galley, txt);
                    };
                    draw_side(&lcell, cx_l, &off_ms);
                    draw_side(&rcell, cx_r, &par_ms);
                    if diff {
                        let sk = egui::StrokeKind::Inside;
                        lcell.rect_stroke(Rect::from_min_size(egui::pos2(cx_l, screen_y), egui::vec2(CW, MS_H)), 0.0, Stroke::new(1.0, red), sk);
                        rcell.rect_stroke(Rect::from_min_size(egui::pos2(cx_r, screen_y), egui::vec2(CW, MS_H)), 0.0, Stroke::new(1.0, red), sk);
                    }
                }
                p.hline(r.left()..=(r.left() + inner_w), screen_y + MS_H, Stroke::new(1.0, Color32::from_gray(70)));
            }
            RowKind::DevBlock { off, par, dev, name, workers, off_planned, par_planned, off_decl, par_decl } => {
                let block_h = row.h;
                // Nome dev in colore normale: sono mostrati solo i dev diversi,
                // quindi il rosso sul nome sarebbe ridondante.
                let col = text();
                lname.text(egui::pos2(left_x + 4.0, screen_y + ROW_H / 2.0), Align2::LEFT_CENTER, name, namef.clone(), col);
                rname.text(egui::pos2(right_x + 4.0, screen_y + ROW_H / 2.0), Align2::LEFT_CENTER, name, namef.clone(), col);

                // A destra nella colonna nome: effort stimato + % dichiarata,
                // ciascuno rosso se differisce tra i due file. La % è al margine
                // destro, l'effort stimato subito a sinistra.
                let ty = screen_y + ROW_H / 2.0;
                let pcol = if off_planned != par_planned { red } else { text_dim() };
                let dcol = if off_decl != par_decl { red } else { text_dim() };
                let ld = lname.text(egui::pos2(left_x + NAME_W - 4.0, ty), Align2::RIGHT_CENTER, format!("{off_decl}%"), namef.clone(), dcol);
                lname.text(egui::pos2(ld.left() - 6.0, ty), Align2::RIGHT_CENTER, off_planned.to_string(), namef.clone(), pcol);
                let rd = rname.text(egui::pos2(right_x + NAME_W - 4.0, ty), Align2::RIGHT_CENTER, format!("{par_decl}%"), namef.clone(), dcol);
                rname.text(egui::pos2(rd.left() - 6.0, ty), Align2::RIGHT_CENTER, par_planned.to_string(), namef.clone(), pcol);

                // Freccia per importare il dev nel file ufficiale (parallelo→
                // ufficiale). Non c'è la direzione opposta: il file parallelo non
                // viene mai salvato. Dopo la copia il dev diventa identico e
                // sparisce dalla vista (si mostrano solo i dev diversi).
                {
                    let cy = screen_y + block_h / 2.0;
                    let lb = Rect::from_center_size(egui::pos2(gutter_x + GUTTER_W / 2.0, cy), egui::vec2((GUTTER_W - 14.0).min(42.0), 18.0));
                    let lr = ui
                        .interact(lb, ui.id().with(("cmp_l", off.0, dev.0)), Sense::click())
                        .on_hover_text("Importa questo dev nel file ufficiale");
                    let fill = if lr.hovered() { Color32::from_gray(120) } else { Color32::from_gray(90) };
                    p.rect_filled(lb, 3.0, fill);
                    p.text(lb.center(), Align2::CENTER_CENTER, "←", namef.clone(), Color32::WHITE);
                    if lr.clicked() {
                        actions.push(Action::CompareCopyDev { off: *off, par: *par, dev: *dev, to_official: true });
                    }
                }

                // Celle "Nome|effort": una sotto-riga per worker (posizione fissa,
                // così i due lati restano allineati e le differenze combaciano).
                for (wi, worker) in workers.iter().enumerate() {
                    let sub_y = screen_y + wi as f32 * ROW_H;
                    let wname = app.workers.get_name_by_id(*worker).to_string();
                    for (i, w) in weeks.iter().enumerate() {
                        let cx_l = l_week_x + i as f32 * CW - sx;
                        if cx_l + CW < l_week_x || cx_l > l_week_x + week_area_w {
                            continue;
                        }
                        let cx_r = r_week_x + i as f32 * CW - sx;
                        let off_e = eff_at(app, *off, *dev, *worker, *w);
                        let par_e = eff_at(&cmp.other_app, *par, *dev, *worker, *w);
                        let diff = off_e != par_e;
                        let cc = if diff { red } else { text() };
                        if off_e > 0 {
                            let s = fit_cell(ui, &format!("{wname}|{off_e}"), &cellf, CW - 6.0);
                            lcell.text(egui::pos2(cx_l + CW / 2.0, sub_y + ROW_H / 2.0), Align2::CENTER_CENTER, s, cellf.clone(), cc);
                        }
                        if par_e > 0 {
                            let s = fit_cell(ui, &format!("{wname}|{par_e}"), &cellf, CW - 6.0);
                            rcell.text(egui::pos2(cx_r + CW / 2.0, sub_y + ROW_H / 2.0), Align2::CENTER_CENTER, s, cellf.clone(), cc);
                        }
                        if diff {
                            let sk = egui::StrokeKind::Inside;
                            lcell.rect_stroke(Rect::from_min_size(egui::pos2(cx_l, sub_y), egui::vec2(CW, ROW_H)), 0.0, Stroke::new(1.0, red), sk);
                            rcell.rect_stroke(Rect::from_min_size(egui::pos2(cx_r, sub_y), egui::vec2(CW, ROW_H)), 0.0, Stroke::new(1.0, red), sk);
                        }
                    }
                }

                // Riga separatrice sotto il blocco dev.
                p.hline(r.left()..=(r.left() + inner_w), screen_y + block_h, Stroke::new(1.0, Color32::from_gray(70)));
            }
        }
    }

    cmp.scroll_x = sx;
    cmp.scroll_y = sy;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn common_pairs_match_by_tripletta_only() {
        let mut off = App::new();
        off.projects.add("P1", Some("ABC"), None);
        off.projects.add("P2", Some("XYZ"), None);
        off.projects.add("SenzaTripletta", None, None);
        let mut par = App::new();
        par.projects.add("Copia di P1", Some("ABC"), None); // stessa tripletta
        par.projects.add("Altro", Some("QQQ"), None); // solo nel parallelo

        let pairs = common_project_pairs(&off, &par);
        assert_eq!(pairs.len(), 1, "solo ABC è in comune");
        assert_eq!(pairs[0].tripletta, "ABC");
        assert!(!pairs[0].selected, "le coppie partono deselezionate");
    }

    #[test]
    fn copy_dev_overwrites_official_dev() {
        let mut off = App::new();
        let d = off.devs.add("Dev");
        let pid = off.projects.add("P", Some("ABC"), Some(WeekId(100)));
        off.projects.add_dev(pid, d);
        off.projects.add_effort(pid, d, WeekId(100), WorkerId(0), Effort(8));

        // File parallelo (id coincidenti): stesso dev con effort diverso.
        let mut par = App::new();
        let d2 = par.devs.add("Dev");
        let pid2 = par.projects.add("P", Some("ABC"), Some(WeekId(100)));
        par.projects.add_dev(pid2, d2);
        par.projects.add_effort(pid2, d2, WeekId(100), WorkerId(0), Effort(20));

        let src = par.projects.get_single_dev(pid2, d2).cloned();
        copy_dev_into(&mut off, pid, d, src);
        assert_eq!(
            off.projects.get_single_dev(pid, d).unwrap().get_effort_by_week(WeekId(100)),
            Effort(20),
        );
    }

    #[test]
    fn copy_dev_with_none_removes_it() {
        let mut off = App::new();
        let d = off.devs.add("Dev");
        let pid = off.projects.add("P", Some("ABC"), Some(WeekId(100)));
        off.projects.add_dev(pid, d);
        off.projects.add_effort(pid, d, WeekId(100), WorkerId(0), Effort(8));

        copy_dev_into(&mut off, pid, d, None);
        assert!(off.projects.get_single_dev(pid, d).is_none());
    }
}
