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
| Warning `cargo clippy` | ~135 | **0** (anche sui test) |
| Test | 275 verdi | 290 verdi |

## Ordine consigliato

1. ~~**A1, A3–A7, D3** — pulizia a rischio nullo~~ ✅ fatto
2. ~~**D1, D2** — bug latenti~~ ✅ fatto
3. ~~**C1, C2** — il guadagno di velocità più visibile~~ ✅ fatto
4. ~~**A2** — migrazione API egui deprecate~~ ✅ fatto
5. ~~**B2–B7, C3–C5** — duplicati e ottimizzazioni minori, in due blocchi:
   5a: B2–B7 (duplicati) · 5b: C3–C5 (ottimizzazioni)~~ ✅ fatto
6. ~~**B1, B9, E1–E3** — riordino~~ ✅ fatto
7. ~~**B10** — a parte: la modifica più invasiva~~ ✅ fatto

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

- [x] **A8. Aggiornamento egui 0.34 → 0.37** (eframe, egui_extras 0.37.0;
  egui_commonmark 0.23 → 0.26). Misurato su una copia: 7 errori di compilazione
  (`scroll_area::State.offset` privato → `offset()`, 2 cambi sugli `Id`, un campo
  nuovo negli eventi dei test) e 52 API deprecate (25 `Id::new` →
  `Id::unique`/`IdSalt::new`, 14 `show_inside` → `show`, 11 `ui.id()` →
  `scope_id()`, 2 `horizontal_align` → `align`). Rischi: nuovo motore del testo
  (harfrust/skrifa/vello_cpu → misure dei testi), scroll sincronizzato
  (API dell'offset cambiata), scelta degli `Id` (stato di menù/scroll).
  Piano: commit + branch dedicato, errori, deprecazioni fino a 0 warning, test,
  **prova a mano** dell'interfaccia. _Impatto: Medio · Rischio: Medio._
  _(approvato il 2026-10-24)_
  _(fatto, branch `egui-0.37`: punti 1–5. **Errori**: offset → 
  `clamped_offset()`, salt `impl Hash + Debug`, campo `source` negli eventi dei
  test. **Deprecazioni**: 52 → 0 (`Id::new` = `Id::unique` in 0.37, quindi
  comportamento identico; `scope_id`, `show`, `align(Align2::CENTER_TOP)`).
  **Scoperto durante il lavoro**: (1) egui 0.37 attiva il «rimbalzo» dello scroll:
  disattivato, altrimenti intestazione/footer/colonna sinistra si disallineano;
  (2) egui 0.37 non carica più i font con simboli/emoji → attivata l'opzione
  `monochrome_emoji_fonts` (stessi font e regolazioni di 0.34), altrimenti ℹ ⚠ ⛔
  ✔ ⚑ ⚡ ⌘ 🔎 🗑 sarebbero quadratini; (3) **difetto già presente in 0.34**: «✕»
  non c'è in nessun font di egui, quindi i pulsanti «Pulisci» (manuale) e «Chiudi»
  (toast) mostravano un quadratino → sostituito con «✖»; nuovo test
  `every_ui_symbol_is_renderable` che controlla tutti i simboli delle stringhe;
  (4) nuovo controllo di egui 0.37 sui test (texture non applicate) → helper
  `run_ui_test`; (5) **segnalato dall'utente nella prova a mano**: lo scroll
  trascinando col mouse (tasto destro e trascina, usato ovunque) non funzionava
  più, perché in 0.37 il trascinamento del contenuto vale di default solo sui
  touch screen → `drag_scroll()` su tutte le 27 aree di scorrimento dell'utente
  (le 5 che seguono un'altra area restano escluse), test
  `right_drag_scrolls_the_grid_both_ways`. Build e clippy 0 warning, 289 test
  verdi. **Prova a mano superata** (confermata dall'utente il 2026-10-24).)_

- [x] **A9. Aggiornamento printpdf 0.9.1 → 0.12.8.** Misurato su una copia:
  nessuna modifica al codice (0 errori, 0 warning), 290 test verdi, aspetto
  identico (prima pagina di Gantt/report/andamento: 7–83 pixel diversi su 500.000,
  bordi dei caratteri), stesso testo cercabile (nel Gantt stesse 1.720 parole, in
  ordine di lettura diverso), PDF **10–40 volte più piccoli** (Gantt 2,1 MB →
  180 KB: solo i caratteri usati del font vengono incorporati), dipendenze
  589 → 542. _Impatto: Basso · Rischio: Basso._ _(approvato il 2026-10-24)_
  _(applicato sul branch `egui-0.37`: cambiata solo la versione in `Cargo.toml`
  (+ `Cargo.lock`); build e clippy 0 warning, 290 test verdi. **Prova a mano
  superata** (confermata dall'utente il 2026-10-24).)_

- [x] **A10. Aggiornamento delle dipendenze indirette** (`cargo update`, solo
  versioni compatibili). Le 10 dipendenze dirette sono già all'ultima versione;
  disponibili 35 aggiornamenti compatibili di dipendenze indirette (tokio, uuid,
  smallvec, zerovec…) e la rimozione di 9 pacchetti Windows non più usati.
  `cargo audit` (542 pacchetti): **nessuna vulnerabilità**; 6 avvisi «non più
  mantenuto» su dipendenze indirette (bincode, ttf-parser, zbus_names, zvariant,
  zvariant_derive, clipboard-win), non risolvibili da qui: arrivano da egui, rfd
  e printpdf. Prova a mano: dialog Apri/Salva/Esporta, copia/incolla delle celle.
  _Impatto: Basso · Rischio: Basso._ _(approvato il 2026-10-24)_
  _(applicato sul branch `egui-0.37`: `cargo update`, cambia solo `Cargo.lock`;
  pacchetti 542 → 532, nessun altro aggiornamento compatibile rimasto. Build e
  clippy 0 warning, 290 test verdi. `cargo audit`: nessuna vulnerabilità, avvisi
  «non più mantenuto» 6 → 5 (sparito `clipboard-win`). **Prova a mano superata**
  (dialog Apri/Salva/Esporta, copia/incolla; confermata dall'utente il 2026-10-24).)_

## B. Codice duplicato

- [x] **B1. Export PDF ripetuto** — `build_pdf` e `build_pdf_selected` quasi
  identici (~50 righe ciascuno); la preparazione del documento (font regular/bold,
  `PdfDocument`, salvataggio) è ripetuta **6 volte** in `pdf_export.rs`. Proposta:
  helper `new_doc()` / `save_doc(pages)` e un unico `build_pdf_filtered`.
  _(fatto: `pdf_document(title, &pages)` è l'unico punto che crea il PDF (font,
  pagine, salvataggio; `None` senza pagine) e lo usano tutti gli export: Gantt,
  progetto singolo, andamento, report, matrice. `build_pdf` e `build_pdf_selected`
  passano da `gantt_pdf(app, bar, filtro)`. Report e matrice prima salvavano anche
  con zero pagine: verificato che non può succedere (almeno una pagina per
  progetto / per blocco di worker), quindi il comportamento non cambia)_

- [x] **B2. Data della settimana** — `primo_giorno_settimana_corrente(&days_to_local(w)).format("%y-%m-%d")`
  ripetuto **21 volte** in 8 file. Proposta: helper `week_label(w)`.
  _(fatto: `date_utils::dates::week_label(days)` (lunedì in `yy-mm-dd`, inverso di
  `parse_date_str`) usato nei 15 punti dell'applicazione; restano a parte il titolo
  della minuta (è la data di oggi, non una settimana) e i test, che formattano da
  sé per controllare in modo indipendente. Test
  `week_label_is_the_monday_and_round_trips_with_parse`)_

- [x] **B3. Lettura di `"Worker|ore"`** — `split('|')` ripetuto 7 volte (6 in
  `grid.rs`). Proposta: helper `cell_worker(text)` / `parse_cell(text)`.
  _(fatto: `cell_worker(text)` (nome) nei 5 punti di `grid.rs` e `parse_cell(text)`
  (nome + ore, `None` se non c'è esattamente un `|`) in `commit_editing` e
  nell'azione `CommitCell`; i controlli propri di ciascuno (nome non vuoto, worker
  esistente) restano lì. Test `cell_text_parsing_matches_the_old_split_rules`
  confronta i due helper con la vecchia regola su 9 casi limite)_

- [x] **B4. Ricerca per testo** — `matches(q, hay)` esiste in `filters.rs` ma è
  riscritto a mano in `dialogs.rs` (inserimento multiplo, clona progetto, finestra
  Worker) e in `matrix.rs`. Proposta: renderlo condiviso.
  _(fatto: `search_matches(q, testo)` in `filters.rs`, condivisa; usata da tutti i
  filtri, dall'inserimento multiplo, da «Clona progetto», dalla finestra Worker,
  dalla matrice e dalla ricerca nel manuale)_

- [x] **B5. Finestre di conferma** — rimozione dev, cancella effort, cancella
  milestone hanno lo stesso scheletro. Proposta: unico
  `confirm_window(ctx, titolo, testo, etichetta_ok) -> Option<bool>`.
  _(fatto: `confirm_window(ctx, titolo, etichetta_ok, contenuto) -> Option<bool>`
  in `dialogs.rs` (`Some(true)` conferma, `Some(false)` Annulla o ✕, `None` ancora
  aperta); ci passano rimozione dev, cancella effort, cancella milestone e anche
  `bound_move_window`. Test `confirm_window_reports_ok_cancel_or_still_open` più i
  test esistenti delle quattro finestre)_

- [x] **B6. Ricerca nomi tramite liste complete** — `dev_name` ordina **tutti** i
  dev per trovarne uno; `pdf_export::project_name` fa lo stesso con tutti i progetti.
  Proposta: accesso diretto per id (`Devs::get_name(id)`, `get_info(proj)`).
  _(fatto: nuovo `Devs::get(id)`; `dev_name`, e anche `dev_color` e
  `dev_text_color` che facevano lo stesso con `list_full`, leggono il dev
  direttamente; `pdf_export::project_name` usa `Projects::get_info`)_

- [x] **B7. Due `project_label` con regole diverse** (`ui/mod.rs` e
  `sync_merge.rs`): lo stesso progetto può avere nomi diversi in finestre diverse.
  Proposta: una sola funzione.
  _(fatto: regola unica `Project::label(id)`: tripletta → prima riga non vuota
  della descrizione → «Progetto N». `ui::project_label` e i messaggi di conflitto
  di `sync_merge` la usano. Effetti: negli elenchi un progetto senza tripletta e
  con descrizione su più righe mostra solo la prima; i messaggi di conflitto
  nominano sempre il progetto (prima potevano restare senza nome). Test
  `label_is_tripletta_then_first_description_line_then_number`)_

- [x] **B8. Conversioni per jiff** (richiesto dal date-picker di egui) sparse tra
  `ui/mod.rs` e `matrix.rs`. Proposta: riunirle in `date_utils`.
  _(fatto: nuovo modulo `date_utils::picker` con `today_jiff`, `days_to_jiff`,
  `jiff_to_days`, `date_str_to_jiff`, `jiff_to_date_str`, costruite su due sole
  conversioni chrono ⇄ jiff. La lettura `yy-mm-dd`/`yyyy-mm-dd` non è più
  duplicata: `parse_date_str` e il calendario usano lo stesso `parse_ymd`. Unica
  differenza: un anno fuori dall'intervallo di jiff (±9999) ora dà `None` invece
  di finire troncato. Test
  `conversions_round_trip_and_parse_like_the_rest_of_the_app`)_

- [x] **B9. Test** — sequenza di click (sposta/premi/rilascia) copiata ~11 volte,
  estrazione dei testi disegnati ~19, preparazione dello schermo 29. Proposta:
  modulo di helper per i test (`click_at`, `drawn_texts`, `screen`).
  _(fatto: `src/ui/test_util.rs` (solo test) con `screen(w, h)`,
  `click_events(pos, tasto)`, `drawn_texts(out)` e `text_pos(out, testo)` (spostato
  da `tests.rs`). Usati nei 6 array di click copiati, in `queue_click`/
  `queue_right_click`, in 5 estrazioni di testo e in 29 definizioni di schermo,
  anche nei test di `saturation.rs` e `matrix.rs`. Le estrazioni con filtri propri
  (colore, solo visibili, prefisso) restano nei loro test. Sistemato anche il
  commento di `queue_click`, già troncato a metà frase nel commit)_

- [x] **B10. 9 opzioni di export come stato globale** (thread-local:
  `set_show_pct`, `set_show_year`, scope, allow-list, ghost, ghost_red, corner,
  range…) impostate prima di ogni `build_*` e azzerate a mano. Proposta: struttura
  `ExportOptions` passata come parametro. _Tocca tutte le firme dell'export._
  _(fatto: in `pdf_export.rs` non c'è più nessun `thread_local`. Le 7 scelte
  dell'utente (percentuali, anno, ambito milestone, milestone scelte, ghost, colore
  ghost, periodo) sono in `ExportOptions` (default = quelli delle dialog), passata
  a `build_pdf_selected`, `build_pdf_project_fmt`, `build_svg_project_fmt` e giù
  fino ai disegni; `window()` e `milestone_shown()` sono suoi metodi. La
  smussatura degli angoli non è una scelta della dialog ma del file: letta da
  `app.corner_pct` e passata come valore a `rect_round`/`round_rect_pts`. Lato UI
  `UiState::export_options(...)` costruisce le opzioni e `scoped_exports` ne fa una
  copia per ambito: non c'è più niente da «rimettere a posto» dopo un export (prima
  bastava un percorso che dimenticasse il reset per sporcare l'export successivo).
  I test passano le opzioni esplicitamente invece di impostare e ripristinare
  globali; `scoped_exports_builds_one_item_per_scope` verifica anche che le
  opzioni di base arrivino a ogni ambito. Comportamento invariato: 286 test verdi)_

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

- [x] **C3. Ricerche ripetute per cella** — `get_id_by_name` scorre tutti i worker,
  due volte per cella; in `draw_left_devs` lo stesso dev viene cercato ~5 volte;
  `dev_name` ordina la lista dei dev a ogni chiamata. Proposta: mappa nome→id per
  frame, una sola lettura di `get_single_dev`.
  _(fatto, senza mappa: la causa era che `gather_slots` costruiva il testo
  «Worker|ore» **dall'id** e `draw_cell_static` ricavava di nuovo l'id **dal
  nome**. Ora `gather_slots` restituisce anche l'id e la cella lo usa: zero
  ricerche per nome. In `draw_left_devs` il dev si legge una volta per blocco
  (erano 9 letture) e `project_has_cells` (voce «Cancella gli effort di tutti i
  dev») si calcola una volta per progetto invece che per ogni dev (costo che
  cresceva col quadrato dei dev). `dev_name` già risolto con B6)_

- [x] **C4. Saturazione** — `saturation_cell` calcolata **3 volte** per ogni
  worker × colonna a ogni frame (riepilogo, Σ, celle). Proposta: una matrice per frame.
  _(fatto: matrice `cells[worker][colonna]` calcolata una volta per frame e letta
  da riepilogo, colonna Σ e celle)_

- [x] **C5. Footer** — `worker_remaining_hours` ricostruisce `weeks_vec` per ogni
  worker a ogni frame; `Workers::list()` copia e ordina tutti i nomi a ogni
  chiamata. Proposta: settimane calcolate una volta per frame.
  _(fatto: `remaining_weeks(app, anno, this_week)` calcola le settimane una volta
  per frame; `worker_remaining_hours` riceve l'elenco e somma solo le ore del
  worker. `Workers::list()` lasciata com'è: nel footer è chiamata una volta per
  frame, altrove solo da finestre aperte, quindi una cache non vale la
  complessità. Test `remaining_hours_follow_the_year_and_the_null_filter`, che
  prima non c'era)_

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

- [x] **E1.** `ui/mod.rs` ha ~6.300 righe, di cui ~2.480 di test: spostarli in
  `src/ui/tests.rs`. _(fatto: `mod.rs` 6.484 → 3.799 righe, test in `tests.rs`
  (modulo figlio, `use super::*` invariato); verificato prima che nessuna stringa
  su più righe dipendesse dal rientro tolto)_
- [x] **E2.** Refusi nei nomi dei file: `sinlge_effort.rs`,
  `sinlge_effort_week.rs` → `single_…`. _(fatto con `git mv`, così git conserva la
  storia; aggiornati moduli e `use`. Le rinomine sono già in staging)_
- [x] **E3.** Stile clippy (53 warning nel codice, ~85 contando i test): ~32 `if`
  annidabili, 21 assegnazioni dopo `Default::default()` (test), `mut` inutili, 4
  `run_ui` con risultato ignorato nei test, ecc. Nota: il repo **non** è formattato
  con `rustfmt` (152 differenze già all'ultimo commit): un `cargo fmt` globale
  toccherebbe molti file, va deciso a parte.
  _(fatto: `cargo clippy --fix` (51 correzioni: `if` annidati uniti con le *let
  chains*, `is_some_and`, `values()`, `sort_by_key`, `#[derive(Default)]`…) e il
  resto a mano (inizializzazioni dirette al posto di `Default` + assegnazione, `mut`
  e risultati `run_ui`, un alias di tipo, 5 `#[allow(too_many_arguments)]` come da
  convenzione del progetto). `clippy --fix` lascia la formattazione rotta: le sole
  righe toccate sono state formattate con `rustfmt +nightly --file-lines` (il resto
  dei file resta com'era), più 6 blocchi a mano; ripristinate 9 riformattazioni
  fuori ambito nei test. **Attenzione**: per `clear_efforts` clippy suggeriva
  `.any()`, che si fermerebbe al primo dev: scritto come ciclo esplicito con un
  commento. **Il `cargo fmt` globale resta da decidere a parte.** Clippy: 0
  warning, anche sui test)_
- [x] **E4.** Nella radice ~10 copie/backup di `.ron` (già ignorati da git). Dati
  dell'utente: **non si toccano** senza richiesta esplicita.
  _(fatto su richiesta: le 5 copie manuali (`workers (1).ron`, `workers_old.ron`,
  `workers_old2.ron`, `workers copy.ron`, `workers copy 2.ron`) spostate nel
  **Cestino di macOS** (recuperabili con «Rimetti a posto» finché non si svuota).
  Prima verificato il contenuto: tutte tranne `workers copy.ron` avevano progetti
  assenti dal file in uso (`OVT Incr. 2.3` — probabilmente rinominato in
  `OVT - Incr. 2.3` —, `ALTRO`, `OVT - 3400/4600`, `BCN`, `BM6070`, `CL8000`,
  `CLAM 2040?)`, `CO8`, `P&P Terze parti`, `STX`); per questo il Cestino e non la
  cancellazione definitiva. Restano `workers.ron` e i backup automatici
  `.bak1–5`)_

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
- **2026-10-24** — Punto 5, blocco 5a (B2–B7, duplicati). Build 0 warning, clippy
  53, 284 test verdi (+4).
- **2026-10-24** — Punto 5, blocco 5b (C3–C5, ottimizzazioni). Build 0 warning,
  clippy 53, 285 test verdi (+1). Punto 5 completato.
- **2026-10-24** — Fatto il punto 6 (B1, B9, E1, E2, E3). Build 0 warning,
  **clippy 0 warning** (anche sui test), 285 test verdi.
- **2026-10-24** — Fatto B8 (conversioni jiff in `date_utils::picker`). Build e
  clippy 0 warning, 286 test verdi (+1).
- **2026-10-24** — Chiuso E4: copie manuali dei `.ron` spostate nel Cestino.
- **2026-10-24** — Fatto B10 (opzioni di export come parametro `ExportOptions`,
  niente più stato globale). Build e clippy 0 warning, 286 test verdi. Tutte le
  voci del report sono chiuse.
- **2026-10-24** — Aggiunta e approvata A8 (aggiornamento egui 0.37).
- **2026-10-24** — A8, punti 1–4 sul branch `egui-0.37`: compila con egui 0.37,
  0 warning (build e clippy), 288 test verdi. In attesa della prova a mano.
- **2026-10-24** — A8: ripristinato lo scroll trascinando col mouse in tutte le
  viste (egui 0.37 lo limitava ai touch screen). 289 test verdi.
- **2026-10-24** — Chiusa A8: prova a mano dell'utente superata. egui 0.37,
  build e clippy 0 warning, 290 test verdi. Tutte le voci del report sono chiuse.
- **2026-10-24** — Aggiunta e approvata A9 (aggiornamento printpdf 0.12.8).
- **2026-10-24** — A9 applicata (printpdf 0.12.8), in attesa della prova a mano.
- **2026-10-24** — Chiusa A9: prova a mano dell'utente superata. Tutte le voci del
  report sono chiuse.
- **2026-10-24** — Aggiunta e approvata A10 (aggiornamento dipendenze indirette).
- **2026-10-24** — A10 applicata (`cargo update`), in attesa della prova a mano.
- **2026-10-24** — Chiusa A10: prova a mano dell'utente superata. Tutte le voci del
  report sono chiuse.
