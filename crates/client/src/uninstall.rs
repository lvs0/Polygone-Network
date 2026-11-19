//! Désinstallation énumérée — `polygone desinstaller`.
//!
//! # Le contrat
//!
//! 1. **Énumérer avant de supprimer.** Chaque chemin est imprimé avant toute
//!    écriture, y compris ceux qui n'existent pas : l'utilisateur doit pouvoir
//!    comparer ce qu'il croyait installé avec ce qui l'est.
//! 2. **Ne jamais sortir des racines connues.** Les seules racines sont
//!    `~/.polygone`, `~/.config/polygone`, `~/.local/share/polygone` et les
//!    binaires `~/.local/bin/polygone*`. Un chemin hors de là est une erreur,
//!    pas une suppression.
//! 3. **Rien de silencieux.** Chaque suppression est rendue à l'appelant pour
//!    être imprimée ; un dossier non vide est supprimé avec son contenu, mais
//!    son contenu avait été énuméré.
//!
//! Ce que ce module ne fait pas : retirer la ligne `PATH` du rc shell. Elle a
//! été ajoutée par l'installateur dans un fichier qui ne nous appartient pas ;
//! la supprimer à l'aveugle abîmerait une configuration utilisateur. La sortie
//! CLI le dit.

use std::path::{Path, PathBuf};

/// Une entrée du plan : ce qui existe, ou ce qui manque.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanEntry {
    /// Chemin exact concerné.
    pub path: PathBuf,
    /// Nature lisible (`dossier (3 entrées)`, `fichier 21 Kio`, `absent`).
    pub label: String,
    /// Vrai si le chemin existe au moment de l'énumération.
    pub exists: bool,
}

/// Ce qu'une désinstallation toucherait, dans l'ordre d'affichage.
#[derive(Debug, Clone)]
pub struct Plan {
    /// Répertoires racines appartenant au produit.
    pub roots: Vec<PathBuf>,
    /// Binaires connus, installés ou non.
    pub binaries: Vec<PathBuf>,
    /// Contenu direct des racines existantes.
    pub entries: Vec<PlanEntry>,
}

impl Plan {
    /// Nombre de chemins réellement présents — ce que la suppression enlèvera.
    pub fn existing_count(&self) -> usize {
        self.entries
            .iter()
            .filter(|entry| entry.path.exists())
            .count()
            + self.binaries.iter().filter(|path| path.exists()).count()
            + self.roots.iter().filter(|path| path.exists()).count()
    }
}

/// `$HOME`. Refuse de deviner : sans variable, aucune suppression n'est sûre.
pub fn home_dir() -> Result<PathBuf, String> {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .filter(|path| !path.as_os_str().is_empty())
        .ok_or_else(|| "HOME absent : impossible de savoir quoi désinstaller".to_string())
}

/// Les binaires que l'installateur copie dans `~/.local/bin`.
pub const BINARIES: [&str; 4] = [
    "polygone",
    "polygoned",
    "polygone-relay",
    "polygone-gateway",
];

fn describe(path: &Path) -> String {
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string());
    match std::fs::metadata(path) {
        Ok(metadata) if metadata.is_dir() => {
            let count = std::fs::read_dir(path)
                .map(|entries| entries.count())
                .unwrap_or(0);
            format!("{name}/ — dossier ({count} entrée(s))")
        }
        Ok(metadata) => format!("{name} — fichier ({} o)", metadata.len()),
        Err(_) => format!("{name} — absent"),
    }
}

/// Énumère sans rien modifier.
pub fn plan(home: &Path) -> Plan {
    let roots = vec![
        home.join(".polygone"),
        home.join(".config").join("polygone"),
        home.join(".local").join("share").join("polygone"),
    ];
    let binaries: Vec<PathBuf> = BINARIES
        .iter()
        .map(|name| home.join(".local").join("bin").join(name))
        .collect();

    let mut entries = Vec::new();
    for root in &roots {
        if !root.exists() {
            entries.push(PlanEntry {
                path: root.clone(),
                label: describe(root),
                exists: false,
            });
            continue;
        }
        let mut children: Vec<PathBuf> = std::fs::read_dir(root)
            .map(|read| {
                read.filter_map(|entry| entry.ok().map(|e| e.path()))
                    .collect()
            })
            .unwrap_or_default();
        children.sort();
        for child in children {
            entries.push(PlanEntry {
                path: child.clone(),
                label: describe(&child),
                exists: true,
            });
        }
    }

    Plan {
        roots,
        binaries,
        entries,
    }
}

/// Vrai seulement si `path` est l'une des racines du plan ou vit dedans.
fn inside(path: &Path, plan: &Plan) -> bool {
    plan.roots.iter().any(|root| path.starts_with(root))
}

