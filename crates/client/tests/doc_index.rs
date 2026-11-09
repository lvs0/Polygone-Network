use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

const STATUSES: &[&str] = &["autorité", "daté", "brouillon", "à supprimer"];

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn collect_markdown(dir: &Path, repo: &Path, out: &mut Vec<String>) {
    let entries = fs::read_dir(dir)
        .unwrap_or_else(|error| panic!("lecture de {} impossible : {error}", dir.display()));

    for entry in entries {
        let entry = entry.expect("entrée de répertoire illisible");
        let path = entry.path();

        if path.is_dir() {
            collect_markdown(&path, repo, out);
        } else if path.extension().is_some_and(|extension| extension == "md") {
            let relative = path
                .strip_prefix(repo)
                .expect("le document doit être dans le dépôt")
                .to_string_lossy()
                .replace('\\', "/");
            out.push(relative);
        }
    }
}

#[test]
fn every_markdown_document_is_indexed_once_with_a_status() {
    let repo = repository_root();
    let docs = repo.join("docs");
    let index_path = docs.join("INDEX.md");
    let index = fs::read_to_string(&index_path)
        .unwrap_or_else(|error| panic!("lecture de docs/INDEX.md impossible : {error}"));

    let mut indexed = BTreeMap::<String, (String, String)>::new();

    for (line_number, line) in index.lines().enumerate() {
        let line = line.trim();
        if !line.starts_with("| `docs/") {
            continue;
        }

        let cells: Vec<_> = line.split('|').map(str::trim).collect();
        assert_eq!(
            cells.len(),
            8,
            "docs/INDEX.md:{} : ligne de tableau attendue avec 6 colonnes",
            line_number + 1
        );

        let path = cells[1]
            .strip_prefix('`')
            .and_then(|value| value.strip_suffix('`'))
            .unwrap_or_else(|| panic!("docs/INDEX.md:{} : chemin mal formé", line_number + 1))
            .to_owned();

        let title = cells[2];
        let date = cells[3];
        let status = cells[4];
        let theme = cells[5];
        let justification = cells[6];

        assert!(
            !title.is_empty(),
            "docs/INDEX.md:{} : titre absent pour {path}",
            line_number + 1
        );
        assert!(
            !date.is_empty(),
            "docs/INDEX.md:{} : date absente pour {path}",
            line_number + 1
        );
        assert!(
            STATUSES.contains(&status),
            "docs/INDEX.md:{} : statut absent ou invalide « {status} » pour {path}",
            line_number + 1
        );
        assert!(
            !theme.is_empty(),
            "docs/INDEX.md:{} : thème absent pour {path}",
            line_number + 1
        );
        assert!(
            !justification.is_empty(),
            "docs/INDEX.md:{} : justification absente pour {path}",
            line_number + 1
        );

        assert!(
            indexed
                .insert(path.clone(), (status.to_owned(), title.to_owned()))
                .is_none(),
            "docs/INDEX.md:{} : {path} apparaît plusieurs fois",
            line_number + 1
        );
    }

    let mut actual = Vec::new();
    collect_markdown(&docs, &repo, &mut actual);
    actual.sort();

    let indexed_paths: Vec<_> = indexed.keys().cloned().collect();
    let missing: Vec<_> = actual
        .iter()
        .filter(|path| !indexed.contains_key(*path))
        .cloned()
        .collect();
    let stale: Vec<_> = indexed_paths
        .iter()
        .filter(|path| !actual.contains(path))
        .cloned()
        .collect();

    assert!(
        missing.is_empty() && stale.is_empty(),
        "docs/INDEX.md ne correspond pas à docs/\nDocuments oubliés : {missing:#?}\nEntrées sans fichier : {stale:#?}"
    );
}
