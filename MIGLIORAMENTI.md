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

- [x] **7. Toast/area di stato in-app per errori** _(fatto: `src/ui/toasts.rs` — `toasts_layer` disegna in basso a destra le notifiche di `UiState.toasts`; `ToastLevel {Info, Success, Warning, Error}` decide icona/colore/durata, **solo gli errori sono permanenti** (✖ per chiudere; era ✕, che nessun font di egui contiene), un testo ripetuto rinfresca invece di impilare, cap a `MAX_TOASTS`. Ha assorbito `external_notice` (rimosso dalla toolbar) e **tutti** gli `eprintln!`: `App::save` → `Result<(), String>`, `save_to_disk` → `bool` (solo il salvataggio riuscito azzera `changed`; in uscita l'errore annulla la chiusura), `save_export_dialog` notifica il file scritto, gli errori del thread git passano da `git_autosync::report`/`take_error` → `poll_background_errors`. Restano modali gli errori di caricamento (`load_error_window`). Test: `toast_icons_are_renderable`, `repeated_toasts_refresh_instead_of_stacking`, `toast_list_is_capped_keeping_the_newest`, `only_errors_are_sticky`, `toasts_layer_draws_and_expires`, `failed_save_reports_an_error_toast`)_
- [x] **8. Ricerca/salto rapido a un progetto** — utile con molti progetti. _(fatto: unificato nella finestra "Progetti" — `⌘/Ctrl+P` o Filtri ▸ Progetti… — con ricerca, spunta visibilità e tripletta cliccabile che scrolla alla riga via `pending_scroll_y`)_
- [x] **19. Cancella gli effort di un dev o di tutto il progetto** — tasto destro sul nome del dev ▸ «Cancella gli effort…» / «Cancella gli effort di tutti i dev…», con conferma. _(fatto 2026-10-24: svuota le celle della griglia — worker, ore, note di cella —; restano effort pianificato, nota dev e % dichiarata. Voci disabilitate se non c'è nulla da cancellare. `Action::ClearEfforts`, `clear_efforts_window`)_
- [x] **20. Cancella tutte le milestone di un progetto** — in fondo al sottomenù «Aggiungi milestone qui», con conferma che le elenca. _(fatto 2026-10-24: toglie le milestone collocate nel progetto, di sistema e personalizzate, senza eliminarle — restano nel gestore e si possono ricollocare —; gli altri progetti non cambiano. `Action::ClearProjectMilestones`)_
- [x] **21. Evidenziare un worker in tutta la griglia** — click sinistro sul nome nel footer: le sue righe «Worker|ore» nelle celle dei dev diventano gialle su scuro; riclic per spegnere, click su un altro per spostare. _(fatto 2026-10-24: solo sessione. Il massimo ore globale del worker è passato al tasto destro ▸ «Ore max…», accanto a «Ghost»)_
- [x] **22. Limite ore della settimana per tutti anche dalla data del footer** — prima solo dalla data in alto. _(fatto 2026-10-24: stessa finestra «Limite settimana (tutti)»)_
- [x] **23. Mettere in ferie tutti i worker di una settimana** — spunta «Metti tutti i worker in ferie» nella finestra del limite settimanale. _(fatto 2026-10-24: stato «Ferie» a tutti tranne chi è in «Malattia»; le ore restano quelle inserite; spunta già accesa se tutti sono in ferie, togliendola si rimuovono)_
- [x] **24. Autocompletamento senza i worker nascosti** — scrivendo in una cella non vengono più proposti i worker con la spunta «Elenchi» spenta. _(fatto 2026-10-24: scritto per intero, un nome nascosto viene comunque accettato)_

## D. Reportistica & export

- [x] **13. PDF: selezione multipla di progetti** — più flessibile dell'attuale trigger a singolo progetto. _(fatto: con più progetti visibili l'export apre `pdf_multi_export_window` (Select All + checkbox per progetto) → `Action::ExportPdfSelected` → `build_pdf_selected`; test `build_pdf_selected_only_includes_selected_projects`)_
- [x] **14. Cruscotto saturazione worker** (chi è sovra-allocato, per settimana/mese) — vista sintetica per la pianificazione. _(fatto: Vista ▸ Saturazione worker… — `saturation_window`; selettore settimana/mese, lista criticità con salto, heatmap worker×periodo, totali Σ; ferie/malattia **non** azzerano la capacità: conta il max ore effettivo della settimana)_
- [x] **25. Saturazione più leggibile e navigabile** — date senza anno (`mm-gg`, o `mm` per mese) con una riga dell'anno sopra; finestra ridimensionabile fino a tutto schermo; intestazione sempre visibile scorrendo tra i worker; col cursore su una cella si evidenziano data e worker; si apre già con «Solo da settimana corrente». _(fatto 2026-10-24)_
- [x] **26. Anno corretto nei PDF con periodo scelto a mano** — con un «Da» non di lunedì a inizio mese (es. 1/1/2026) l'asse partiva da pochi giorni del mese prima e l'etichetta dell'anno mostrava quello sbagliato («2025 · Jan Feb…»). _(fatto 2026-10-24: gli anni ai lati sono quelli del primo e ultimo mese leggibili; caso reale «OVT 3»)_
- [x] **27. PDF molto più leggeri** — 10–40 volte più piccoli (Gantt 2,1 MB → 180 KB) a parità di aspetto e testo cercabile. _(fatto 2026-10-24 con l'aggiornamento a printpdf 0.12, voce A9 di `REVISIONE_CODICE.md`: vengono incorporati solo i caratteri usati)_

## E. Qualità del codice / manutenzione

Le voci #15–#18 sono state spostate in [`REVISIONE_CODICE.md`](REVISIONE_CODICE.md)
(sezione «Fatti in precedenza», come M15–M18).

## F. Piattaforma & compatibilità

- [x] **28. Scorrere trascinando col mouse, ovunque** — tasto premuto e trascina, in orizzontale e verticale, su griglia, saturazione, matrice, manuale, filtri e dialog. _(fatto 2026-10-24: con egui 0.37 il trascinamento funzionava solo sui touch screen; ripristinato su tutte le aree di scorrimento)_
- [x] **29. Niente scatti su Windows con due schede grafiche** — su un portatile con NVIDIA RTX 2000 Ada + integrata lo scroll col trascinamento scattava. _(fatto 2026-10-24: su Windows si usa di default la GPU a basso consumo; la variabile `WGPU_POWER_PREF` resta prioritaria, vedi README)_
- [x] **30. Icone sempre visibili** — simboli di toast e milestone (ℹ ⚠ ⛔ ✔ ⚑ ⚡), ⌘, 🔎, 🗑 e il pulsante di chiusura. _(fatto 2026-10-24: con egui 0.37 servono i font dei simboli, ora attivati; «✕» non era mai stato disegnabile e mostrava un quadratino → «✖». Un test controlla che ogni simbolo dell'interfaccia sia disegnabile)_

---

## Consigliati per primi (quick win ad alto valore)

1. **#9** Persistenza preferenze UI
2. **#5** Undo/Redo (il salto di qualità UX più grande)
3. **#11** UI colore del dev

> Nota: per le voci grosse (#5 Undo/Redo, #6 navigazione tastiera, #10 elimina/rinomina)
> conviene concordare prima il comportamento nei casi limite.
