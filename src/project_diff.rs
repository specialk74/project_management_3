//! Confronto (diff) tra due versioni **dello stesso** progetto provenienti da due
//! file `.ron` diversi: quello "ufficiale" (aperto nell'app) e uno "parallelo"
//! (una copia su cui si è sperimentato).
//!
//! **Assunzione**: il file parallelo è una copia dell'ufficiale, quindi gli id
//! (`ProjectId`, `DevId`, `WorkerId`, …) coincidono tra i due file. Il confronto
//! avviene perciò **per id**, senza rimappatura per nome. Se in futuro i file
//! potessero avere tabelle diverse, questo modulo andrebbe rivisto.
//!
//! È logica pura (nessuna UI): produce un `ProjectDiff` che la finestra di
//! confronto userà per evidenziare le differenze e scegliere cosa importare.

use std::collections::BTreeSet;

use crate::dev_utils::dev::DevId;
use crate::project_utils::project::Project;

/// Un campo dell'intestazione (dati generali) del progetto che può differire tra
/// le due versioni. NB: l'ordinamento nella colonna sinistra (`order`) è escluso
/// di proposito — è uno stato di visualizzazione, non contenuto del progetto.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum HeaderField {
    Info,
    StartWeek,
    EndWeek,
    Tripletta,
    Category,
    Closed,
    Milestones,
    Notes,
}

/// Esito del confronto per un singolo dev.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum DiffStatus {
    /// Presente e identico in entrambi i file.
    Identical,
    /// Presente in entrambi ma con contenuto diverso.
    Changed,
    /// Presente solo nel file ufficiale.
    OnlyOfficial,
    /// Presente solo nel file parallelo (candidato all'aggiunta).
    OnlyParallel,
}

/// Differenza relativa a un singolo dev del progetto.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct DevDiff {
    pub dev: DevId,
    pub status: DiffStatus,
}

/// Differenze complete tra la versione ufficiale e quella parallela di un progetto.
#[derive(Debug, PartialEq, Eq, Clone, Default)]
pub struct ProjectDiff {
    /// Campi dell'intestazione che differiscono (vuoto = intestazioni uguali).
    pub header: Vec<HeaderField>,
    /// Un elemento per ogni `DevId` presente in almeno uno dei due progetti,
    /// ordinato per `DevId`. Include anche i dev `Identical`.
    pub devs: Vec<DevDiff>,
}

impl ProjectDiff {
    /// True se non c'è alcuna differenza: intestazione uguale e tutti i dev
    /// identici (nessun dev aggiunto/rimosso/modificato).
    pub fn is_empty(&self) -> bool {
        self.header.is_empty() && self.devs.iter().all(|d| d.status == DiffStatus::Identical)
    }

    /// I soli dev che differiscono (diverso o presente in un solo file).
    pub fn changed_devs(&self) -> impl Iterator<Item = &DevDiff> {
        self.devs
            .iter()
            .filter(|d| d.status != DiffStatus::Identical)
    }
}

/// Confronta l'intestazione (dati generali) delle due versioni e restituisce
/// l'elenco dei campi che differiscono. Gli stessi campi di `Project::header_eq`,
/// esclusi `order` (visualizzazione) e reso esplicito campo per campo.
fn header_diff(off: &Project, par: &Project) -> Vec<HeaderField> {
    let mut out = Vec::new();
    if off.get_info() != par.get_info() {
        out.push(HeaderField::Info);
    }
    if off.get_start_week() != par.get_start_week() {
        out.push(HeaderField::StartWeek);
    }
    if off.get_end_week() != par.get_end_week() {
        out.push(HeaderField::EndWeek);
    }
    if off.get_tripletta() != par.get_tripletta() {
        out.push(HeaderField::Tripletta);
    }
    if off.get_category() != par.get_category() {
        out.push(HeaderField::Category);
    }
    if off.is_closed() != par.is_closed() {
        out.push(HeaderField::Closed);
    }
    // Le milestone sono una mappa id→settimana: l'uguaglianza è indipendente
    // dall'ordine, quindi ordino le liste prima di confrontarle.
    let mut m_off = off.list_milestones();
    let mut m_par = par.list_milestones();
    m_off.sort();
    m_par.sort();
    if m_off != m_par {
        out.push(HeaderField::Milestones);
    }
    if off.get_notes() != par.get_notes() {
        out.push(HeaderField::Notes);
    }
    out
}

