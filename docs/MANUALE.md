# Manuale d'uso — Project Management Effort Tracker

Versione applicazione: **0.3.0**

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
15. [Viste: compatta, bianco/nero, tema, zoom](#15-viste-compatta-biancnero-tema-zoom)
16. [Esportazione PDF](#16-esportazione-pdf)
17. [Salvataggio e modifiche esterne](#17-salvataggio-e-modifiche-esterne)
18. [Scorciatoie da tastiera](#18-scorciatoie-da-tastiera)
19. [Codici colore e indicatori](#19-codici-colore-e-indicatori)

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
     inizio, fine) e la striscia dei dev con nome, effort pianificato e residuo.
   - **Griglia (destra)**: la matrice settimane × (dev/worker) dove si inserisce
     l'effort.
4. **Footer**: in basso, i totali e il controllo di saturazione dei worker.

**Scroll sincronizzato**: la colonna sinistra e la griglia scorrono insieme in
verticale; header, griglia e footer scorrono insieme in orizzontale.

La **settimana corrente** è evidenziata con una tinta verde su tutta la colonna.

---

## 5. Barra dei menù

### File
- **Salva** — salva il file `.ron` corrente (scorciatoia `Cmd/Ctrl+S`).
- **Apri…** — apre un file `.ron` (selettore nativo).
- **Esporta PDF…** — esporta in PDF (vedi §16; il comportamento cambia se è
  visibile un solo progetto).
- **Esci** — chiude il programma. Se ci sono modifiche non salvate, chiede
  conferma (Salva ed esci / Esci senza salvare / Annulla).

### Aggiungi
- **+ Progetto** — crea un nuovo progetto vuoto.
- **Worker** — campo di testo + `+ Worker`: aggiunge una persona. Premi Invio o
  il bottone; il campo resta aperto per inserimenti multipli.
- **Dev** — aggiunge un nuovo ruolo/disciplina.
- **Categoria** — aggiunge una categoria di progetto.
- **Milestone** — crea una nuova milestone (poi assegnabile ai progetti).

### Filtri
- **Progetti…** — finestra con l'elenco dei progetti (non chiusi) e una spunta per
  ciascuno: attiva/disattiva la **visibilità** del progetto nella griglia
  ("Select All" per agire su tutti).
- **Workers…** — filtro per worker (vedi §14). La voce mostra una spunta quando un
  filtro è attivo. Scorciatoia `Cmd/Ctrl+F`.
- **Milestone…** — gestione milestone: elenco con selettore colore e cestino per
  eliminarle.
- **Closed…** — finestra per marcare/smarcare i progetti come **chiusi**. Un
  progetto chiuso sparisce dagli elenchi "attivi" e dall'export.

### Vista
- **Vista compatta** — vista a barre compresse (vedi §15).
- **Bianco/Nero** — resa in scala di grigi (toglie i colori).
- **Tema** — *Auto (sistema)* / *Chiaro* / *Scuro* (vedi §15).
- **Zoom settimane** — *Normale* / *2 settimane* / *4 settimane* (vedi §15).
- **Saturazione worker…** — cruscotto di sintesi del carico (vedi §15).

---

## 6. Gestione dei progetti

Ogni progetto occupa un blocco nella colonna sinistra + le relative righe nella
griglia.

### Creare un progetto
**Aggiungi ▸ + Progetto**. Il nuovo progetto compare vuoto; poi si impostano
tripletta, nome, inizio, fine, categoria e dev.

### Campi del progetto (colonna sinistra)
- **Tripletta**: prima riga. Se vuota mostra un `—` tenue. **Tasto destro** per
  modificarla.
- **Categoria**: sotto la tripletta (nascosta in vista compatta). **Click** per
  sceglierla dall'elenco.
- **Nome/descrizione**: campo di testo editabile su più righe. Basta cliccarci e
  scrivere; la modifica si conferma perdendo il focus.
- **Inizio** e **Fine**: date del progetto (nascoste in vista compatta). **Tasto
  destro** su ciascuna riga per modificarle. L'inizio tinge la sua colonna di
  azzurro, la fine (deadline) di verde.

### Riordinare i progetti
Sulla riga della tripletta, a destra, i pulsanti **▲ / ▼** spostano il progetto
su/giù nell'elenco.

### Visibilità, chiusura
- **Abilita/disabilita** dalla finestra **Filtri ▸ Progetti…**: un progetto
  disabilitato non è mostrato nella griglia (ma resta nell'elenco del filtro).
- **Chiudi** dalla finestra **Filtri ▸ Closed…**: un progetto chiuso è escluso da
  elenchi attivi ed export.

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
- **Effort pianificato** (editabile): le ore previste per quel dev.
- **Residuo**: pianificato − totale allocato. Se il residuo è negativo (o pari al
  pianificato con pianificato ≠ 0) lo sfondo diventa rosso e il numero è sempre
  bianco.

### Menù del dev (tasto destro sul nome del dev)
- **Aggiungi riga** — aggiunge una riga worker per quel dev (anche con doppio
  click sul nome).
- **Elimina riga** — rimuove l'ultima riga worker.
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

### Modificare una cella
1. **Click** sulla cella: entra in modalità modifica (sfondo evidenziato, cursore).
2. Digita il nome del worker: parte l'**autocompletamento** (suggerisce i worker
   esistenti). Poi il separatore `|` e le ore.
3. Conferma con **Invio** o **Tab**; annulla con **Esc**.

Le celle prima dell'inizio o dopo la fine del progetto non sono editabili.

### Copia / Taglia / Incolla
Durante la modifica di una cella funzionano `Cmd/Ctrl+C`, `Cmd/Ctrl+X`,
`Cmd/Ctrl+V`. Incollando una cella copiata dal programma si porta con sé anche la
sua nota; incollando testo esterno si incolla solo il testo.

### Colori del testo nelle celle
- Worker **nascosto nel footer**: grigio.
- Worker **in sovra-saturazione** (oltre il massimo ore della settimana): rosso.
- Altrimenti: colore testo standard (adattato al tema chiaro/scuro).

---

## 9. Note

Ci sono tre tipi di nota, tutte segnalate da un **triangolo giallo** nell'angolo
dell'elemento:

- **Nota di cella (effort)**: **tasto destro** su una cella non vuota → editor
  della nota per quel worker/settimana.
- **Nota del dev**: tasto destro sul nome del dev → **Nota Dev…**.
- **Nota worker/settimana** (nel footer): tasto destro sulla cella del worker →
  **Note**.

Passando il mouse su un elemento con nota, il testo compare come tooltip.

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

---

## 11. Milestone

Le milestone sono traguardi con **nome** e **colore**, condivisi tra i progetti.

- **Creare**: Aggiungi ▸ Milestone (nome). Il colore si imposta dal gestore.
- **Gestire**: Filtri ▸ Milestone… — elenco con selettore colore per ciascuna e
  cestino per eliminarle (l'eliminazione le toglie anche da tutti i progetti).
- **Assegnare a una settimana**: tasto destro sulla riga alta di una colonna-dev
  → **Aggiungi milestone qui** → scegli la milestone. Dallo stesso menù puoi
  **rimuoverle**.
- **Visualizzazione**: la colonna della settimana con milestone assume il colore
  della milestone; passando il mouse compaiono i nomi.

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
- **Click sul nome del worker** (colonna sinistra del footer) → massimo ore
  **globale** del worker.
- **Click sull'intestazione di una settimana** (header) → imposta il massimo ore
  di quella settimana **per tutti i worker**.

### Filtro effort del footer
Sopra la sezione worker, tre pulsanti: **Tutti** / **Nulli** / **≥40**, per
mostrare rispettivamente tutti i valori, solo quelli a zero, o solo quelli ≥ 40.

---

## 14. Filtri

### Progetti — visibilità + salto rapido (Filtri ▸ Progetti…, `Cmd/Ctrl+P`)
Un'unica finestra con:
- una **casella di ricerca** in cima (auto-focus): digita e l'elenco si filtra in
  tempo reale (la ricerca combacia con tripletta o nome);
- per ogni progetto una **spunta di visibilità** (disabilitarlo lo nasconde dalla
  griglia) e la **tripletta cliccabile**;
- in elenco si mostra **solo la tripletta** (o il nome se la tripletta è assente).

Fai **click sulla tripletta**, o premi **Invio** per il primo risultato, e la
griglia scorre fino a quel progetto. "Select All" agisce sui progetti elencati.
`Esc` chiude.

> Suggerimento: se resta **un solo progetto visibile**, l'esportazione PDF passa
> alla modalità "singolo progetto" (vedi §16).

### Filtro worker (Filtri ▸ Workers…, `Cmd/Ctrl+F`)
Permette di mostrare solo alcuni worker. Con un filtro attivo:
- vengono mostrate **solo le righe dei worker selezionati**;
- i progetti senza worker corrispondenti spariscono;
- nella colonna sinistra, per ogni progetto **resta visibile solo la tripletta**
  (categoria, nome, inizio/fine spariscono) così da non occupare spazio.

Scorciatoie: `Cmd/Ctrl+F` apre il filtro; `Shift+Cmd/Ctrl+F` **deseleziona tutti**
i worker.

### Progetti chiusi (Filtri ▸ Closed…)
Finestra per marcare/smarcare i progetti come chiusi.

---

## 15. Viste: compatta, bianco/nero, tema, zoom

### Vista compatta (Vista ▸ Vista compatta)
Comprime la griglia: al posto delle celle mostra, per ogni dev e settimana attiva,
una **barra** la cui altezza è proporzionale all'effort. Nasconde categoria,
inizio/fine e riduce le info di progetto alla sola tripletta. Utile per una
panoramica su periodi lunghi.

### Bianco/Nero (Vista ▸ Bianco/Nero)
Rende tutta l'interfaccia in scala di grigi (nessun colore).

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
allocate `sovra`, capacità = max ore effettive della settimana). Mostra **solo i
worker visibili nel footer** (esclude quelli nascosti o esclusi dal filtro).
Contiene:

- un selettore **Settimana / Mese** (granularità);
- una casella **"Solo da settimana corrente"** che nasconde le settimane passate
  (mostra solo presente e futuro);
- un **riepilogo**: numero di sovra-allocazioni e ore in eccesso totali;
- una colonna **Σ** (subito dopo il nome del worker, quindi sempre visibile senza
  scorrere) con totale allocato/capacità e numero di settimane in sovra;
- una **heatmap** worker × (settimana|mese) colorata per saturazione
  (**verde** = libero, **giallo** = pieno, **rosso** = oltre capacità); ogni cella
  mostra le ore, col tooltip `allocato / capacità`, ed è cliccabile per saltare a
  quella settimana nella griglia; la **settimana corrente** è evidenziata (chip
  verde nell'intestazione e bordo verde brillante sulle celle).

La finestra **non è ridimensionabile**: per vedere quale settimana è stata
selezionata basta **spostarla**. Nota: **ferie/malattia non azzerano la capacità**
(si possono fare pochi giorni di ferie e lavorare gli altri), quindi il conteggio
ore resta quello reale.

---

## 16. Esportazione PDF

Ogni progetto idoneo diventa una **pagina** in stile Gantt con: titolo (tripletta)
e descrizione, asse dei mesi, milestone come bandierine, marker "Today", e una
riga per dev.

Un progetto è **idoneo** se è abilitato, non chiuso e ha **sia inizio sia fine**.

### Esportazione di tutti i progetti
**File ▸ Esporta PDF…** con più progetti visibili: genera un PDF con una pagina per
ogni progetto idoneo. Nel Gantt i dev **con effort** sono ordinati per data di
inizio dell'effort; i dev senza effort non compaiono.

### Esportazione di un singolo progetto
Se hai **filtrato fino a un solo progetto visibile**, **File ▸ Esporta PDF…** apre
una **finestra di selezione**:

- elenca **tutti i dev del progetto**, anche quelli **senza effort**, tutti
  pre-selezionati e nell'ordine della lista dev;
- **Select All** in cima per selezionare/deselezionare tutti;
- **checkbox** per includere/escludere ciascun dev;
- **trascinamento** per riordinare i dev (una linea arancione indica il punto di
  inserimento); il rilascio sposta il dev;
- **Esporta…** genera il PDF con i dev selezionati, nell'ordine scelto; **Annulla**
  chiude.

Regole del PDF a singolo progetto:
- i dev **con effort** producono la barra colorata normale;
- i dev **senza effort** producono una **riga sottile** del colore del dev, che
  copre tutta la larghezza del calendario;
- si può esportare **anche senza alcun dev selezionato**: la pagina esce comunque
  con asse, milestone e resto delle informazioni.

---

## 17. Salvataggio e modifiche esterne

- **File ▸ Salva** o `Cmd/Ctrl+S` scrive il file `.ron` corrente.
- Le modifiche non salvate sono segnalate in alto a destra (asterisco e colore
  arancione). Alla chiusura con modifiche pendenti viene chiesta conferma.
- **Rilevamento modifiche esterne**: se il file `.ron` viene cambiato da un altro
  programma mentre è aperto, l'app lo segnala. Puoi scegliere di **mantenere le
  tue** modifiche o **ricaricare** (scartando le tue). In alcuni casi
  l'aggiornamento esterno viene applicato automaticamente con notifica in alto a
  destra (chiudibile con ✕).

---

## 18. Scorciatoie da tastiera

| Scorciatoia | Azione |
|---|---|
| `Cmd/Ctrl + S` | Salva il file |
| `Cmd/Ctrl + F` | Apri il filtro worker |
| `Shift + Cmd/Ctrl + F` | Deseleziona tutti i worker nel filtro |
| `Cmd/Ctrl + P` | Vai a progetto (ricerca e salto rapido) |
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
| Nome dev | Tasto destro | Menù dev (righe, nota, nascondi effort) |
| Tripletta / Inizio / Fine | Tasto destro | Modifica il valore |
| Categoria progetto | Click | Scegli categoria |
| Striscia "Dev" | Click | Aggiungi/rimuovi dev |
| Intestazione settimana | Click | Max ore settimana per tutti i worker |
| Cella footer worker | Click / Tasto destro | Max ore settimana / stato e nota |
| Nome worker (footer) | Click | Max ore globale del worker |

---

## 19. Codici colore e indicatori

- **Verde (tinta colonna)**: settimana corrente.
- **Azzurro (colonna)**: settimana di inizio progetto.
- **Verde (colonna)**: settimana di fine/deadline progetto.
- **Colonna gialla stretta**: confine di fine anno ("Effort residuo").
- **Tinta colonna con colore milestone**: settimana con milestone.
- **Triangolo giallo**: presenza di una nota (cella, dev o worker/settimana).
- **Triangolo azzurro (alto-sx) / rosso (basso-sx)** nel footer: ferie / malattia.
- **Testo grigio** in cella: worker nascosto nel footer.
- **Sfondo rosso** su residuo/valore: sotto zero o oltre il massimo.
- **Riga cumulativa** dal verde al rosso: avanzamento verso il pianificato.

---

*Fine del manuale.*
