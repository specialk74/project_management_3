#![allow(unused)]
#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::{
    categories::CategoryId,
    dev_utils::dev::DevId,
    milestones::MilestoneId,
    single_dev_utils::single_dev::{SingleDev, WEEK_STEP, WeekId},
    single_effort_utils::sinlge_effort::Effort,
    workers_utils::worker::WorkerId,
};

#[derive(Serialize, Deserialize, Hash, PartialEq, Eq, Clone, Copy, PartialOrd, Ord)]
pub struct ProjectId(pub usize);

#[derive(Serialize, Deserialize, Hash, PartialEq, Eq, Clone, Copy, PartialOrd, Ord)]
pub struct Enable(pub bool);

/// Come gestire lo sforamento di un confine di progetto quando si sposta un
/// blocco di effort oltre l'inizio o la fine.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum OverflowResolution {
    /// Nessuno sforamento da gestire (i confini restano invariati).
    None,
    /// Estende il confine del progetto (inizio o fine) per contenere l'effort.
    MoveBoundary,
    /// Perde le settimane di effort finite fuori dal confine.
    Truncate,
}

/// Default di `enable`: abilitato. Usato quando il campo non è serializzato
/// (è uno stato di filtro transitorio, ricalcolato da `closed` al caricamento).
fn enable_default() -> Enable {
    Enable(true)
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Project {
    info: String,
    // `enable` è lo stato del filtro "Progetti ▼": puramente di visualizzazione,
    // non va salvato né riletto dal file. Al load viene ricalcolato da `closed`.
    #[serde(skip, default = "enable_default")]
    enable: Enable,
    dev_id: HashMap<DevId, SingleDev>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_week: Option<WeekId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_week: Option<WeekId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    tripletta: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    category: Option<CategoryId>,
    /// Posizione di ordinamento nella colonna sinistra. I file vecchi (senza
    /// campo) partono tutti da 0 → tie-break per ProjectId = ordine attuale.
    #[serde(default, skip_serializing_if = "is_zero")]
    order: usize,
    /// Progetto chiuso/archiviato. Optional per retro-compatibilità con i file
    /// .ron privi del campo. Un progetto chiuso è automaticamente non-enabled.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    closed: Option<bool>,
    /// Milestone collocate nel progetto: id milestone → settimana. La chiave
    /// garantisce che la stessa milestone non compaia più volte nel progetto.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    milestones: HashMap<MilestoneId, WeekId>,
    /// Note del progetto per settimana: WeekId → testo libero. La chiave è il
    /// primo giorno della settimana (come le etichette della griglia). Usate
    /// come diario settimanale. `#[serde(default)]` per retro-compatibilità con
    /// i .ron vecchi (privi del campo).
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    notes: HashMap<WeekId, String>,
}

/// Helper per `skip_serializing_if`: vero se l'usize è 0 (valore di default).
fn is_zero(n: &usize) -> bool {
    *n == 0
}

impl Project {
    pub fn new(info: &str) -> Self {
        Self {
            info: info.to_string(),
            dev_id: HashMap::new(),
            enable: Enable(true),
            start_week: None,
            end_week: None,
            tripletta: None,
            category: None,
            order: 0,
            closed: None,
            milestones: HashMap::new(),
            notes: HashMap::new(),
        }
    }

    pub fn new_with_start(info: &str, start_week: Option<WeekId>) -> Self {
        Self {
            info: info.to_string(),
            dev_id: HashMap::new(),
            enable: Enable(true),
            start_week,
            end_week: None,
            tripletta: None,
            category: None,
            order: 0,
            closed: None,
            milestones: HashMap::new(),
            notes: HashMap::new(),
        }
    }

    pub fn get_start_week(&self) -> Option<WeekId> {
        self.start_week
    }
    pub fn set_start_week(&mut self, week: Option<WeekId>) {
        self.start_week = week;
    }
    pub fn get_end_week(&self) -> Option<WeekId> {
        self.end_week
    }
    pub fn set_end_week(&mut self, week: Option<WeekId>) {
        self.end_week = week;
    }

    pub fn del_row(&mut self, id_dev: DevId) {
        self.dev_id.get_mut(&id_dev).unwrap().del_row();
    }

    pub fn reset_effort(&mut self, id_dev: DevId, week: WeekId) {
        self.dev_id.get_mut(&id_dev).unwrap().reset_effort(week);
    }

    pub fn set_info(&mut self, info: &str) {
        self.info = info.to_string();
    }

    pub fn add_dev(&mut self, id_dev: DevId) {
        self.dev_id.entry(id_dev).or_insert_with(SingleDev::new);
        // if !self.dev_id.contains_key(&id_dev) {
        //     self.dev_id.insert(id_dev, SingleDev::new());
        // }
    }

    pub fn add_dev_effort(&mut self, id_dev: DevId, effort: Effort) {
        self.add_dev(id_dev);
        self.dev_id.get_mut(&id_dev).unwrap().set_effort(effort);
    }

    /// Registra la % dichiarata dal dev per la settimana `week` (storico).
    /// Ritorna `true` se lo storico è cambiato. Vedi `SingleDev::set_declared_pct`.
    pub fn set_dev_declared_pct(&mut self, id_dev: DevId, week: WeekId, pct: u8) -> bool {
        self.add_dev(id_dev);
        self.dev_id
            .get_mut(&id_dev)
            .unwrap()
            .set_declared_pct(week, pct)
    }

    /// Somme (sui dev con pianificato > 0) che generano le percentuali di
    /// avanzamento, così i valori mostrati e i numeri del tooltip vengono da
    /// un'unica fonte: `(usato_fino_a_today, pianificato, Σ pianificato·dichiarata)`.
    /// `None` se non c'è alcun pianificato.
    pub fn progress_breakdown(&self, today: WeekId) -> Option<(u64, u64, u64)> {
        let mut used: u64 = 0; // Σ effort usato fino a today
        let mut planned: u64 = 0; // Σ pianificato
        let mut weighted_declared: u64 = 0; // Σ pianificato · dichiarata
        for sd in self.dev_id.values() {
            let p = sd.planned_effort().0 as u64;
            if p == 0 {
                continue;
            }
            used += sd.effort_up_to(today).0 as u64;
            planned += p;
            weighted_declared += p * sd.declared_pct() as u64;
        }
        (planned != 0).then_some((used, planned, weighted_declared))
    }

    /// Avanzamento complessivo del progetto: media delle percentuali dichiarate
    /// dai dev, pesata sull'effort pianificato di ciascuno (earned value):
    /// `Σ(pianificato_i · dichiarata_i) / Σ(pianificato_i)`. Considera solo i dev
    /// con pianificato > 0 (gli unici con % dichiarata). `None` se non c'è alcun
    /// pianificato, cioè nessun dato su cui misurare l'avanzamento.
    pub fn progress_pct(&self) -> Option<u8> {
        // La parte dichiarata non dipende da `today`: qualunque valore va bene.
        self.progress_breakdown(WeekId(0))
            .map(|(_, planned, wdecl)| ((wdecl + planned / 2) / planned) as u8)
    }

    /// Avanzamento "presunto" del progetto in base all'effort fornito fino a
    /// `today`: `Σ(effort_usato_fino_a_today) / Σ(pianificato)`. È la quota di
    /// budget consumata finora (può superare 100 in caso di sforamento).
    /// Considera solo i dev con pianificato > 0. `None` se non c'è alcun
    /// pianificato (stesso criterio di `progress_pct`, così le due % ci sono
    /// sempre insieme o nessuna delle due).
    pub fn presumed_progress_pct(&self, today: WeekId) -> Option<u32> {
        self.progress_breakdown(today)
            .map(|(used, planned, _)| ((used * 100 + planned / 2) / planned) as u32)
    }

    pub fn set_dev_note(&mut self, id_dev: DevId, note: &str) {
        if let Some(sd) = self.dev_id.get_mut(&id_dev) {
            sd.set_dev_note(note);
        }
    }

    pub fn set_note(&mut self, id_dev: DevId, week: WeekId, id_worker: WorkerId, note: &str) {
        self.add_dev(id_dev);
        self.dev_id
            .get_mut(&id_dev)
            .unwrap()
            .set_note(week, id_worker, note);
    }

    pub fn add_effort(&mut self, id_dev: DevId, week: WeekId, id_worker: WorkerId, effort: Effort) {
        self.add_dev(id_dev);
        self.dev_id
            .get_mut(&id_dev)
            .unwrap()
            .add(week, id_worker, effort);
    }

    pub fn del_dev(&mut self, id_dev: DevId) {
        self.dev_id.remove(&id_dev);
    }

    pub fn list_dev_id(&self) -> Vec<DevId> {
        let mut ids: Vec<DevId> = self.dev_id.keys().copied().collect();
        ids.sort();
        ids
    }

    pub fn get_week_with_max_worker(&self, id_dev: DevId) -> Option<WeekId> {
        self.dev_id.get(&id_dev).unwrap().get_week_with_max_worker()
    }

    pub fn get_tripletta(&self) -> Option<&str> {
        self.tripletta.as_deref()
    }

    pub fn set_tripletta(&mut self, tripletta: &str) {
        self.tripletta = if tripletta.is_empty() {
            None
        } else {
            Some(tripletta.to_string())
        };
    }

    pub fn get_notes(&self) -> &HashMap<WeekId, String> {
        &self.notes
    }

    pub fn set_notes(&mut self, notes: HashMap<WeekId, String>) {
        self.notes = notes;
    }

    pub fn get_category(&self) -> Option<CategoryId> {
        self.category
    }

    pub fn set_category(&mut self, category: Option<CategoryId>) {
        self.category = category;
    }

    pub fn get_info(&self) -> String {
        self.info.clone()
    }

    pub fn get_order(&self) -> usize {
        self.order
    }

    pub fn set_order(&mut self, order: usize) {
        self.order = order;
    }

    pub fn get_enable(&self) -> Enable {
        self.enable
    }

    pub fn set_enable(&mut self, enable: Enable) {
        self.enable = enable;
    }

    /// Mappa dev → SingleDev (per il merge a 3 vie).
    pub fn devs(&self) -> &HashMap<DevId, SingleDev> {
        &self.dev_id
    }

    /// Sostituisce l'intera mappa dei dev (usato dal merge).
    pub fn set_devs(&mut self, devs: HashMap<DevId, SingleDev>) {
        self.dev_id = devs;
    }

    /// Uguaglianza dei soli "dati generali" del progetto (tutto tranne i dev e il
    /// campo `enable`, che è un filtro transitorio). Usata dal merge per capire se
    /// l'intestazione del progetto è stata modificata.
    pub fn header_eq(&self, o: &Project) -> bool {
        self.info == o.info
            && self.start_week == o.start_week
            && self.end_week == o.end_week
            && self.tripletta == o.tripletta
            && self.category == o.category
            && self.order == o.order
            && self.closed == o.closed
            && self.milestones == o.milestones
            && self.notes == o.notes
    }

    /// Uguaglianza di contenuto (dati generali + dev), ignorando `enable`.
    pub fn content_eq(&self, o: &Project) -> bool {
        self.header_eq(o) && self.dev_id == o.dev_id
    }

    pub fn is_closed(&self) -> bool {
        self.closed.unwrap_or(false)
    }

    /// Imposta lo stato "chiuso". Chiudere un progetto lo rende anche
    /// automaticamente non-enabled; riaprirlo lo rende di nuovo enabled.
    pub fn set_closed(&mut self, closed: bool) {
        self.closed = if closed { Some(true) } else { None };
        self.enable = Enable(!closed);
    }

    pub fn get_keys(
        &self,
    ) -> std::iter::Cloned<std::collections::hash_map::Keys<'_, DevId, SingleDev>> {
        self.dev_id.keys().cloned()
    }

    pub fn get_dev_id(&self, dev_id: &DevId) -> std::option::Option<&SingleDev> {
        self.dev_id.get(dev_id)
    }

    pub fn set_dev_hide_effort(&mut self, id_dev: DevId, hide: bool) {
        if let Some(sd) = self.dev_id.get_mut(&id_dev) {
            sd.set_hide_effort(hide);
        }
    }

    /// Colloca (o sposta) una milestone in una settimana. La stessa milestone
    /// non può comparire più volte: se già presente, ne aggiorna la settimana.
    pub fn add_milestone(&mut self, id: MilestoneId, week: WeekId) {
        self.milestones.insert(id, week);
    }

    pub fn remove_milestone(&mut self, id: MilestoneId) {
        self.milestones.remove(&id);
    }

    /// Rimuove ogni collocazione della milestone (usato quando viene eliminata
    /// globalmente).
    pub fn purge_milestone(&mut self, id: MilestoneId) {
        self.milestones.remove(&id);
    }

    /// Elenco (milestone, settimana) collocate nel progetto.
    pub fn list_milestones(&self) -> Vec<(MilestoneId, WeekId)> {
        let mut v: Vec<(MilestoneId, WeekId)> =
            self.milestones.iter().map(|(&id, &w)| (id, w)).collect();
        v.sort_by_key(|(id, _)| *id);
        v
    }

    /// Dev con almeno una settimana di effort (>0), ordinati.
    pub fn devs_with_effort(&self) -> Vec<DevId> {
        let mut v: Vec<DevId> = self
            .dev_id
            .iter()
            .filter(|(_, sd)| sd.effort_span().is_some())
            .map(|(id, _)| *id)
            .collect();
        v.sort();
        v
    }

    /// Blocco contiguo di settimane con effort del dev, attorno a `week`.
    pub fn dev_contiguous_block(&self, dev: DevId, week: WeekId) -> Vec<WeekId> {
        self.dev_id
            .get(&dev)
            .map(|sd| sd.contiguous_block(week))
            .unwrap_or_default()
    }

    /// Settimane con effort di un dev (tutte, ordinate).
    pub fn dev_effort_weeks(&self, dev: DevId) -> Vec<WeekId> {
        self.dev_id
            .get(&dev)
            .map(|sd| sd.effort_weeks())
            .unwrap_or_default()
    }

    /// Almeno un dev ha effort nella settimana indicata.
    fn has_effort_at(&self, week: WeekId) -> bool {
        self.dev_id
            .values()
            .any(|sd| sd.get_effort_by_week(week).0 > 0)
    }

    /// Ultima settimana con effort su tutti i dev.
    fn last_effort_week(&self) -> Option<WeekId> {
        self.dev_id
            .values()
            .filter_map(|sd| sd.effort_span().map(|(_, hi)| hi))
            .max()
    }

    /// Prima settimana con effort su tutti i dev.
    fn first_effort_week(&self) -> Option<WeekId> {
        self.dev_id
            .values()
            .filter_map(|sd| sd.effort_span().map(|(lo, _)| lo))
            .min()
    }

    /// Intervallo [min, max] (in giorni-WeekId) coperto da un insieme di spostamenti.
    fn moves_day_span(moves: &[(DevId, Vec<WeekId>)]) -> Option<(i64, i64)> {
        let mut lo = i64::MAX;
        let mut hi = i64::MIN;
        for (_, weeks) in moves {
            for w in weeks {
                lo = lo.min(w.0 as i64);
                hi = hi.max(w.0 as i64);
            }
        }
        (lo <= hi).then_some((lo, hi))
    }

    /// Se lo spostamento di `delta_weeks` sforerebbe un confine del progetto,
    /// restituisce `Some(true)` per la fine (delta>0) o `Some(false)` per
    /// l'inizio (delta<0); `None` se non c'è sforamento.
    pub fn move_overflow_side(
        &self,
        moves: &[(DevId, Vec<WeekId>)],
        delta_weeks: i64,
    ) -> Option<bool> {
        if delta_weeks == 0 {
            return None;
        }
        let dd = delta_weeks * WEEK_STEP as i64;
        let (lo, hi) = Self::moves_day_span(moves)?;
        if dd > 0 {
            if let Some(end) = self.end_week {
                if hi + dd > end.0 as i64 {
                    return Some(true);
                }
            }
        } else if let Some(start) = self.start_week {
            if lo + dd < start.0 as i64 {
                return Some(false);
            }
        }
        None
    }

    /// Sposta gruppi di settimane di effort di `delta_weeks` (positivo = a
    /// destra). `moves` = per ogni dev, quali settimane spostare. `milestones` =
    /// quali milestone del progetto trascinare insieme. `resolution` decide come
    /// gestire un eventuale sforamento dei confini.
    pub fn move_effort(
        &mut self,
        moves: &[(DevId, Vec<WeekId>)],
        delta_weeks: i64,
        milestones: &[MilestoneId],
        resolution: OverflowResolution,
    ) {
        if delta_weeks == 0 {
            return;
        }
        // Le chiavi settimana sono in giorni: converti "n settimane" in giorni.
        let dd = delta_weeks * WEEK_STEP as i64;
        // Stato PRIMA dello spostamento: serve alla contrazione automatica del
        // confine "posteriore" (opposto alla direzione dello spostamento).
        let (start0, end0) = (self.start_week, self.end_week);
        let had_effort_at_start = start0.is_some_and(|s| self.has_effort_at(s));
        let had_effort_at_end = end0.is_some_and(|e| self.has_effort_at(e));
        // Sposta il confine dello stesso numero di settimane (fine += n se n>0,
        // inizio += n se n<0), solo se l'utente l'ha richiesto.
        if resolution == OverflowResolution::MoveBoundary {
            if dd > 0 {
                if let Some(end) = self.end_week {
                    self.end_week = Some(WeekId((end.0 as i64 + dd).max(0) as usize));
                }
            } else if let Some(start) = self.start_week {
                self.start_week = Some(WeekId((start.0 as i64 + dd).max(0) as usize));
            }
        }
        // Limiti di troncamento (solo se si è scelto di perdere le settimane).
        let (trunc_start, trunc_end) = if resolution == OverflowResolution::Truncate {
            if dd > 0 {
                (None, self.end_week.map(|w| w.0 as i64))
            } else {
                (self.start_week.map(|w| w.0 as i64), None)
            }
        } else {
            (None, None)
        };

        for (dev, weeks) in moves {
            if let Some(sd) = self.dev_id.get_mut(dev) {
                sd.shift_weeks(weeks, dd, trunc_start, trunc_end);
            }
        }

        // Milestone selezionate: stesso spostamento, con le stesse regole.
        for m in milestones {
            let Some(w) = self.milestones.get(m).copied() else {
                continue;
            };
            let nw = w.0 as i64 + dd;
            let drop = nw < 0
                || trunc_start.is_some_and(|ts| nw < ts)
                || trunc_end.is_some_and(|te| nw > te);
            if drop {
                self.milestones.remove(m);
            } else {
                self.milestones.insert(*m, WeekId(nw as usize));
            }
        }

        // Contrazione automatica del confine "posteriore": se lo spostamento ha
        // lasciato scoperto quel confine (prima c'era effort, ora no), il confine
        // segue l'effort fino alla prima/ultima settimana ancora occupata.
        if dd < 0 {
            // a sinistra → può liberarsi la FINE
            if let Some(end0) = end0 {
                if had_effort_at_end && !self.has_effort_at(end0) {
                    if let Some(last) = self.last_effort_week() {
                        if last < end0 {
                            self.end_week = Some(last);
                        }
                    }
                }
            }
        } else {
            // a destra → può liberarsi l'INIZIO
            if let Some(start0) = start0 {
                if had_effort_at_start && !self.has_effort_at(start0) {
                    if let Some(first) = self.first_effort_week() {
                        if first > start0 {
                            self.start_week = Some(first);
                        }
                    }
                }
            }
        }
    }

    /// Milestone collocate nella settimana indicata (ordinate per id).
    pub fn milestones_at_week(&self, week: WeekId) -> Vec<MilestoneId> {
        let mut v: Vec<MilestoneId> = self
            .milestones
            .iter()
            .filter(|(_, w)| **w == week)
            .map(|(&id, _)| id)
            .collect();
        v.sort();
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enable_not_serialized_and_defaults_true() {
        let p = Project::new("demo");
        let s = ron::ser::to_string(&p).unwrap();
        assert!(
            !s.contains("enable"),
            "enable non deve essere serializzato: {s}"
        );

        // File "vecchio" che contiene ancora enable:(false): il campo (skip) va
        // ignorato e il valore ripristinato al default (abilitato).
        let old = s.replacen('(', "(enable:(false),", 1);
        let back: Project = ron::from_str(&old).expect("il vecchio formato deve caricarsi");
        assert!(
            back.get_enable().0,
            "enable deve tornare al default (abilitato)"
        );
    }

    // Helper: progetto con un dev e effort nelle settimane indicate.
    fn proj_with_weeks(weeks: &[usize]) -> (Project, DevId) {
        let dev = DevId(1);
        let w = WorkerId(1);
        let mut p = Project::new("t");
        p.add_dev(dev);
        for &wk in weeks {
            p.add_effort(dev, WeekId(wk), w, Effort(8));
        }
        (p, dev)
    }

    fn effort_weeks(p: &Project, dev: DevId) -> Vec<usize> {
        p.dev_effort_weeks(dev).into_iter().map(|w| w.0).collect()
    }

    // Le settimane reali sono numeri di giorno distanti WEEK_STEP (7).
    const W0: usize = 700;
    const W1: usize = 700 + WEEK_STEP; // 707
    const W2: usize = 700 + 2 * WEEK_STEP; // 714
    const W3: usize = 700 + 3 * WEEK_STEP; // 721

    #[test]
    fn contiguous_block_around_week() {
        // Blocco [W0,W1,W2] + una settimana staccata; adiacenza a passo 7.
        let (p, dev) = proj_with_weeks(&[W0, W1, W2, W0 + 10 * WEEK_STEP]);
        let block: Vec<usize> = p
            .dev_contiguous_block(dev, WeekId(W1))
            .into_iter()
            .map(|w| w.0)
            .collect();
        assert_eq!(block, vec![W0, W1, W2]);
        // settimana senza effort → nessun blocco
        assert!(
            p.dev_contiguous_block(dev, WeekId(W0 + 5 * WEEK_STEP))
                .is_empty()
        );
    }

    #[test]
    fn move_block_right_no_boundary() {
        let (mut p, dev) = proj_with_weeks(&[W0, W1, W2]);
        // +3 settimane
        p.move_effort(
            &[(dev, vec![WeekId(W0), WeekId(W1), WeekId(W2)])],
            3,
            &[],
            OverflowResolution::None,
        );
        assert_eq!(
            effort_weeks(&p, dev),
            vec![W3, W3 + WEEK_STEP, W3 + 2 * WEEK_STEP]
        );
    }

    #[test]
    fn move_right_truncate_beyond_end() {
        let (mut p, dev) = proj_with_weeks(&[W0, W1, W2]);
        p.set_end_week(Some(WeekId(W3))); // fine a 721
        p.move_effort(
            &[(dev, vec![WeekId(W0), WeekId(W1), WeekId(W2)])],
            3,
            &[],
            OverflowResolution::Truncate,
        );
        assert_eq!(effort_weeks(&p, dev), vec![W3]); // le due oltre la fine perse
        assert_eq!(p.get_end_week(), Some(WeekId(W3))); // confine invariato
    }

    #[test]
    fn move_right_shifts_end_by_n() {
        // La fine si sposta di n settimane (721 + 3*7 = 742), non "fino a contenere".
        let (mut p, dev) = proj_with_weeks(&[W0, W1, W2]);
        p.set_end_week(Some(WeekId(W3)));
        p.move_effort(
            &[(dev, vec![WeekId(W0), WeekId(W1), WeekId(W2)])],
            3,
            &[],
            OverflowResolution::MoveBoundary,
        );
        assert_eq!(
            effort_weeks(&p, dev),
            vec![W3, W3 + WEEK_STEP, W3 + 2 * WEEK_STEP]
        );
        assert_eq!(p.get_end_week(), Some(WeekId(W3 + 3 * WEEK_STEP)));
    }

    #[test]
    fn move_left_below_zero_is_dropped() {
        let (mut p, dev) = proj_with_weeks(&[WEEK_STEP, 2 * WEEK_STEP]);
        p.move_effort(
            &[(dev, vec![WeekId(WEEK_STEP), WeekId(2 * WEEK_STEP)])],
            -3,
            &[],
            OverflowResolution::None,
        );
        assert!(effort_weeks(&p, dev).is_empty());
    }

    #[test]
    fn move_left_shifts_start_by_n() {
        // L'inizio si sposta di n settimane.
        let (mut p, dev) = proj_with_weeks(&[W1, W2]);
        p.set_start_week(Some(WeekId(W0)));
        p.move_effort(
            &[(dev, vec![WeekId(W1), WeekId(W2)])],
            -3,
            &[],
            OverflowResolution::MoveBoundary,
        );
        assert_eq!(
            effort_weeks(&p, dev),
            vec![W1 - 3 * WEEK_STEP, W2 - 3 * WEEK_STEP]
        );
        assert_eq!(p.get_start_week(), Some(WeekId(W0 - 3 * WEEK_STEP)));
    }

    #[test]
    fn overflow_side_detects_end_and_start() {
        let (mut p, dev) = proj_with_weeks(&[W0, W1, W2]);
        p.set_end_week(Some(WeekId(W2)));
        p.set_start_week(Some(WeekId(W0)));
        let moves = [(dev, vec![WeekId(W0), WeekId(W1), WeekId(W2)])];
        assert_eq!(p.move_overflow_side(&moves, 1), Some(true)); // sfora la fine
        assert_eq!(p.move_overflow_side(&moves, -1), Some(false)); // sfora l'inizio
        assert_eq!(p.move_overflow_side(&moves, 0), None);
    }

    #[test]
    fn left_move_contracts_end_to_last_effort() {
        // Unico dev con effort fino alla fine; spostandolo a sinistra la fine
        // si contrae all'ultima settimana ancora occupata.
        let (mut p, dev) = proj_with_weeks(&[W0, W1, W2]);
        p.set_end_week(Some(WeekId(W2)));
        p.move_effort(
            &[(dev, vec![WeekId(W0), WeekId(W1), WeekId(W2)])],
            -1,
            &[],
            OverflowResolution::None,
        );
        // effort ora [W0-7, W1-7, W2-7]; ultima = W2-7 = W1
        assert_eq!(p.get_end_week(), Some(WeekId(W1)));
    }

    #[test]
    fn left_move_no_contract_if_other_dev_at_end() {
        let (mut p, dev_a) = proj_with_weeks(&[W0, W1, W2]);
        let dev_b = DevId(2);
        p.add_dev(dev_b);
        p.add_effort(dev_b, WeekId(W2), WorkerId(1), Effort(8)); // altro dev sulla fine
        p.set_end_week(Some(WeekId(W2)));
        p.move_effort(
            &[(dev_a, vec![WeekId(W0), WeekId(W1), WeekId(W2)])],
            -1,
            &[],
            OverflowResolution::None,
        );
        assert_eq!(p.get_end_week(), Some(WeekId(W2))); // resta: dev_b è ancora sulla fine
    }

    #[test]
    fn right_move_contracts_start_to_first_effort() {
        let (mut p, dev) = proj_with_weeks(&[W0, W1, W2]);
        p.set_start_week(Some(WeekId(W0)));
        p.move_effort(
            &[(dev, vec![WeekId(W0), WeekId(W1), WeekId(W2)])],
            1,
            &[],
            OverflowResolution::None,
        );
        // effort ora [W1, W2, W3]; prima = W1
        assert_eq!(p.get_start_week(), Some(WeekId(W1)));
    }

    #[test]
    fn right_move_no_contract_if_effort_not_at_start() {
        let (mut p, dev) = proj_with_weeks(&[W1, W2]);
        p.set_start_week(Some(WeekId(W0))); // inizio prima del primo effort
        p.move_effort(
            &[(dev, vec![WeekId(W1), WeekId(W2)])],
            1,
            &[],
            OverflowResolution::None,
        );
        assert_eq!(p.get_start_week(), Some(WeekId(W0))); // invariato: nessun effort era sull'inizio
    }

    #[test]
    fn milestone_moves_with_block() {
        let (mut p, dev) = proj_with_weeks(&[W0, W1, W2]);
        let m = MilestoneId(1);
        p.add_milestone(m, WeekId(W1));
        p.move_effort(
            &[(dev, vec![WeekId(W0), WeekId(W1), WeekId(W2)])],
            3,
            &[m],
            OverflowResolution::None,
        );
        assert_eq!(p.list_milestones(), vec![(m, WeekId(W1 + 3 * WEEK_STEP))]);
    }

    #[test]
    fn unselected_milestone_stays() {
        let (mut p, dev) = proj_with_weeks(&[W0, W1, W2]);
        let m = MilestoneId(1);
        p.add_milestone(m, WeekId(W1));
        p.move_effort(
            &[(dev, vec![WeekId(W0), WeekId(W1), WeekId(W2)])],
            3,
            &[],
            OverflowResolution::None,
        );
        assert_eq!(p.list_milestones(), vec![(m, WeekId(W1))]);
    }

    #[test]
    fn progress_pct_is_planned_weighted_average_of_declared() {
        let mut p = Project::new("t");
        // dev A: 200h @ 60%, dev B: 40h @ 90% → (12000+3600)/240 = 65%.
        p.add_dev_effort(DevId(1), Effort(200));
        p.set_dev_declared_pct(DevId(1), WeekId(W0), 60);
        p.add_dev_effort(DevId(2), Effort(40));
        p.set_dev_declared_pct(DevId(2), WeekId(W0), 90);
        assert_eq!(p.progress_pct(), Some(65));

        // Un dev con pianificato 0 non pesa (né compare la sua dichiarata).
        p.add_dev_effort(DevId(3), Effort(0));
        p.set_dev_declared_pct(DevId(3), WeekId(W0), 100);
        assert_eq!(p.progress_pct(), Some(65));

        // Nessun pianificato → nessun dato su cui misurare.
        assert_eq!(Project::new("vuoto").progress_pct(), None);
    }

    #[test]
    fn presumed_progress_pct_is_used_effort_over_planned_up_to_today() {
        let mut p = Project::new("t");
        let (a, b) = (DevId(1), DevId(2));
        p.add_dev_effort(a, Effort(100));
        p.add_dev_effort(b, Effort(100));
        // A: 30h in W0 (passata) + 40h in W2 (futura); B: 50h in W0.
        p.add_effort(a, WeekId(W0), WorkerId(1), Effort(30));
        p.add_effort(a, WeekId(W2), WorkerId(1), Effort(40));
        p.add_effort(b, WeekId(W0), WorkerId(1), Effort(50));

        // Fino a W1: usato = 30 (A) + 50 (B) = 80 su 200 pianificate = 40%.
        assert_eq!(p.presumed_progress_pct(WeekId(W1)), Some(40));
        // Fino a W2: si aggiungono i 40h di A → 120/200 = 60%.
        assert_eq!(p.presumed_progress_pct(WeekId(W2)), Some(60));
        // Nessun pianificato → None.
        assert_eq!(Project::new("v").presumed_progress_pct(WeekId(W2)), None);
    }
}
