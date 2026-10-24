# Revisione del codice

Analisi del codebase alla ricerca di codice **duplicato, obsoleto, ridondante o
non ottimizzato**. Ogni voce ha un codice stabile (es. «facciamo C1»). Nessuna
voce si esegue senza approvazione.

**Come si usa**: quando una voce è fatta, si spunta (`- [x]`), si aggiunge una
nota su cosa è stato fatto e si aggiornano i numeri qui sotto. Il file va
aggiornato **a ogni modifica** che tocca una voce (anche parziale).

Qui sta solo la **qualità interna del codice**. Le nuove **funzionalità e la UX**
sono in [`MIGLIORAMENTI.md`](MIGLIORAMENTI.md).

Legenda impatto/rischio: **Alto** / **Medio** / **Basso** / **Nullo**.

## Stato attuale

| Misura | Inizio (2026-10-24) | Ora |
|---|---|---|
| Righe Rust | ~28.000 | ~28.000 |
| Warning `cargo build` | 82 | **0** |
| Warning `cargo clippy` | ~135 | 53 (stile → E3) |
| Test | 275 verdi | 280 verdi |

## Ordine consigliato

1. ~~**A1, A3–A7, D3** — pulizia a rischio nullo~~ ✅ fatto
2. ~~**D1, D2** — bug latenti~~ ✅ fatto
3. ~~**C1, C2** — il guadagno di velocità più visibile~~ ✅ fatto
4. ~~**A2** — migrazione API egui deprecate~~ ✅ fatto
5. **B2–B7, C3–C5** — duplicati e ottimizzazioni minori
6. **B1, B9, E1–E3** — riordino
7. **B10** — a parte: la modifica più invasiva

---

## A. Codice obsoleto o morto

- [x] **A1. 15 warning "falling back to `f32`"** — diventeranno **errori** in una
  futura versione di Rust. In `compare.rs` (7), `grid.rs` (3), `saturation.rs` (3),
  `export.rs` (2). _Impatto: Alto · Rischio: Nullo._
  _(fatto: letterali `1.0_f32`/`2.0_f32`… nei `Stroke::new`; 0 warning rimasti)_

- [x] **A2. 50 usi di API egui deprecate** — `close_menu` (35: 31 in `toolbar.rs`,
  4 in `grid.rs`), `SidePanel`/`TopBottomPanel` (8), `exact_width` (3),
  `screen_rect` (4), `enable_scrolling` (3), `SelectableLabel` (2), `menu::bar`,
  `allocate_new_ui`, `style_mut`, `default_width`. Spariranno al prossimo
  aggiornamento di egui. Proposta: passare alle API nuove (sostituzione 1:1; i test
  dei menù lo coprono). _Impatto: Medio · Rischio: Basso._
  _(fatto: verificato nel sorgente di egui 0.34.3 che ogni funzione deprecata chiama
  la nuova, quindi il comportamento è identico: `close_menu()` →
  `close_kind(UiKind::Menu)` (non `close()`, che chiuderebbe il contenitore più
  vicino), `SidePanel`/`TopBottomPanel` → `Panel`, `exact_width`/`default_width` →
  `exact_size`/`default_size`, `enable_scrolling(false)` →
  `scroll_source(ScrollSource::NONE)`, `screen_rect` → `content_rect`,
  `SelectableLabel::new` → `Button::selectable`, `allocate_new_ui` →
  `scope_builder`, `style_mut` → `global_style_mut`. **Unica differenza**:
  `menu::bar` → `MenuBar`: con un menù aperto, passando il mouse su un altro menù
  della barra si apre quello (come nelle barre dei menù normali); lo stile è lo
  stesso. Migrati anche i 13 usi deprecati **nei test** (`Context::run` →
  `run_ui`, `Panel::show(ctx)` → `show_inside`), non conteggiati all'inizio.
  Build: 0 warning)_

