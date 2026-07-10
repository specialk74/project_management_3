//! Sincronizzazione git opzionale del file dati.
//!
//! Se la cartella che contiene il file `.ron` è (dentro) un repository git **e il
//! file è già tracciato** (aggiunto in precedenza), dopo ogni salvataggio il file
//! viene **committato** (solo se è effettivamente cambiato) e inviato con
//! **`git push`**. Un file `.ron` **non ancora tracciato non viene aggiunto**: il
//! programma non mette da solo sotto controllo di versione un file che l'utente
//! non ha scelto di tracciare.
//!
//! Tutto è best-effort: se la cartella non è un repo, se il file non è tracciato,
//! se manca il remoto, se la rete o le credenziali non ci sono, l'operazione
//! non fa nulla (al più un log su stderr) e non blocca né altera il salvataggio
//! su disco. `GIT_TERMINAL_PROMPT=0` impedisce a git di restare bloccato a
//! chiedere le credenziali in modo interattivo.

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};

/// Evita sync git sovrapposti: ne gira al massimo uno alla volta.
static BUSY: AtomicBool = AtomicBool::new(false);

/// Rimette `BUSY` a `false` all'uscita dallo scope (anche in caso di panico).
struct BusyGuard;
impl Drop for BusyGuard {
    fn drop(&mut self) {
        BUSY.store(false, Ordering::SeqCst);
    }
}

/// Scompone il percorso del file in `(cartella, nome_file)`.
fn split(path: &str) -> Option<(PathBuf, OsString)> {
    let file = Path::new(path);
    let filename = file.file_name()?.to_os_string();
    let dir = file
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."));
    Some((dir, filename))
}

/// Aggiunge, committa e pusha il file **in background** (non blocca la UI).
/// Se un sync è già in corso, non fa nulla: il prossimo salvataggio riproverà.
pub fn commit_and_push(path: &str) {
    let Some((dir, filename)) = split(path) else {
        return;
    };
    // Un solo sync alla volta.
    if BUSY.swap(true, Ordering::SeqCst) {
        return;
    }
    std::thread::spawn(move || {
        let _guard = BusyGuard; // libera BUSY qualunque cosa accada
        run(&dir, &filename);
    });
}

/// Come [`commit_and_push`] ma **bloccante** (sul thread chiamante): usata in
/// uscita, così il push fa in tempo a completare prima che il processo termini.
pub fn commit_and_push_blocking(path: &str) {
    if let Some((dir, filename)) = split(path) {
        run(&dir, &filename);
    }
}

