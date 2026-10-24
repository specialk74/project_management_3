# Project Management Effort Tracker

Applicazione desktop per pianificare l'**effort** (ore di lavoro) dei progetti su
una griglia settimanale in stile Gantt. Scritta in **Rust** con
[egui/eframe](https://github.com/emilk/egui); l'interfaccia è in italiano, gli
export PDF in inglese.

## Cosa fa

- **Progetti** con tripletta, descrizione, categoria, inizio/fine e stato
  (attivo, on-hold, chiuso).
- **Dev** (ruoli/discipline) per progetto, con effort pianificato, residuo,
  avanzamento presunto e dichiarato (con storico).
- **Effort settimanale** dei **worker** (persone) su ogni dev, con
  autocompletamento, note, copia/incolla e inserimento multiplo.
- **Footer** con i totali per worker e settimana, limiti di ore, ferie/malattia
  e controllo della saturazione.
- **Milestone** di sistema o di singolo progetto, con tipo (traguardo/trigger) e
  categorie di stampa (Internal/External).
- **Filtri** per worker, progetti, dev, categorie e anni; vista compatta, zoom
  delle settimane, tema chiaro/scuro, bianco/nero.
- **Cruscotti**: saturazione dei worker e matrice progetti × worker.
- **Export**: Gantt PDF/SVG (con periodo, formati delle barre, percentuali,
  worker ghost), andamento nel tempo, report di riepilogo, matrice e minuta delle
  note in Markdown.
- **Confronto e importazione** di progetti tra due file.
- **Salvataggio** in un unico file `.ron`, con backup a rotazione, salvataggio
  automatico, rilevamento delle modifiche esterne (con fusione) e, se il file è
  in un repository git, commit e push automatici.

Il manuale d'uso completo è in [`docs/MANUALE.md`](docs/MANUALE.md) ed è
consultabile anche dall'app: **Aiuto ▸ Manuale d'uso…**.

## Requisiti

- **Rust 1.95 o successivo** (edition 2024; lo richiede egui 0.37).
- macOS, Windows o Linux. Su Linux servono le librerie di sistema per finestre e
  dialog di file usate da eframe e `rfd`.

## Avvio

```bash
cargo run                  # apre workers.ron dalla cartella corrente
cargo run -- mio_file.ron  # apre un altro file dati
cargo build --release      # eseguibile ottimizzato in target/release/
```

Se il file non esiste si parte da vuoto. Se esiste ma non si riesce a leggerlo,
l'app lo segnala all'avvio prima di partire da vuoto (salvando lo si
sovrascriverebbe). Da **File ▸ Apri…** si può scegliere un altro `.ron`.

## Il file dati

Tutto lo stato (progetti, dev, worker, categorie, milestone, impostazioni) vive in
un unico file **RON**, di default `workers.ron`.

- A ogni salvataggio il programma conserva le ultime 5 versioni come
  `workers.ron.bak1` … `bak5`.
- Alcune impostazioni esistono solo nel file, senza interfaccia: colore della
  settimana corrente, colori e intensità dei mesi, smussatura degli angoli negli
  export. Se mancano vengono scritte con i valori predefiniti al primo avvio.
- Se il file è dentro un repository git **ed è già tracciato**, ogni salvataggio
  fa anche `git add`, `commit` e `push` di quel file (mai dei backup). Un file non
  tracciato non viene toccato.

I file `.ron` e i backup sono esclusi da git (`.gitignore`): sono dati, non
codice.

## Sviluppo

```bash
cargo test                       # tutti i test (~290)
cargo test nome_del_test         # un test solo
cargo clippy --all-targets       # nessun warning ammesso
cargo audit                      # vulnerabilità note nelle dipendenze
```

Build e `cargo clippy --all-targets` sono a **zero warning**: va mantenuto così.
Il repository non è formattato con `rustfmt`, quindi non va lanciato un
`cargo fmt` globale.

### Struttura

```
src/
├── main.rs            avvio, caricamento del file, finestra
├── app.rs             stato persistito (App), lettura/scrittura RON, validazione
├── ui/                interfaccia egui
│   ├── mod.rs         nucleo: PjmApp, UiState, Action, ciclo ui(), helper condivisi
│   ├── grid.rs        griglia e colonna sinistra (disegnate a coordinate)
│   ├── toolbar.rs     menù e intestazione delle settimane
│   ├── footer.rs      totali per worker e settimana
│   ├── dialogs.rs     finestre (conferme, note, popup…)
│   ├── export.rs      dialog di export e minuta
│   ├── filters.rs     finestra unica dei filtri
│   ├── saturation.rs  cruscotto saturazione
│   ├── matrix.rs      matrice progetti × worker
│   ├── compare.rs     confronto/importazione tra file
│   ├── help.rs        manuale integrato
│   ├── toasts.rs      notifiche in-app
│   ├── tests.rs       test dell'interfaccia
│   └── test_util.rs   helper per i test
├── pdf_export.rs      export PDF/SVG (Gantt, andamento, report, matrice)
├── worker_matrix.rs   dati della matrice
├── sync_merge.rs      fusione a 3 vie con le modifiche esterne
├── project_diff.rs    confronto tra progetti
├── git_autosync.rs    commit/push automatici del file dati
├── ui_style.rs        dimensioni, colori, tema
└── date_utils/, project_utils/, single_dev_utils/, single_effort_utils/,
    workers_utils/, dev_utils/, milestones.rs, categories.rs   modello dati
```

L'interfaccia è *immediate mode*: a ogni frame si disegna dallo stato corrente,
le interazioni producono delle `Action` che vengono applicate a fine frame.

### Documenti

| File | Contenuto |
|---|---|
| [`docs/MANUALE.md`](docs/MANUALE.md) | manuale d'uso (incluso nell'app) |
| [`CLAUDE.md`](CLAUDE.md) | guida tecnica all'architettura e alle convenzioni del codice |
| [`MIGLIORAMENTI.md`](MIGLIORAMENTI.md) | roadmap delle funzionalità |
| [`REVISIONE_CODICE.md`](REVISIONE_CODICE.md) | revisione della qualità del codice e registro degli interventi |

### Dipendenze principali

eframe/egui 0.37 (interfaccia), egui_extras (calendario), egui_commonmark
(manuale), printpdf 0.12 (PDF), rfd (dialog di file), ron + serde (file dati),
chrono e jiff (date).

## Font

Gli export PDF usano i font **DejaVu Sans** inclusi in `assets/fonts/`, con la
loro licenza (`LICENSE-DejaVu.txt`). L'interfaccia usa i font di egui, compresi
quelli per simboli ed emoji (opzione `monochrome_emoji_fonts`).