- [x] **A3. `#![allow(unused)]` / `#![allow(dead_code)]` in 10 file del modello**
  (`projects.rs`, `project.rs`, `single_dev.rs`, `worker.rs`, `workers.rs`,
  `devs.rs`, `dev.rs`, `sinlge_effort.rs`, `sinlge_effort_week.rs`, `milestones.rs`)
  — nascondevano il codice morto. _Impatto: Medio · Rischio: Nullo._
  _(fatto: direttive tolte; emersi e rimossi 3 import inutili e una variabile
  inutilizzata in un test)_

- [x] **A4. Codice morto nel modello** (emerso da A3). _Impatto: Basso · Rischio: Basso._
  _(fatto: nessuna funzione era del tutto inutilizzata, ognuna è usata almeno da un
  test → tutte marcate `#[cfg(test)]`: `MilestoneScope::label`,
  `Milestone::in_scope`, `is_global`, `Milestones::add`/`add_with_kind`/`get`/
  `has_category`/`purge_project`, `Project::new`, `Projects::set_closed`,
  `Worker::set_name`/`set_bg_color`/`set_font_color`, `Workers::del`)_

- [x] **A5. Wrapper usati solo dai test** — `project_shapes`, `build_pdf`,
  `build_pdf_project`, `build_svg_project` (`pdf_export.rs`),
  `ProjectDiff::changed_devs` (`project_diff.rs`). _Impatto: Basso · Rischio: Nullo._
  _(fatto: marcati `#[cfg(test)]`, nota in `CLAUDE.md`)_

- [x] **A6. Codice commentato** — test in `dates.rs`, `reset_effort` in
  `sinlge_effort_week.rs`, un `if` in `project.rs`. _(fatto: rimossi)_

- [x] **A7. Commenti obsoleti** — riferimenti a Slint (`ui_style.rs`, `ui/mod.rs`);
  esempi di documentazione in `dates.rs` che citavano un crate `project_app`
  inesistente, e il commento di `get_default_weeks` attaccato a `parse_date_str`.
  _(fatto: commenti aggiornati; il codice di `dates.rs` verificato identico riga per
  riga)_

## B. Codice duplicato

- [ ] **B1. Export PDF ripetuto** — `build_pdf` e `build_pdf_selected` quasi
  identici (~50 righe ciascuno); la preparazione del documento (font regular/bold,
  `PdfDocument`, salvataggio) è ripetuta **6 volte** in `pdf_export.rs`. Proposta:
  helper `new_doc()` / `save_doc(pages)` e un unico `build_pdf_filtered`.

- [ ] **B2. Data della settimana** — `primo_giorno_settimana_corrente(&days_to_local(w)).format("%y-%m-%d")`
  ripetuto **21 volte** in 8 file. Proposta: helper `week_label(w)`.

- [ ] **B3. Lettura di `"Worker|ore"`** — `split('|')` ripetuto 7 volte (6 in
  `grid.rs`). Proposta: helper `cell_worker(text)` / `parse_cell(text)`.

- [ ] **B4. Ricerca per testo** — `matches(q, hay)` esiste in `filters.rs` ma è
  riscritto a mano in `dialogs.rs` (inserimento multiplo, clona progetto, finestra
  Worker) e in `matrix.rs`. Proposta: renderlo condiviso.

- [ ] **B5. Finestre di conferma** — rimozione dev, cancella effort, cancella
  milestone hanno lo stesso scheletro. Proposta: unico
  `confirm_window(ctx, titolo, testo, etichetta_ok) -> Option<bool>`.

- [ ] **B6. Ricerca nomi tramite liste complete** — `dev_name` ordina **tutti** i
  dev per trovarne uno; `pdf_export::project_name` fa lo stesso con tutti i progetti.
  Proposta: accesso diretto per id (`Devs::get_name(id)`, `get_info(proj)`).

- [ ] **B7. Due `project_label` con regole diverse** (`ui/mod.rs` e
  `sync_merge.rs`): lo stesso progetto può avere nomi diversi in finestre diverse.
  Proposta: una sola funzione.