/// Diff completo tra la versione ufficiale (`off`) e quella parallela (`par`)
/// dello stesso progetto (id coincidenti).
pub fn diff_projects(off: &Project, par: &Project) -> ProjectDiff {
    let header = header_diff(off, par);

    // Unione dei DevId di entrambe le versioni, ordinata per id.
    let mut ids: BTreeSet<DevId> = BTreeSet::new();
    ids.extend(off.devs().keys().copied());
    ids.extend(par.devs().keys().copied());

    let devs = ids
        .into_iter()
        .map(|dev| {
            let status = match (off.devs().get(&dev), par.devs().get(&dev)) {
                (Some(a), Some(b)) if a == b => DiffStatus::Identical,
                (Some(_), Some(_)) => DiffStatus::Changed,
                (Some(_), None) => DiffStatus::OnlyOfficial,
                (None, Some(_)) => DiffStatus::OnlyParallel,
                // Impossibile: l'id proviene dall'unione delle due mappe.
                (None, None) => DiffStatus::Identical,
            };
            DevDiff { dev, status }
        })
        .collect();

    ProjectDiff { header, devs }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::milestones::MilestoneId;
    use crate::project_utils::project::Project;
    use crate::single_dev_utils::single_dev::WeekId;
    use crate::single_effort_utils::sinlge_effort::Effort;
    use crate::workers_utils::worker::WorkerId;

    /// Progetto con due dev, ciascuno con un po' di effort.
    fn base_project() -> Project {
        let mut p = Project::new("Progetto");
        p.set_tripletta("ABC");
        p.set_start_week(Some(WeekId(20000)));
        p.add_dev(DevId(0));
        p.add_dev(DevId(1));
        p.add_effort(DevId(0), WeekId(20000), WorkerId(0), Effort(8));
        p.add_effort(DevId(1), WeekId(20000), WorkerId(0), Effort(8));
        p
    }

    #[test]
    fn identical_projects_have_no_diff() {
        let off = base_project();
        let par = off.clone();
        let d = diff_projects(&off, &par);
        assert!(d.is_empty(), "atteso nessuna differenza: {d:?}");
        assert!(d.header.is_empty());
        assert!(d.devs.iter().all(|x| x.status == DiffStatus::Identical));
        assert_eq!(d.devs.len(), 2);
    }

    #[test]
    fn single_header_field_change_is_reported() {
        let off = base_project();
        let mut par = off.clone();
        par.set_tripletta("XYZ");
        let d = diff_projects(&off, &par);
        assert_eq!(d.header, vec![HeaderField::Tripletta]);
        // I dev non sono toccati.
        assert!(d.devs.iter().all(|x| x.status == DiffStatus::Identical));
        assert!(!d.is_empty());
    }

    #[test]
    fn end_week_and_category_changes_are_both_reported() {
        let off = base_project();
        let mut par = off.clone();
        par.set_end_week(Some(WeekId(20050)));
        par.set_category(Some(crate::categories::CategoryId(3)));
        let d = diff_projects(&off, &par);
        assert!(d.header.contains(&HeaderField::EndWeek));
        assert!(d.header.contains(&HeaderField::Category));
        assert_eq!(d.header.len(), 2);
    }

    #[test]
    fn changed_dev_effort_marks_that_dev_changed() {
        let off = base_project();
        let mut par = off.clone();
        // Aggiungo effort in una nuova settimana al dev 0.
        par.add_effort(DevId(0), WeekId(20007), WorkerId(0), Effort(5));
        let d = diff_projects(&off, &par);
        assert!(d.header.is_empty());
        let d0 = d.devs.iter().find(|x| x.dev == DevId(0)).unwrap();
        let d1 = d.devs.iter().find(|x| x.dev == DevId(1)).unwrap();
        assert_eq!(d0.status, DiffStatus::Changed);
        assert_eq!(d1.status, DiffStatus::Identical);
    }

    #[test]
    fn dev_only_in_one_file_is_flagged() {
        let off = base_project();
        let mut par = off.clone();
        // Il file parallelo aggiunge un dev nuovo (id 2).
        par.add_dev(DevId(2));
        par.add_effort(DevId(2), WeekId(20000), WorkerId(0), Effort(4));
        // …e rimuove un dev esistente (id 1).
        par.del_dev(DevId(1));

        let d = diff_projects(&off, &par);
        let d1 = d.devs.iter().find(|x| x.dev == DevId(1)).unwrap();
        let d2 = d.devs.iter().find(|x| x.dev == DevId(2)).unwrap();
        assert_eq!(d1.status, DiffStatus::OnlyOfficial);
        assert_eq!(d2.status, DiffStatus::OnlyParallel);
        // changed_devs elenca solo i due dev diversi.
        assert_eq!(d.changed_devs().count(), 2);
    }

    #[test]
    fn milestone_order_does_not_produce_false_diff() {
        let mut off = base_project();
        let mut par = off.clone();
        // Stesse milestone inserite in ordine diverso → nessuna differenza.
        off.add_milestone(MilestoneId(0), WeekId(20000));
        off.add_milestone(MilestoneId(1), WeekId(20007));
        par.add_milestone(MilestoneId(1), WeekId(20007));
        par.add_milestone(MilestoneId(0), WeekId(20000));
        let d = diff_projects(&off, &par);
        assert!(!d.header.contains(&HeaderField::Milestones), "diff: {d:?}");
    }
}