/// Esegue effettivamente add + commit (se serve) + push. Sincrona.
fn run(dir: &Path, filename: &OsStr) {
    // `git -C <dir>` con prompt credenziali disabilitato.
    let git = || {
        let mut c = Command::new("git");
        c.arg("-C").arg(dir).env("GIT_TERMINAL_PROMPT", "0");
        c
    };

    // La cartella è dentro un work tree git? In caso contrario non facciamo nulla.
    let inside = git().args(["rev-parse", "--is-inside-work-tree"]).output();
    let inside = matches!(&inside, Ok(o)
        if o.status.success() && String::from_utf8_lossy(&o.stdout).trim() == "true");
    if !inside {
        return;
    }

    // Sincronizziamo **solo** se il file è GIÀ tracciato dal repo (aggiunto in
    // precedenza). Se non lo è, non lo aggiungiamo noi: il programma non deve
    // mettere sotto controllo di versione un file che l'utente non ha tracciato.
    let tracked = git().args(["ls-files", "--"]).arg(filename).output();
    let tracked = matches!(&tracked, Ok(o) if o.status.success() && !o.stdout.is_empty());
    if !tracked {
        return;
    }

    // Stage del solo file dati (non tocchiamo altri file, es. i .bak).
    if git().arg("add").arg("--").arg(filename).status().is_err() {
        eprintln!("git: 'add' non eseguibile");
        return;
    }

    // Commit limitato al file dati; se non c'è nulla di nuovo, `commit` fallisce
    // (niente da committare) e ci fermiamo senza fare push inutili.
    let msg = format!(
        "Auto-save {} ({})",
        filename.to_string_lossy(),
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
    );
    let committed = git()
        .arg("commit")
        .arg("-m")
        .arg(&msg)
        .arg("--")
        .arg(filename)
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if !committed {
        return;
    }

    // Push best-effort.
    match git().arg("push").status() {
        Ok(s) if s.success() => {}
        Ok(_) => eprintln!("git: 'push' non riuscito (remoto/credenziali/rete?)"),
        Err(e) => eprintln!("git: 'push' non eseguibile: {e}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    /// Cartella temporanea unica (per test paralleli).
    fn unique_dir(tag: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("pjm_{tag}_{}_{nanos}", std::process::id()))
    }

    fn git_ok(dir: &Path, args: &[&str]) {
        let ok = Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(args)
            .status()
            .expect("git eseguibile")
            .success();
        assert!(ok, "git {args:?} è fallito");
    }

    #[test]
    fn commit_and_push_syncs_ron_to_remote() {
        let base = unique_dir("gitsync");
        let remote = base.join("remote.git");
        let work = base.join("work");
        std::fs::create_dir_all(&remote).unwrap();
        std::fs::create_dir_all(&work).unwrap();

        // Remoto "bare" locale (nessuna rete).
        assert!(
            Command::new("git")
                .args(["init", "--bare"])
                .arg(&remote)
                .status()
                .unwrap()
                .success()
        );
        // Repo di lavoro con config LOCALE (indipendente dal git globale del tester).
        assert!(
            Command::new("git")
                .args(["init"])
                .arg(&work)
                .status()
                .unwrap()
                .success()
        );
        git_ok(&work, &["symbolic-ref", "HEAD", "refs/heads/main"]);
        git_ok(&work, &["config", "user.email", "test@example.com"]);
        git_ok(&work, &["config", "user.name", "Test"]);
        git_ok(&work, &["config", "commit.gpgsign", "false"]);
        git_ok(&work, &["remote", "add", "origin", remote.to_str().unwrap()]);

        // Commit iniziale + upstream, così un semplice `git push` funziona.
        std::fs::write(work.join("workers.ron"), b"(v1)").unwrap();
        git_ok(&work, &["add", "workers.ron"]);
        git_ok(&work, &["commit", "-m", "init"]);
        git_ok(&work, &["push", "-u", "origin", "main"]);

        // Modifica + sync tramite la funzione reale.
        std::fs::write(work.join("workers.ron"), b"(v2-changed)").unwrap();
        run(&work, OsStr::new("workers.ron"));

        // Il remoto deve contenere la nuova versione del file.
        let show = Command::new("git")
            .arg("-C")
            .arg(&remote)
            .args(["show", "main:workers.ron"])
            .output()
            .unwrap();
        assert!(show.status.success(), "il remoto deve avere il file dopo il push");
        assert_eq!(String::from_utf8_lossy(&show.stdout), "(v2-changed)");

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn untracked_ron_is_left_alone() {
        // Repo con un commit iniziale (un README) e upstream, ma il .ron NON è
        // ancora stato aggiunto al repo (file non tracciato): non deve essere
        // aggiunto/committato/pushato dal programma.
        let base = unique_dir("gituntracked");
        let remote = base.join("remote.git");
        let work = base.join("work");
        std::fs::create_dir_all(&remote).unwrap();
        std::fs::create_dir_all(&work).unwrap();

        assert!(
            Command::new("git")
                .args(["init", "--bare"])
                .arg(&remote)
                .status()
                .unwrap()
                .success()
        );
        assert!(
            Command::new("git")
                .args(["init"])
                .arg(&work)
                .status()
                .unwrap()
                .success()
        );
        git_ok(&work, &["symbolic-ref", "HEAD", "refs/heads/main"]);
        git_ok(&work, &["config", "user.email", "test@example.com"]);
        git_ok(&work, &["config", "user.name", "Test"]);
        git_ok(&work, &["config", "commit.gpgsign", "false"]);
        git_ok(&work, &["remote", "add", "origin", remote.to_str().unwrap()]);

        // Commit iniziale SENZA il .ron, poi upstream.
        std::fs::write(work.join("README.md"), b"hello").unwrap();
        git_ok(&work, &["add", "README.md"]);
        git_ok(&work, &["commit", "-m", "init"]);
        git_ok(&work, &["push", "-u", "origin", "main"]);

        // Ora arriva il file dati, MAI aggiunto al repo: la funzione NON deve
        // aggiungerlo.
        std::fs::write(work.join("workers.ron"), b"(nuovo)").unwrap();
        run(&work, OsStr::new("workers.ron"));

        // Il remoto non deve avere il file.
        let show = Command::new("git")
            .arg("-C")
            .arg(&remote)
            .args(["show", "main:workers.ron"])
            .output()
            .unwrap();
        assert!(
            !show.status.success(),
            "un .ron non tracciato non deve finire nel remoto"
        );
        // E localmente deve restare non tracciato (non messo in stage).
        let staged = Command::new("git")
            .arg("-C")
            .arg(&work)
            .args(["ls-files", "--", "workers.ron"])
            .output()
            .unwrap();
        assert!(
            staged.stdout.is_empty(),
            "il .ron non tracciato non deve essere aggiunto al repo"
        );

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn no_git_repo_is_a_noop() {
        let base = unique_dir("nogit");
        std::fs::create_dir_all(&base).unwrap();
        std::fs::write(base.join("workers.ron"), b"(x)").unwrap();
        // Cartella NON git: non deve far nulla, né creare un repo, né dare panico.
        run(&base, OsStr::new("workers.ron"));
        assert!(!base.join(".git").exists(), "non deve inizializzare un repo");
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn split_extracts_dir_and_filename() {
        let (dir, name) = split("/tmp/some/workers.ron").unwrap();
        assert_eq!(dir, PathBuf::from("/tmp/some"));
        assert_eq!(name, OsString::from("workers.ron"));
        // Senza cartella → cartella corrente ".".
        let (dir, name) = split("workers.ron").unwrap();
        assert_eq!(dir, PathBuf::from("."));
        assert_eq!(name, OsString::from("workers.ron"));
    }
}