- [ ] **B8. Conversioni per jiff** (richiesto dal date-picker di egui) sparse tra
  `ui/mod.rs` e `matrix.rs`. Proposta: riunirle in `date_utils`.

- [ ] **B9. Test** — sequenza di click (sposta/premi/rilascia) copiata ~11 volte,
  estrazione dei testi disegnati ~19, preparazione dello schermo 29. Proposta:
  modulo di helper per i test (`click_at`, `drawn_texts`, `screen`).

- [ ] **B10. 9 opzioni di export come stato globale** (thread-local:
  `set_show_pct`, `set_show_year`, scope, allow-list, ghost, ghost_red, corner,
  range…) impostate prima di ogni `build_*` e azzerate a mano. Proposta: struttura
  `ExportOptions` passata come parametro. _Tocca tutte le firme dell'export._

## C. Prestazioni (lavoro ripetuto a ogni frame)

- [x] **C1. La griglia elabora tutto a ogni frame** — ogni progetto × dev ×
  settimana (~104 colonne), anche fuori schermo; per ogni cella interazione,
  stringhe, ricerche nomi. Nessun taglio sulla parte visibile (nessun `clip_rect` in
  `grid.rs`). **Il collo di bottiglia principale.** Proposta: saltare righe e
  colonne fuori vista, mantenendo il calcolo del cumulativo.
  _(fatto: `grid` salta i blocchi dev fuori dall'area visibile (+ margine di una
  colonna, `CULL_MARGIN`) e `draw_dev_cells` le colonne fuori vista, sommando però
  le loro ore al cumulativo. Il blocco/colonna con la cella in modifica si disegna
  sempre, altrimenti l'editor perderebbe il focus e confermerebbe da solo. La
  colonna sinistra non è tagliata: ha campi modificabili e costa poco (una riga per
  dev, non per settimana). **Misura** con il `workers.ron` reale (29 progetti,
  153 dev, 158 colonne, schermo 1600×900, build release): **10,4 → 0,5 ms per
  frame** per la griglia. Test `grid_skips_offscreen_columns_but_keeps_the_cumulative`,
  `grid_keeps_drawing_the_cell_being_edited_when_scrolled_away`; entrambi falliscono
  se si toglie il taglio o l'eccezione per la cella in modifica)_

- [x] **C2. `project_layout` calcolato due volte per frame** (colonna sinistra e
  griglia; tre col salto a progetto). Proposta: calcolarlo una volta in `body`.
  _(fatto: `body_layout` lo calcola una volta in `body` e lo passa a
  `left_column`, `grid` e al salto a progetto; `grid` non riceve più `dev_filter`,
  che gli serviva solo per il layout)_

- [ ] **C3. Ricerche ripetute per cella** — `get_id_by_name` scorre tutti i worker,
  due volte per cella; in `draw_left_devs` lo stesso dev viene cercato ~5 volte;
  `dev_name` ordina la lista dei dev a ogni chiamata. Proposta: mappa nome→id per
  frame, una sola lettura di `get_single_dev`.

- [ ] **C4. Saturazione** — `saturation_cell` calcolata **3 volte** per ogni
  worker × colonna a ogni frame (riepilogo, Σ, celle). Proposta: una matrice per frame.

- [ ] **C5. Footer** — `worker_remaining_hours` ricostruisce `weeks_vec` per ogni
  worker a ogni frame; `Workers::list()` copia e ordina tutti i nomi a ogni
  chiamata. Proposta: settimane calcolate una volta per frame.

## D. Incoerenze e possibili bug

