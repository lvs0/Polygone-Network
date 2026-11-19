//! SEC.8 — aucun secret ne doit entrer dans l'arbre par un `git add`.
//!
//! `.gitignore` protège les fichiers déjà connus, mais il ne bloque rien : un
//! `git add -f` passe outre, et un fichier renommé échappe au motif. Ce test
//! regarde ce que git **suit réellement**, pas ce qu'on croit qu'il ignore.
//!
//! Il échoue en nommant le chemin fautif. Il ne nettoie pas l'historique : un
//! secret déjà commité exige une rotation de clé, pas une réécriture (c'est le
//! refus explicite de la tâche).

use std::path::{Path, PathBuf};
use std::process::Command;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crates/client doit vivre sous la racine du dépôt")
        .to_path_buf()
}

/// Les gabarits sont faits pour être commités : ils ne contiennent pas de secret.
const TEMPLATES: [&str; 3] = [".env.example", ".env.sample", ".env.template"];

/// Vrai si le chemin ressemble à un porteur de secret.
///
/// Volontairement étroit : `identity.json` (clé privée du produit), `*.key`,
/// `.env` et ses variantes. Un motif plus large produirait des faux positifs,
/// et un test qui crie au loup est désactivé — donc ne protège plus.
fn looks_like_secret(path: &str) -> bool {
    let name = path.rsplit('/').next().unwrap_or(path);
    if TEMPLATES.contains(&name) {
        return false;
    }
    name == "identity.json" || name == ".env" || name.starts_with(".env.") || name.ends_with(".key")
}

/// Fichiers suivis par git, ou parcours du dépôt si git est indisponible
/// (source extraite d'une archive). Les deux chemins refusent les mêmes noms.
fn tracked_files() -> Vec<String> {
    let git = Command::new("git")
        .args(["ls-files", "-z"])
        .current_dir(repo_root())
        .output();

    if let Ok(output) = git {
        if output.status.success() {
            let listing = String::from_utf8_lossy(&output.stdout);
            let files: Vec<String> = listing
                .split('\0')
                .filter(|path| !path.is_empty())
                .map(str::to_string)
                .collect();
            if !files.is_empty() {
                return files;
            }
        }
    }

    let mut files = Vec::new();
    walk(&repo_root(), &repo_root(), &mut files);
    files
}

fn walk(root: &Path, dir: &Path, out: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if path.is_dir() {
            if matches!(name.as_str(), ".git" | "target" | "node_modules") {
                continue;
            }
            walk(root, &path, out);
        } else if let Ok(relative) = path.strip_prefix(root) {
            out.push(relative.to_string_lossy().replace('\\', "/"));
        }
    }
}

#[test]
fn no_tracked_secret_paths() {
    let files = tracked_files();
    assert!(
        files.len() > 20,
        "le dépôt doit contenir des fichiers suivis, trouvé {}",
        files.len()
    );

    let offenders: Vec<&String> = files
        .iter()
        .filter(|path| looks_like_secret(path))
        .collect();
    assert!(
        offenders.is_empty(),
        "SEC.8 : {} chemin(s) ressemblant à un secret sont SUIVIS par git :\n{}\n\
         Retirer du suivi (`git rm --cached`), puis faire tourner la clé si elle a été commitée.",
        offenders.len(),
        offenders
            .iter()
            .map(|path| format!("  - {path}"))
            .collect::<Vec<_>>()
            .join("\n")
    );
}

#[test]
fn matcher_names_exactly_what_it_refuses() {
    for path in [
        "identity.json",
        "secrets/id.key",
        "node.key",
        ".env",
        ".env.local",
        ".env.production",
    ] {
        assert!(looks_like_secret(path), "{path} doit être refusé");
    }
    for path in [
        ".env.example",
        ".env.sample",
        ".env.template",
        "src/identity.rs",
        "docs/identity.json.md",
        "Cargo.toml",
    ] {
        assert!(!looks_like_secret(path), "{path} ne doit pas être refusé");
    }
}