/// Supprime tout ce que le plan a énuméré. Idempotent : un second appel ne
/// trouve plus rien et rend une liste vide au lieu d'une erreur.
pub fn remove(plan: &Plan) -> Result<Vec<String>, String> {
    let mut removed = Vec::new();

    for entry in plan.entries.iter().filter(|entry| entry.path.exists()) {
        if !inside(&entry.path, plan) {
            return Err(format!(
                "refus de supprimer {} : hors des racines Polygone",
                entry.path.display()
            ));
        }
    }
    for binary in plan.binaries.iter().filter(|path| path.exists()) {
        if !binary.starts_with(plan.roots[0].parent().unwrap_or(Path::new(""))) {
            return Err(format!(
                "refus de supprimer {} : hors de $HOME",
                binary.display()
            ));
        }
    }

    for entry in plan.entries.iter().filter(|entry| entry.path.exists()) {
        if entry.path.is_dir() {
            // Le contenu a été énuméré juste au-dessus : c'est le dossier du
            // produit, pas une arborescence inconnue.
            std::fs::remove_dir_all(&entry.path)
                .map_err(|error| format!("suppression de {} : {error}", entry.path.display()))?;
        } else {
            std::fs::remove_file(&entry.path)
                .map_err(|error| format!("suppression de {} : {error}", entry.path.display()))?;
        }
        removed.push(entry.path.display().to_string());
    }

    for root in plan.roots.iter().filter(|root| root.exists()) {
        std::fs::remove_dir_all(root)
            .map_err(|error| format!("suppression de {} : {error}", root.display()))?;
        removed.push(root.display().to_string());
    }

    for binary in plan.binaries.iter().filter(|path| path.exists()) {
        std::fs::remove_file(binary)
            .map_err(|error| format!("suppression de {} : {error}", binary.display()))?;
        removed.push(binary.display().to_string());
    }

    let leftovers: Vec<&Path> = plan
        .roots
        .iter()
        .chain(plan.binaries.iter())
        .filter(|path| path.exists())
        .map(|path| path.as_path())
        .collect();
    if !leftovers.is_empty() {
        let names: Vec<String> = leftovers
            .iter()
            .map(|path| path.display().to_string())
            .collect();
        return Err(format!(
            "suppression incomplète, restes : {}",
            names.join(", ")
        ));
    }

    Ok(removed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    struct FakeHome(PathBuf);

    impl FakeHome {
        fn new(tag: &str) -> Self {
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "polygone-desinstalleur-{tag}-{}-{nonce}",
                std::process::id()
            ));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }

        fn touch(&self, relative: &str) {
            let path = self.0.join(relative);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, b"x").unwrap();
        }
    }

    impl Drop for FakeHome {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn plan_lists_state_config_share_and_binaries() {
        let home = FakeHome::new("plan");
        home.touch(".polygone/identity.json");
        home.touch(".polygone/ledger.json");
        home.touch(".config/polygone/daemon.toml");
        home.touch(".local/bin/polygone");

        let planned = plan(home.path());
        let labels: Vec<String> = planned
            .entries
            .iter()
            .filter(|entry| entry.exists)
            .map(|entry| entry.label.clone())
            .collect();
        assert!(
            labels
                .iter()
                .any(|label| label.starts_with("identity.json")),
            "{labels:?}"
        );
        assert!(
            labels.iter().any(|label| label.starts_with("daemon.toml")),
            "{labels:?}"
        );
        assert!(
            planned
                .entries
                .iter()
                .any(|entry| entry.path.ends_with(".local/share/polygone") && !entry.exists),
            "une racine absente est énumérée quand même"
        );
        assert!(
            planned
                .binaries
                .iter()
                .any(|path| path.ends_with(".local/bin/polygone") && path.exists()),
            "le binaire installé est énuméré"
        );
        assert!(
            planned.existing_count() >= 4,
            "{}",
            planned.existing_count()
        );
    }

    #[test]
    fn remove_deletes_everything_without_touching_other_files() {
        let home = FakeHome::new("remove");
        home.touch(".polygone/identity.json");
        home.touch(".polygone/received/message.txt");
        home.touch(".config/polygone/daemon.toml");
        home.touch(".local/share/polygone/web/index.html");
        home.touch(".local/bin/polygone");
        home.touch(".local/bin/polygoned");
        home.touch("Documents/keep.txt");

        let planned = plan(home.path());
        let removed = remove(&planned).expect("suppression complète");

        assert!(removed.len() >= 5, "{removed:?}");
        for root in &planned.roots {
            assert!(!root.exists(), "racine restée : {}", root.display());
        }
        assert!(!home.path().join(".local/bin/polygone").exists());
        assert!(
            home.path().join("Documents/keep.txt").exists(),
            "un fichier hors Polygone ne doit jamais être touché"
        );
        assert_eq!(plan(home.path()).existing_count(), 0);
    }

    #[test]
    fn remove_is_idempotent() {
        let home = FakeHome::new("idempotent");
        home.touch(".polygone/identity.json");
        let planned = plan(home.path());
        remove(&planned).expect("premier passage");
        let removed = remove(&planned).expect("second passage sans erreur");
        assert!(removed.is_empty(), "{removed:?}");
    }

    #[test]
    fn remove_refuses_a_path_outside_the_known_roots() {
        let home = FakeHome::new("garde");
        home.touch("Documents/keep.txt");
        let mut planned = plan(home.path());
        planned.entries.push(PlanEntry {
            path: home.path().join("Documents/keep.txt"),
            label: "keep.txt".into(),
            exists: true,
        });
        let error = remove(&planned).expect_err("chemin hors racines");
        assert!(error.contains("hors des racines"), "{error}");
        assert!(home.path().join("Documents/keep.txt").exists());
    }
}