- [x] **D1. «Oggi» calcolato in modi diversi** — `UiState.this_week` (footer,
  saturazione) è calcolato **solo all'avvio**, `current_week_id()` (griglia) è sempre
  aggiornato: se l'app resta aperta oltre il lunedì, griglia e footer segnano
  settimane diverse. Inoltre la griglia usa **UTC** e il PDF l'ora **locale**: in
  Italia, il lunedì tra le 00:00 e le 01:00/02:00, la griglia mostra ancora la
  settimana precedente. Proposta: un'unica funzione «settimana di oggi», in ora
  locale, ricalcolata a ogni frame.
  _(fatto: in `date_utils::dates` le funzioni `today()` (data locale),
  `today_days()` e `current_monday_days()` sono l'unica fonte di «oggi»;
  `current_week_id()`, il default delle date, «Nuovo progetto», minuta, matrice e
  tutti gli export PDF passano da lì; nessun `Utc::now` rimasto. `UiState.this_week`
  è ricalcolato in cima a ogni frame. Restano su `Local::now()` solo i timestamp con
  l'ora: commit git, nome file, ora dei toast. Test
  `today_and_current_monday_agree`, `grid_and_footer_share_the_same_current_week`)_

- [x] **D2. `unwrap()` che possono far crashare l'app** se l'id di un progetto o di
  un dev non esiste più (es. dopo un ricaricamento esterno): `Projects::del_row`/
  `add_dev`/`del_dev`/`list_devs`, `Project::del_row`/`reset_effort`/`set_effort`…
  Proposta: no-op sicuri.
  _(fatto: `Projects::del_row`/`add_dev`/`del_dev`/`add_effort`/`add_dev_effort`
  ignorano un progetto inesistente, `list_devs` torna vuoto; `Project::del_row`/
  `reset_effort` ignorano un dev inesistente, `get_week_with_max_worker` torna
  `None`; gli `unwrap()` dopo `add_dev` (sicuri ma fragili) sostituiti da
  `Project::dev_mut` (crea il dev se manca). Nessun `unwrap()` rimasto nel modello
  fuori dai test. Verificati anche quelli dell'interfaccia e del PDF andamento: sono
  protetti da un controllo subito prima. Test
  `calls_on_missing_project_or_dev_do_not_panic`)_

- [x] **D3. `MIGLIORAMENTI.md` #14** diceva «ferie/malattia = capacità 0», il codice
  fa il contrario. _(fatto: testo corretto)_

## E. Struttura (facoltativo)

- [ ] **E1.** `ui/mod.rs` ha ~6.300 righe, di cui ~2.480 di test: spostarli in
  `src/ui/tests.rs`.
- [ ] **E2.** Refusi nei nomi dei file: `sinlge_effort.rs`,
  `sinlge_effort_week.rs` → `single_…`.
