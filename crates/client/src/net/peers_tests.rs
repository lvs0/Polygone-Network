//! Migration du magasin d'ancres de confiance `peers.json` : v1 (identifiant de
//! 16 caractères, dérivé de la clé ML-KEM) → v2 (identifiant de 32 caractères,
//! dérivé de la clé de signature ML-DSA).
//!
//! Règle non négociable, testée ici : une entrée n'est **jamais** supprimée. Les
//! identifiants v1 restent à côté des nouveaux, pour qu'un binaire antérieur qui
//! lit le même fichier continue de reconnaître les pairs qu'il connaissait.

use super::*;
use crate::identity::LocalIdentity;

fn temp_root(label: &str) -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!(
        "polygone-{label}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(root.join(".polygone")).unwrap();
    root
}

fn store_path(root: &std::path::Path) -> std::path::PathBuf {
    root.join(".polygone").join("peers.json")
}

fn write_store(path: &std::path::Path, peers: &HashMap<String, String>) {
    let raw = serde_json::to_vec_pretty(peers).unwrap();
    crate::identity::write_private_atomic(path, &raw).unwrap();
}

fn read_store(path: &std::path::Path) -> HashMap<String, String> {
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

/// Un identifiant v1 : 16 caractères hexadécimaux.
const LEGACY_ID: &str = "0123456789abcdef";

#[test]
fn a_v1_store_gains_the_derived_id_and_keeps_the_legacy_entry() {
    let identity = LocalIdentity::generate();
    let root = temp_root("peers-v1");
    let path = store_path(&root);
    write_store(
        &path,
        &HashMap::from([(LEGACY_ID.to_string(), identity.sign_pk_hex.clone())]),
    );

    let peers = load_peers_from_path(&path).unwrap();
    let derived = polygone_core::sign::node_id_from_sign_pk_hex(&identity.sign_pk_hex).unwrap();

    assert_eq!(derived.len(), NODE_ID_HEX_LEN);
    assert_eq!(
        peers.len(),
        2,
        "l'entrée v1 et l'entrée dérivée doivent coexister"
    );
    assert_eq!(peers.get(LEGACY_ID), Some(&identity.sign_pk_hex));
    assert_eq!(
        peers.get(&derived),
        Some(&identity.sign_pk_hex),
        "le pair doit être reconnu sous son identifiant courant"
    );

    // Le fichier a bien été réécrit, et l'original est conservé tel quel.
    let on_disk = read_store(&path);
    assert_eq!(on_disk, peers);
    let backup = peers_v1_backup_path(&path);
    assert!(backup.exists(), "l'original v1 doit être conservé");
    let backup_raw = std::fs::read_to_string(&backup).unwrap();
    assert!(
        backup_raw.contains(LEGACY_ID),
        "la copie doit contenir l'identifiant v1 d'origine"
    );
    assert!(
        !backup_raw.contains(&derived),
        "la copie doit être l'original, pas la version migrée"
    );

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn the_migration_is_idempotent_and_rewrites_nothing_on_the_second_load() {
    let identity = LocalIdentity::generate();
    let root = temp_root("peers-idempotent");
    let path = store_path(&root);
    write_store(
        &path,
        &HashMap::from([(LEGACY_ID.to_string(), identity.sign_pk_hex.clone())]),
    );

    load_peers_from_path(&path).unwrap();
    let after_first = std::fs::read(&path).unwrap();
    let backup = peers_v1_backup_path(&path);
    let backup_after_first = std::fs::read(&backup).unwrap();

    let peers = load_peers_from_path(&path).unwrap();

    assert_eq!(peers.len(), 2);
    assert_eq!(
        std::fs::read(&path).unwrap(),
        after_first,
        "la seconde lecture ne doit rien réécrire"
    );
    assert_eq!(
        std::fs::read(&backup).unwrap(),
        backup_after_first,
        "la copie v1 ne doit pas être écrasée par la version migrée"
    );

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn an_entry_whose_key_is_unreadable_is_kept_and_reported_not_dropped() {
    let root = temp_root("peers-unreadable");
    let path = store_path(&root);
    let bogus = HashMap::from([(LEGACY_ID.to_string(), "pas-une-cle".to_string())]);
    write_store(&path, &bogus);

    let peers = load_peers_from_path(&path).unwrap();

    assert_eq!(
        peers, bogus,
        "une entrée illisible est conservée telle quelle, jamais supprimée"
    );
    assert!(
        !peers_v1_backup_path(&path).exists(),
        "aucune migration n'a eu lieu, donc aucune copie n'est écrite"
    );

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_current_store_is_left_untouched() {
    let identity = LocalIdentity::generate();
    let derived = polygone_core::sign::node_id_from_sign_pk_hex(&identity.sign_pk_hex).unwrap();
    let root = temp_root("peers-current");
    let path = store_path(&root);
    write_store(
        &path,
        &HashMap::from([(derived.clone(), identity.sign_pk_hex.clone())]),
    );
    let before = std::fs::read(&path).unwrap();

    let peers = load_peers_from_path(&path).unwrap();

    assert_eq!(peers.len(), 1);
    assert_eq!(std::fs::read(&path).unwrap(), before);
    assert!(!peers_v1_backup_path(&path).exists());

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn the_migration_never_exceeds_the_peer_cap() {
    let identity = LocalIdentity::generate();
    let root = temp_root("peers-cap");
    let path = store_path(&root);
    let mut full: HashMap<String, String> = (0..MAX_KNOWN_PEERS)
        .map(|index| (format!("pair-{index}"), "cle".to_string()))
        .collect();
    full.insert(LEGACY_ID.to_string(), identity.sign_pk_hex.clone());
    write_store(&path, &full);

    let peers = load_peers_from_path(&path).unwrap();

    assert_eq!(
        peers.len(),
        MAX_KNOWN_PEERS + 1,
        "le plafond borne les ajouts, il ne supprime pas l'existant"
    );
    assert!(peers.contains_key(LEGACY_ID));

    std::fs::remove_dir_all(root).unwrap();
}
