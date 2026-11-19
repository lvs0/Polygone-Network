//! Données locales des domaines « Mon nœud » et « Réglages » (F1-API-01).
//!
//! Deux fichiers, deux lectures, aucune écriture : l'identité publique vient
//! de `~/.polygone/identity.json`, la configuration lecture seule de
//! `daemon.toml`. Les chemins sont injectables (`LocalPaths`) pour que chaque
//! route se teste sur des fichiers temporaires, jamais sur ceux de l'opérateur.
//!
//! Règle absolue : les clés SECRÈTES ne sont même pas désérialisées. La
//! structure `PublicIdentity` ne porte que des champs publics ; serde ignore
//! le reste du fichier. Un secret ne peut pas fuiter par un champ qu'on n'a
//! jamais lu.

use std::path::PathBuf;

use serde::Deserialize;
use serde_json::{Value, json};

/// Chemins des données locales, résolus par défaut comme le client et le
/// daemon les résolent eux-mêmes (`HOME/.polygone`, `config_dir/polygone`,
/// `POLYGONE_DAEMON_SOCKET` prioritaire).
#[derive(Debug, Clone)]
pub struct LocalPaths {
    pub identity_file: PathBuf,
    pub config_file: PathBuf,
    pub daemon_socket: PathBuf,
}

impl Default for LocalPaths {
    fn default() -> Self {
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."));
        let config_dir = dirs::config_dir()
            .unwrap_or_else(|| home.join(".config"))
            .join("polygone");
        Self {
            identity_file: home.join(".polygone").join("identity.json"),
            config_file: config_dir.join("daemon.toml"),
            daemon_socket: polygoned::socket::socket_path(),
        }
    }
}

/// Ce qui peut mal tourner en lisant une donnée locale.
#[derive(Debug)]
pub enum LocalError {
    /// Le fichier n'existe pas.
    Absent,
    /// Le fichier existe mais n'est pas lisible/analysable.
    Unreadable(String),
    /// La configuration existe mais est invalide — jamais de repli silencieux
    /// sur des defaults : une config cassée doit se voir.
    ConfigInvalid(String),
}

/// Identité publique, seule partie du fichier d'identité qui sorte de la
/// machine. Les champs `*_sk_hex` du fichier ne sont PAS dans cette
/// structure : serde les ignore, ils n'entrent jamais en mémoire ici.
#[derive(Debug, Deserialize)]
pub struct PublicIdentity {
    pub pseudo: String,
    pub kem_pk_hex: String,
    pub sign_pk_hex: String,
    #[serde(default)]
    pub revoked_sign_pk_hex: Vec<String>,
    #[serde(default)]
    pub rotated_at_unix: Option<u64>,
}

/// Lit l'identité publique et dérive le `node_id` (ADR-0002 : BLAKE3 du
/// contexte et de la clé publique ML-DSA-65, même dérivation que le client).
///
/// `node_id` vaut `null` si la clé publique est illisible — un champ nul est
/// honnête, un identifiant inventé ne l'est pas.
pub fn read_public_identity(path: &std::path::Path) -> Result<Value, LocalError> {
    let raw = match std::fs::read_to_string(path) {
        Ok(raw) => raw,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Err(LocalError::Absent),
        Err(e) => return Err(LocalError::Unreadable(e.to_string())),
    };
    let identity: PublicIdentity =
        serde_json::from_str(&raw).map_err(|e| LocalError::Unreadable(e.to_string()))?;
    let node_id = polygone_core::sign::node_id_from_sign_pk_hex(&identity.sign_pk_hex);
    Ok(json!({
        "pseudo": identity.pseudo,
        "node_id": node_id,
        "kem_pk_hex": identity.kem_pk_hex,
        "sign_pk_hex": identity.sign_pk_hex,
        "revoked_sign_pk_hex": identity.revoked_sign_pk_hex,
        "rotated_at_unix": identity.rotated_at_unix,
    }))
}

