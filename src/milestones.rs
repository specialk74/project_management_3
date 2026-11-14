use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::project_utils::project::ProjectId;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(
    Serialize, Deserialize, Hash, PartialEq, Eq, Clone, Copy, PartialOrd, Ord, Debug, Default,
)]
pub struct MilestoneId(pub usize);

/// Palette di colori distinti (0xRRGGBB) da cui pescare per le nuove milestone.
/// Alla creazione si sceglie a caso un colore non ancora usato; se sono tutti
/// occupati si genera un colore pseudo-casuale (`fallback_color`).
/// Tutti **leggibili con entrambi i temi** (`readable_on_both_themes`): niente
/// colori troppo scuri (spariscono sul fondo scuro) né troppo chiari (spariscono
/// su quello chiaro); per questo giallo e ambra sono tinte ocra.
const PALETTE: [u32; 16] = [
    0xE53935, // rosso
    0xAB47BC, // viola
    0x5C6BC0, // indaco
    0x1E88E5, // blu
    0x00ACC1, // ciano
    0x00897B, // teal
    0x43A047, // verde
    0x689F38, // lime
    0x9E9D24, // giallo-verde
    0xB59A00, // giallo (ocra)
    0xC79100, // ambra
    0xEF6C00, // arancione
    0xF4511E, // arancione scuro
    0x8D6E63, // marrone
    0x757575, // grigio
    0xD81B60, // rosa
];

/// Sfondi dei due temi dell'app (`ui_style::bg()`: scuro `#1E1E1E`, chiaro
/// `#F5F5F5`), contro cui si misura il contrasto dei colori nuovi.
const DARK_BG: u32 = 0x1E1E1E;
const LIGHT_BG: u32 = 0xF5F5F5;
/// Contrasto minimo (rapporto WCAG) di un colore nuovo con **ciascuno** dei
/// due sfondi. Il massimo possibile per entrambi insieme è ~3,9 (un colore di
/// luminosità media); 2,5 tiene i colori vivaci ma visibili su tutti e due.
#[cfg(test)]
const MIN_THEME_CONTRAST: f32 = 2.5;

/// Luminanza relativa (WCAG) di un colore 0xRRGGBB, 0 (nero) … 1 (bianco).
fn luminance(rgb: u32) -> f32 {
    let lin = |shift: u32| {
        let c = ((rgb >> shift) & 0xFF) as f32 / 255.0;
        if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
    };
    0.2126 * lin(16) + 0.7152 * lin(8) + 0.0722 * lin(0)
}

/// Rapporto di contrasto WCAG fra due colori (1 … 21).
#[cfg(test)]
fn contrast(a: u32, b: u32) -> f32 {
    let (la, lb) = (luminance(a), luminance(b));
    (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
}

/// Il colore si vede bene sia col tema scuro sia con quello chiaro.
#[cfg(test)]
pub fn readable_on_both_themes(rgb: u32) -> bool {
    contrast(rgb, DARK_BG) >= MIN_THEME_CONTRAST && contrast(rgb, LIGHT_BG) >= MIN_THEME_CONTRAST
}

/// Colore di riserva (palette esaurita): tinta presa da `seed`, saturazione
/// fissa e luminosità cercata (bisezione) per stare a metà fra i due sfondi,
/// dove il contrasto con entrambi è massimo; quindi sempre leggibile.
fn fallback_color(seed: u128) -> u32 {
    let hue = (seed % 360) as f32;
    let hsl = |l: f32| -> u32 {
        let s = 0.75;
        let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
        let h = hue / 60.0;
        let x = c * (1.0 - (h % 2.0 - 1.0).abs());
        let (r, g, b) = match h as u32 {
            0 => (c, x, 0.0),
            1 => (x, c, 0.0),
            2 => (0.0, c, x),
            3 => (0.0, x, c),
            4 => (x, 0.0, c),
            _ => (c, 0.0, x),
        };
        let m = l - c / 2.0;
        let ch = |v: f32| ((v + m) * 255.0).round().clamp(0.0, 255.0) as u32;
        (ch(r) << 16) | (ch(g) << 8) | ch(b)
    };
    // Luminanza con lo stesso contrasto verso i due sfondi.
    let target = ((luminance(DARK_BG) + 0.05) * (luminance(LIGHT_BG) + 0.05)).sqrt() - 0.05;
    let (mut lo, mut hi) = (0.0_f32, 1.0_f32);
    for _ in 0..24 {
        let mid = (lo + hi) / 2.0;
        if luminance(hsl(mid)) < target {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    hsl((lo + hi) / 2.0)
}

/// Tipo di milestone. Una milestone non è solo un traguardo: può essere il
/// **trigger** di un evento. Il tipo è una proprietà della milestone stessa
/// (come nome e colore), quindi vale in tutti i progetti in cui è collocata.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum MilestoneKind {
    /// Traguardo: nell'export è la classica bandierina (pennant).
    #[default]
    Goal,
    /// Trigger di evento: nell'export la bandierina è sostituita da un fulmine.
    Trigger,
}

impl MilestoneKind {
    /// Etichetta mostrata nella GUI (menù di creazione, gestore milestone).
    pub fn label(self) -> &'static str {
        match self {
            MilestoneKind::Goal => "Traguardo",
            MilestoneKind::Trigger => "Trigger",
        }
    }

    /// Icona del tipo, da anteporre al nome negli elenchi della GUI: bandierina
    /// per il traguardo, fulmine per il trigger — gli stessi simboli disegnati
    /// in cima all'asta nell'export. Sono glifi dei font già caricati da egui
    /// (fallback compreso, vedi `install_symbol_fallback`), niente immagini.
    pub fn icon(self) -> &'static str {
        match self {
            MilestoneKind::Goal => "⚑",
            MilestoneKind::Trigger => "⚡",
        }
    }

    pub fn is_trigger(self) -> bool {
        matches!(self, MilestoneKind::Trigger)
    }
}

