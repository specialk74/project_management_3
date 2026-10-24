# Miglioramenti / Roadmap

Elenco numerato dei possibili miglioramenti. Ogni voce ha un numero stabile per
riferirsi ad essa velocemente ("facciamo la #7").

**Come usarlo**: quando una voce è gestita, spunta la casella mettendo una `x`
(`- [x] ...`) e spostala nella sezione **Fatti**. Aggiungere note tra parentesi se
serve (es. data, PR, decisioni).

Qui stanno le **funzionalità e la UX** (cosa vede l'utente). La qualità interna del
codice (duplicati, codice morto, prestazioni, bug latenti) è in
[`REVISIONE_CODICE.md`](REVISIONE_CODICE.md).

Legenda difficoltà: **Bassa** / **Media** / **Alta**.
Il "tempo" è una scala di complessità/impegno (lavoro sviluppatore + verifica),
non il tempo di digitazione.

---

# Da fare

## A. Robustezza & dati condivisi

- [ ] **1. Lock consigliato sul file** (crate `fs2`/flock o file `.lock`) — chiude il *lost update* rimasto dopo la scrittura atomica, serializzando i salvataggi tra colleghi. _Tempo: ~2-3 h · Difficoltà: Media (poco affidabile su share di rete/OneDrive)._

## B. Produttività nell'editing (UX)

- [ ] **5. Undo/Redo** — snapshot di `to_ron_string` in uno stack (le mutazioni passano già da `Action`). _Tempo: ~half day · Difficoltà: Media._
- [ ] **6. Navigazione da tastiera nella griglia** (frecce/Tab tra celle) — velocizza molto l'inserimento effort. _Tempo: ~1 giorno · Difficoltà: Media-Alta (rendering a coordinate, serve un modello di focus)._
- [ ] **9. Persistenza preferenze UI** (tema, compatta, zoom, footer, B/N) — ora `UiState` si resetta a ogni avvio; salvarle in un file di config. _Tempo: ~2-3 h · Difficoltà: Bassa._

## C. Gestione anagrafiche (oggi si può solo aggiungere)

- [ ] **10. Elimina/Rinomina worker, dev, categoria** — gestendo i riferimenti (es. worker con effort). _Tempo: ~half day · Difficoltà: Media (integrità dati)._
- [ ] **11. UI colore del dev** (sfondo/testo) — i colori sono nel `.ron` ma non modificabili da UI; replicare il color-picker delle milestone. _Tempo: ~2-3 h · Difficoltà: Bassa-Media._

## D. Reportistica & export

- [ ] **12. Export CSV/Excel** della matrice effort — per analisi esterne/pivot. _Tempo: ~half day · Difficoltà: Media._

---

# Fatti

## A. Robustezza & dati condivisi

- [x] **2. Backup a rotazione** — tiene le ultime N versioni del `.ron` come rete di sicurezza. _(fatto: `App::rotate_backups`, `<file>.bak1..bak5` a ogni salvataggio; test `backup_rotation_keeps_previous_version`)_
- [x] **3. Autosave periodico** — salvataggio ogni N minuti sfruttando la scrittura atomica. _(fatto: `PjmApp::maybe_autosave`, ogni 120s se ci sono modifiche e nessun conflitto in attesa)_
- [x] **4. Validazione dello schema al load** — messaggio chiaro se il RON è incoerente, invece del fallimento silenzioso. _(fatto: `App::validate` + finestra "Problema nel file" su avvio/apertura falliti o riferimenti pendenti)_

## B. Produttività nell'editing (UX)

- [x] **7. Toast/area di stato in-app per errori** _(fatto: `src/ui/toasts.rs` — `toasts_layer` disegna in basso a destra le notifiche di `UiState.toasts`; `ToastLevel {Info, Success, Warning, Error}` decide icona/colore/durata, **solo gli errori sono permanenti** (✕ per chiudere), un testo ripetuto rinfresca invece di impilare, cap a `MAX_TOASTS`. Ha assorbito `external_notice` (rimosso dalla toolbar) e **tutti** gli `eprintln!`: `App::save` → `Result<(), String>`, `save_to_disk` → `bool` (solo il salvataggio riuscito azzera `changed`; in uscita l'errore annulla la chiusura), `save_export_dialog` notifica il file scritto, gli errori del thread git passano da `git_autosync::report`/`take_error` → `poll_background_errors`. Restano modali gli errori di caricamento (`load_error_window`). Test: `toast_icons_are_renderable`, `repeated_toasts_refresh_instead_of_stacking`, `toast_list_is_capped_keeping_the_newest`, `only_errors_are_sticky`, `toasts_layer_draws_and_expires`, `failed_save_reports_an_error_toast`)_
- [x] **8. Ricerca/salto rapido a un progetto** — utile con molti progetti. _(fatto: unificato nella finestra "Progetti" — `⌘/Ctrl+P` o Filtri ▸ Progetti… — con ricerca, spunta visibilità e tripletta cliccabile che scrolla alla riga via `pending_scroll_y`)_

## D. Reportistica & export

- [x] **13. PDF: selezione multipla di progetti** — più flessibile dell'attuale trigger a singolo progetto. _(fatto: con più progetti visibili l'export apre `pdf_multi_export_window` (Select All + checkbox per progetto) → `Action::ExportPdfSelected` → `build_pdf_selected`; test `build_pdf_selected_only_includes_selected_projects`)_
- [x] **14. Cruscotto saturazione worker** (chi è sovra-allocato, per settimana/mese) — vista sintetica per la pianificazione. _(fatto: Vista ▸ Saturazione worker… — `saturation_window`; selettore settimana/mese, lista criticità con salto, heatmap worker×periodo, totali Σ; ferie/malattia **non** azzerano la capacità: conta il max ore effettivo della settimana)_

## E. Qualità del codice / manutenzione

Le voci #15–#18 sono state spostate in [`REVISIONE_CODICE.md`](REVISIONE_CODICE.md)
(sezione «Fatti in precedenza», come M15–M18).

---

## Consigliati per primi (quick win ad alto valore)

1. **#9** Persistenza preferenze UI
2. **#5** Undo/Redo (il salto di qualità UX più grande)
3. **#11** UI colore del dev

> Nota: per le voci grosse (#5 Undo/Redo, #6 navigazione tastiera, #10 elimina/rinomina)
> conviene concordare prima il comportamento nei casi limite.