/// Configuration du daemon en lecture seule.
///
/// Fichier absent → defaults PRODUIT avec `source: "defaults"` (le daemon
/// ferait exactement ce choix au démarrage). Fichier présent mais invalide →
/// erreur explicite : masquer une config cassée derrière des defaults serait
/// un mensonge de plus.
pub fn read_settings(path: &std::path::Path) -> Result<Value, LocalError> {
    let raw = match std::fs::read_to_string(path) {
        Ok(raw) => raw,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok(settings_json(
                &polygoned::policy::glow_up::DaemonConfig::default(),
                "defaults",
                path,
            ));
        }
        Err(e) => return Err(LocalError::Unreadable(e.to_string())),
    };
    let config: polygoned::policy::glow_up::DaemonConfig =
        toml::from_str(&raw).map_err(|e| LocalError::ConfigInvalid(e.to_string()))?;
    Ok(settings_json(&config, "file", path))
}

/// Publie la configuration avec le MÊME vocabulaire que le fil daemon :
/// `tier` en minuscules (`balanced`, `performance`…), pas la forme PascalCase
/// de l'enum Rust. Une API qui parle deux langues pour la même valeur est
/// un piège à clients.
fn settings_json(
    config: &polygoned::policy::glow_up::DaemonConfig,
    source: &str,
    path: &std::path::Path,
) -> Value {
    let mut config_value = serde_json::to_value(config).unwrap_or_else(|_| json!({}));
    config_value["tier"] = json!(config.tier.as_str());
    json!({
        "config": config_value,
        "source": source,
        "path": path,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "polygone-local-{}-{}-{}",
            tag,
            std::process::id(),
            crate::now_secs()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn identity_absente_est_une_erreur_absent() {
        let dir = temp_dir("absent");
        let err = read_public_identity(&dir.join("identity.json")).unwrap_err();
        assert!(matches!(err, LocalError::Absent));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn identity_publique_ne_contient_jamais_de_secret() {
        let dir = temp_dir("public");
        let path = dir.join("identity.json");
        // Fichier COMPLET tel que le client l'écrit, secrets inclus.
        std::fs::write(
            &path,
            r#"{
                "pseudo": "vox-test",
                "kem_pk_hex": "aa",
                "kem_sk_hex": "SECRET-KEM",
                "sign_pk_hex": "bb",
                "sign_sk_hex": "SECRET-SIGN",
                "revoked_sign_pk_hex": [],
                "rotated_at_unix": null
            }"#,
        )
        .unwrap();
        let value = read_public_identity(&path).unwrap();
        let text = value.to_string();
        assert!(!text.contains("SECRET-KEM"), "la clé secrète KEM a fuité");
        assert!(
            !text.contains("SECRET-SIGN"),
            "la clé secrète de signature a fuité"
        );
        assert!(!text.contains("sk_hex"), "un champ secret est publié");
        assert_eq!(value["pseudo"], "vox-test");
        // Clés publiques factices (« bb ») : la dérivation échoue proprement,
        // node_id est null — jamais un identifiant inventé.
        assert!(value["node_id"].is_null());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn identity_illisible_est_une_erreur_explicite() {
        let dir = temp_dir("broken");
        let path = dir.join("identity.json");
        std::fs::write(&path, "{ pas du json").unwrap();
        let err = read_public_identity(&path).unwrap_err();
        assert!(matches!(err, LocalError::Unreadable(_)));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn settings_absentes_rendent_les_defaults_produit() {
        let dir = temp_dir("defaults");
        let value = read_settings(&dir.join("daemon.toml")).unwrap();
        assert_eq!(value["source"], "defaults");
        assert_eq!(value["config"]["tier"], "balanced");
        assert_eq!(value["config"]["memory_limit_enabled"], true);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn settings_cassees_ne_sont_jamais_masquees() {
        let dir = temp_dir("badconfig");
        let path = dir.join("daemon.toml");
        std::fs::write(&path, "tier = \"n'importe quoi\"\n").unwrap();
        let err = read_settings(&path).unwrap_err();
        assert!(matches!(err, LocalError::ConfigInvalid(_)));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn settings_lues_depuis_le_fichier() {
        let dir = temp_dir("file");
        let path = dir.join("daemon.toml");
        std::fs::write(
            &path,
            "tier = \"performance\"\nmemory_limit_enabled = false\n",
        )
        .unwrap();
        let value = read_settings(&path).unwrap();
        assert_eq!(value["source"], "file");
        assert_eq!(value["config"]["tier"], "performance");
        assert_eq!(value["config"]["memory_limit_enabled"], false);
        // Les champs absents du fichier restent aux defaults produit.
        assert_eq!(value["config"]["bandwidth_shaping"], true);
        std::fs::remove_dir_all(&dir).ok();
    }
}