/// Categoria di stampa di una milestone: una milestone può appartenere a una o
/// più categorie. Serve **solo** in export: decide in quale PDF la bandierina
/// compare (vedi `MilestoneScope`).
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug, Hash, PartialOrd, Ord)]
pub enum MilestoneCategory {
    Internal,
    External,
}

impl MilestoneCategory {
    /// Tutte le categorie esistenti, nell'ordine in cui vanno mostrate.
    pub const ALL: [MilestoneCategory; 2] = [MilestoneCategory::Internal, MilestoneCategory::External];

    pub fn label(self) -> &'static str {
        match self {
            MilestoneCategory::Internal => "Internal",
            MilestoneCategory::External => "External",
        }
    }

    /// Etichetta breve per i controlli compatti (gestore milestone).
    pub fn short(self) -> &'static str {
        match self {
            MilestoneCategory::Internal => "Int",
            MilestoneCategory::External => "Ext",
        }
    }
}

/// Ambito di stampa scelto nelle dialog di export: quali milestone finiscono nel
/// PDF/SVG. Regola voluta dall'utente:
/// - `Internal` («Solo internal») → le milestone marcate **Internal** (anche
///   Int/Ext; quelle senza categoria valgono Internal);
/// - `External` («Solo external») → le milestone marcate **External** (anche
///   Int/Ext);
/// - `Mix` → **tutte**.
///
/// Ogni ambito scelto produce un file a sé. `Mix` è il default del flag di
/// rendering (nessuna restrizione).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum MilestoneScope {
    Internal,
    External,
    #[default]
    Mix,
}