- [ ] **E3.** Stile clippy (53 warning nel codice, ~85 contando i test): ~32 `if`
  annidabili, 21 assegnazioni dopo `Default::default()` (test), `mut` inutili, 4
  `run_ui` con risultato ignorato nei test, ecc. Sistemabili in modo meccanico con
  `cargo clippy --fix`. Nota: il repo **non** è formattato con `rustfmt` (152
  differenze già all'ultimo commit): un `cargo fmt` globale toccherebbe molti file,
  va deciso a parte.
- [ ] **E4.** Nella radice ~10 copie/backup di `.ron` (già ignorati da git). Dati
  dell'utente: **non si toccano** senza richiesta esplicita.

## Fatti in precedenza (da `MIGLIORAMENTI.md`)

Voci di qualità del codice nate nella roadmap prima di questo report, spostate qui
con il loro numero originale (M = `MIGLIORAMENTI.md`).

- [x] **M15. Fixare il test rosso** `get_name_by_id_not_found_returns_empty` — rosso da prima ("unknown" vs ""); decidere il comportamento e allineare. _(fatto: `Workers::get_name_by_id` ora ritorna `""` per un id inesistente invece di `"unknown"`, coerente con `WORKER_ID_ZERO` (anch'esso `""`) ed evita che una stringa spuria finisca in UI/filtri; test verde)_
- [x] **M16. Spezzare `ui.rs` in moduli** (toolbar, grid, footer, dialogs) — migliora navigabilità/manutenzione. _(fatto: `ui.rs` (6550 righe) → cartella `src/ui/` con `mod.rs` (nucleo: `UiState`/`PjmApp`/`Action`/`ui()` + tipi + helper condivisi + test) e i sottomoduli `toolbar`, `grid`, `footer`, `dialogs`, `export`, `help`, `saturation`. Solo funzioni spostate (i tipi restano in `mod.rs`); i figli accedono al nucleo via `use super::*`, il padre ai figli via re-export `pub(crate) use`. Stessi test verdi.)_
- [x] **M17. Più test sulla logica** (merge a 3 vie, effort, layout) _(fatto: da 128 a 165 test. **Merge a 3 vie** (`sync_merge.rs`, 5→15): conflitto sui dati generali del progetto e stessa modifica identica su entrambi i lati, intestazione del collega + mio dev che convivono, progetto cancellato dal collega / cancellato da me e modificato da lui, dev aggiunto dal collega, messaggio di conflitto che nomina progetto e dev, globali prese in blocco (fissa la perdita del worker del collega, comportamento noto) e stato ricalcolato dopo il merge. **Effort**: `sinlge_effort_week.rs` da 0 a 5 test (`add` sostituisce e non somma, la nota sopravvive alla correzione delle ore, `remove_zero`); `single_dev.rs` 6→15 (`contiguous_block` con buchi e settimane a zero, `shift_weeks`: troncamento ai bordi, mai sotto zero, **unione** dei worker in collisione; pianificato vs assegnato, `max_num_efforts`, `effort_span`); `project.rs` +3 (presunta oltre il 100% = sforamento, dev pianificato ma mai lavorato, `progress_breakdown` come sorgente unica). **Layout** (`ui/mod.rs`, prima a copertura zero): `columns_vec` a zoom 0/1/2 senza settimane perse o duplicate, confine d'anno che spezza i gruppi e inserisce la colonna canarino, offset colonne coerenti con la larghezza totale, `centered_scroll_x` mai negativo, `dev_inner_h`/`dev_block_height` per modalità, e `project_layout` in una UI headless (altezze che quadrano con `total_content_h`, header compresso col filtro, progetti senza worker che spariscono, compatta più bassa della normale). Le asserzioni chiave sono state verificate con mutation check: rompendo `zoom_group`, la scelta dell'intestazione in conflitto e l'unione in collisione, i rispettivi test falliscono)_
- [x] **M18. Dedup "Select All" nei dialog** — il pattern "checkbox Select All + lista selezionabile", prima copiato in 8 finestre, è stato estratto in helper condivisi. _(fatto: `select_all_checkbox` — primitiva usata da tutte le 8 sedi; `project_checklist` — blocco lista progetti condiviso da `pdf_multi_export_window` e `minuta_window`, con dimensionamento adattivo via `available_height()` + `auto_shrink` invece del margine fisso sullo schermo; test headless `project_checklist_shrinks_and_caps`)_

---

## Registro delle modifiche

- **2026-10-24** — Report iniziale.
- **2026-10-24** — Fatto il punto 1 dell'ordine consigliato (A1, A3, A4, A5, A6, A7,
  D3). Warning build 82 → 61, clippy ~135 → 114, 275 test verdi.
- **2026-10-24** — Spostate qui da `MIGLIORAMENTI.md` le voci di qualità del codice
  già fatte (#15–#18 → M15–M18); aggiunti i rimandi tra i due file.
- **2026-10-24** — Fatto il punto 2 dell'ordine consigliato (D1, D2). Warning build
  61, clippy 114 (invariati), 278 test verdi (+3).
- **2026-10-24** — Fatto il punto 3 dell'ordine consigliato (C1, C2). Griglia
  10,4 → 0,5 ms per frame sul file reale. Warning build 61, clippy 114 (invariati),
  280 test verdi (+2).
- **2026-10-24** — Fatto il punto 4 dell'ordine consigliato (A2), compresi gli usi
  deprecati nei test. Warning build 61 → **0**, clippy 114 → 53, 280 test verdi.
