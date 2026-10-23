# Manuale d'uso — Project Management Effort Tracker

Versione applicazione: **{{VERSION}}**

Questo manuale descrive tutte le funzionalità del programma. È scritto per essere
riutilizzato in un futuro "Help" interno all'applicazione.

---

## Indice

1. [Cos'è il programma](#1-cosè-il-programma)
2. [Avvio e file dati](#2-avvio-e-file-dati)
3. [Concetti e modello dati](#3-concetti-e-modello-dati)
4. [Struttura dell'interfaccia](#4-struttura-dellinterfaccia)
5. [Barra dei menù](#5-barra-dei-menù)
6. [Gestione dei progetti](#6-gestione-dei-progetti)
7. [Gestione dei dev nel progetto](#7-gestione-dei-dev-nel-progetto)
8. [Inserimento e modifica dell'effort](#8-inserimento-e-modifica-delleffort)
9. [Note](#9-note)
10. [Spostare l'effort](#10-spostare-leffort)
11. [Milestone](#11-milestone)
12. [Il confine di fine anno](#12-il-confine-di-fine-anno)
13. [Il footer (worker e totali)](#13-il-footer-worker-e-totali)
14. [Filtri](#14-filtri)
15. [Viste: compatta, bianco/nero, tema, zoom](#15-viste-compatta-bianconero-tema-zoom)
16. [Esportazione PDF](#16-esportazione-pdf)
17. [Salvataggio e modifiche esterne](#17-salvataggio-e-modifiche-esterne)
18. [Scorciatoie da tastiera](#18-scorciatoie-da-tastiera)
19. [Codici colore e indicatori](#19-codici-colore-e-indicatori)
20. [Confronto e importazione tra file](#20-confronto-e-importazione-tra-file)

---

## 1. Cos'è il programma

È uno strumento di pianificazione dell'**effort** (ore di lavoro) per progetti,
organizzato come una griglia settimanale (stile Gantt). Permette di:

- definire **progetti**, con inizio, fine, categoria e descrizione;
- assegnare a ogni progetto uno o più **dev** (ruoli/discipline, es. Frontend,
  Test, Sistema…);
- distribuire, settimana per settimana, l'effort dei singoli **worker** (persone)
  su ciascun dev;
- tenere sotto controllo saturazione delle persone, milestone, scadenze;
- esportare il tutto in **PDF**.

Tutti i dati vivono in un unico file `.ron` (vedi §2 e §17).

---

## 2. Avvio e file dati

### Compilazione ed esecuzione

```bash
cargo build      # compila
cargo run        # avvia (apre workers.ron nella cartella corrente)
cargo test       # esegue i test
```

### File dati

- All'avvio il programma carica **`workers.ron`** dalla cartella di lavoro.
- Si può indicare un altro file come argomento:

  ```bash
  cargo run -- percorso/miofile.ron
  ```

- **File ▸ Apri…** apre un selettore di file nativo (filtrato ai `.ron`): il file
  scelto diventa il file corrente.
- Il nome del file corrente è mostrato in alto a destra nella barra strumenti.
  Un asterisco `(*)` e il colore arancione segnalano modifiche non salvate.

### Colore della settimana corrente (`week_color`)

L'evidenziazione della **settimana corrente** (tinta sulla colonna in griglia e
nel footer, intestazione della settimana) usa un colore che si cambia **solo
editando il file** `.ron`: non c'è una voce di menù. In testa al file:

```ron
(
    start_week: (20290),
    week_color: "#CCFF00",
    ...
)
```

- Formato `"#RRGGBB"` (il `#` è facoltativo, maiuscole o minuscole indifferenti).
- Il valore è **sempre scritto** nel file, anche quando è quello di default
  (`#CCFF00`, giallo fosforescente): basta modificarlo e riaprire il file.
- Se il colore è scritto male il programma usa il default e lo segnala nella
  finestra "Problema nel file" al caricamento.
- In modalità **Bianco/Nero** (§15) il colore viene reso in scala di grigi come
  tutti gli altri.

### Colori dei mesi (`month_colors`)

Le due righe di **date** — l'intestazione delle settimane in cima alla griglia e
la riga delle date del footer — hanno uno sfondo colorato **diverso per ogni
mese**, così si vede a colpo d'occhio dove finisce un mese e comincia il
successivo. Anche questi 12 colori si cambiano solo dal file:

```ron
(
    start_week: (20290),
    week_color: "#CCFF00",
    month_colors: [
        "#FF0000",
        "#FF8000",
        ...
    ],
    month_tint_pct: 60,
    ...
)
```

- **12 valori in ordine da gennaio a dicembre**, stesso formato `"#RRGGBB"`.
- Una settimana **a cavallo di due mesi** ha la cella divisa in **5 parti**
  (lunedì–venerdì): a sinistra il mese che sta finendo, largo quanti sono i suoi
  giorni, a destra il mese nuovo. Es. con il lunedì in agosto e gli altri quattro
  giorni in settembre, un quinto della cella è del colore di agosto. Se il mese
  cambia di sabato o domenica la cella resta di un colore solo.
- Con lo **zoom** (settimane accorpate) vale la stessa regola sull'intero gruppo:
  le settimane si accodano e le bande sono proporzionali ai giorni di ciascun mese.
- **`month_tint_pct`** regola quanto è marcata la tinta: `0` = invisibile,
  `100` = colore pieno, default `60`. Valori fuori scala vengono riportati nei
  limiti e segnalati.
- Il testo della data si adatta da solo allo sfondo che ne risulta (nero sulle
  tinte chiare, bianco su quelle scure), quindi resta leggibile anche a
  percentuali alte.
- La velatura sta comunque **sotto** all'evidenziazione della settimana
  corrente, che rimane il segnale più forte.
- Se una voce manca o è scritta male, **solo quel mese** torna al suo colore di
  default; il problema è segnalato nella finestra "Problema nel file".
- Anche questi valori sono sempre riscritti nel file al salvataggio: eventuali
  commenti aggiunti a mano nel `.ron` vengono persi.

### Smussatura degli angoli nella stampa (`corner_pct`)

I rettangoli "pieni" del PDF/SVG — le **barre dell'effort** dei dev (in tutti e
tre i formati barra), le **celle della banda dei mesi** in alto e la **barra
rossa di avanzamento "Today"** sull'asse — hanno gli
**angoli arrotondati**. Quanto, lo decide un valore del file, letto a ogni avvio:

```ron
(
    start_week: (20290),
    week_color: "#CCFF00",
    month_colors: [...],
    month_tint_pct: 60,
    corner_pct: 40,
    ...
)
```

- È una **percentuale del raggio massimo**: `0` = spigolo vivo (come prima di
  questa funzione), `100` = raggio massimo, cioè metà del lato corto — gli
  estremi delle barre diventano semicerchi. Default `40`.
- Il raggio è calcolato **sul lato corto** di ogni rettangolo, quindi barre
  basse e celle alte restano proporzionate tra loro.
- Vale sia per il PDF sia per l'SVG; nella griglia a schermo non cambia nulla.
- Come gli altri parametri "solo file": non c'è una voce di menù, il valore è
  sempre riscritto al salvataggio e, se è fuori dall'intervallo 0–100, viene
  riportato nei limiti e segnalato nella finestra "Problema nel file".
- Se il file non contiene ancora `corner_pct` (o un altro parametro "solo file"),
  all'apertura il programma lo **risalva subito** con il valore di default e lo
  segnala con una notifica, così il campo compare nel `.ron` pronto da modificare.

---

## 3. Concetti e modello dati

Gerarchia dei dati:

```
App (file .ron)
├── Workers   — le persone (risorse umane)
├── Devs      — i ruoli/discipline (es. "Frontend", "Test", "Sistema")
├── Categorie — categorie di progetto
├── Milestone — traguardi (con nome e colore)
└── Progetti
    └── Progetto
        ├── tripletta, nome (descrizione), inizio, fine, categoria, stato
        └── per ogni Dev:
            ├── effort pianificato (ore previste)
            ├── nota del dev
            └── settimane
                └── per ogni Worker: effort (ore) + nota
```

Termini chiave:

- **Worker**: una persona. Ha un limite di ore settimanali (default **40**), che
  può essere sovrascritto globalmente o per singola settimana.
- **Dev**: un ruolo/disciplina all'interno di un progetto. Ha un colore proprio
  (sfondo + testo) memorizzato nel file.
- **Effort**: ore di lavoro (valore intero, non percentuale).
- **Effort pianificato**: le ore previste per un dev in un progetto (colonna a
  sinistra, editabile).
- **Sovra**: le ore effettivamente allocate a un worker in una settimana (usato
  nel footer per il controllo di saturazione).
- **Tripletta**: codice/etichetta breve del progetto (es. sigla), mostrato in
  grassetto e usato come nome corto nelle finestre.
- **Settimana**: internamente identificata da un numero di giorno; settimane
  adiacenti distano 7. L'etichetta mostrata è la data del primo giorno della
  settimana (formato `AA-MM-GG`).

---

## 4. Struttura dell'interfaccia

Dall'alto verso il basso:

1. **Barra strumenti** (menù File, Aggiungi, Filtri, Vista; a destra nome file e
   versione).
2. **Header**: le date delle settimane (una colonna per settimana).
3. **Corpo**, diviso in due colonne sincronizzate verticalmente:
   - **Colonna sinistra**: per ogni progetto le info (tripletta, categoria, nome,
     inizio, fine, avanzamento) e la striscia dei dev con nome, effort pianificato
     e residuo.
   - **Griglia (destra)**: la matrice settimane × (dev/worker) dove si inserisce
     l'effort.
4. **Footer**: in basso, i totali e il controllo di saturazione dei worker.

**Scroll sincronizzato**: la colonna sinistra e la griglia scorrono insieme in
verticale; header, griglia e footer scorrono insieme in orizzontale.

La **settimana corrente** è evidenziata con una tinta verde su tutta la colonna.
All'avvio la griglia si posiziona su questa settimana; in qualsiasi momento
`Cmd/Ctrl+T` la riporta lì, centrandola nella parte visibile.

**Elenchi di progetti nelle finestre** (Esporta, Minuta, Report, Filtri ▸
Progetti, Stato progetti, Confronta): mostrano **al massimo 15 progetti**, gli
altri si raggiungono scorrendo l'elenco. Così anche su uno schermo piccolo i
bottoni in fondo (es. **Esporta PDF…**) restano sempre visibili.

---

## 5. Barra dei menù

### File
- **Salva** — salva il file `.ron` corrente (scorciatoia `Cmd/Ctrl+S`).
- **Apri…** — apre un file `.ron` (selettore nativo).
- **Confronta/Importa progetto…** — confronta i progetti del file aperto con quelli
  di un altro file `.ron` (una copia su cui hai sperimentato) e importa i dev o i
  progetti scelti (vedi §20).
- **Esporta…** — esporta il PDF Gantt (vedi §16; il comportamento cambia se è
  visibile un solo progetto).
- **Andamento…** — esporta un PDF con l'**andamento nel tempo** delle percentuali
  presunta e dichiarata (vedi §16, «Andamento nel tempo»), una pagina per progetto
  visibile.
- **Minuta…** — genera un file Markdown con le note dei progetti selezionati.
  Apre una finestra dove scegli i **progetti non chiusi** da includere e se
  riportare **tutte le note** o **solo la settimana corrente**. La casella
  **Includi le note dei worker** (spenta di default) aggiunge sotto ogni settimana
  le **note delle celle effort** (capitolo 9) come elenco puntato
  `- **Worker** (Dev): testo`, in ordine di dev e poi per nome del worker; una
  settimana che ha *solo* note dei worker compare comunque, con il solo elenco. La
  casella **Solo progetti con note** (attiva di default) esclude i progetti senza
  note nell'ambito scelto — e, con le note dei worker attive, **anche quelle
  contano**, quindi un progetto senza note di progetto ma con note di cella viene
  incluso; disattivandola i progetti vuoti compaiono comunque con un segnaposto
  `_(nessuna nota)_`. Ogni progetto compare con la sua **tripletta** (o la
  descrizione se la tripletta è vuota) e le note settimanali con la data di
  riferimento (più recenti prima).
- **Report…** — genera un **PDF di riepilogo** dei progetti scelti. La finestra
  propone i progetti visibili nel corpo centrale (tutti spuntati, con Select All).
  Ogni progetto parte su una **nuova pagina** e riporta:
  - **tripletta**, **categoria**, **info** del progetto, **data di inizio e di fine**;
  - l'**avanzamento complessivo**: *presunto* (ore usate fino a oggi sullo stimato)
    ed *effettivo* (media delle % dichiarate dai dev, pesata sullo stimato);
  - una tabella con **tutti i dev** del progetto e, per ciascuno, ore e % di
    (intestazioni stampate in inglese, vedi §16):
    **Stimato** / *Estimated* (sempre 100%), **Usato fino a oggi** / *Used to date*
    (ore in griglia fino alla settimana corrente), **Allocato in griglia** /
    *Allocated in grid* (tutte le ore assegnate, anche nelle settimane future) e
    **Mancante** / *Remaining* (= stimato − usato fino a oggi). Le %
    sono calcolate sullo **stimato del dev**; con stimato 0 compare «—». Un
    mancante **negativo** (sforamento) è scritto **in rosso**.
  - solo per i progetti **a cavallo della fine dell'anno corrente** (iniziano entro
    il 31/12 e finiscono l'anno dopo; servono entrambe le date), due colonne in più
    che **scompongono il Mancante**: **Fino al 31/12/AAAA** (*Through 12/31/AAAA*) = ore allocate in griglia
    dalla settimana successiva a oggi fino a fine anno (una settimana che inizia
    entro il 31/12 conta nell'anno, come in griglia), e **Dal 1/1/AAAA a fine** (*From 1/1/AAAA to end*) =
    il resto del mancante (Mancante − la colonna precedente), cioè quanto resta da
    fare dall'inizio del nuovo anno alla fine del progetto. Le due colonne sommano
    al Mancante; se l'allocato fino a fine anno supera già lo stimato, la seconda è
    **negativa e in rosso**.
  Se i dev non entrano in una pagina, la tabella continua sulla successiva.
- **Esci** — chiude il programma. Se ci sono modifiche non salvate, chiede
  conferma (Salva ed esci / Esci senza salvare / Annulla).

### Aggiungi
- **+ Progetto** — crea un nuovo progetto vuoto.
- **Clona progetto…** — crea una copia completa di un progetto esistente (vedi
  «Clonare un progetto»).
- **Worker** — campo di testo + `+ Worker`: aggiunge una persona. Premi Invio o
  il bottone; il campo resta aperto per inserimenti multipli.
- **Dev** — aggiunge un nuovo ruolo/disciplina.
- **Categoria** — aggiunge una categoria di progetto.
- **Milestone** — crea una nuova milestone (poi assegnabile ai progetti), con
  **tipo** e **categorie di stampa** scelti prima del nome.

### Filtri
Le prime sei voci aprono **la stessa dialog «Filtri»** (cinque colonne: Workers,
Progetti, Dev, Categorie, Anni — vedi §14): cambia solo la colonna che riceve il focus.
- **Progetti…** — colonna Progetti: elenco dei progetti (non chiusi) con una spunta
  per ciascuno che attiva/disattiva la **visibilità** nella griglia, ricerca e salto
  rapido ("Select All" per agire su tutti). Scorciatoia `Cmd/Ctrl+P`.
- **Workers…** — colonna Workers (vedi §14). La voce mostra una spunta quando un
  filtro è attivo. Scorciatoia `Cmd/Ctrl+F`.
- **Workers (settimana corrente)…** — stessa colonna e stessa selezione del filtro
  worker, ma mostra **solo i progetti in cui un worker selezionato sta lavorando nella
  settimana corrente** (ha effort in questa settimana). Dentro ai progetti mostrati la
  visualizzazione resta identica al filtro Workers normale (tutte le settimane). La
  modalità corrisponde alla spunta **«Solo settimana corrente»** in cima alla colonna.
  Scorciatoia `Cmd/Ctrl+G`; premere `Cmd/Ctrl+F` torna al filtro su tutte le settimane.
- **Dev…** — colonna Dev (vedi §14): restano solo i progetti che hanno un dev
  selezionato e, dentro il progetto, solo quel dev. Scorciatoia `Cmd/Ctrl+D`.
- **Categorie…** — colonna Categorie (vedi §14): restano solo i progetti delle
  categorie selezionate. Scorciatoia `Cmd/Ctrl+K`.
- **Anni…** — colonna Anni (vedi §14): restano solo i progetti che iniziano,
  finiscono o stanno tutti negli anni selezionati. Scorciatoia `Cmd/Ctrl+Y`.
- **Milestone…** — gestione milestone: elenco con selettore colore e cestino per
  eliminarle.
- **Worker (visibilità e ghost)…** — finestra con **tutti** i worker (campo di
  ricerca per nome) e tre spunte per ciascuno, salvate nel file:
  - **Footer** — la riga del worker nel footer (nome e ore per settimana). Tolta,
    la riga sparisce e nella griglia le sue celle diventano grigie;
  - **Elenchi** — il worker compare negli elenchi di scelta: colonna Workers dei
    filtri, matrice progetti/worker, inserimento multiplo, saturazione;
  - **Ghost** — il worker è un'anomalia (vedi «Worker "ghost"» nel capitolo 8).

  Per **Footer** ed **Elenchi** la spunta nell'intestazione è il **Select All** di
  quella colonna e vale per i worker **elencati** (rispetta la ricerca); **Ghost**
  non ce l'ha: i ghost si marcano uno per uno.
- **Stato progetti…** — finestra con lo **stato** di ogni progetto: **Attivo**,
  **On-Hold** o **Closed** (vedi «Stato dei progetti» nel capitolo 14). Un progetto
  on-hold o closed sparisce dagli elenchi "attivi" e dall'export.

### Vista
- **Vista compatta** — vista a barre compresse (vedi §15).
- **Bianco/Nero** — resa in scala di grigi (toglie i colori).
- **Progetti** — filtra quali progetti compaiono nel corpo centrale (vedi §15):
  *Solo progetti aperti* (`Cmd/Ctrl+1`), *Solo progetti chiusi* (`Cmd/Ctrl+2`),
  *Tutti* (`Cmd/Ctrl+3`).
- **Tema** — *Auto (sistema)* / *Chiaro* / *Scuro* (vedi §15).
- **Zoom settimane** — *Normale* / *2 settimane* / *4 settimane* (vedi §15).
- **Saturazione worker…** — cruscotto di sintesi del carico (vedi §15).
- **Matrice progetti/worker…** — chi lavora su cosa in un intervallo di date, con
  export PDF (vedi §15).

---

## 6. Gestione dei progetti

Ogni progetto occupa un blocco nella colonna sinistra + le relative righe nella
griglia.

### Creare un progetto
**Aggiungi ▸ + Progetto**. Il nuovo progetto compare vuoto; poi si impostano
tripletta, nome, inizio, fine, categoria e dev.

### Clonare un progetto
**Aggiungi ▸ Clona progetto…** apre una finestra con l'elenco di **tutti** i
progetti (anche chiusi e on-hold), indicati dalla sola **tripletta** (il nome
solo per i progetti che non ne hanno una), e un campo di ricerca sulla tripletta.
Scegli un progetto e premi **Clona**; **Annulla** chiude senza fare nulla. La
copia ha lo **stesso nome** e la tripletta dell'originale seguita da **«Copia»**
(es. `ABC` → `ABC Copia`; solo `Copia` se l'originale non ha tripletta): si
possono poi cambiare dalla griglia come per ogni progetto.

La copia è **completa**: dev con effort, note di cella e note dev, percentuali
dichiarate, date di inizio e fine, categoria, note settimanali del progetto e
milestone. In più:

- la copia è **sempre attiva**, anche se l'originale è chiuso o on-hold;
- compare **subito sotto l'originale** e la griglia vi si sposta;
- le milestone **di sistema** vengono ricollocate nelle stesse settimane (sono
  le stesse milestone, condivise);
- le milestone **personalizzate** dell'originale vengono **duplicate** come
  milestone personalizzate della copia (stesso nome, colore, tipo e categorie,
  stesse settimane): modificarle in un progetto non tocca l'altro.

Da lì in poi i due progetti sono indipendenti.

### Campi del progetto (colonna sinistra)
- **Tripletta**: prima riga. Se vuota mostra un `—` tenue. **Tasto sinistro** per
  modificarla; **tasto destro** apre le **note del progetto** (vedi sotto).
- **Note del progetto**: **tasto destro** sulla tripletta apre le note del
  progetto, organizzate **per settimana**, con l'intestazione della data
  (`--- aa-mm-gg ---`). Solo la **settimana corrente** (in cima ed evidenziata)
  è **modificabile**; le settimane passate sono mostrate in **sola lettura**
  (solo testo). L'elenco **scorre** quando le note sono molte. Con **Salva**
  vengono memorizzate solo le settimane che contengono testo (una settimana
  lasciata vuota non viene salvata); **Annulla** scarta le modifiche.
- **Categoria**: sotto la tripletta (nascosta in vista compatta). **Click** per
  sceglierla dall'elenco.
- **Nome/descrizione**: campo di testo editabile su più righe. Basta cliccarci e
  scrivere; la modifica si conferma perdendo il focus.
- **Inizio** e **Fine**: date del progetto (nascoste in vista compatta). **Tasto
  destro** su ciascuna riga per modificarle. L'inizio tinge la sua colonna di
  azzurro, la fine (deadline) di verde.
- **Avanz.** (avanzamento complessivo, nascosto in vista compatta): due percentuali
  dell'intero progetto nel formato **`presunta%/attuale%`**, entrambe pesate
  sull'effort pianificato (un dev con molte ore incide più di uno con poche):
  - **presunta** = effort fornito fino a oggi / pianificato totale
    (`Σ(usato_fino_a_oggi) / Σ(pianificato)`): la quota di budget consumata finora;
    può superare il 100% se si sta sforando.
  - **attuale** = media delle percentuali **dichiarate** dai dev pesata sul
    pianificato (`Σ(pianificato × dichiarata) / Σ(pianificato)`).

  Contano solo i dev con pianificato > 0; mostra `—` se il progetto non ha alcun
  effort pianificato. Passando il **mouse sopra la riga** appare un tooltip con il
  **calcolo e i numeri** che generano le due percentuali (es. «Presunta 60% = usato
  120h / pianificato 200h», «Attuale 65% = dichiarate ≈130h / pianificato 200h»).

### Riordinare i progetti
Sulla riga della tripletta, a destra, i pulsanti **▲ / ▼** spostano il progetto
su/giù nell'elenco.

### Visibilità, chiusura
- **Abilita/disabilita** dalla finestra **Filtri ▸ Progetti…**: un progetto
  disabilitato non è mostrato nella griglia (ma resta nell'elenco del filtro).
- **Chiudi o metti in on-hold** dalla finestra **Filtri ▸ Stato progetti…**: un
  progetto chiuso (o on-hold) è escluso da elenchi attivi ed export.

> Nota: non esiste un'eliminazione definitiva del progetto; per toglierlo dalla
> vista lo si disabilita o lo si chiude.

---

## 7. Gestione dei dev nel progetto

### Aggiungere/rimuovere dev
La **striscia verticale "Dev"** (tra le info del progetto e le righe dei dev):
- **Click** apre la finestra **"Dev del progetto"** con l'elenco di tutti i dev.
  I dev già nel progetto sono colorati; cliccando un dev lo si aggiunge o rimuove.
- Se si rimuove un dev che ha **già dati** (effort pianificato o settimane
  valorizzate), viene chiesta conferma; senza dati la rimozione è immediata.

### Colonna dei dev (a sinistra della griglia)
Per ogni dev del progetto:
- **Nome dev** su sfondo del colore del dev.
- **Percentuali di avanzamento** (sotto il nome, non in vista compatta):
  - **% presunta** (sola lettura): effort usato fino a oggi ÷ effort pianificato.
    Diventa **rossa oltre il 100 %** (stai sforando il pianificato); non compare se
    il pianificato è 0.
  - **% dichiarata** (editabile): la percentuale di completamento che dichiara lo
    sviluppatore (default 0). Lo **sfondo** del campo è **rosso se la dichiarata è
    minore della presunta**, **verde** se è maggiore o uguale. Serve ad accorgersi
    subito quando il lavoro dichiarato non sta al passo con l'effort consumato. Se
    l'**effort pianificato è 0** il campo **non compare** e non è modificabile.
    Ogni modifica viene **salvata nel file `.ron` con la settimana** in cui avviene
    (uno **storico** con una voce per settimana, l'ultima è il valore corrente): così
    in futuro si potrà tracciare l'**andamento nel tempo** della percentuale. Il campo
    mostra sempre il valore corrente (l'ultima voce).
- **Effort pianificato** (editabile): le ore previste per quel dev.
- **Residuo**: pianificato − totale allocato. Se il residuo è negativo (o pari al
  pianificato con pianificato ≠ 0) lo sfondo diventa rosso e il numero è sempre
  bianco.

### Menù del dev (tasto destro sul nome del dev)
- **Aggiungi riga** — aggiunge una riga worker per quel dev (anche con doppio
  click sul nome).
- **Elimina riga** — rimuove l'ultima riga worker.
- **Cancella gli effort…** — svuota **tutte** le celle di quel dev nella griglia
  (worker assegnati, ore e note di cella), dopo una richiesta di conferma.
  L'effort pianificato, la nota del dev e la % dichiarata restano invariati.
  Disabilitata se il dev non ha celle.
- **Cancella gli effort di tutti i dev…** — come sopra, ma per **tutti i dev del
  progetto** (sempre con conferma). Disabilitata se nessun dev del progetto ha celle.
- **Nota Dev…** — apre l'editor per la nota a livello di dev (un triangolo giallo
  segnala la presenza di una nota).
- **Nascondi effort / Visualizza effort** — nasconde/mostra i numeri di effort di
  quel dev (utile per alleggerire la vista).

I **colori del dev** (sfondo e testo) sono salvati nel file `.ron` per ogni dev.

---

## 8. Inserimento e modifica dell'effort

Nella griglia, ogni dev ha:
- una **riga cumulativa** (sola lettura) in alto, che mostra `svolto | residuo`
  (o solo il residuo se coincidono), con colore che vira dal verde al rosso man
  mano che ci si avvicina/supera il pianificato;
- una o più **righe worker** (editabili), una per ogni persona assegnata.

### Formato della cella
Ogni cella worker ha il formato **`NomeWorker|ore`** (es. `rossi|8`).

### Chi e quando: evidenziazione di data e worker
Passando con il mouse sulla griglia, la **data della colonna** sotto il cursore
viene evidenziata (testo **giallo in grassetto su fondo scuro**, come nella
matrice progetti/worker) sia nell'**intestazione in alto** sia nella **riga delle
date del footer**. Se il cursore è su una cella con un worker, nel footer si
evidenziano anche il **nome del worker** e la sua **cella di quella settimana**
(il suo carico in quella data), così si vede subito chi e quando si sta per
modificare. Mentre una cella è **in modifica** l'evidenziazione resta su quella
cella anche spostando il mouse, e il worker segue il nome che stai digitando. Le
colonne di confine d'anno non vengono evidenziate.

Lo stesso vale passando sul **footer**: sulla riga delle date si evidenzia la data
(anche in alto); su una cella worker × settimana la data, il nome del worker e la
cella stessa; sul **nome di un worker** (parte sinistra del footer) il solo worker.

### Modificare una cella
1. **Click** sulla cella: entra in modalità modifica (sfondo evidenziato, cursore).
2. Digita il nome del worker: parte l'**autocompletamento** (suggerisce i worker
   esistenti). Poi il separatore `|` e le ore.
3. Conferma con **Invio** o **Tab**; annulla con **Esc**.

Le celle prima dell'inizio o dopo la fine del progetto non sono editabili.

### Inserimento multiplo (più worker, più settimane)
**Tasto destro su una cella vuota** apre la finestra **Inserimento multiplo**, il
modo rapido per riempire un blocco di effort senza scrivere cella per cella:

- **elenco worker con spunta** (gli stessi worker della dialog dei filtri, cioè
  quelli con *Mostra nella ricerca* attivo), con **campo di ricerca** e
  **Select All** (il Select All agisce sui worker attualmente elencati, quindi
  rispetta la ricerca);
- **Ore a settimana**: le ore assegnate a **ciascun** worker in **ciascuna**
  settimana (non un totale da distribuire);
- **Per quante settimane**: il numero di settimane consecutive da riempire, a
  partire dalla settimana della cella cliccata (**inclusa**) e andando **in
  avanti**.

Sotto ai campi c'è il riepilogo dell'operazione (`N worker × H h × S settimane`
con la prima e l'ultima settimana). Il pulsante **Inserisci** è attivo solo se
c'è almeno un worker selezionato e i valori sono validi; **Invio** nei campi
numerici equivale a Inserisci, **Annulla**/**Esc** chiude senza scrivere.

Note importanti:
- se un worker selezionato ha **già** un effort in una delle settimane toccate,
  il valore viene **sovrascritto** (la sua eventuale nota resta);
- le settimane oltre la **fine della griglia** vengono scartate: la finestra
  segnala quante settimane sono effettivamente disponibili;
- il tasto destro su una cella **non vuota** continua a fare quello di sempre,
  cioè aprire la **nota** della cella (vedi capitolo 9).

### Copia / Taglia / Incolla
Durante la modifica di una cella funzionano `Cmd/Ctrl+C`, `Cmd/Ctrl+X`,
`Cmd/Ctrl+V`. Incollando una cella copiata dal programma si porta con sé anche la
sua nota; incollando testo esterno si incolla solo il testo.

### Colori del testo nelle celle
- Worker **ghost**: **sempre porpora** (ha di fatto massimo ore 0 → qualsiasi
  inserimento è un'anomalia); ha la precedenza su tutti gli altri colori. Il
  porpora lo distingue dal rosso dei sovra-allocati. Vedi **Worker "ghost"** più
  sotto.
- Worker **nascosto nel footer**: grigio.
- Worker **in sovra-saturazione** (oltre il massimo ore della settimana): rosso.
- Altrimenti: colore testo standard (adattato al tema chiaro/scuro).

### Worker "ghost"
Un worker può essere marcato come **ghost**. Serve a segnalare un'assegnazione
anomala: quando un worker ghost viene inserito nell'effort di un dev,

1. il **nome del dev lampeggia** (alterna colore normale/porpora);
2. la **cella è sempre porpora** nella GUI, indipendentemente dal massimo ore (il
   suo massimo è di fatto 0); anche il suo **effort nel footer** è sempre porpora.
   Il porpora distingue il ghost dai worker sovra-allocati (rossi). **Negli export
   PDF/SVG il ghost resta rosso** (punti 3–4);
3. nel **PDF/SVG dei progetti** il rettangolo della barra è **rosso** nelle
   settimane in cui è stato inserito (le settimane **consecutive** formano un
   tratto unico, non una fila di tessere), anche se in mezzo alla barra del
   colore del dev, e il **nome del dev** (etichetta della riga) è scritto in
   **rosso**. Tutto
   questo dipende dalla spunta **«Includi i worker ghost»** delle finestre di
   export (attiva per impostazione predefinita): con la spunta sono segnate anche
   le settimane in cui il ghost è **assegnato senza ore** (effort 0) e un dev che
   ha **solo** un ghost a zero viene stampato lo stesso; senza spunta il PDF/SVG
   non li evidenzia affatto. Con la sotto-opzione **colore: Colore del
   dev** le stesse settimane sono disegnate nel colore del dev e il nome resta
   nero (vedi capitolo 16);
4. nel **PDF degli andamenti** il tratto di linea (presunta) che sale in quella
   settimana — e il relativo pallino — è **rosso e più spesso**, per far risaltare
   il problema anche in mezzo alla linea del colore del dev.

**Come marcare un worker come ghost:**
- **Filtri ▸ Worker (visibilità e ghost)…** — finestra che elenca **tutti** i
  worker (anche quelli nascosti nel footer o esclusi dal filtro Ctrl+F): la
  colonna **Ghost** ha una spunta per ciascuno. È il modo sempre disponibile.
- In alternativa, **tasto destro sul nome del worker nel footer ▸ Ghost** (comodo,
  ma solo per i worker visibili nel footer).

---

## 9. Note

Ci sono tre tipi di nota, tutte segnalate da un **triangolo giallo** nell'angolo
dell'elemento:

- **Nota di cella (effort)**: **tasto destro** su una cella non vuota → editor
  della nota per quel worker/settimana. (Sulle celle **vuote** il tasto destro
  apre invece l'**inserimento multiplo**, capitolo 8.)
- **Nota del dev**: tasto destro sul nome del dev → **Nota Dev…**.
- **Nota worker/settimana** (nel footer): tasto destro sulla cella del worker →
  **Note**.

Passando il mouse su un elemento con nota, il testo compare come tooltip.

Le note **di progetto** (quelle settimanali) e, se richieste, le note **di cella**
finiscono nella **Minuta** (File ▸ Minuta…, capitolo 5).

---

## 10. Spostare l'effort

Dal **tasto destro sulla riga in alto** di una colonna-dev (la fascia delle
milestone), sottomenù **Sposta**:

- **Sposta blocco** — sposta il blocco contiguo di settimane del dev attorno alla
  settimana selezionata. Si indica di **quante settimane** spostare (positivo =
  avanti, negativo = indietro) e, opzionalmente, quali **milestone** spostare
  insieme.
- **Sposta devs** — permette di scegliere più dev del progetto (con effort) e
  spostarli insieme.

### Spostare inizio e fine del progetto

Nello stesso menù, sotto **Sposta**, ci sono le voci per spostare l'inizio o la
fine del **progetto** alla settimana cliccata. Il menù compare anche **fuori dal
periodo del progetto**:

- **dentro il periodo** — **Sposta inizio progetto qua** e **Sposta fine progetto
  qua** (sulla settimana che è già l'inizio, o già la fine, la voce corrispondente
  è disattivata);
- **prima dell'inizio** — il menù contiene **solo** **Sposta inizio progetto
  qua** (niente milestone né Sposta): il progetto si allunga all'indietro;
- **dopo la fine** — il menù contiene **solo** **Sposta fine progetto qua**: il
  progetto si allunga in avanti.

Se il progetto non ha ancora la data di inizio (o di fine), la voce si chiama
**Imposta inizio progetto qua** (o **Imposta fine progetto qua**).

Se con la nuova data restano **fuori dal progetto** degli effort già inseriti
(di **qualunque dev**, anche le celle con un worker assegnato a 0 ore) o delle
milestone collocate, compare la finestra **Conferma spostamento**. Elenca gli
effort per dev (settimane, ore e intervallo) e le milestone coinvolte:

- **Cancella e sposta** — cancella **solo** quegli effort (con le loro note) e
  toglie dal progetto quelle milestone, poi sposta la data;
- **Annulla** (o la chiusura della finestra) — non cambia niente.

Gli effort e le milestone che restano dentro il nuovo periodo non vengono toccati.
Allungare il progetto (inizio prima, fine dopo) non chiede mai conferma.

Sulle colonne unite dallo **Zoom settimane** il menù non c'è: torna a Normale.

---

## 11. Milestone

Le milestone hanno **nome**, **colore**, **tipo** e **categorie di stampa**.
Possono essere **di sistema** (condivise da tutti i progetti) oppure
**personalizzate di un progetto**: queste ultime compaiono solo negli elenchi di
quel progetto e non in quelli degli altri.

Il **tipo** distingue due usi:

- **Traguardo** — il punto di arrivo classico; nell'export è una **bandierina**.
- **Trigger** — la milestone segna un **evento che scatta** in quella settimana;
  nell'export la bandierina è sostituita da un **mini fulmine** dello stesso
  colore (asta ed etichetta restano identiche).

Il tipo è una proprietà della **milestone**, non della singola collocazione:
cambiarlo vale per tutti i progetti in cui quella milestone è collocata.

Le **categorie di stampa** sono **Internal** ed **External** e una milestone può
appartenere a **una o a entrambe**. Servono **solo in fase di stampa**: decidono in
quale PDF/SVG la bandierina compare (vedi «Milestone da stampare» nel capitolo
16). Nella griglia non cambiano nulla.

- **Internal** — la milestone finisce nella stampa «Solo internal».
- **External** — la milestone finisce nella stampa «Solo external».
- **Internal + External** — finisce in **entrambe**.
- Una milestone **senza categorie** — è il caso di tutte quelle create prima di
  questa funzione — vale **Internal**.
- La stampa **«Mix»** contiene sempre **tutte** le milestone.
- Una milestone ha **sempre almeno una categoria**: togliendo l'ultima spunta si
  ricade su Internal (la spunta rimasta da sola non è cliccabile).

- **Creare una milestone di progetto**: tasto destro sulla riga alta di una
  colonna-dev del progetto → **Aggiungi milestone qui**. In cima al sottomenù ci
  sono la voce **«Categorie: …»** (si apre passandoci sopra col mouse: **Internal**,
  **External** o entrambe — la spunta rimasta da sola non è cliccabile, perché una
  milestone deve avere almeno una categoria) e il campo **«Nuova milestone solo
  qui…»**: scegli le categorie, scrivi il nome e premi Invio (o «+ Milestone di
  progetto»). La milestone nasce **di quel progetto**, con le categorie scelte, e
  viene collocata subito nella settimana su cui hai cliccato. La scelta delle
  categorie resta per l'inserimento successivo. Nasce come **traguardo** con un
  colore assegnato in automatico: tipo e colore (e, volendo, di nuovo le
  categorie) si cambiano poi dal gestore.
- **Creare una milestone di sistema**: Aggiungi ▸ Milestone — prima si scelgono il **tipo** dalla voce
  «Tipo: …» e le **categorie** dalla voce «Categorie: …» (si aprono passandoci
  sopra col mouse), poi si scrive il nome. Tipo e categorie restano per
  l'inserimento successivo, comodo per creare più milestone simili di fila. Il
  colore si imposta dal gestore.
- **Gestire**: Filtri ▸ Milestone… — elenco di **tutte** le milestone (di sistema
  e personalizzate) con selettore colore, **tendina del tipo**, le spunte
  **Int / Ext** delle categorie (tutto modificabile in qualsiasi momento, anche
  dopo l'inserimento) e cestino per eliminarle (l'eliminazione le toglie anche da
  tutti i progetti). Le milestone personalizzate hanno accanto al nome, in
  piccolo, la **tripletta del progetto** a cui appartengono.
- **L'ambito si sceglie alla creazione** e non si cambia più: una milestone di
  sistema resta tale, una di progetto resta legata al suo progetto.
- **Assegnare a una settimana**: tasto destro sulla riga alta di una colonna-dev
  → **Aggiungi milestone qui** → scegli la milestone. L'elenco contiene le
  milestone **di sistema** più quelle **personalizzate di questo progetto**
  (marcate con un «·» finale): le milestone personalizzate di un altro progetto
  non compaiono. Ogni nome è preceduto dall'icona del tipo — **⚑** traguardo,
  **⚡** trigger — nel colore della milestone, così si vede subito quale si sta
  mettendo. Dallo stesso menù
  puoi **rimuoverle**. Nella stessa settimana puoi mettere **più milestone diverse**
  (quelle già presenti sono marcate con «●»); la stessa milestone invece compare
  una sola volta per progetto — riassegnarla a un'altra settimana la sposta.
- **Cancella tutte le milestone…** (in fondo allo stesso sottomenù): toglie dal
  progetto **tutte** le milestone collocate, sia di sistema sia personalizzate,
  dopo una finestra di conferma che le elenca. Le milestone **non vengono
  eliminate**: restano nel gestore e si possono ricollocare. Gli altri progetti
  non cambiano. La voce è disabilitata se il progetto non ha milestone.
- **Visualizzazione**: nella griglia il tipo non cambia nulla (il fulmine compare
  solo negli export). La colonna della settimana con milestone assume il colore
  della milestone. Con **più milestone nella stessa settimana** la colonna è
  divisa in **bande verticali di uguale larghezza**, una per milestone, in
  ordine di creazione. Passando il mouse compaiono i nomi (tutti, anche se in
  vista compatta le bande disegnate sono meno).

---

## 12. Il confine di fine anno

Tra l'ultima settimana di un anno e la prima del successivo il programma inserisce
una **colonna gialla ("Effort residuo")**, più stretta:

- Per i progetti **a cavallo** dell'anno, mostra per ogni dev le **ore mancanti**
  al confine (pianificato − allocato nell'anno che finisce), e in cima il
  **totale del progetto** (`T:…`) da completare oltre il confine.
- Le scritte su questa colonna gialla sono sempre in nero (indipendenti dal tema).

---

## 13. Il footer (worker e totali)

Il footer si può nascondere/mostrare con la maniglia (triangolino) sul bordo.

### Footer sinistro
- **Selettore categoria** (sopra i nomi dev): limita i totali-anno a una categoria
  ("Tutte" per nessun filtro).
- **Selettore anno** (sopra i totali): sceglie l'anno dei totali per dev
  ("Tot"/"—" per nessuno).
- **Totali per dev**: per ogni dev il totale ore nell'anno/categoria selezionati.
- **Riga "Totale"**: somma dei totali-dev.
- **Dev esclusi dai totali**: un dev può essere tolto da questo riquadro — la sua
  riga sparisce e le sue ore **non entrano nel "Totale"** — restando visibile in
  tutto il resto del programma (griglia, menù, filtri, export). Per escluderlo:
  **tasto destro sul nome del dev nel footer ▸ Nascondi dai totali**, oppure
  **Filtri ▸ Dev nei totali…**, la finestra che elenca **tutti** i dev con una
  spunta ciascuno (più **Select All**): togli la spunta per escludere, rimettila
  per far tornare il dev. È l'unico punto da cui si può rimettere un dev già
  nascosto. La scelta è **salvata nel file**.
- **Ore rimanenti per worker**: a destra di ogni riga worker, le ore ancora
  disponibili (colore verde).

### Footer destro (per worker × settimana)
Per ogni worker e ogni settimana mostra il valore **sovra** (ore allocate), con:
- **Colore**: rosso se supera il massimo; giallo se zero; marrone se il massimo è
  stato forzato a zero; verde altrimenti. (I colori si adattano al tema.)
- **`valore | max`** quando il massimo effettivo differisce da quello globale.
- **Triangoli di stato**: ferie (azzurro, in alto a sinistra) / malattia (rosso,
  in basso a sinistra).
- **Triangolo giallo**: nota worker/settimana presente.

Interazioni:
- **Click** sulla cella worker/settimana → imposta il **massimo ore** per quella
  settimana (con pulsanti OK / Default / Zero / Annulla).
- **Tasto destro** → menù **Ferie** / **Malattia** (fanno da interruttore) /
  **Note**.
- **Click sul nome del worker** (colonna sinistra del footer) → **evidenzia il
  worker** nelle celle dei dev di tutti i progetti: la sua riga «Worker|ore» diventa
  gialla in grassetto su fondo scuro, e resta così anche spostando il mouse. Un
  nuovo click sullo stesso nome toglie l'evidenziazione; un click su un altro
  worker la sposta su quello. Vale solo per la sessione e non si vede nella vista
  compatta né sulle colonne con zoom, dove i nomi dei worker non compaiono.
- **Tasto destro sul nome del worker** → menù con **Ore max…** (massimo ore
  **globale** del worker) e il toggle **Ghost** (vedi §8, «Worker "ghost"»).
- **Click sull'intestazione di una settimana** (header) → imposta il massimo ore
  di quella settimana **per tutti i worker**.

### Filtro effort del footer
Sopra la sezione worker, tre pulsanti: **Tutti** / **Nulli** / **≥40**, per
mostrare rispettivamente tutti i valori, solo quelli a zero, o solo quelli ≥ 40.

Per i worker **ghost** questi filtri non valgono: con **Nulli** o **≥40** la loro
cella non mostra nulla (i ghost compaiono nel footer solo con **Tutti**). Il colore
delle celle **ghost nella griglia** resta comunque sempre porpora, anche quando il
ghost non è visualizzato nel footer.

---

## 14. Filtri

Workers, Progetti, Dev, Categorie e Anni vivono in **un'unica dialog «Filtri»** a
cinque colonne affiancate. La aprono indifferentemente `Cmd/Ctrl+F` (Workers),
`Cmd/Ctrl+G` (Workers sulla settimana corrente), `Cmd/Ctrl+P` (Progetti), `Cmd/Ctrl+D`
(Dev), `Cmd/Ctrl+K` (Categorie) e `Cmd/Ctrl+Y` (Anni), oppure le voci corrispondenti del menù **Filtri**: cambia
solo la colonna che riceve il **focus** sul proprio campo di ricerca. Ogni colonna ha
**ricerca**, **«Select All»** ed elenco con spunte, e i filtri restano indipendenti
(si combinano tra loro). `Esc` o un click fuori chiudono la dialog.

Premendo di nuovo la **stessa** scorciatoia a dialog aperta si fa il **toggle di
"Select All"** della sua colonna (quindi la seconda pressione deseleziona tutto);
il toggle agisce sugli elementi **attualmente elencati**, cioè rispetta la ricerca.
`Shift+Cmd/Ctrl+F` / `+G` / `+D` / `+K` / `+Y` deselezionano tutto senza aprire la dialog.

**`Cmd/Ctrl+J` azzera tutti i filtri**: worker, dev, categorie e anni tornano *tutti
selezionati*, tutti i progetti aperti tornano visibili, la modalità «Solo settimana
corrente» si spegne e le ricerche si svuotano — cioè si torna a vedere tutto. Non è un
toggle: premuto di nuovo, anche a dialog aperta, **non deseleziona nulla** e
ripristina semplicemente lo stesso stato. Non tocca i progetti **chiusi** (restano
nascosti finché non li riapri da «Filtri ▸ Stato progetti…») né la modalità **Vista ▸
Progetti** (`Cmd/Ctrl+1/2/3`).

### Progetti — visibilità + salto rapido (colonna «Progetti», `Cmd/Ctrl+P`)
La colonna contiene:
- una **casella di ricerca** in cima (auto-focus): digita e l'elenco si filtra in
  tempo reale (la ricerca combacia con tripletta o nome);
- per ogni progetto una **spunta di visibilità** (disabilitarlo lo nasconde dalla
  griglia) e la **tripletta cliccabile**;
- in elenco si mostra **solo la tripletta** (o il nome se la tripletta è assente).

Fai **click sulla tripletta**, o premi **Invio** per il primo risultato, e la
griglia scorre fino a quel progetto (la dialog si chiude e la ricerca si azzera).
"Select All" agisce sui progetti elencati.

> Suggerimento: se resta **un solo progetto visibile**, l'esportazione PDF passa
> alla modalità "singolo progetto" (vedi §16).

### Filtro worker (colonna «Workers», `Cmd/Ctrl+F`)
Permette di mostrare solo alcuni worker. Con un filtro attivo:
- vengono mostrate **solo le righe dei worker selezionati**;
- i progetti senza worker corrispondenti spariscono;
- nella colonna sinistra, per ogni progetto **resta visibile solo la tripletta**
  (categoria, nome, inizio/fine spariscono) così da non occupare spazio.

Scorciatoie: `Cmd/Ctrl+F` apre la dialog sulla colonna Workers; premuto a **dialog
già aperta** fa il **toggle di "Select All"**; `Shift+Cmd/Ctrl+F` **deseleziona
tutti** i worker.

Nell'elenco compaiono solo i worker con la proprietà **`show_in_find`** attiva
(impostazione predefinita: attiva). Un worker con `show_in_find` disattivato — si
imposta con la spunta **Elenchi** di **Filtri ▸ Visibilità worker…** — non appare
in questa lista.

Accanto a ogni nome è indicato **in quanti progetti si trova** il worker (dove ha
effort assegnato), suddiviso in aperti e chiusi nel formato **`N/Aperti - M/Chiuso`**.
Il conteggio **segue la modalità**: con `Cmd/Ctrl+F` considera **tutte le settimane**
(progetti aperti/chiusi in assoluto); con `Cmd/Ctrl+G` (settimana corrente) conta solo
i progetti aperti/chiusi in cui il worker è presente **nella settimana corrente**.

### Filtro worker sulla settimana corrente (`Cmd/Ctrl+G`)
Variante del filtro worker che risponde alla domanda **«a quali progetti sta
lavorando questo worker in questa settimana?»**. Usa la **stessa finestra e la stessa
selezione** del filtro Workers normale, ma cambia il criterio con cui i progetti
compaiono:
- un progetto è mostrato **solo se** almeno un worker selezionato è **assegnato nella
  settimana corrente** (la settimana di oggi), anche con **effort 0**;
- **dentro** i progetti mostrati la visualizzazione è identica al filtro normale
  (righe dei worker selezionati su **tutte** le settimane): Ctrl+G decide solo *quali*
  progetti compaiono, non nasconde nulla all'interno.

La modalità è mostrata (e commutabile) dalla spunta **«Solo settimana corrente»** in
cima alla colonna Workers. Premere `Cmd/Ctrl+G` a dialog aperta **in questa modalità**
fa il **toggle di "Select All"**; premerlo mentre è attiva l'altra modalità si limita a
commutarla (e viceversa con `Cmd/Ctrl+F`). `Shift+Cmd/Ctrl+G` deseleziona tutti i worker.

### Filtro dev (colonna «Dev», `Cmd/Ctrl+D`)
Risponde alla domanda **«su quali progetti si lavora per questo dev?»**. La colonna
elenca **tutti i dev definiti** con una spunta ciascuno (più ricerca e "Select All"):
- restano visibili **solo i progetti** in cui almeno un dev selezionato ha **almeno un
  worker assegnato** in una qualsiasi settimana, **anche con effort 0** (conta
  l'assegnazione, non le ore); un dev aggiunto al progetto e mai compilato non lo fa
  comparire;
- **dentro** il progetto sono disegnate **solo le righe dei dev selezionati**; gli
  altri dev spariscono;
- come per il filtro worker, l'intestazione del progetto mostra **solo la tripletta**,
  così l'elenco resta compatto.

Il filtro dev si **combina in AND** con il filtro worker (`Cmd/Ctrl+F` / `Cmd/Ctrl+G`):
con entrambi attivi restano i progetti che soddisfano tutti e due i criteri, le righe
dei dev selezionati e, dentro le celle, solo i worker selezionati.

Scorciatoie: `Cmd/Ctrl+D` apre la dialog sulla colonna Dev; premuto a **dialog già
aperta** fa il **toggle di "Select All"**; `Shift+Cmd/Ctrl+D` **deseleziona tutti** i
dev. Con tutti i dev selezionati il filtro è considerato spento. La selezione **non** è
salvata sul file.

### Filtro categoria (colonna «Categorie», `Cmd/Ctrl+K`)
Risponde alla domanda **«quali progetti appartengono a questa categoria?»**. La colonna
elenca, con una spunta ciascuna (più ricerca e "Select All"), la voce **«Senza
categoria»** (progetti a cui non è stata assegnata una categoria) seguita da **tutte le
categorie definite**:
- restano visibili **solo i progetti** la cui categoria è selezionata;
- il filtro agisce sul **progetto intero**: dentro il progetto non sparisce nulla e
  l'intestazione resta **completa** (a differenza dei filtri worker e dev);
- come la visibilità della colonna Progetti, vale anche per gli **elenchi di
  esportazione** (PDF, SVG, Andamento, Minuta, Report), che partono dai progetti
  mostrati a schermo.

Si **combina in AND** con gli altri filtri e con la modalità **Vista ▸ Progetti**.
Scorciatoie: `Cmd/Ctrl+K` apre la dialog sulla colonna Categorie; premuto a **dialog
già aperta** fa il **toggle di "Select All"**; `Shift+Cmd/Ctrl+K` **deseleziona tutte**
le categorie. Con tutte le voci selezionate il filtro è considerato spento. La
selezione **non** è salvata sul file. (Il selettore di categoria nel footer, che
limita i totali-anno per dev, è indipendente da questo filtro.)

### Filtro anno (colonna «Anni», `Cmd/Ctrl+Y`)
Risponde alla domanda **«quali progetti riguardano questo anno?»**. La colonna elenca
gli **anni** che compaiono nelle date di inizio o di fine dei progetti, con ricerca,
"Select All" e una spunta per anno. Sopra l'elenco si sceglie il **criterio**:
- **Inizia nell'anno** (predefinito) — la data di **inizio** del progetto cade in uno
  degli anni spuntati;
- **Finisce nell'anno** — la data di **fine** cade in uno degli anni spuntati;
- **Tutto nell'anno** — inizio **e** fine cadono nello **stesso** anno, e quell'anno è
  spuntato. Un progetto che va dal 2025 al 2026 **non** passa nemmeno con entrambi
  gli anni spuntati.

Con il filtro attivo, un progetto **senza la data** richiesta dal criterio (inizio,
fine o entrambe) è **nascosto**. Come il filtro categoria agisce sul **progetto
intero** (intestazione completa, nessuna riga nascosta), vale anche per gli **elenchi
di esportazione** e si combina in **AND** con gli altri filtri e con **Vista ▸
Progetti**.

Scorciatoie: `Cmd/Ctrl+Y` apre la dialog sulla colonna Anni; premuto a **dialog già
aperta** fa il **toggle di "Select All"**; `Shift+Cmd/Ctrl+Y` **deseleziona tutti** gli
anni. Con **tutti gli anni spuntati** il filtro è considerato **spento**, qualunque sia
il criterio. Né la selezione né il criterio sono salvati sul file.

### Worker: visibilità e ghost (Filtri ▸ Worker (visibilità e ghost)…)
Finestra che elenca **tutti** i worker (anche quelli nascosti nel footer o esclusi
dal filtro Ctrl+F), con ricerca per nome e tre spunte per ciascuno: **Footer**,
**Elenchi** (vedi «Filtri» nel capitolo 3) e **Ghost**. È il modo sempre
raggiungibile per associare/togliere il ghost. Vedi §8, «Worker "ghost"».

### Stato dei progetti (Filtri ▸ Stato progetti…)
Finestra con tutti i progetti in ordine alfabetico e, accanto a ciascuno, una
**combo** con il suo stato:

- **Attivo** — il progetto è aperto;
- **On-Hold** — il progetto è sospeso: **si comporta in tutto come un progetto
  chiuso** (grigio nella griglia, escluso dagli elenchi attivi e dall'export,
  visibile con *Solo chiusi*), ma nella **matrice progetti × worker** è indicato
  come «on-hold» invece che «chiuso»;
- **Closed** — il progetto è chiuso.

Nel file un progetto on-hold resta registrato come chiuso più l'indicazione
`on_hold: true`: una versione precedente del programma lo apre semplicemente come
chiuso.

---

## 15. Viste: compatta, bianco/nero, tema, zoom

### Vista compatta (Vista ▸ Vista compatta)
Comprime la griglia: al posto delle celle mostra, per ogni dev e settimana attiva,
una **barra** la cui altezza è proporzionale all'effort. Nasconde categoria,
inizio/fine e riduce le info di progetto alla sola tripletta. Utile per una
panoramica su periodi lunghi.

### Bianco/Nero (Vista ▸ Bianco/Nero)
Rende tutta l'interfaccia in scala di grigi (nessun colore).

### Progetti: aperti / chiusi / tutti (Vista ▸ Progetti)
Sceglie quali progetti mostrare nel corpo centrale, **in aggiunta** al filtro di
visibilità (Filtri ▸ Progetti…) e al filtro worker:
- **Solo progetti aperti** (`Cmd/Ctrl+1`) — nasconde i progetti chiusi.
- **Solo progetti chiusi** (`Cmd/Ctrl+2`) — mostra solo i progetti chiusi.
- **Tutti** (`Cmd/Ctrl+3`) — mostra sia gli aperti sia i chiusi.

I **progetti chiusi si vedono sempre in grigio** (scala di grigi), anche quando il
Bianco/Nero generale è spento, così si distinguono a colpo d'occhio dagli aperti.

Questa scelta **non viene salvata**: a ogni avvio si riparte da *Solo progetti
aperti*.

Gli elenchi di **esportazione PDF, SVG e Minuta** seguono questa modalità: propongono
esattamente i progetti presenti nel corpo centrale (ad es. in *Solo progetti chiusi*
esportano i progetti chiusi).

### Tema (Vista ▸ Tema)
Tre stati:
- **Auto (sistema)** — segue l'aspetto di macOS. Se il sistema è su "Automatico",
  l'app diventa chiara di giorno e scura la sera.
- **Chiaro** — forza il tema chiaro.
- **Scuro** — forza il tema scuro.

Il tema cambia sfondi e testi (gli accenti restano); si adeguano anche menù,
popup e campi di testo.

### Zoom settimane (Vista ▸ Zoom settimane)
Disponibile solo in vista normale (non compatta). Unisce più settimane in una sola
colonna, **sommandone l'effort**:
- **Normale** — una colonna per settimana.
- **2 settimane** — colonne da 2 settimane unite.
- **4 settimane** — colonne da 4 settimane unite.

Nelle colonne unite:
- per ogni dev restano la **riga cumulativa** e **una cella con la somma** delle
  ore del gruppo; **i nomi dei worker spariscono**;
- le celle sono in **sola lettura** (non modificabili);
- **restano visibili** milestone, inizio e fine del progetto;
- un gruppo **non attraversa mai il confine di fine anno** (la colonna gialla
  resta separata e il raggruppamento riparte a ogni anno).

Lo zoom si può aumentare e diminuire liberamente (avanti/indietro).

### Saturazione worker (Vista ▸ Saturazione worker…)
Cruscotto di sintesi del carico delle persone, per **pianificare e ribilanciare**
senza scorrere la griglia. È in sola lettura e aggrega i dati esistenti (ore
allocate `sovra`, capacità = max ore effettive della settimana). Mostra **tutti i
worker con `show_in_find` attivo _oppure_ non nascosti nel footer** (non dipende dal
filtro worker Ctrl+F). Contiene:

- un selettore **Settimana / Mese** (granularità);
- una casella **"Solo da settimana corrente"** che nasconde le settimane passate
  (mostra solo presente e futuro);
- un **riepilogo**: numero di sovra-allocazioni e ore in eccesso totali;
- una **colonna fissa** a sinistra con il **nome del worker** e la colonna **Σ**
  (totale allocato/capacità e numero di settimane in sovra): resta **sempre visibile**
  anche scorrendo la heatmap verso destra (scorre solo in verticale, insieme alle
  righe);
- **selezione delle righe**: un **click sul nome** di un worker **seleziona** tutta la
  sua riga di effort (evidenziata con bordo arancione); un secondo click la
  **deseleziona**; si possono tenere selezionate **più righe** contemporaneamente (il
  click su un altro worker non annulla le selezioni precedenti);
- una **heatmap** worker × (settimana|mese) colorata per saturazione
  (**verde** = libero, **giallo** = pieno, **rosso** = oltre capacità); ogni cella
  mostra le ore, col tooltip `allocato / capacità`, ed è cliccabile per saltare a
  quella settimana nella griglia; la **settimana corrente** è evidenziata (chip
  verde nell'intestazione e bordo verde brillante sulle celle).

La finestra **non è ridimensionabile**: per vedere quale settimana è stata
selezionata basta **spostarla**. Nota: **ferie/malattia non azzerano la capacità**
(si possono fare pochi giorni di ferie e lavorare gli altri), quindi il conteggio
ore resta quello reale.

### Matrice progetti × worker (Vista ▸ Matrice progetti/worker…)
Una tabella «chi lavora su cosa»: **una colonna per worker**, **una riga per
progetto**.

1. **Scelta dei worker** — si apre prima una finestra con l'elenco dei worker (quelli
   con «mostra nella ricerca» attivo), un campo di ricerca e *Select All*. Spunta i
   worker e premi **Mostra matrice**. La selezione resta **ricordata finché il
   programma è aperto**: chiudendo e riaprendo la matrice ritrovi gli stessi worker
   (al riavvio dell'app riparte vuota). Dalla matrice il pulsante **Worker…** riapre
   la scelta.
2. **Intervallo di date** — in alto, a sinistra **Da:** e a destra **A:**; ognuno può
   essere **Oggi** oppure una **Data** scelta col calendario. **In mezzo** c'è la
   combo **«Anni»**, con gli anni di inizio/fine dei progetti (come il filtro Anni,
   `Cmd/Ctrl+Y`): spuntando uno o più anni, **Da** diventa il **1° gennaio dell'anno
   più vecchio** e **A** il **31 dicembre del più recente** (con 2024 e 2026
   l'intervallo copre anche il 2025). Da e A restano modificabili: se li cambi a mano
   la combo si svuota. Accanto c'è la combo **«Categorie»** (prima «Senza
   categoria», poi le categorie): spuntandone una o più la matrice mostra **solo i
   progetti di quelle categorie**, e riepilogo per categoria e «Totale progetti»
   contano solo quelli; nessuna spunta = tutte le categorie. Ancora accanto, la
   combo **«Stato»** (**Attivi**, **On-Hold**, **Closed**): per default mostra
   **tutti** gli stati; spuntandone uno o più la matrice mostra solo i progetti in
   quegli stati, e il **riepilogo** si adegua: conta solo quei progetti e ha le
   sotto-righe solo per gli stati scelti. Una settimana conta se
   **si sovrappone anche di un solo giorno** all'intervallo. Se la data iniziale è
   successiva alla finale compare un avviso e la matrice non viene mostrata.
3. **Righe** — compaiono **tutti i progetti** (aperti e chiusi, indipendentemente da
   Vista e filtri) in cui almeno un worker scelto è **assegnato** nell'intervallo,
   **anche con effort 0**. I progetti sono **raggruppati per categoria**: ogni gruppo
   ha un'intestazione col nome della categoria e righe colorate con la sua tinta
   (colore automatico per categoria, grigio per «Senza categoria», che va in fondo).
   Un **progetto chiuso** resta nel gruppo della sua categoria: la sua riga mantiene
   il colore della categoria ma **spento verso il grigio e a strisce**, con
   l'indicazione «chiuso» (oppure «on-hold», se il progetto è on-hold) accanto
   alla tripletta; conta comunque nel riepilogo.
4. **Celle** — all'incrocio progetto × worker ci sono i **dev** con cui il worker è
   assegnato nell'intervallo, **uno per riga** se sono più d'uno.
5. **Riepilogo** — subito sotto la matrice (allineato alle sue colonne), una riga per
   categoria: per ogni worker, in quanti progetti di quella categoria è presente
   (vuoto = nessuno), seguita da tre sotto-righe con la **suddivisione per stato**:
   **Attivi**, **On-Hold** e **Closed**; in fondo il **Totale progetti** in cui il
   worker è coinvolto, con la stessa suddivisione. Nel PDF, se il riepilogo non
   entra nella pagina, continua sulla successiva con l'intestazione dei worker.
6. **Lettura** — la riga con i **nomi dei worker resta sempre visibile** in cima
   mentre scorri la matrice in verticale. Passando col mouse su una **cella**:
   - **tutta la colonna** del worker, riepilogo compreso, ha un velo leggero
     (scuro col tema chiaro, chiaro col tema scuro);
   - la **tripletta** della riga (o l'intestazione a sinistra, nelle righe del
     riepilogo) e il contenuto della cella puntata (i **dev**, o il **valore**
     nel riepilogo) diventano **gialli in grassetto su fondo scuro**;
   - il **nome del worker** è a colori invertiti.

   Tolto il mouse, le scritte tornano del colore normale. Sulla tripletta si
   evidenzia solo lei, sul nome del worker solo la colonna.

**Esporta PDF…** crea subito il PDF (si sceglie solo dove salvarlo) con la matrice,
il riepilogo e l'intervallo di date in testa; se nella combo «Anni» hai scelto degli
anni, delle categorie o degli stati, il sottotitolo li riporta dopo l'intervallo (es. `From
01/01/2024 to 12/31/2026  ·  Years: 2024, 2026  ·  Categories: Cliente A`). Come gli altri export è in inglese
(date `mm/dd/yyyy`), su **A4 orizzontale**: se i worker non entrano in larghezza le
colonne vengono divise in più blocchi (in ogni blocco solo i progetti dei suoi
worker), e se le righe non entrano la tabella continua sulle pagine successive con
l'intestazione ripetuta.

---

## 16. Esportazione PDF

Ogni progetto idoneo diventa una **pagina** in stile Gantt con: titolo (tripletta)
e descrizione, asse dei mesi, milestone come bandierine (**fulmine** per quelle di
tipo trigger), marker "Today", e una riga per dev.

> **Lingua e date degli export.** Tutti i PDF (Gantt, Andamento, Report) e gli SVG
> sono stampati **in inglese** (mesi `Jan`…`Dec`, «Today», «today», intestazioni del
> report…) e con le **date all'americana**: `mm/dd/yyyy` per le date complete (data di
> creazione nel footer, inizio/fine nel report) e `Sep 22` (mese prima del giorno) per
> le etichette brevi delle barre e delle milestone (con l'anno opzionale `Sep 22 '26`
accanto alle barre, vedi «Anno nelle date dei dev»). La GUI resta in italiano.

Un progetto è **idoneo** se è abilitato, non chiuso e ha **sia inizio sia fine**.

Barre dell'effort e celle dei mesi sono disegnate con gli **angoli arrotondati**;
quanto, lo decide `corner_pct` nel file dati (vedi capitolo 2).

### Esportazione con più progetti visibili
**File ▸ Esporta…** con più progetti visibili apre una **finestra di selezione dei
progetti**:

- elenca i **progetti visibili** (abilitati e non chiusi), identificati dalla **tripletta** (o dalla descrizione se assente);
- **Select All** in cima per selezionare/deselezionare tutti;
- **checkbox** per includere/escludere ciascun progetto. Le spunte sono **ricordate
  finché il programma è aperto**: riaprendo la finestra ritrovi le scelte
  dell'ultima volta; un progetto mai comparso prima nell'elenco parte spuntato. Alla
  chiusura del programma le scelte si perdono;
- **Esporta PDF…** genera un PDF con una pagina per ogni progetto **selezionato e
  idoneo** (uno o due file, secondo le categorie milestone scelte); **Annulla**
  chiude senza esportare.

Nel Gantt i dev **con effort** sono ordinati per data di inizio dell'effort; i dev
senza effort non compaiono.

### Esportazione di un singolo progetto
Se hai **filtrato fino a un solo progetto visibile**, **File ▸ Esporta PDF…** apre
una **finestra di selezione**, intitolata **«Esporta PDF/SVG»** seguito dalla
tripletta del progetto:

- in cima, **«Periodo da disegnare»**: due date, **Da** e **A**, scelte con il
  calendario. Nel PDF/SVG viene disegnata **solo questa parte** del grafico:
  - l'asse dei mesi parte da **Da** e finisce con **A** (i mesi agli estremi
    possono restare tagliati); ogni data vale per **l'intera settimana** che la
    contiene;
  - le barre dei dev che escono dal periodo vengono **tagliate ai bordi**, ma la
    data accanto alla barra resta quella **reale** del dev;
  - un dev selezionato che ha effort **solo fuori dal periodo** non ha né barra
    né linea: compaiono solo le sue **date reali** (e le percentuali, se
    richieste) — al **bordo sinistro** del grafico se ha **già finito**, al
    **bordo destro** se **deve ancora iniziare**, **al centro** se lavora prima e
    dopo il periodo (il periodo cade in una sua pausa);
  - un dev **senza alcun effort** resta la solita **riga sottile**;
  - le **milestone fuori dal periodo** non vengono disegnate, anche se spuntate.

  La **prima volta** che apri l'export di un progetto le date sono il suo
  **inizio** e la sua **fine**; da lì in poi il programma **ricorda** il periodo
  scelto **per quel progetto** (fino alla chiusura del programma) e non lo cambia
  più, nemmeno se sposti inizio o fine del progetto. Ogni progetto ha il suo
  periodo. Il pulsante **Inizio/fine progetto** rimette le date attuali del
  progetto. Le date possono anche uscire dal periodo del progetto. Se **Da** è
  dopo **A** compare un avviso e i pulsanti di export sono disattivati;
- elenca **tutti i dev del progetto**, anche quelli **senza effort**; la prima
  volta sono tutti spuntati e nell'ordine della lista dev;
- **Select All** in cima all'elenco dei dev (scorre insieme a loro) per
  selezionare/deselezionare tutti;
- **checkbox** per includere/escludere ciascun dev;
- **trascinamento** per riordinare i dev (una linea arancione indica il punto di
  inserimento); il rilascio sposta il dev;
- un **menu del formato barra** accanto a ogni dev (*Barra continua*,
  *Segmentata*, *Segmentata + altezza %*, ciascuna segmentata anche nella variante
  *date sotto*; vedi «Formato delle barre dei dev»);
- accanto all'elenco dei dev (a destra), **«Milestone da includere»**: l'elenco delle milestone
  **collocate in questo progetto**, in ordine di settimana, con **Select All** in
  cima e una **checkbox** per ciascuna. Ogni riga mostra, nel colore della
  milestone, l'icona del tipo (**⚑**/**⚡**), il nome, la settimana e le categorie
  fra parentesi quadre (`[Int]`, `[Int/Ext]`, `[Ext]`). Vengono disegnate **solo
  le milestone spuntate**; se il progetto non ne ha nessuna compare una scritta
  al posto dell'elenco;
- l'elenco dei dev e quello delle milestone mostrano **al massimo 10 righe**
  ciascuno: se sono di più, l'elenco **scorre**;
- **memoria delle scelte**: spunte dei dev, loro **ordine** e spunte delle
  milestone sono ricordate **per progetto** finché il programma è aperto (anche se
  chiudi con Annulla): riaprendo l'export dello stesso progetto ritrovi tutto come
  l'avevi lasciato. Dev o milestone aggiunti nel frattempo compaiono spuntati (i dev
  in fondo). Alla chiusura del programma le scelte si perdono;
- **Esporta…** genera il PDF con i dev selezionati, nell'ordine scelto; **Annulla**
  chiude.

Regole del PDF a singolo progetto:
- i dev **con effort** producono la barra colorata normale;
- i dev **senza effort** producono una **riga sottile** del colore del dev, che
  copre tutta la larghezza del calendario;
- si può esportare **anche senza alcun dev selezionato**: la pagina esce comunque
  con asse, milestone e resto delle informazioni;
- la scelta delle milestone si combina **in AND** con le categorie: una milestone
  finisce nel file solo se è **spuntata** nell'elenco **e** rientra nell'ambito di
  quel file (nel PDF External, quindi, solo le milestone spuntate **e** marcate
  External). Togliendo tutte le spunte la pagina esce senza bandierine.

### Esportazione del solo grafico in SVG
Nella stessa finestra c'è **Esporta SVG…**: salva **solo il grafico** (lo stesso
Gantt del PDF) in formato **SVG** vettoriale, **senza** tripletta/descrizione del
progetto e **senza** la data in fondo. Usa la stessa selezione/ordine dei dev e la
stessa selezione delle milestone.
L'immagine è ritagliata al contenuto effettivo del grafico.

### Milestone da stampare: Solo internal / Solo external / Mix
In tutte le finestre di export (PDF a più progetti, PDF del singolo progetto, SVG)
c'è il riquadro **«Milestone da stampare»** con tre caselle:

- **Solo internal** — il file contiene le milestone marcate **Internal** (comprese
  quelle Internal + External e quelle senza categoria, che valgono Internal).
- **Solo external** — il file contiene le milestone marcate **External** (comprese
  quelle Internal + External).
- **Mix** — il file contiene **tutte** le milestone.

Le caselle sono indipendenti e **ognuna spuntata produce un file**:

- **una sola spuntata** → viene creato **un file**, con il nome scelto nel dialog
  di salvataggio;
- **più di una** → vengono creati **due o tre file** con una sola scelta del nome: al
  nome indicato viene aggiunto il suffisso dell'ambito, per esempio
  `progetti_2026_07_13_internal.pdf`, `progetti_2026_07_13_external.pdf` e
  `progetti_2026_07_13_mix.pdf` (una notifica per ciascun file salvato);
- **nessuna** → i pulsanti di export sono disattivati: non c'è niente da stampare.

A cambiare è **solo** quali bandierine vengono disegnate: progetti, dev, barre,
date e percentuali restano identici nei vari file. Nella finestra del **singolo
progetto** questa scelta si somma a quella per singola milestone (vedi sopra):
viene stampato solo ciò che passa **entrambi** i filtri. La scelta è ricordata tra un
export e l'altro (non è salvata sul file); all'avvio sono spuntate **tutte e
tre**, quindi di default escono tre file.

### Worker ghost nella stampa
In tutte le finestre di export c'è la casella **«Includi i worker ghost (anche a
effort 0)»**, **attiva** per impostazione predefinita:

- **attiva** — i worker ghost sono evidenziati in rosso come sempre (rettangoli
  rossi sulle settimane interessate e nome del dev in rosso) e, in più, contano
  anche le settimane in cui il ghost è **assegnato senza ore**: quelle settimane
  vengono marcate lo stesso, sulla barra o sulla riga sottile del dev. Un dev la
  cui **unica** assegnazione è un ghost a effort 0 compare nel PDF anche
  nell'export a più progetti, che normalmente stampa solo i dev con effort;
- **disattiva** — la stampa ignora del tutto i ghost: nessun rosso, e i dev senza
  ore restano fuori come prima.

Sulla stessa riga della casella, attiva solo quando è spuntata, c'è la scelta del **colore**:

- **Rosso** (predefinito) — come descritto sopra: settimane ghost e nome del dev
  in rosso;
- **Colore del dev** — le settimane ghost sono disegnate **nel colore del dev**
  (sulla barra o sulla riga sottile) e il nome del dev resta **nero**: il ghost
  non è segnalato come anomalia. Resta però valido il resto della spunta: le
  settimane con il ghost a effort 0 sono comunque disegnate (con la barra
  continua possono quindi allungarla) e il dev con solo un ghost a zero compare.

Le scelte valgono sia per il PDF sia per l'SVG ed è ricordata tra un export e l'altro
(non è salvata sul file). Il PDF **Andamento…** non è interessato: lì i ghost sono
sempre segnati sulle settimane in cui hanno ore.

### Formato delle barre dei dev
Il formato della barra si sceglie **per ogni dev** nell'export di un singolo
progetto, e per tutti insieme nell'export con più progetti. Le scelte valgono sia
per il PDF sia per l'SVG e sono ricordate finché il programma è aperto (non sono
salvate sul file).

- **Export di un singolo progetto** — accanto a ogni dev c'è un menu a tendina con
  il suo formato. Sotto gli elenchi, a sinistra (a destra ci sono le opzioni di stampa), il selettore con le **anteprime** si chiama
  **«Formato barre — applica a tutti i dev»**: un click su un'opzione la imposta su
  **tutti** i dev, dopodiché puoi cambiare il singolo dev dal suo menu. Le scelte
  sono ricordate **per progetto** (come spunte e ordine dei dev); un dev mai visto
  prima parte dall'ultimo formato applicato a tutti.
- **Export con più progetti** — resta un unico selettore **Formato barre** con le
  anteprime, che vale per tutti i dev di tutti i progetti, più la casella **«Date
  sotto ogni tratto»** (attiva solo con un formato segmentato).

**Date sotto ogni tratto.** Con i due formati segmentati il menu del dev offre anche
la variante **«…, date sotto»** (*Segmentata, date sotto* e *Segmentata + altezza %,
date sotto*). Le date non vanno più a destra della barra ma **sotto ogni tratto**,
centrate, su **due righe**: sopra l'inizio (lunedì della prima settimana del tratto),
sotto la fine (venerdì dell'ultima). L'anno segue la stessa regola dell'etichetta a
destra (solo sulla fine se le due date sono nello stesso anno). A destra della barra
restano solo le **percentuali**, se attive. Le righe di questi dev sono un po' più
alte per far posto alle date; se i dev sono tanti, tutte le righe si stringono
insieme per stare nella pagina.

Le tre opzioni:

- **Barra continua** *(predefinito)* — un **unico rettangolo** dalla prima
  all'ultima settimana con effort; gli eventuali buchi interni non si vedono.
- **Segmentata** — un **rettangolo per ogni tratto** di settimane consecutive con
  effort; dove una settimana è a zero resta un **buco**. Altezza fissa.
- **Segmentata + altezza %** — come la segmentata ma con un rettangolo per
  settimana, la cui **altezza è proporzionale** al massimo settimanale del dev
  (la settimana più carica — somma degli effort dei worker in quella settimana —
  ha l'altezza piena, uguale alle altre due opzioni; tutte le altre sono più basse).
  Il riferimento non scende mai **sotto le 40 ore**: se il dev non supera mai le
  40h in una settimana, una settimana da 40h resta comunque all'altezza piena e le
  più scariche restano proporzionalmente più basse.

### Percentuali di avanzamento nel PDF/SVG
In tutte le finestre di export c'è la casella **«Includi percentuali di avanzamento
(presunta/dichiarata)»**. Se attiva, le due percentuali nel formato
`presunta%/dichiarata%` (un trattino al posto della presunta quando il pianificato è
0) compaiono **a destra, accanto all'etichetta delle date** a fine barra, con lo
stesso font delle date. Compaiono
**solo per i dev con effort**: un dev senza effort
(riga sottile) non mostra alcuna percentuale. Con la stessa casella attiva viene
stampato anche l'**avanzamento complessivo del progetto sotto il marker «Today»**:
«Today» in **grassetto** e, sulla riga sotto, le percentuali tra parentesi
`(presunta%/attuale%)` (stesso font della data delle bandierine), quando «oggi»
ricade nell'intervallo del grafico. Se la casella è disattivata
(impostazione predefinita) le percentuali non vengono stampate. La scelta vale sia
per il PDF sia per l'SVG ed è ricordata tra un export e l'altro (non è salvata sul
file).

### Anno nelle date dei dev
Sempre nelle finestre di export, la casella **«Includi l'anno nelle date dei dev
('26)»** aggiunge l'**anno abbreviato** all'etichetta delle date a fine barra, nello
stile americano con l'apostrofo. Con la casella attiva l'anno compare **sempre**,
anche per i progetti che stanno dentro un solo anno solare: se inizio e fine del dev
sono nello **stesso anno** è scritto una volta sola, dopo la fine
(`Sep 21 - Oct 9 '26`); se sono in anni diversi compare in entrambe
(`Dec 14 '26 - Jan 8 '27`). Senza casella: `Sep 21 - Oct 9`. Le date delle bandierine delle
milestone non cambiano. È **attiva** per impostazione predefinita, vale per PDF e
SVG ed è ricordata tra un export e l'altro (non è salvata sul file).

La **prima data** è il lunedì della prima settimana con effort; l'**ultima** è il
**venerdì** dell'ultima settimana con effort (lunedì + 4 giorni). Un dev con effort in
una sola settimana mostra quindi lunedì e venerdì di quella settimana, ad esempio
`Sep 21 - Sep 25 '26`.

### Andamento nel tempo (PDF)
**File ▸ Andamento…** esporta un PDF con l'**andamento nel tempo** delle percentuali
di avanzamento, **una pagina per progetto** visibile. In ogni pagina, per ciascun dev
(nel **colore della griglia**) ci sono due linee:

- **presunta** (linea **continua**): la % usata sul pianificato settimana per settimana;
  parte dal **primo effort di quel dev** (non prima) e prosegue fino all'**ultima
  settimana con effort**, quindi **può andare oltre oggi**;
- **dichiarata** (linea **tratteggiata**): l'andamento delle percentuali dichiarate
  dallo sviluppatore; si **ferma alla settimana corrente**.

Ogni **vertice** delle linee è segnato da un **pallino**, così si vedono i punti
esatti con cui è costruito il grafico. C'è anche una coppia di linee **nere** per
l'andamento **aggregato di progetto** (che parte dal **primo effort in assoluto**). L'asse
verticale si adatta al valore massimo (se una presunta supera il 100% lo sforamento è
visibile); una **riga rossa orizzontale** segna il **100%** e una linea rossa verticale
segna **«today»**. In alto la tripletta+nome del
progetto e una legenda dei colori; in basso la solita banda grigia con la data. Non ci
sono le bandierine delle milestone. Un progetto senza dev con effort pianificato non
produce pagina.

---

## 17. Salvataggio e modifiche esterne

- **File ▸ Salva** o `Cmd/Ctrl+S` scrive il file `.ron` corrente.
- Le modifiche non salvate sono segnalate in alto a destra (asterisco e colore
  arancione). Alla chiusura con modifiche pendenti viene chiesta conferma.
- **Sincronizzazione git automatica**: se la cartella del file `.ron` è (dentro) un
  repository git **e il file è già tracciato** (aggiunto in precedenza al repo), a
  **ogni salvataggio** (manuale, automatico o in uscita) viene **committato** (solo se
  è cambiato) e inviato con **`git push`**. Un file `.ron` **non ancora tracciato non
  viene aggiunto**: il programma non lo mette da solo sotto controllo di versione.
  Avviene in background senza rallentare l'app; se la cartella non è un repository, il
  file non è tracciato, manca il remoto o la rete/credenziali non sono disponibili,
  semplicemente non succede nulla (il salvataggio su disco funziona comunque). Vengono
  committati solo i file `.ron`, non i backup `.bak`.
- **Rilevamento modifiche esterne**: se il file `.ron` viene cambiato da un altro
  programma mentre è aperto, l'app lo segnala. Puoi scegliere di **mantenere le
  tue** modifiche o **ricaricare** (scartando le tue). In alcuni casi
  l'aggiornamento esterno viene applicato automaticamente, con una notifica
  (vedi sotto).

### Notifiche in-app

In **basso a destra** compaiono le notifiche: l'esito delle operazioni e, cosa più
importante, gli **errori** che prima restavano invisibili (salvataggio non riuscito,
export non scritto, `git push` fallito). Ogni notifica ha un'icona per gravità:

| Icona | Significato | Durata |
|---|---|---|
| ✔ | Operazione riuscita (file salvato, PDF/SVG/minuta scritti) | pochi secondi |
| ℹ | Informazione (il file è stato aggiornato o unito da un collega) | ~10 secondi |
| ⚠ | Niente da fare (es. «Nessun progetto visibile: PDF non creato») | ~15 secondi |
| ⛔ | **Errore** | **resta finché non la chiudi** con ✕ |

Gli errori non spariscono da soli, proprio per non passare inosservati; le altre si
possono comunque chiudere subito con la ✕. Un errore che si ripete (per esempio un
`git push` che fallisce a ogni salvataggio automatico) **non impila copie**: la
notifica esistente viene solo rinfrescata.

Se il salvataggio **in uscita** fallisce, il programma **non si chiude**: mostra
l'errore e resta aperto, così le modifiche non vanno perse.

---

## 18. Scorciatoie da tastiera

| Scorciatoia | Azione |
|---|---|
| `Cmd/Ctrl + S` | Salva il file |
| `Cmd/Ctrl + F` | Apri la dialog «Filtri» sulla colonna Workers (tutte le settimane) |
| `Shift + Cmd/Ctrl + F` | Deseleziona tutti i worker nel filtro |
| `Cmd/Ctrl + G` | Come sopra ma sulla **settimana corrente** (solo progetti con il worker attivo questa settimana) |
| `Shift + Cmd/Ctrl + G` | Deseleziona tutti i worker nel filtro (settimana corrente) |
| `Cmd/Ctrl + D` | Apri la dialog «Filtri» sulla colonna Dev (solo i progetti col dev selezionato, e solo quel dev) |
| `Shift + Cmd/Ctrl + D` | Deseleziona tutti i dev nel filtro |
| `Cmd/Ctrl + K` | Apri la dialog «Filtri» sulla colonna Categorie (solo i progetti delle categorie selezionate) |
| `Shift + Cmd/Ctrl + K` | Deseleziona tutte le categorie nel filtro |
| `Cmd/Ctrl + Y` | Apri la dialog «Filtri» sulla colonna Anni (progetti che iniziano / finiscono / stanno tutti nell'anno) |
| `Shift + Cmd/Ctrl + Y` | Deseleziona tutti gli anni nel filtro |
| `Cmd/Ctrl + P` | Apri la dialog «Filtri» sulla colonna Progetti (ricerca, visibilità e salto rapido) |
| `Cmd/Ctrl + J` | **Azzera tutti i filtri**: torna a vedere tutto (ripetibile, non deseleziona mai) |
| `Cmd/Ctrl + T` | Torna a **oggi**: scorre la griglia sulla settimana corrente, centrandola |
| `Invio` / `Tab` | Conferma la modifica della cella |
| `Esc` | Annulla la modifica della cella |
| `Cmd/Ctrl + C / X / V` | Copia / Taglia / Incolla nella cella in modifica |
| Doppio click sul nome dev | Aggiungi una riga worker |

> Su macOS si usa `Cmd`, su Windows/Linux `Ctrl`.

### Azioni con il mouse (riepilogo)
| Dove | Azione | Effetto |
|---|---|---|
| Cella worker | Click sinistro | Modifica la cella |
| Cella worker (non vuota) | Tasto destro | Nota della cella |
| Riga alta della colonna-dev | Tasto destro | Milestone / Sposta |
| Nome dev | Tasto destro | Menù dev (righe, cancella effort, nota, nascondi effort) |
| Tripletta / Inizio / Fine | Tasto destro | Modifica il valore |
| Categoria progetto | Click | Scegli categoria |
| Striscia "Dev" | Click | Aggiungi/rimuovi dev |
| Intestazione settimana | Click | Max ore settimana per tutti i worker |
| Cella footer worker | Click / Tasto destro | Max ore settimana / stato e nota |
| Nome worker (footer) | Click | Evidenzia/toglie il worker nelle celle dei dev |
| Nome worker (footer) | Tasto destro | Ore max… (globale) / Ghost |

---

## 19. Codici colore e indicatori

- **Giallo fosforescente (tinta colonna)**: settimana corrente (colore
  personalizzabile dal file — vedi §2).
- **Tinta sulle righe delle date** (in alto e nel footer): il mese della data,
  con 12 colori diversi; celle **bicolori** nelle settimane a cavallo di due mesi
  (colori e intensità personalizzabili dal file — vedi §2).
- **Azzurro (colonna)**: settimana di inizio progetto.
- **Verde (colonna)**: settimana di fine/deadline progetto.
- **Colonna gialla stretta**: confine di fine anno ("Effort residuo").
- **Tinta colonna con colore milestone**: settimana con milestone; più bande
  verticali = più milestone nella stessa settimana.
- **Triangolo giallo**: presenza di una nota (cella, dev o worker/settimana).
- **Triangolo azzurro (alto-sx) / rosso (basso-sx)** nel footer: ferie / malattia.
- **Testo grigio** in cella: worker nascosto nel footer.
- **Riquadro in basso a destra**: notifica (✔ riuscito, ℹ informazione,
  ⚠ avviso, ⛔ errore — vedi §17).
- **Sfondo rosso** su residuo/valore: sotto zero o oltre il massimo.
- **Riga cumulativa** dal verde al rosso: avanzamento verso il pianificato.

---

## 20. Confronto e importazione tra file

Serve quando lavori sul file `.ron` "ufficiale" (quello aperto nel programma) ma
hai fatto delle prove su una **copia parallela**: puoi confrontare lo **stesso
progetto** nei due file e **importare** nel file ufficiale ciò che ti interessa.

Si apre da **File ▸ Confronta/Importa progetto…**, che chiede il file `.ron`
parallelo. Mentre la finestra di confronto è aperta l'**autosave è sospeso**: le
modifiche (gli import) restano volontarie e vengono salvate solo dopo aver chiuso la
finestra (o con `Cmd/Ctrl+S`).

> Il file parallelo va inteso come una **copia** dell'ufficiale: gli identificatori
> interni (progetti, dev, worker) devono coincidere. I progetti vengono accoppiati
> per **tripletta**.

### Scelta dei progetti

La prima schermata elenca **solo i progetti diversi** tra i due file (quelli
identici non compaiono). Spunta uno o più progetti e premi **Confronta »**. Con
**« Torna alla selezione** torni all'elenco; con **Chiudi** esci (e riparte
l'autosave).

### La vista di confronto

Due pannelli affiancati come **due griglie originali**: a **sinistra il file
ufficiale**, a **destra il parallelo**. Ogni cella mostra `Worker|effort` e le date
di settimana (formato `aa-mm-gg`) sono in cima. I due pannelli **scorrono
sincronizzati** in orizzontale e in verticale: usa la **rotella** (con **Shift** per
scorrere nel tempo), il **trascinamento** dello sfondo, o le **barre di
scorrimento**.

Vengono mostrati **solo i dev diversi**. Per ogni dev, a destra del nome, compaiono
l'**effort stimato** e la **% dichiarata** dei due lati, che diventano **rossi** se
differiscono. Anche le **celle effort** diventano rosse dove i valori non
coincidono.

### Importare nel file ufficiale

- **←** accanto al dev: **importa quel dev** dal parallelo nel file ufficiale.
- **«** sull'intestazione del progetto: **importa l'intero progetto**.

Non esiste la direzione opposta (verso il parallelo): il file parallelo **non viene
mai modificato né salvato** su disco. Dopo un import il dev (o il progetto) diventa
identico e **sparisce dalla vista**, perché vengono mostrate solo le differenze. Le
modifiche importate finiscono nel file ufficiale al successivo salvataggio.

---

*Fine del manuale.*