impl MilestoneScope {
    #[cfg(test)]
    pub fn label(self) -> &'static str {
        match self {
            MilestoneScope::Internal => "Solo internal",
            MilestoneScope::External => "Solo external",
            MilestoneScope::Mix => "Mix",
        }
    }

    /// Suffisso aggiunto al nome file quando si esportano più ambiti.
    pub fn file_suffix(self) -> &'static str {
        match self {
            MilestoneScope::Internal => "internal",
            MilestoneScope::External => "external",
            MilestoneScope::Mix => "mix",
        }
    }

    /// Una milestone con queste categorie (già «effettive»: vuoto = Internal)
    /// va stampata in questo ambito?
    pub fn accepts(self, categories: &[MilestoneCategory]) -> bool {
        match self {
            MilestoneScope::Internal => {
                categories.is_empty() || categories.contains(&MilestoneCategory::Internal)
            }
            MilestoneScope::External => categories.contains(&MilestoneCategory::External),
            MilestoneScope::Mix => true,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub struct Milestone {
    pub name: String,
    /// Colore associato, formato 0xRRGGBB.
    pub color: u32,
    /// Traguardo (default) o trigger di evento. `serde(default)` per i file
    /// scritti prima dell'introduzione del tipo: si rileggono come traguardi.
    #[serde(default)]
    pub kind: MilestoneKind,
    /// Categorie di stampa (vedi `MilestoneCategory`). **Elenco vuoto = solo
    /// Internal**: è così che si rileggono i file scritti prima delle categorie,
    /// che quindi continuano a comparire nel PDF Internal come prima.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub categories: Vec<MilestoneCategory>,
    /// Progetto proprietario: `None` = milestone **di sistema** (globale,
    /// disponibile in tutti i progetti), `Some(p)` = milestone **personalizzata**
    /// di quel progetto, che non compare nell'elenco degli altri. `serde(default)`
    /// per i file scritti prima: si rileggono come globali.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner: Option<ProjectId>,
}

impl Milestone {
    /// La milestone appartiene alla categoria? L'elenco vuoto vale `Internal`.
    pub fn has_category(&self, cat: MilestoneCategory) -> bool {
        match cat {
            MilestoneCategory::Internal => {
                self.categories.is_empty() || self.categories.contains(&cat)
            }
            MilestoneCategory::External => self.categories.contains(&cat),
        }
    }

    /// Categorie effettive (mai vuote: l'elenco vuoto vale `Internal`).
    pub fn effective_categories(&self) -> Vec<MilestoneCategory> {
        MilestoneCategory::ALL
            .into_iter()
            .filter(|c| self.has_category(*c))
            .collect()
    }

    /// Va stampata in questo ambito di export?
    #[cfg(test)]
    pub fn in_scope(&self, scope: MilestoneScope) -> bool {
        scope.accepts(&self.effective_categories())
    }

    /// Milestone di sistema (usabile in tutti i progetti)?
    #[cfg(test)]
    pub fn is_global(&self) -> bool {
        self.owner.is_none()
    }

    /// È disponibile per questo progetto? Le globali sempre, le personalizzate
    /// solo nel progetto che le possiede.
    pub fn available_for(&self, proj: ProjectId) -> bool {
        self.owner.map(|o| o == proj).unwrap_or(true)
    }
}

#[derive(Serialize, Deserialize, Default, Clone, PartialEq)]
pub struct Milestones {
    last_id: MilestoneId,
    milestones: HashMap<MilestoneId, Milestone>,
}

/// Ordina le categorie come `MilestoneCategory::ALL`, toglie i doppioni e
/// garantisce che l'elenco non sia mai vuoto (vuoto = Internal, quindi si
/// scrive `Internal` esplicito).
fn normalize_categories(cats: Vec<MilestoneCategory>) -> Vec<MilestoneCategory> {
    let out: Vec<MilestoneCategory> = MilestoneCategory::ALL
        .into_iter()
        .filter(|c| cats.contains(c))
        .collect();
    if out.is_empty() {
        vec![MilestoneCategory::Internal]
    } else {
        out
    }
}

impl Milestones {
    pub fn new() -> Self {
        Self {
            last_id: MilestoneId(0),
            milestones: HashMap::new(),
        }
    }

    /// Colori attualmente in uso.
    fn used_colors(&self) -> Vec<u32> {
        self.milestones.values().map(|m| m.color).collect()
    }

    /// Sceglie un colore distinto: uno a caso tra quelli della palette non
    /// ancora usati; se sono tutti occupati, ne genera uno pseudo-casuale.
    /// In entrambi i casi leggibile con tutti e due i temi.
    fn pick_color(&self) -> u32 {
        let used = self.used_colors();
        let free: Vec<u32> = PALETTE
            .iter()
            .copied()
            .filter(|c| !used.contains(c))
            .collect();
        let seed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        if !free.is_empty() {
            free[(seed as usize) % free.len()]
        } else {
            fallback_color(seed)
        }
    }

    /// Crea una nuova milestone (traguardo) con un colore distinto assegnato in
    /// automatico.
    #[cfg(test)]
    pub fn add(&mut self, name: &str) -> MilestoneId {
        self.add_with_kind(name, MilestoneKind::default())
    }

    /// Come `add` ma scegliendo il tipo (traguardo / trigger).
    #[cfg(test)]
    pub fn add_with_kind(&mut self, name: &str, kind: MilestoneKind) -> MilestoneId {
        self.add_with(name, kind, Vec::new())
    }

    /// Come `add_with_kind` ma fissando anche le categorie di stampa
    /// (elenco vuoto = solo Internal). Milestone **globale**.
    pub fn add_with(
        &mut self,
        name: &str,
        kind: MilestoneKind,
        categories: Vec<MilestoneCategory>,
    ) -> MilestoneId {
        self.add_owned(name, kind, categories, None)
    }

    /// Crea una milestone **personalizzata** del progetto `owner` (`None` =
    /// globale, come `add_with`). Non comparirà negli elenchi degli altri
    /// progetti.
    pub fn add_owned(
        &mut self,
        name: &str,
        kind: MilestoneKind,
        categories: Vec<MilestoneCategory>,
        owner: Option<ProjectId>,
    ) -> MilestoneId {
        let color = self.pick_color();
        let id = self.last_id;
        self.milestones.insert(
            id,
            Milestone {
                name: name.to_string(),
                color,
                kind,
                categories: normalize_categories(categories),
                owner,
            },
        );
        self.last_id.0 += 1;
        id
    }

    pub fn del(&mut self, id: MilestoneId) {
        self.milestones.remove(&id);
    }

    #[cfg(test)]
    pub fn get(&self, id: MilestoneId) -> Option<&Milestone> {
        self.milestones.get(&id)
    }

    pub fn get_name(&self, id: MilestoneId) -> Option<&str> {
        self.milestones.get(&id).map(|m| m.name.as_str())
    }

    pub fn get_color(&self, id: MilestoneId) -> Option<u32> {
        self.milestones.get(&id).map(|m| m.color)
    }

    /// Tipo della milestone (traguardo se l'id non esiste più).
    pub fn get_kind(&self, id: MilestoneId) -> MilestoneKind {
        self.milestones
            .get(&id)
            .map(|m| m.kind)
            .unwrap_or_default()
    }

    pub fn is_trigger(&self, id: MilestoneId) -> bool {
        self.get_kind(id).is_trigger()
    }

    pub fn set_kind(&mut self, id: MilestoneId, kind: MilestoneKind) {
        if let Some(m) = self.milestones.get_mut(&id) {
            m.kind = kind;
        }
    }

    /// Categorie effettive della milestone (mai vuote: senza categorie è
    /// `Internal`). Milestone inesistente → `Internal`.
    pub fn get_categories(&self, id: MilestoneId) -> Vec<MilestoneCategory> {
        self.milestones
            .get(&id)
            .map(|m| m.effective_categories())
            .unwrap_or_else(|| vec![MilestoneCategory::Internal])
    }

    #[cfg(test)]
    pub fn has_category(&self, id: MilestoneId, cat: MilestoneCategory) -> bool {
        self.milestones
            .get(&id)
            .map(|m| m.has_category(cat))
            .unwrap_or(cat == MilestoneCategory::Internal)
    }

    /// Aggiunge/toglie una categoria. Una milestone non resta mai senza
    /// categorie: togliendo l'ultima si ricade su `Internal`.
    pub fn set_category(&mut self, id: MilestoneId, cat: MilestoneCategory, on: bool) {
        if let Some(m) = self.milestones.get_mut(&id) {
            let mut cats = m.effective_categories();
            cats.retain(|c| *c != cat);
            if on {
                cats.push(cat);
            }
            m.categories = normalize_categories(cats);
        }
    }

    /// La milestone va stampata in questo ambito di export? Un id sconosciuto
    /// (non dovrebbe capitare) è trattato come Internal.
    pub fn in_scope(&self, id: MilestoneId, scope: MilestoneScope) -> bool {
        scope.accepts(&self.get_categories(id))
    }

    /// Progetto proprietario (`None` = milestone di sistema).
    pub fn owner(&self, id: MilestoneId) -> Option<ProjectId> {
        self.milestones.get(&id).and_then(|m| m.owner)
    }

    /// La milestone è utilizzabile in questo progetto? (globale, oppure
    /// personalizzata **di** questo progetto).
    pub fn available_for(&self, id: MilestoneId, proj: ProjectId) -> bool {
        self.milestones
            .get(&id)
            .map(|m| m.available_for(proj))
            .unwrap_or(false)
    }

    /// Elenco (id, nome, colore) disponibile per un progetto: le globali più le
    /// personalizzate di quel progetto, ordinate per id di creazione.
    pub fn list_for_project(&self, proj: ProjectId) -> Vec<(MilestoneId, String, u32)> {
        let mut items: Vec<(MilestoneId, String, u32)> = self
            .milestones
            .iter()
            .filter(|(_, m)| m.available_for(proj))
            .map(|(&id, m)| (id, m.name.clone(), m.color))
            .collect();
        items.sort_by_key(|(id, _, _)| *id);
        items
    }

    /// Elimina tutte le milestone personalizzate di un progetto (da chiamare se
    /// il progetto sparisce, per non lasciare milestone orfane).
    #[cfg(test)]
    pub fn purge_project(&mut self, proj: ProjectId) -> Vec<MilestoneId> {
        let doomed: Vec<MilestoneId> = self
            .milestones
            .iter()
            .filter(|(_, m)| m.owner == Some(proj))
            .map(|(&id, _)| id)
            .collect();
        for id in &doomed {
            self.milestones.remove(id);
        }
        doomed
    }

    pub fn set_color(&mut self, id: MilestoneId, color: u32) {
        if let Some(m) = self.milestones.get_mut(&id) {
            m.color = color;
        }
    }

    /// Elenco (id, nome, colore) ordinato per id di creazione.
    pub fn list(&self) -> Vec<(MilestoneId, String, u32)> {
        let mut items: Vec<(MilestoneId, String, u32)> = self
            .milestones
            .iter()
            .map(|(&id, m)| (id, m.name.clone(), m.color))
            .collect();
        items.sort_by_key(|(id, _, _)| *id);
        items
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_returns_incrementing_ids() {
        let mut ms = Milestones::new();
        let a = ms.add("Alpha");
        let b = ms.add("Beta");
        assert_eq!(a, MilestoneId(0));
        assert_eq!(b, MilestoneId(1));
    }

    /// Colori delle nuove milestone: tutti leggibili su entrambi i temi, sia
    /// quelli della palette sia quelli di riserva (palette esaurita); i
    /// vecchi troppo scuri / troppo chiari sono scartati dalla regola.
    #[test]
    fn new_milestone_colors_are_readable_on_both_themes() {
        for c in PALETTE {
            assert!(readable_on_both_themes(c), "palette {c:06X}");
        }
        for seed in 0..720u128 {
            let c = fallback_color(seed * 7919);
            assert!(readable_on_both_themes(c), "riserva {c:06X} (seed {seed})");
        }
        // Prima nella palette: indaco e marrone troppo scuri, giallo troppo chiaro.
        for c in [0x3949AB, 0x6D4C41, 0xFDD835, 0xFFB300] {
            assert!(!readable_on_both_themes(c), "{c:06X}");
        }
        // Palette esaurita: anche le milestone in più hanno colori leggibili.
        let mut ms = Milestones::new();
        for i in 0..40 {
            let id = ms.add(&format!("M{i}"));
            let c = ms.get_color(id).unwrap();
            assert!(readable_on_both_themes(c), "M{i}: {c:06X}");
        }
        let distinct: std::collections::HashSet<u32> =
            ms.list().into_iter().map(|(_, _, c)| c).collect();
        assert!(distinct.len() >= PALETTE.len(), "i 16 della palette prima di ripetere");
    }

    #[test]
    fn add_assigns_distinct_colors() {
        let mut ms = Milestones::new();
        let a = ms.add("Alpha");
        let b = ms.add("Beta");
        assert_ne!(ms.get_color(a), ms.get_color(b));
    }

    #[test]
    fn del_removes_milestone() {
        let mut ms = Milestones::new();
        let a = ms.add("Alpha");
        ms.del(a);
        assert!(ms.get(a).is_none());
    }

    #[test]
    fn set_color_updates() {
        let mut ms = Milestones::new();
        let a = ms.add("Alpha");
        ms.set_color(a, 0x123456);
        assert_eq!(ms.get_color(a), Some(0x123456));
    }

    #[test]
    fn new_milestone_is_a_goal_and_kind_can_change() {
        let mut ms = Milestones::new();
        let a = ms.add("Alpha");
        assert_eq!(ms.get_kind(a), MilestoneKind::Goal);
        assert!(!ms.is_trigger(a));
        ms.set_kind(a, MilestoneKind::Trigger);
        assert!(ms.is_trigger(a));
    }

    #[test]
    fn add_with_kind_keeps_the_requested_kind() {
        let mut ms = Milestones::new();
        let t = ms.add_with_kind("Consegna", MilestoneKind::Trigger);
        assert!(ms.is_trigger(t));
    }

    #[test]
    fn old_files_without_kind_load_as_goal() {
        // I .ron scritti prima del tipo non hanno il campo `kind`.
        let m: Milestone = ron::from_str(r#"(name: "Vecchia", color: 123)"#).unwrap();
        assert_eq!(m.kind, MilestoneKind::Goal);
    }

    #[test]
    fn milestone_without_categories_counts_as_internal() {
        // I .ron scritti prima delle categorie non hanno il campo: valgono
        // Internal, quindi restano nella stampa Internal e fuori da External.
        let m: Milestone = ron::from_str(r#"(name: "Vecchia", color: 123)"#).unwrap();
        assert!(m.has_category(MilestoneCategory::Internal));
        assert!(!m.has_category(MilestoneCategory::External));
        assert!(m.in_scope(MilestoneScope::Internal));
        assert!(!m.in_scope(MilestoneScope::External));
    }

    #[test]
    fn scopes_select_internal_external_or_all() {
        let mut ms = Milestones::new();
        let int = ms.add("Interna");
        let ext = ms.add_with(
            "Esterna",
            MilestoneKind::Goal,
            vec![MilestoneCategory::External],
        );
        let both = ms.add_with(
            "Entrambe",
            MilestoneKind::Goal,
            vec![MilestoneCategory::Internal, MilestoneCategory::External],
        );
        // Mix: tutte.
        for id in [int, ext, both] {
            assert!(ms.in_scope(id, MilestoneScope::Mix));
        }
        // Solo internal: Internal e Int/Ext.
        assert!(ms.in_scope(int, MilestoneScope::Internal));
        assert!(!ms.in_scope(ext, MilestoneScope::Internal));
        assert!(ms.in_scope(both, MilestoneScope::Internal));
        // Solo external: External e Int/Ext.
        assert!(!ms.in_scope(int, MilestoneScope::External));
        assert!(ms.in_scope(ext, MilestoneScope::External));
        assert!(ms.in_scope(both, MilestoneScope::External));
    }

    #[test]
    fn set_category_never_leaves_a_milestone_without_categories() {
        let mut ms = Milestones::new();
        let a = ms.add("Alpha");
        ms.set_category(a, MilestoneCategory::External, true);
        assert_eq!(
            ms.get_categories(a),
            vec![MilestoneCategory::Internal, MilestoneCategory::External]
        );
        // Tolta Internal resta solo External…
        ms.set_category(a, MilestoneCategory::Internal, false);
        assert_eq!(ms.get_categories(a), vec![MilestoneCategory::External]);
        // …e togliendo anche l'ultima si ricade su Internal.
        ms.set_category(a, MilestoneCategory::External, false);
        assert_eq!(ms.get_categories(a), vec![MilestoneCategory::Internal]);
    }

    #[test]
    fn add_with_normalizes_categories() {
        let mut ms = Milestones::new();
        // Ordine invertito e doppione: l'elenco salvato è ordinato e unico.
        let a = ms.add_with(
            "Alpha",
            MilestoneKind::Goal,
            vec![
                MilestoneCategory::External,
                MilestoneCategory::Internal,
                MilestoneCategory::External,
            ],
        );
        assert_eq!(
            ms.get(a).unwrap().categories,
            vec![MilestoneCategory::Internal, MilestoneCategory::External]
        );
    }

    #[test]
    fn categories_survive_a_ron_round_trip() {
        let m = Milestone {
            name: "Consegna".to_string(),
            color: 0x112233,
            kind: MilestoneKind::Trigger,
            categories: vec![MilestoneCategory::Internal, MilestoneCategory::External],
            owner: Some(ProjectId(3)),
        };
        let txt = ron::to_string(&m).unwrap();
        let back: Milestone = ron::from_str(&txt).unwrap();
        assert_eq!(back.categories, m.categories);
        assert_eq!(back.kind, MilestoneKind::Trigger);
        assert_eq!(back.owner, Some(ProjectId(3)));
    }

    #[test]
    fn project_milestones_only_show_up_in_their_own_project() {
        let p1 = ProjectId(1);
        let p2 = ProjectId(2);
        let mut ms = Milestones::new();
        let globale = ms.add("Globale");
        let solo_p1 = ms.add_owned("Solo P1", MilestoneKind::Goal, Vec::new(), Some(p1));

        assert_eq!(ms.owner(globale), None);
        assert_eq!(ms.owner(solo_p1), Some(p1));
        assert!(ms.available_for(globale, p2));
        assert!(!ms.available_for(solo_p1, p2));

        let ids = |proj| -> Vec<MilestoneId> {
            ms.list_for_project(proj)
                .into_iter()
                .map(|(id, _, _)| id)
                .collect()
        };
        assert_eq!(ids(p1), vec![globale, solo_p1]);
        assert_eq!(ids(p2), vec![globale], "la milestone di P1 non compare in P2");
        // Il gestore invece le elenca tutte.
        assert_eq!(ms.list().len(), 2);
    }

    #[test]
    fn a_project_milestone_can_be_internal_external_or_both() {
        let p1 = ProjectId(1);
        let mut ms = Milestones::new();
        // Categorie scelte alla creazione dal menù della griglia.
        let ext = ms.add_owned(
            "Solo External",
            MilestoneKind::Goal,
            vec![MilestoneCategory::External],
            Some(p1),
        );
        let both = ms.add_owned(
            "Entrambe",
            MilestoneKind::Goal,
            vec![MilestoneCategory::External, MilestoneCategory::Internal],
            Some(p1),
        );
        let int = ms.add_owned("Solo Internal", MilestoneKind::Goal, Vec::new(), Some(p1));

        assert_eq!(ms.get_categories(ext), vec![MilestoneCategory::External]);
        assert_eq!(
            ms.get_categories(both),
            vec![MilestoneCategory::Internal, MilestoneCategory::External]
        );
        assert_eq!(ms.get_categories(int), vec![MilestoneCategory::Internal]);
        // Restano milestone del solo progetto, categorie a parte.
        for id in [ext, both, int] {
            assert_eq!(ms.owner(id), Some(p1));
            assert!(ms.in_scope(id, MilestoneScope::Mix));
        }
        assert!(!ms.in_scope(ext, MilestoneScope::Internal));
        assert!(ms.in_scope(both, MilestoneScope::Internal));
        assert!(ms.in_scope(int, MilestoneScope::Internal));
        assert!(ms.in_scope(ext, MilestoneScope::External));
        assert!(ms.in_scope(both, MilestoneScope::External));
        assert!(!ms.in_scope(int, MilestoneScope::External));
    }

    #[test]
    fn purge_project_removes_only_its_own_milestones() {
        let p1 = ProjectId(1);
        let mut ms = Milestones::new();
        let globale = ms.add("Globale");
        let solo_p1 = ms.add_owned("Solo P1", MilestoneKind::Goal, Vec::new(), Some(p1));

        assert_eq!(ms.purge_project(p1), vec![solo_p1]);
        assert!(ms.get(solo_p1).is_none());
        assert!(ms.get(globale).is_some());
    }

    #[test]
    fn old_files_without_owner_load_as_global() {
        let m: Milestone = ron::from_str(r#"(name: "Vecchia", color: 123)"#).unwrap();
        assert!(m.is_global());
        assert!(m.available_for(ProjectId(7)));
    }

    #[test]
    fn list_sorted_by_id() {
        let mut ms = Milestones::new();
        ms.add("Alpha");
        ms.add("Beta");
        ms.add("Gamma");
        let ids: Vec<MilestoneId> = ms.list().into_iter().map(|(id, _, _)| id).collect();
        assert_eq!(ids, vec![MilestoneId(0), MilestoneId(1), MilestoneId(2)]);
    }
}
