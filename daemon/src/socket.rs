//! Socket du daemon — protocole de décision v1.
//!
//! Deux moitiés volontairement séparées :
//!
//! 1. **Le cœur pur du protocole** (`encode` / `decode` / `dispatch`) : aucune E/S,
//!    aucun socket, aucun thread. C'est l'incrément 1 de `docs/PLAN_SOCKET.md`.
//! 2. **Le serveur** (`bind_listener` / `serve` / `handle_client`) : il *sert* ce
//!    chemin, il ne s'y connecte plus.
//!
//! La direction sortante historique (`send` / `DaemonMsg` / `notify_*`) a été
//! supprimée à l'incrément 2b-3b. Elle écrivait vers `~/.polygone/daemon.sock`,
//! donc vers le daemon lui-même dès que ce serveur écoute. Ces fonctions n'avaient
//! au demeurant jamais rien envoyé : `send` sortait en silence tant que le fichier
//! n'existait pas, et rien ne l'a jamais créé.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json;
use std::collections::hash_map::DefaultHasher;
use std::collections::{BTreeMap, HashMap};
use std::ffi::OsString;
use std::fs::{self, File};
use std::hash::{Hash, Hasher};
use std::io::{self, Read, Write};
use std::os::unix::fs::{FileTypeExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::panic::{self, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex, MutexGuard, OnceLock, RwLock};
use std::thread;
use std::time::{Duration, Instant};

/// Nom de la variable d'environnement qui impose le chemin du socket.
///
/// Le client Petals lit **exactement le même nom** (`crates/petals/src/daemon.rs:20`) :
/// deux côtés qui ne s'accordent que par un défaut compilé divergent au premier
/// `HOME` différent (voir `docs/PLAN_CONSOMMATEUR.md` §3, écart 1).
pub const ENV_SOCKET_PATH: &str = "POLYGONE_DAEMON_SOCKET";

static SOCKET_PATH: OnceLock<PathBuf> = OnceLock::new();

/// Le chemin réel du socket du daemon.
///
/// `POLYGONE_DAEMON_SOCKET` est **prioritaire** ; à défaut, `~/.polygone/daemon.sock`.
/// C'est le chemin que le daemon **sert** (`bind_listener`) ; il ne s'y connecte
/// plus depuis l'incrément 2b-3b. Exposé pour que `status`/`doctor` affichent et
/// vérifient LE MÊME chemin que celui écouté — l'indicateur ne doit pas regarder
/// un chemin mort.
pub fn socket_path() -> PathBuf {
    SOCKET_PATH
        .get_or_init(|| socket_path_from(std::env::var_os(ENV_SOCKET_PATH), dirs::home_dir()))
        .clone()
}

/// Décision pure du chemin : la variable d'environnement d'abord, `$HOME` ensuite.
///
/// Séparée de `socket_path()` parce que le `OnceLock` du chemin réel ne
/// s'initialise qu'une fois par processus — une fonction pure, elle, se teste de
/// façon répétable. Une variable **vide** est traitée comme absente : un chemin
/// vide donnerait un socket introuvable au lieu d'un message clair.
fn socket_path_from(from_env: Option<OsString>, home: Option<PathBuf>) -> PathBuf {
    if let Some(path) = from_env.filter(|p| !p.is_empty()) {
        return PathBuf::from(path);
    }

    home.unwrap_or_else(|| PathBuf::from("."))
        .join(".polygone")
        .join("daemon.sock")
}

/// Ensure the .polygone directory exists.
pub fn ensure_dir() -> Result<()> {
    let path = socket_path();
    let dir = path.parent().unwrap();
    fs::create_dir_all(dir).context(format!("failed to create {}", dir.display()))?;
    Ok(())
}

// ============================================================================
// Protocole v1 — cœur pur (incrément 1)
// ============================================================================
//
// Rien ici ne touche le disque, /proc, /sys ou un socket. `dispatch` lit
// uniquement l'état déjà en mémoire qui lui est passé — c'est ce qui rend
// une rafale de requêtes inoffensive pour la boucle d'allocation.

/// Version courante du protocole, portée par le champ `v` de chaque message.
pub const PROTOCOL_VERSION: u8 = 2;

/// Plus ancienne version encore lue. Une requête `v: 1` reste valide : une
/// montée de version ne doit pas se transformer en rupture pour les clients
/// déjà installés.
pub const MIN_PROTOCOL_VERSION: u8 = 1;

/// Capacités annoncées dans `hello`, en plus de la version.
pub const CAPABILITIES: [&str; 4] = ["refusals", "renew", "storage_capacity", "cpu_tier_cap"];

/// Valeur de repli pour un `hello` v1 qui ne porte pas encore ce champ.
fn default_min_protocol_version() -> u8 {
    MIN_PROTOCOL_VERSION
}

/// Longueur maximale d'une ligne de protocole, en octets (64 Kio).
pub const MAX_LINE_BYTES: usize = 64 * 1024;

/// Types de requêtes acceptés. Sert à distinguer « type inconnu » de
/// « message mal formé » avant la désérialisation.
///
/// `subscribe` / `unsubscribe` ont été retirés à l'incrément 2b-6 : ils étaient
/// annoncés et acceptés alors que `tick_update` n'était **construit nulle part**
/// (`grep -rn TickUpdate daemon/src` ne rendait que sa déclaration). Un message
/// annoncé et jamais émis est un mensonge de plus ; ils reviendront dans
/// l'incrément qui livrera un consommateur ET son test de réception.
///
/// Cette liste est vérifiée par `test_known_request_types_covers_every_variant` :
/// un variant ajouté au type mais oublié ici serait refusé en `unknown_type`,
/// des deux côtés du fil.
pub const KNOWN_REQUEST_TYPES: [&str; 6] = [
    "status_request",
    "ping",
    "alloc_request",
    "release",
    // `renew` existe parce que les baux expirent : sans renouvellement, un
    // consommateur légitime perdrait sa réservation au bout de `lease_secs()`.
    "renew",
    // Compteurs de refus (OPS.2) : requête séparée pour ne pas faire grossir
    // `Reply::Status`, déjà au plafond de taille porté par `NodeError`.
    "refusals_request",
];

/// Types de réponses que le client sait lire (noms sérialisés des variants de
/// `Reply`) — le pendant de `KNOWN_REQUEST_TYPES` pour la direction inverse.
///
/// Vérifiée par `test_known_reply_types_covers_every_variant` : sans ce test, la
/// liste dériverait du type au premier variant ajouté, en silence.
pub const KNOWN_REPLY_TYPES: [&str; 9] = [
    "hello",
    "status_response",
    "alloc_granted",
    "alloc_denied",
    "released",
    "renewed",
    "pong",
    "error",
    "refusals_response",
];

/// Codes d'erreur publiés sur le fil (champ `code` d'un `Reply::Error`).
pub const ERR_MALFORMED_JSON: &str = "malformed_json";
pub const ERR_UNSUPPORTED_VERSION: &str = "unsupported_version";
pub const ERR_UNKNOWN_TYPE: &str = "unknown_type";
pub const ERR_LINE_TOO_LONG: &str = "line_too_long";

/// Motifs de refus d'une demande d'allocation (champ `reason`).
pub const REASON_INVALID_REQUEST: &str = "invalid_request";
pub const REASON_INSUFFICIENT_HEADROOM: &str = "insufficient_headroom";
pub const REASON_INSUFFICIENT_CPU: &str = "insufficient_cpu";
pub const REASON_INSUFFICIENT_VRAM: &str = "insufficient_vram";
pub const REASON_INSUFFICIENT_STORAGE: &str = "insufficient_storage";
pub const REASON_POLICY_SHRINKING: &str = "policy_shrinking";
/// La demande dépasse ce que la **machine possède**, pas seulement ce qui reste
/// réservable. Distinct de `insufficient_headroom` : le premier est un refus
/// définitif (aucune libération ne le rendra possible), le second est un refus
/// temporaire qui se débloque quand un pair rend son bail.
///
/// Un client qui reçoit `insufficient_headroom` doit réessayer ; un client qui
/// reçoit `exceeds_measured` doit réduire sa demande, sans quoi il réessaiera
/// indéfiniment contre un mur.
pub const REASON_EXCEEDS_MEASURED: &str = "exceeds_measured";

/// `renew` sur un identifiant que le ledger ne connaît pas, ou dont le bail est
/// déjà échu. Distinct de `insufficient_headroom` : aucune libération ne rendra
/// ce bail, il faut en redemander un.
pub const REASON_UNKNOWN_ALLOCATION: &str = "unknown_allocation";

/// Compteurs de refus d'allocation, par motif (OPS.2).
///
/// Ils vivent dans le **serveur**, jamais dans `DaemonState` : celui-ci a un
/// écrivain unique (`publish_shared`) et y ajouter un compteur mutable romprait
/// cet invariant. Ils sont locaux au processus, non exportés et perdus à
/// l'arrêt — c'est cohérent avec l'absence de télémétrie : on répond à « pourquoi
/// mes nœuds sont refusés ? » sans rien envoyer nulle part.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct RefusalCounters {
    counts: BTreeMap<String, u64>,
}

impl RefusalCounters {
    /// Incrémente le compteur du motif. Un motif inconnu n'est pas avalé : il
    /// apparaît dans l'instantané, ce qui rend visible un refus ajouté sans
    /// passer par les constantes.
    pub fn record(&mut self, reason: &str) {
        *self.counts.entry(reason.to_string()).or_insert(0) += 1;
    }

    /// Copie triée (BTreeMap) pour publication dans `Status`.
    pub fn snapshot(&self) -> BTreeMap<String, u64> {
        self.counts.clone()
    }

    /// Nombre total de refus comptés.
    pub fn total(&self) -> u64 {
        self.counts.values().sum()
    }

    /// Motif le plus fréquent, s'il y en a un — pour un résumé d'une ligne.
    pub fn top(&self) -> Option<(&str, u64)> {
        self.counts
            .iter()
            .max_by_key(|(_, count)| **count)
            .map(|(reason, count)| (reason.as_str(), *count))
    }
}

/// Pourquoi une ligne n'a pas pu être décodée.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecodeError {
    /// JSON invalide, racine non-objet, ou champ obligatoire manquant.
    MalformedJson,
    /// Champ `v` absent, ou différent de `PROTOCOL_VERSION`.
    UnsupportedVersion,
    /// Champ `type` présent mais inconnu du protocole.
    UnknownType,
    /// Ligne dépassant `MAX_LINE_BYTES`.
    LineTooLong,
}

impl DecodeError {
    /// Code à publier sur le fil (voir `ERR_*`).
    pub fn code(&self) -> &'static str {
        match self {
            DecodeError::MalformedJson => ERR_MALFORMED_JSON,
            DecodeError::UnsupportedVersion => ERR_UNSUPPORTED_VERSION,
            DecodeError::UnknownType => ERR_UNKNOWN_TYPE,
            DecodeError::LineTooLong => ERR_LINE_TOO_LONG,
        }
    }
}

impl std::fmt::Display for DecodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let msg = match self {
            DecodeError::MalformedJson => "message JSON invalide ou incomplet",
            DecodeError::UnsupportedVersion => {
                "version de protocole non supportée (supportées : 1 à 2)"
            }
            DecodeError::UnknownType => "type de message inconnu",
            DecodeError::LineTooLong => "ligne trop longue (maximum 64 Kio)",
        };
        write!(f, "{}", msg)
    }
}

impl std::error::Error for DecodeError {}

/// Message client → daemon.
///
/// Les DEUX sens depuis 2b-7a : `Deserialize` côté serveur, `Serialize` côté
/// client (`encode_request`). Aucun `skip_serializing_if` : `Option` est déjà
/// facultatif par défaut, et un `null` explicite reste accepté des deux côtés.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Request {
    #[serde(rename = "status_request")]
    Status { id: Option<String> },
    #[serde(rename = "ping")]
    Ping { id: Option<String> },
    #[serde(rename = "alloc_request")]
    Alloc {
        id: Option<String>,
        client: Option<String>,
        cpu_cores: u32,
        ram_gb: f32,
        gpu_vram_gb: Option<f32>,
        /// Stockage demandé en Gio. **Optionnel** : absent = 0 (aucun stockage),
        /// jamais illimité. Un ancien client sans ce champ continue à fonctionner
        /// à l'identique — le `#[serde(default)]` garantit la valeur par défaut.
        #[serde(default)]
        storage_gb: Option<f32>,
    },
    #[serde(rename = "release")]
    Release {
        id: Option<String>,
        allocation_id: String,
    },
    /// Prolonge un bail avant son échéance.
    ///
    /// Un bail qui n'est jamais renouvelé expire : c'est voulu (un processus
    /// tué ne doit pas bloquer la marge). Ce variant est la contrepartie pour
    /// les consommateurs vivants — sans lui, le correctif d'expiration
    /// évincerait le travail légitime au bout d'un bail.
    #[serde(rename = "renew")]
    Renew {
        id: Option<String>,
        allocation_id: String,
    },
    /// Compteurs de refus d'allocation depuis le démarrage (OPS.2).
    ///
    /// Requête séparée, et non un champ de `status_request` : `Reply::Status`
    /// est déjà au plafond de taille de `Reply` (portée par `NodeError`), et
    /// quelques octets de plus déclenchent `clippy::result_large_err` chez le
    /// client. Un daemon antérieur répond `unknown_type` : le client le dit
    /// « non mesuré », il n'invente pas zéro.
    #[serde(rename = "refusals_request")]
    Refusals { id: Option<String> },
}

/// Message daemon → client.
///
/// Les DEUX sens depuis 2b-7a : `Serialize` côté serveur, `Deserialize` côté
/// client (`decode_reply`). Aucun attribut `default` nécessaire : un `Option`
/// absent à la lecture vaut `None`, ce qui rend l'omission symétrique du
/// `skip_serializing_if`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Reply {
    #[serde(rename = "hello")]
    Hello {
        daemon_version: String,
        platform: String,
        tier: String,
        /// Version de base, lisible par un client v1.
        protocol_version: u8,
        /// Version la plus récente que ce daemon sait parler. Absent d'un
        /// daemon v1 : le client retombe alors sur `protocol_version`.
        #[serde(default)]
        max_protocol_version: Option<u8>,
        #[serde(default = "default_min_protocol_version")]
        min_protocol_version: u8,
        #[serde(default)]
        capabilities: Vec<String>,
    },
    #[serde(rename = "status_response")]
    Status {
        #[serde(skip_serializing_if = "Option::is_none")]
        id: Option<String>,
        tier: String,
        seq: u64,
        max_cpu_percent: u8,
        max_ram_percent: u8,
        max_bandwidth_percent: u8,
        max_gpu_percent: u8,
        ram_bytes: u64,
        ram_mb: u64,
        bandwidth_mbps: u32,
        cpu_cores: u32,
        gpu_free_vram_mb: u32,
        shrinking: bool,
        uptime_secs: u64,
        /// Capacité RAM calculée par le ledger au moment de cette réponse.
        /// Optionnelle pour accepter les réponses des daemons v1 antérieurs.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        ram_capacity: Option<Box<Capacity>>,
        /// Capacité de stockage annoncée par le daemon ; absente si non mesurée.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        storage_capacity: Option<Box<Capacity>>,
    },
    /// Allocation accordée — une **licence, pas une contrainte**.
    ///
    /// Le daemon ne peut rien imposer : depuis l'incrément 2b-5b il n'écrit plus
    /// aucun limiteur système (le plafond mémoire du cgroup a été retiré du
    /// binaire, gardé par `test_no_cgroup_write_anywhere`), il n'a pas de cgroup
    /// à lui, et le CPU n'est alloué par personne. Ce qui suit est donc un plafond que
    /// le **consommateur** s'engage à respecter : c'est l'option A de
    /// `docs/PLAN_CONSOMMATEUR.md` §4. Le daemon, lui, enregistre la réservation
    /// pour ne plus sur-accorder — il ne peut pas la faire tenir.
    ///
    /// `expires_at` porte l'échéance réelle du bail : le ledger la fait
    /// respecter (`sweep_expired`), et un consommateur vivant peut la
    /// prolonger avec `Request::Renew`.
    #[serde(rename = "alloc_granted")]
    AllocGranted {
        #[serde(skip_serializing_if = "Option::is_none")]
        id: Option<String>,
        allocation_id: String,
        cpu_cores: u32,
        ram_gb: f32,
        #[serde(skip_serializing_if = "Option::is_none")]
        gpu_vram_gb: Option<f32>,
        /// Stockage accordé — absent (et omis du JSON) si aucun stockage n'a été
        /// demandé. Un vieux client qui ne connaît pas ce champ l'ignore.
        #[serde(skip_serializing_if = "Option::is_none")]
        storage_gb: Option<f32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        expires_at: Option<i64>,
        /// `true` seulement si l'OS contraint réellement ce qui vient d'être
        /// accordé. `false` = licence que le client doit s'auto-imposer.
        ///
        /// `#[serde(default)]` : un daemon antérieur qui n'envoie pas ce champ
        /// produit `false`, ce qui est le repli le plus sûr — on ne prétend
        /// jamais une garantie qu'on n'a pas reçue.
        #[serde(default)]
        enforced: bool,
    },
    #[serde(rename = "alloc_denied")]
    AllocDenied {
        #[serde(skip_serializing_if = "Option::is_none")]
        id: Option<String>,
        reason: String,
    },
    #[serde(rename = "renewed")]
    Renewed {
        #[serde(skip_serializing_if = "Option::is_none")]
        id: Option<String>,
        allocation_id: String,
        /// Nouvelle échéance, en secondes Unix.
        expires_at: i64,
    },
    #[serde(rename = "released")]
    Released {
        #[serde(skip_serializing_if = "Option::is_none")]
        id: Option<String>,
        allocation_id: String,
    },
    #[serde(rename = "pong")]
    Pong {
        #[serde(skip_serializing_if = "Option::is_none")]
        id: Option<String>,
    },
    #[serde(rename = "error")]
    Error {
        #[serde(skip_serializing_if = "Option::is_none")]
        id: Option<String>,
        code: String,
        message: String,
    },
    /// Répartition des refus d'allocation par motif (OPS.2).
    #[serde(rename = "refusals_response")]
    Refusals {
        #[serde(skip_serializing_if = "Option::is_none")]
        id: Option<String>,
        /// Motifs triés par nom ; vide = aucun refus depuis le démarrage.
        counts: BTreeMap<String, u64>,
    },
}

/// État déjà en mémoire, lu par `dispatch` — jamais recalculé.
///
/// Il est volontairement identique pour tous les clients : le daemon n'a
/// qu'une allocation globale, pas une allocation par client (le suivi par
/// client appartient à l'incrément 2).
#[derive(Debug, Clone)]
pub struct DaemonState {
    /// Compteur monotone de tick : avance dès qu'une nouvelle décision a été
    /// publiée. Donne aux clients un moyen bon marché de savoir si l'état a
    /// bougé — `uptime_secs` n'a qu'une granularité d'une seconde et `ram_bytes`
    /// reste stable sur une machine stable, donc aucun des deux ne suffit.
    pub seq: u64,
    pub tier: String,
    pub max_cpu_percent: u8,
    pub max_ram_percent: u8,
    pub max_bandwidth_percent: u8,
    pub max_gpu_percent: u8,
    pub total_ram_bytes: u64,
    /// RAM actuellement allouée au réseau.
    pub ram_bytes: u64,
    pub bandwidth_mbps: u32,
    /// Cœurs **physiques** mesurés sur la machine.
    ///
    /// Ce n'est pas le pool réservable : `allowed_cpu_cores()` applique le
    /// plafond `max_cpu_percent` du palier. Le champ garde son nom et sa
    /// signification de mesure pour ne pas casser le protocole v1.
    pub cpu_cores: u32,
    pub gpu_free_vram_mb: u32,
    pub shrinking: bool,
    pub uptime_secs: u64,
    pub daemon_version: String,
    pub platform: String,
    /// Espace disque total en octets disponible pour le réseau. Optionnel dans
    /// le sens v1 : un état sans ce champ (ou 0) signifie qu'aucune capacité
    /// de stockage n'est annoncée — toute demande de stockage sera refusée.
    pub total_storage_bytes: u64,
    /// RAM réellement **contrainte** par l'OS sur les consommateurs.
    ///
    /// Vaut `0` aujourd'hui, et c'est la vérité : `GlowUpEngine::apply` ne pose
    /// qu'une priorité `nice` ; ni plafond mémoire ni quota CPU de cgroup ne
    /// sont écrits par le daemon (garde structurel : `runtime::tests`). Ce champ existe pour que le jour où une isolation est écrite
    /// il devienne non nul **par mesure**, et jamais par déclaration.
    pub enforced_ram_bytes: u64,
    /// Idem pour le stockage : `0` signifie qu'aucun quota disque n'est imposé.
    pub enforced_storage_bytes: u64,
}

impl DaemonState {
    /// État **de test uniquement** : les valeurs sont inventées (16 Gio de RAM,
    /// 3 cœurs, 1 Gio déjà alloué) et n'ont rien à voir avec la machine.
    ///
    /// ⚠️ Ne JAMAIS publier cette valeur sur le socket. Un client qui se
    /// connecte lirait une machine qui n'existe pas. Le daemon publie un
    /// instantané réel construit par `runtime_state()` (voir `crate::runtime`) ;
    /// les tests de serveur, eux, l'utilisent comme point de départ inerte.
    pub fn new(tier: impl Into<String>) -> Self {
        DaemonState {
            seq: 0,
            tier: tier.into(),
            max_cpu_percent: 50,
            max_ram_percent: 50,
            max_bandwidth_percent: 60,
            max_gpu_percent: 40,
            total_ram_bytes: 16 * 1024 * 1024 * 1024,
            ram_bytes: 1024 * 1024 * 1024,
            bandwidth_mbps: 10,
            cpu_cores: 3,
            gpu_free_vram_mb: 0,
            shrinking: false,
            uptime_secs: 0,
            daemon_version: env!("CARGO_PKG_VERSION").to_string(),
            platform: "unknown".to_string(),
            total_storage_bytes: 0,
            // Rien n'est contraint dans un état de test inerte : si un test
            // veut simuler une isolation, il le fait explicitement.
            enforced_ram_bytes: 0,
            enforced_storage_bytes: 0,
        }
    }

    /// Plafond de RAM que le palier s'autorise, d'après le total de la machine.
    pub fn allowed_ram_bytes(&self) -> u64 {
        (self.total_ram_bytes as u128 * self.max_ram_percent as u128 / 100) as u64
    }

    /// RAM encore allouable sans franchir le plafond du palier.
    pub fn ram_headroom_bytes(&self) -> u64 {
        self.allowed_ram_bytes().saturating_sub(self.ram_bytes)
    }

    /// Cœurs que le palier s'autorise à promettre, d'après le total mesuré.
    ///
    /// Arrondi vers le bas : promettre un cœur entier supplémentaire au-delà
    /// du pourcentage serait exactement le mensonge que le plafond évite.
    pub fn allowed_cpu_cores(&self) -> u32 {
        ((self.cpu_cores as u64 * self.max_cpu_percent as u64) / 100) as u32
    }

    /// Stockage allouable : le total annoncé. Pas de pourcentage ici — le
    /// stockage durable est un engagement ferme, pas une fraction provisoire.
    /// Un état sans stockage (total = 0) refuse toute demande.
    pub fn storage_headroom_bytes(&self) -> u64 {
        self.total_storage_bytes
    }
}

/// Sérialise une valeur de protocole en une ligne JSON terminée par `\n`.
///
/// Le champ `v` est ajouté ICI, en un seul exemplaire pour les deux directions :
/// aucune requête ni réponse ne peut partir sans sa version, même si un futur
/// variant l'oublie.
fn encode_line<T: Serialize>(value: &T) -> String {
    encode_line_with_version(value, PROTOCOL_VERSION)
}

/// Même encodage, avec une version imposée. Le client s'en sert pour parler
/// `v: 1` à un daemon ancien après la négociation du `hello`.
fn encode_line_with_version<T: Serialize>(value: &T, version: u8) -> String {
    let mut obj = match serde_json::to_value(value) {
        Ok(serde_json::Value::Object(map)) => map,
        _ => serde_json::Map::new(),
    };
    obj.insert("v".to_string(), serde_json::Value::from(version));

    let mut line =
        serde_json::to_string(&serde_json::Value::Object(obj)).unwrap_or_else(|_| "{}".to_string());
    line.push('\n');
    line
}

/// Sérialise une réponse (serveur → client). Voir `encode_line` pour `v`.
pub fn encode(reply: &Reply) -> String {
    encode_line(reply)
}

/// Sérialise une réponse avec la version du pair. Le serveur s'en sert pour
/// répondre en v1 à un client v1 : une montée de version ne coupe pas les
/// clients déjà installés.
pub fn encode_with_version(reply: &Reply, version: u8) -> String {
    encode_line_with_version(reply, version)
}

/// Sérialise une requête (client → serveur) — la moitié manquante du protocole
/// jusqu'à 2b-7a.
///
/// Sans elle, un client devait écrire son JSON à la main : c'est exactement ce
/// qui a laissé `crates/petals/src/daemon.rs` dériver du serveur (chemin, `v`,
/// noms de messages) sans que rien ne le signale.
pub fn encode_request(request: &Request) -> String {
    encode_line(request)
}

/// Sérialise une requête avec la version négociée. Ne jamais appeler avec une
/// version hors de `MIN_PROTOCOL_VERSION..=PROTOCOL_VERSION` : le client tire
/// cette valeur du `hello`, jamais d'une constante locale.
pub fn encode_request_with_version(request: &Request, version: u8) -> String {
    encode_line_with_version(request, version)
}

/// Validation commune aux deux sens du fil : taille, JSON, version, liste blanche.
///
/// Factorisée volontairement : deux validations qui divergent seraient un piège
/// à retardement — une direction accepterait ce que l'autre refuse, et le
/// désaccord n'apparaîtrait qu'en production.
fn validated_value(line: &str, known_types: &[&str]) -> Result<serde_json::Value, DecodeError> {
    if line.len() > MAX_LINE_BYTES {
        return Err(DecodeError::LineTooLong);
    }

    let value: serde_json::Value =
        serde_json::from_str(line.trim()).map_err(|_| DecodeError::MalformedJson)?;
    let obj = value.as_object().ok_or(DecodeError::MalformedJson)?;

    // Version : absente ou hors de la fenêtre supportée → refus explicite. On
    // ne devine pas et on ne « réinterprète » pas une version future.
    match obj.get("v") {
        Some(serde_json::Value::Number(n))
            if n.as_u64().is_some_and(|v| {
                (MIN_PROTOCOL_VERSION as u64..=PROTOCOL_VERSION as u64).contains(&v)
            }) => {}
        _ => return Err(DecodeError::UnsupportedVersion),
    }

    let type_str = obj
        .get("type")
        .and_then(|t| t.as_str())
        .ok_or(DecodeError::MalformedJson)?;

    if !known_types.contains(&type_str) {
        return Err(DecodeError::UnknownType);
    }

    Ok(value)
}

/// Décode une ligne de requête (client → serveur). Ne panique jamais, aucune E/S.
///
/// Ordre des contrôles, du moins coûteux au plus coûteux : taille, JSON, version,
/// type, puis désérialisation des champs (voir `validated_value`).
pub fn decode(line: &str) -> Result<Request, DecodeError> {
    decode_with_version(line).map(|(request, _)| request)
}

/// Comme `decode`, mais rend aussi la version lue.
///
/// Le serveur répond avec la version de la requête : un client v1 continue de
/// décoder toutes les réponses, un client v2 reçoit du v2.
pub fn decode_with_version(line: &str) -> Result<(Request, u8), DecodeError> {
    // Le type est déjà known : tout échec restant est un champ absent ou invalide.
    let value = validated_value(line, &KNOWN_REQUEST_TYPES)?;
    let version = value
        .get("v")
        .and_then(|v| v.as_u64())
        .and_then(|v| u8::try_from(v).ok())
        .ok_or(DecodeError::UnsupportedVersion)?;
    let request = serde_json::from_value(value).map_err(|_| DecodeError::MalformedJson)?;
    Ok((request, version))
}

/// Décode une ligne de réponse (serveur → client) — symétrique de `decode`.
///
/// Mêmes contrôles, dans le même ordre, avec les mêmes codes d'erreur : une
/// réponse ne se lit pas plus librement qu'une requête.
pub fn decode_reply(line: &str) -> Result<Reply, DecodeError> {
    let value = validated_value(line, &KNOWN_REPLY_TYPES)?;
    serde_json::from_value(value).map_err(|_| DecodeError::MalformedJson)
}

/// Ce que le daemon peut dire d'une ressource, en trois grandeurs distinctes.
///
/// Le défaut historique du projet tient dans la confusion de ces trois mots :
/// un client lisait un chiffre et croyait à une garantie. Les séparer rend le
/// mensonge structurellement impossible à écrire.
///
/// - `measured` : ce que la machine possède réellement, mesuré au tick.
/// - `reservable` : ce qui peut **encore** être promis — mesuré, moins le
///   plafond du palier et les marges, moins les réservations en cours.
/// - `enforced` : ce que le système d'exploitation **impose** effectivement au
///   consommateur. Zéro signifie « rien n'est contraint » : une allocation
///   accordée est alors une **licence**, que le client peut dépasser.
///
/// Invariant, testé : `reservable <= measured`. Un `enforced` qui dépasserait
/// `reservable` serait une contrainte sur une ressource qui n'a pas été promise.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Capacity {
    pub measured: u64,
    pub reservable: u64,
    /// Ce que l'OS applique réellement. `0` = licence, pas garantie.
    pub enforced: u64,
}

impl Capacity {
    /// Vrai seulement si l'intégralité de la capacité annoncée est contrainte
    /// par l'OS. Un `reservable > 0` avec `enforced == 0` doit être présenté à
    /// l'utilisateur comme un engagement, jamais comme un quota appliqué.
    pub fn is_enforced(&self) -> bool {
        self.enforced > 0 && self.enforced >= self.reservable
    }
}

/// Réservations d'allocation en cours, une par `allocation_id`.
///
/// **Ce ledger vit dans le serveur, jamais dans `DaemonState`.** `publish_shared`
/// reste le seul écrivain de l'état partagé (garde structurel
/// `test_tick_is_the_only_writer`, `runtime.rs:361`) ; des réservations ajoutées
/// à `DaemonState` auraient créé un second écrivain pour une donnée que le tick
/// ne calcule pas.
///
/// Sans ledger, `dispatch_alloc` décidait contre `ram_headroom_bytes()` — qui ne
/// reflète que le dernier tick publié : deux clients demandant la même RAM
/// recevaient chacun `alloc_granted` (`docs/PLAN_CONSOMMATEUR.md` §3, écart 10).
/// Durée par défaut d'un bail, en secondes.
///
/// Quinze minutes : assez long pour qu'un consommateur réel ne perde pas son
/// bail au milieu d'un calcul (le renouvellement explicite viendra avec
/// `Request::Renew`), assez court pour qu'un processus tué ne bloque pas la
/// marge indéfiniment. Sans cette durée, `expires_at` restait `None` et une
/// réservation ne mourait qu'avec la connexion qui l'avait obtenue — jamais
/// avec le processus.
pub const DEFAULT_LEASE_SECS: u64 = 900;

/// Surcharge de la durée de bail, en secondes. Utile aux tests et aux
/// déploiements qui veulent des baux plus courts.
pub const ENV_LEASE_SECS: &str = "POLYGONE_LEASE_SECS";

/// Durée de bail effective. Une valeur absente, vide, nulle ou illisible
/// retombe sur `DEFAULT_LEASE_SECS` : un bail infini serait un retour au défaut
/// qu'on corrige ici.
pub fn lease_secs() -> u64 {
    std::env::var(ENV_LEASE_SECS)
        .ok()
        .and_then(|v| v.trim().parse::<u64>().ok())
        .filter(|v| *v > 0)
        .unwrap_or(DEFAULT_LEASE_SECS)
}

/// Horloge murale en secondes Unix. Une horloge antérieure à l'époque rend 0 :
/// le bail sera long, jamais immédiatement échu.
fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Une réservation du ledger.
///
/// **Forme `Legacy`** : les ledgers écrits avant l'expiration stockaient un
/// triplet `[cores, ram_bytes, storage_bytes]`. Ils doivent rester lisibles —
/// refuser de démarrer sur un fichier d'ancien format perdrait toutes les
/// réservations en cours, ce qui est pire que le défaut qu'on corrige.
///
/// **Forme `Current`** : la même chose plus `expires_at`. Le champ est un objet
/// JSON nommé, donc un ancien daemon qui relit un ledger récent échoue au lieu
/// de mal interpréter un tableau — l'échec est préférable au silence.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Grant {
    Legacy((u32, u64, u64)),
    Current {
        cores: u32,
        ram_bytes: u64,
        storage_bytes: u64,
        expires_at: Option<i64>,
    },
}

impl Grant {
    fn current(cores: u32, ram_bytes: u64, storage_bytes: u64, expires_at: Option<i64>) -> Grant {
        Grant::Current {
            cores,
            ram_bytes,
            storage_bytes,
            expires_at,
        }
    }

    pub fn cores(&self) -> u32 {
        match self {
            Grant::Legacy((c, _, _)) => *c,
            Grant::Current { cores, .. } => *cores,
        }
    }

    pub fn ram_bytes(&self) -> u64 {
        match self {
            Grant::Legacy((_, r, _)) => *r,
            Grant::Current { ram_bytes, .. } => *ram_bytes,
        }
    }

    pub fn storage_bytes(&self) -> u64 {
        match self {
            Grant::Legacy((_, _, s)) => *s,
            Grant::Current { storage_bytes, .. } => *storage_bytes,
        }
    }

    /// `None` pour un bail d'ancien format : il n'expire pas tant que
    /// `Ledger::adopt_legacy_grants` ne lui a pas donné une échéance.
    pub fn expires_at(&self) -> Option<i64> {
        match self {
            Grant::Legacy(_) => None,
            Grant::Current { expires_at, .. } => *expires_at,
        }
    }

    fn is_expired(&self, now: i64) -> bool {
        matches!(self.expires_at(), Some(at) if at <= now)
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Ledger {
    /// `allocation_id` → réservation (cœurs, RAM, stockage, échéance).
    grants: HashMap<String, Grant>,
    /// Octets réservés, tenus à jour à chaque mutation : la décision reste O(1)
    /// au lieu de sommer le map à chaque requête.
    reserved_ram_bytes: u64,
    /// Idem pour les cœurs : un CPU réservé mais non compté serait le même
    /// mensonge que la RAM non comptée, en plus discret.
    reserved_cpu_cores: u32,
    /// Octets de stockage réservés — agrégat O(1) comme RAM et CPU.
    ///
    /// **Persistance** : ce compteur est sérialisé avec le reste du ledger, donc
    /// il **survit** à un redémarrage. La version précédente de ce commentaire
    /// affirmait le contraire (« vit en mémoire, un redémarrage l'oublie ») et
    /// c'était faux : `test_ledger_persistence_preserves_counters` relit 5 Gio
    /// identiques après un cycle écriture/lecture. Un commentaire qui contredit
    /// son propre test est un mensonge de plus, pas une note.
    reserved_storage_bytes: u64,
    /// Départage : deux demandes identiques produisent le même `allocation_id`
    /// déterministe ; sans suffixe, la seconde écraserait la première dans le map
    /// et la réservation du premier client disparaîtrait en silence.
    disambiguator: u64,
    /// Chemin propre à cette instance. Il n'est jamais sérialisé : le fichier
    /// décrit les réservations, pas la configuration locale du processus.
    #[serde(skip)]
    path: Option<PathBuf>,
}

impl Ledger {
    /// Construit un ledger dont la persistance est isolée sur `path`.
    #[cfg(test)]
    fn at_path(path: impl Into<PathBuf>) -> Ledger {
        Ledger {
            path: Some(path.into()),
            ..Ledger::default()
        }
    }

    /// Sérialise le ledger sur son chemin explicite, ou sur le chemin par défaut
    /// du daemon pour une instance de production.
    ///
    /// **Écriture atomique** : le JSON complet est écrit dans un fichier
    /// temporaire du même répertoire, synchronisé, puis renommé sur la cible.
    /// Écrire directement sur `path` laisserait, après une coupure au mauvais
    /// moment, un JSON tronqué que `load_from` lirait comme un ledger vide :
    /// toutes les réservations en cours disparaîtraient en silence. Le fichier
    /// est créé en `0600` — les réservations ne regardent que son propriétaire.
    pub fn persist(&self) -> io::Result<()> {
        let path = self.path.clone().unwrap_or_else(ledger_path);
        let parent = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));
        fs::create_dir_all(&parent)?;

        let name = path.file_name().ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidInput, "ledger sans nom de fichier")
        })?;
        let mut tmp_name = OsString::from(".");
        tmp_name.push(name);
        tmp_name.push(".tmp");
        let tmp = parent.join(tmp_name);

        let bytes = serde_json::to_vec_pretty(self)?;
        {
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .open(&tmp)?;
            file.set_permissions(fs::Permissions::from_mode(0o600))?;
            if let Err(e) = file.write_all(&bytes).and_then(|_| file.sync_all()) {
                let _ = fs::remove_file(&tmp);
                return Err(e);
            }
        }
        if let Err(e) = fs::rename(&tmp, &path) {
            let _ = fs::remove_file(&tmp);
            return Err(e);
        }
        // Rend le rename durable : sans synchronisation du répertoire, une
        // coupure peut laisser l'ancien contenu en place alors que l'appel a
        // réussi. L'échec n'est pas fatal (certains systèmes refusent fsync sur
        // un répertoire ouvert en lecture), mais il ne doit pas être tu.
        if let Ok(dir) = File::open(&parent) {
            if let Err(e) = dir.sync_all() {
                log::debug!("ledger : fsync du répertoire impossible : {}", e);
            }
        }
        Ok(())
    }

    /// Charge le ledger depuis le chemin par défaut du daemon.
    pub fn load() -> Ledger {
        Self::load_from(ledger_path())
    }

    /// Charge un ledger depuis un chemin explicite. Le chemin reste attaché à
    /// l'instance afin que les mutations ultérieures écrivent au même endroit.
    ///
    /// **Aucune perte silencieuse** : un fichier présent mais illisible est mis
    /// de côté (`<nom>.corrupt-<horodatage>-<pid>`) avant que la première
    /// écriture ne puisse l'écraser. Le daemon repart d'un ledger vierge — il
    /// doit démarrer — mais l'opérateur peut inspecter et restaurer l'ancien
    /// contenu. Un fichier absent, lui, est le cas normal du premier lancement.
    fn load_from(path: impl AsRef<Path>) -> Ledger {
        let path = path.as_ref().to_path_buf();
        let mut ledger = match fs::read(&path) {
            Ok(bytes) => match serde_json::from_slice::<Ledger>(&bytes) {
                Ok(ledger) => ledger,
                Err(error) => {
                    let quarantine = path.with_extension(format!(
                        "corrupt-{}-{}",
                        now_unix(),
                        std::process::id()
                    ));
                    match fs::rename(&path, &quarantine) {
                        Ok(()) => log::error!(
                            "ledger illisible ({error}) : conservé dans {}",
                            quarantine.display()
                        ),
                        Err(move_error) => log::error!(
                            "ledger illisible ({error}) et impossible à mettre de côté \
                             ({move_error}) ; il sera écrasé à la prochaine écriture"
                        ),
                    }
                    Ledger::default()
                }
            },
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ledger::default(),
            Err(e) => {
                log::error!("ledger illisible ({e}) : démarrage sur un ledger vierge");
                Ledger::default()
            }
        };
        ledger.path = Some(path);
        ledger
    }
    /// RAM encore réservable **après** les réservations en cours.
    pub fn free_ram_bytes(&self, state: &DaemonState) -> u64 {
        state
            .ram_headroom_bytes()
            .saturating_sub(self.reserved_ram_bytes)
    }

    /// Cœurs encore réservables, même logique que la RAM.
    ///
    /// Le pool n'est pas le total physique : le palier s'autorise
    /// `max_cpu_percent` de la machine, exactement comme pour la RAM. Sans ce
    /// plafond, un nœud en `eco` (25 %) pouvait promettre cent pour cent de ses
    /// cœurs — le contrat de ressources mentait dans un sens que rien ne
    /// mesurait. `cpu_cores` reste le total **mesuré** ; la part réservable est
    /// calculée ici, et les réservations en cours s'en déduisent.
    pub fn free_cpu_cores(&self, state: &DaemonState) -> u32 {
        state
            .allowed_cpu_cores()
            .saturating_sub(self.reserved_cpu_cores)
    }

    /// Stockage encore réservable — agrégat O(1) comme `free_ram_bytes`.
    pub fn free_storage_bytes(&self, state: &DaemonState) -> u64 {
        state
            .storage_headroom_bytes()
            .saturating_sub(self.reserved_storage_bytes)
    }

    /// Les trois grandeurs de la RAM, dans un seul objet.
    ///
    /// `measured` est le total physique publié par le tick ; `reservable` est
    /// ce qui reste après plafond de palier, marges et réservations ; `enforced`
    /// vient de l'état, qui vaut 0 tant que le daemon n'impose aucun plafond
    /// mémoire de cgroup (c'est le cas aujourd'hui : `apply` ne pose qu'un `nice`).
    pub fn ram_capacity(&self, state: &DaemonState) -> Capacity {
        Capacity {
            measured: state.total_ram_bytes,
            reservable: self.free_ram_bytes(state),
            enforced: state.enforced_ram_bytes,
        }
    }

    /// Idem pour le stockage. `measured` = ce que le daemon s'autorise à servir
    /// (le total annoncé, engagement ferme — voir `storage_headroom_bytes`),
    /// jamais le disque entier de la machine.
    pub fn storage_capacity(&self, state: &DaemonState) -> Capacity {
        Capacity {
            measured: state.total_storage_bytes,
            reservable: self.free_storage_bytes(state),
            enforced: state.enforced_storage_bytes,
        }
    }

    /// Réserve `ram_bytes`, `cpu_cores` et `storage_bytes` sous `allocation_id`,
    /// en garantissant l'unicité de l'identifiant s'il est déjà pris. Rend
    /// l'identifiant réellement accordé — celui à renvoyer au client, sinon sa
    /// libération porterait sur un id que le ledger ne connaît pas.
    fn reserve(
        &mut self,
        allocation_id: String,
        cpu_cores: u32,
        ram_bytes: u64,
        storage_bytes: u64,
        expires_at: Option<i64>,
    ) -> String {
        let mut allocation_id = allocation_id;
        if self.grants.contains_key(&allocation_id) {
            self.disambiguator = self.disambiguator.saturating_add(1);
            allocation_id = format!("{allocation_id}-{}", self.disambiguator);
        }

        self.grants.insert(
            allocation_id.clone(),
            Grant::current(cpu_cores, ram_bytes, storage_bytes, expires_at),
        );
        self.reserved_ram_bytes = self.reserved_ram_bytes.saturating_add(ram_bytes);
        self.reserved_cpu_cores = self.reserved_cpu_cores.saturating_add(cpu_cores);
        self.reserved_storage_bytes = self.reserved_storage_bytes.saturating_add(storage_bytes);
        allocation_id
    }

    /// Libère `allocation_id`. **Idempotent** : un identifiant inconnu ne change
    /// rien et n'est pas une erreur — un client qui se déconnecte après avoir
    /// déjà libéré, ou qui n'a jamais rien obtenu, ne doit pas provoquer d'échec.
    fn release(&mut self, allocation_id: &str) {
        if let Some(grant) = self.grants.remove(allocation_id) {
            self.reserved_ram_bytes = self.reserved_ram_bytes.saturating_sub(grant.ram_bytes());
            self.reserved_cpu_cores = self.reserved_cpu_cores.saturating_sub(grant.cores());
            self.reserved_storage_bytes = self
                .reserved_storage_bytes
                .saturating_sub(grant.storage_bytes());
        }
    }

    /// Prolonge un bail encore vivant et rend sa nouvelle échéance.
    ///
    /// `None` signifie « inconnu ou déjà échu » : on ne ressuscite **jamais** un
    /// bail que le balayage a rendu au pool. Sinon deux clients pourraient se
    /// partager la même réservation, l'un ayant renouvelé ce que l'autre venait
    /// de libérer.
    pub fn renew(&mut self, allocation_id: &str, now: i64, lease: u64) -> Option<i64> {
        let grant = self.grants.get(allocation_id)?;
        if grant.expires_at().is_none() || grant.is_expired(now) {
            return None;
        }
        let (cores, ram_bytes, storage_bytes) =
            (grant.cores(), grant.ram_bytes(), grant.storage_bytes());
        let deadline = now.saturating_add(lease as i64);
        self.grants.insert(
            allocation_id.to_string(),
            Grant::current(cores, ram_bytes, storage_bytes, Some(deadline)),
        );
        Some(deadline)
    }

    /// Retire les baux échus et rend leurs identifiants, dans l'ordre.
    ///
    /// C'est la seule chose qui empêche un processus tué de garder sa marge
    /// pour toujours. Un bail sans échéance (`None`, format historique) n'est
    /// jamais retiré ici : c'est `adopt_legacy_grants` qui lui en donne une.
    pub fn sweep_expired(&mut self, now: i64) -> Vec<String> {
        let expired: Vec<String> = self
            .grants
            .iter()
            .filter(|(_, grant)| grant.is_expired(now))
            .map(|(id, _)| id.clone())
            .collect();
        for id in &expired {
            self.release(id);
        }
        expired
    }

    /// Donne une échéance aux baux hérités d'un ledger d'ancien format.
    ///
    /// Sans cela, un bail écrit par un daemon antérieur resterait éternel après
    /// la mise à jour : le défaut survivrait exactement à la version qui le
    /// corrige. La grâce est d'une durée de bail à partir du démarrage.
    pub fn adopt_legacy_grants(&mut self, now: i64, lease: u64) -> usize {
        let deadline = now.saturating_add(lease as i64);
        let mut adopted = 0;
        for grant in self.grants.values_mut() {
            if grant.expires_at().is_none() {
                let cores = grant.cores();
                let ram = grant.ram_bytes();
                let storage = grant.storage_bytes();
                *grant = Grant::current(cores, ram, storage, Some(deadline));
                adopted += 1;
            }
        }
        adopted
    }

    /// Nombre de réservations en cours.
    pub fn len(&self) -> usize {
        self.grants.len()
    }

    /// `true` si aucune réservation n'est en cours.
    pub fn is_empty(&self) -> bool {
        self.grants.is_empty()
    }
}

/// Chemin du fichier de persistance du Ledger.
///
/// `POLYGONE_LEDGER_PATH` est **prioritaire** ; à défaut, `~/.polygone/ledger.json`.
///
/// La surcharge est indispensable aux tests : sans elle, tous les serveurs de test
/// partagent le même fichier et héritent des réservations du test précédent — le
/// premier `alloc` d'un test se voit refusé parce qu'un test antérieur a déjà
/// consommé la marge. C'est le même écart que celui corrigé pour le socket
/// (`POLYGONE_DAEMON_SOCKET`).
pub const ENV_LEDGER_PATH: &str = "POLYGONE_LEDGER_PATH";

fn ledger_path() -> PathBuf {
    if let Some(p) = std::env::var_os(ENV_LEDGER_PATH) {
        if !p.is_empty() {
            let path = PathBuf::from(p);
            if let Some(parent) = path.parent() {
                let _ = fs::create_dir_all(parent);
            }
            return path;
        }
    }
    let mut dir = dirs::home_dir().expect("HOME non disponible");
    dir.push(".polygone");
    // S'assure que le répertoire existe.
    let _ = fs::create_dir_all(&dir);
    dir.join("ledger.json")
}
///
/// Le `Ledger` est passé en paramètre : mutation en mémoire, aucun verrou pris
/// ici, aucune E/S, aucune horloge. `dispatch` reste donc sans effet de bord
/// observable en dehors du ledger qu'on lui tend.
pub fn dispatch(request: Request, state: &DaemonState, ledger: &mut Ledger) -> Reply {
    match request {
        Request::Status { id } => Reply::Status {
            id,
            tier: state.tier.clone(),
            seq: state.seq,
            max_cpu_percent: state.max_cpu_percent,
            max_ram_percent: state.max_ram_percent,
            max_bandwidth_percent: state.max_bandwidth_percent,
            max_gpu_percent: state.max_gpu_percent,
            ram_bytes: state.ram_bytes,
            ram_mb: state.ram_bytes / (1024 * 1024),
            bandwidth_mbps: state.bandwidth_mbps,
            cpu_cores: state.cpu_cores,
            gpu_free_vram_mb: state.gpu_free_vram_mb,
            shrinking: state.shrinking,
            uptime_secs: state.uptime_secs,
            ram_capacity: Some(Box::new(ledger.ram_capacity(state))),
            storage_capacity: Some(Box::new(ledger.storage_capacity(state))),
        },
        // `dispatch` est pur : il ne voit jamais les compteurs du serveur. Le
        // serveur répond lui-même à cette requête (voir `handle_client`) ; un
        // appel direct obtient une carte vide plutôt qu'un mensonge chiffré.
        Request::Refusals { id } => Reply::Refusals {
            id,
            counts: BTreeMap::new(),
        },
        Request::Ping { id } => Reply::Pong { id },
        Request::Release { id, allocation_id } => {
            // Libération immédiate : sans elle, la marge resterait consommée
            // jusqu'à la fermeture de la connexion (voir `LedgerGuard`).
            ledger.release(&allocation_id);
            // Persistance du ledger après chaque libération.
            if let Err(e) = ledger.persist() {
                eprintln!("Erreur lors de la persistance du ledger : {}", e);
            }
            Reply::Released { id, allocation_id }
        }
        Request::Renew { id, allocation_id } => {
            let now = now_unix();
            // Balayer d'abord : renouveler un bail échu le ressusciterait alors
            // que ses agrégats ont déjà été rendus au pool.
            ledger.sweep_expired(now);
            match ledger.renew(&allocation_id, now, lease_secs()) {
                Some(expires_at) => {
                    if let Err(e) = ledger.persist() {
                        eprintln!("Erreur lors de la persistance du ledger : {}", e);
                    }
                    Reply::Renewed {
                        id,
                        allocation_id,
                        expires_at,
                    }
                }
                None => Reply::AllocDenied {
                    id,
                    reason: REASON_UNKNOWN_ALLOCATION.to_string(),
                },
            }
        }
        Request::Alloc {
            id,
            client,
            cpu_cores,
            ram_gb,
            gpu_vram_gb,
            storage_gb,
        } => dispatch_alloc(
            id,
            client,
            AllocRequest {
                cpu_cores,
                ram_gb,
                gpu_vram_gb,
                storage_gb,
            },
            state,
            ledger,
        ),
    }
}

/// Ressources demandées par un client dans une requête d'allocation.
///
/// Regroupées en une struct pour que `dispatch_alloc` garde une signature
/// lisible : ajouter une ressource (stockage, GPU…) ne fait plus grossir
/// la liste d'arguments.
#[derive(Debug, Clone, Copy, PartialEq)]
struct AllocRequest {
    cpu_cores: u32,
    ram_gb: f32,
    gpu_vram_gb: Option<f32>,
    storage_gb: Option<f32>,
}

/// Décision d'allocation — arithmétique pure sur l'état et le ledger fournis.
fn dispatch_alloc(
    id: Option<String>,
    client: Option<String>,
    req: AllocRequest,
    state: &DaemonState,
    ledger: &mut Ledger,
) -> Reply {
    let AllocRequest {
        cpu_cores,
        ram_gb,
        gpu_vram_gb,
        storage_gb,
    } = req;
    // Une demande absurde est refusée explicitement plutôt que convertie
    // en silence (NaN ou négatif donnerait sinon 0 octet « accordé »).
    if !ram_gb.is_finite() || ram_gb <= 0.0 || cpu_cores == 0 {
        return Reply::AllocDenied {
            id,
            reason: REASON_INVALID_REQUEST.to_string(),
        };
    }

    let requested_ram_bytes = (ram_gb as f64 * 1024.0 * 1024.0 * 1024.0) as u64;
    // Un bail échu n'est plus une réservation : on balaie AVANT de décider,
    // sinon un processus tué depuis des heures bloquerait encore la marge et
    // le refus serait faux ("pas assez de RAM" alors que la RAM est libre).
    let now = now_unix();
    let expired = ledger.sweep_expired(now);
    if !expired.is_empty() {
        eprintln!(
            "ledger : {} bail(s) échu(s) libéré(s) : {}",
            expired.len(),
            expired.join(", ")
        );
        if let Err(e) = ledger.persist() {
            eprintln!("Erreur lors de la persistance du ledger : {}", e);
        }
    }

    // Décision contre la MARGE MOINS LES RÉSERVATIONS, pas contre la marge :
    // c'est ce qui empêche deux clients d'obtenir la même RAM.
    // Un `ram_gb` minuscule (`1e-12`) passe le contrôle ci-dessus mais arrondit à
    // ZÉRO octet : accorder cela réserverait 0 octet pour un cœur entier, et
    // assez de demandes vides assécheraient le pool CPU sans qu'un octet soit
    // alloué — après quoi le daemon refuserait toute demande réelle.
    if requested_ram_bytes == 0 {
        return Reply::AllocDenied {
            id,
            reason: REASON_INVALID_REQUEST.to_string(),
        };
    }

    // En période de retrait, ne pas remplacer les consommateurs qui viennent
    // de rendre leur bail : cela créerait un cycle de stop/restart sans jamais
    // réduire les engagements. Les demandes de statut et de libération restent
    // autorisées via `dispatch`.
    if state.shrinking {
        return Reply::AllocDenied {
            id,
            reason: REASON_POLICY_SHRINKING.to_string(),
        };
    }

    // Trois refus distincts, dans cet ordre : la demande dépasse la machine
    // (définitif), puis dépasse ce qui reste réservable (temporaire), puis la
    // politique de retrait. Confondre les deux premiers poussait un client à
    // réessayer indéfiniment une demande que rien ne pourrait satisfaire.
    if requested_ram_bytes > state.total_ram_bytes {
        return Reply::AllocDenied {
            id,
            reason: REASON_EXCEEDS_MEASURED.to_string(),
        };
    }

    if requested_ram_bytes > ledger.free_ram_bytes(state) {
        return Reply::AllocDenied {
            id,
            reason: REASON_INSUFFICIENT_HEADROOM.to_string(),
        };
    }

    if cpu_cores > ledger.free_cpu_cores(state) {
        return Reply::AllocDenied {
            id,
            reason: REASON_INSUFFICIENT_CPU.to_string(),
        };
    }

    if let Some(vram) = gpu_vram_gb {
        if !vram.is_finite() || vram > state.gpu_free_vram_mb as f32 {
            return Reply::AllocDenied {
                id,
                reason: REASON_INSUFFICIENT_VRAM.to_string(),
            };
        }
    }

    // Stockage : `storage_gb` absent ou `None` = 0 octet, jamais illimité.
    // `Some(x)` avec x ≤ 0 ou NaN est une demande invalide.
    let requested_storage_bytes: u64 = match storage_gb {
        None => 0,
        Some(gb) => {
            if !gb.is_finite() || gb < 0.0 {
                return Reply::AllocDenied {
                    id,
                    reason: REASON_INVALID_REQUEST.to_string(),
                };
            }
            (gb as f64 * 1024.0 * 1024.0 * 1024.0) as u64
        }
    };

    if requested_storage_bytes > ledger.free_storage_bytes(state) {
        return Reply::AllocDenied {
            id,
            reason: REASON_INSUFFICIENT_STORAGE.to_string(),
        };
    }

    // La réservation est prise ICI, dans le même souffle que la décision :
    // décider puis réserver en deux temps laisserait une fenêtre où deux
    // requêtes concurrentes lisent la même marge libre.
    let expires_at = now.saturating_add(lease_secs() as i64);
    let allocation_id = ledger.reserve(
        make_allocation_id(state, client.as_deref(), cpu_cores, ram_gb),
        cpu_cores,
        requested_ram_bytes,
        requested_storage_bytes,
        Some(expires_at),
    );
    // Persistance du ledger après chaque réservation.
    if let Err(e) = ledger.persist() {
        eprintln!("Erreur lors de la persistance du ledger : {}", e);
    }

    // `storage_gb` n'est renvoyé que si le client a demandé du stockage.
    // Un vieux client qui n'a pas envoyé le champ reçoit `None`, et le
    // `skip_serializing_if` l'omet du JSON — compatibilité intacte.
    let granted_storage_gb = if requested_storage_bytes > 0 {
        storage_gb
    } else {
        None
    };

    Reply::AllocGranted {
        id,
        allocation_id,
        cpu_cores,
        ram_gb,
        gpu_vram_gb,
        storage_gb: granted_storage_gb,
        // Annoncé au client pour qu'il sache quand renouveler. C'est une
        // échéance réelle depuis l'expiration des baux, plus une valeur
        // décorative : le balayage la fait respecter.
        expires_at: Some(expires_at),
        // Calculé, jamais écrit en dur : vrai seulement si l'OS contraint au
        // moins ce qui vient d'être accordé. Aujourd'hui `enforced_ram_bytes`
        // vaut 0, donc ce champ vaut `false` — et le client doit lire
        // l'allocation comme une licence, pas comme un quota appliqué.
        enforced: ledger.ram_capacity(state).enforced >= requested_ram_bytes,
    }
}

/// Identifiant d'allocation déterministe — même demande + même état = même
/// identifiant. Pas de dépendance à `rand`, et donc testable sans graine.
/// L'unicité stricte (compteur, expiration) appartient au serveur.
fn make_allocation_id(
    state: &DaemonState,
    client: Option<&str>,
    cpu_cores: u32,
    ram_gb: f32,
) -> String {
    let mut hasher = DefaultHasher::new();
    state.tier.hash(&mut hasher);
    state.ram_bytes.hash(&mut hasher);
    cpu_cores.hash(&mut hasher);
    ram_gb.to_bits().hash(&mut hasher);
    client.unwrap_or("").hash(&mut hasher);
    format!("alloc-{:016x}", hasher.finish())
}

// ============================================================================
// Serveur socket (incrément 2a)
// ============================================================================
//
// Réutilise exclusivement `encode` / `decode` / `dispatch`. Aucun runtime
// asynchrone : `std::thread`, comme le reste du daemon. `serve` est bloquant
// jusqu'à ce que le drapeau `shutdown` passe à vrai — c'est l'appelant qui
// décide quand s'arrêter.

/// Code d'erreur renvoyé quand le plafond de connexions est atteint.
pub const ERR_RATE_LIMITED: &str = "rate_limited";

/// Nombre maximal de connexions servies simultanément.
pub const MAX_CONNECTIONS: u32 = 16;

/// Nombre maximal de requêtes complètes servies par seconde et par connexion.
pub const MAX_REQUESTS_PER_SECOND: u32 = 50;

/// Durée de la fenêtre de limitation de débit.
const RATE_WINDOW: Duration = Duration::from_secs(1);

/// Durée maximale d'inactivité d'une connexion, en lecture comme en écriture.
///
/// Le plafond de connexions ne sert à rien si rien ne borne leur durée de vie :
/// seize clients qui se connectent puis se taisent immobiliseraient tous les
/// emplacements et rendraient le daemon définitivement sourd. Ce délai garantit
/// qu'un emplacement finit toujours par être rendu.
pub const CONNECTION_IDLE_TIMEOUT: Duration = Duration::from_secs(60);

/// Taille des blocs lus sur une connexion.
const READ_CHUNK: usize = 4096;

/// Attente entre deux `accept` infructueux (listener non bloquant).
const ACCEPT_IDLE_SLEEP: Duration = Duration::from_millis(50);

/// Délai maximal accordé à la connexion de test qui détecte une instance vivante.
const PROBE_TIMEOUT: Duration = Duration::from_millis(200);

/// Convertit un `DecodeError` en réponse d'erreur.
///
/// L'appelant fournit `id` — quand `decode` a échoué, il l'a obtenu via
/// `salvage_id` (voir `handle_client`), qui tente une lecture indulgente de
/// la ligne. Ici, aucune analyse : la conversion est totale et ne panique pas.
pub fn error_reply(id: Option<String>, err: DecodeError) -> Reply {
    Reply::Error {
        id,
        code: err.code().to_string(),
        message: err.to_string(),
    }
}

/// Récupère `id` dans une ligne refusée par `decode`, si elle reste exploitable.
///
/// Indulgent par construction : on ne revalide ni `v` ni `type`, uniquement
/// l'identification de la requête pour que le client puisse corréler la panne.
fn salvage_id(line: &[u8]) -> Option<String> {
    let text = std::str::from_utf8(line).ok()?;
    let value: serde_json::Value = serde_json::from_str(text.trim()).ok()?;
    value.get("id")?.as_str().map(|s| s.to_string())
}

/// Prépare le socket d'écoute, en traitant le cas du fichier orphelin.
///
/// Ne supprime **jamais** une instance vivante : si quelqu'un répond à la
/// connexion de test, on refuse de démarrer. Un fichier qui n'est pas un
/// socket n'est pas touché non plus — ce n'est pas à nous de l'effacer.
pub fn bind_listener(path: &Path) -> io::Result<UnixListener> {
    // `ensure_dir()` vise le chemin global ; ici on crée le parent du chemin
    // demandé pour que les tests puissent travailler ailleurs.
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    if path.exists() {
        let meta = fs::symlink_metadata(path)?;
        if !meta.file_type().is_socket() {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                format!(
                    "{} existe et n'est pas un socket — refus de le supprimer",
                    path.display()
                ),
            ));
        }

        if probe_listener(path, PROBE_TIMEOUT) {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                format!(
                    "un daemon écoute déjà sur {} — instance vivante, démarrage refusé",
                    path.display()
                ),
            ));
        }

        // Personne ne répond : reste d'un crash précédent, on nettoie.
        fs::remove_file(path)?;
    }

    let listener = UnixListener::bind(path)?;
    // Le socket transporte des demandes de réservation de ressources. Il doit
    // rester accessible uniquement à son propriétaire, même si l'umask du
    // processus est permissif. En cas d'échec du chmod, ne pas exposer le
    // serveur avec des permissions inconnues : fermer et nettoyer.
    if let Err(error) = fs::set_permissions(path, fs::Permissions::from_mode(0o600)) {
        drop(listener);
        let _ = fs::remove_file(path);
        return Err(error);
    }
    Ok(listener)
}

/// Vrai si une instance écoute réellement derrière ce fichier.
///
/// La connexion est tentée dans un thread pour rester bornée dans le temps :
/// un socket mort répond vite (connexion refusée), mais on ne veut pas en
/// dépendre. En cas de dépassement, on considère qu'il n'y a personne.
fn probe_listener(path: &Path, timeout: Duration) -> bool {
    let (tx, rx) = mpsc::channel();
    let target = path.to_path_buf();

    thread::spawn(move || {
        let alive = UnixStream::connect(&target).is_ok();
        // Le canal peut être fermé si l'appelant a déjà renoncé.
        let _ = tx.send(alive);
    });

    rx.recv_timeout(timeout).unwrap_or(false)
}

/// Supprime le fichier de socket après un arrêt explicite.
///
/// Trois cas, dans cet ordre : chemin absent (déjà nettoyé, succès), chemin qui
/// est bien un socket (supprimé), chemin qui n'est **pas** un socket (refus,
/// rien n'est touché). Même prudence que `bind_listener` : le daemon ne détruit
/// jamais un fichier utilisateur.
///
/// `serve` ne l'appelle **pas** — c'est à l'appelant de décider. Un redémarrage
/// immédiat préfère souvent laisser le fichier en place (`bind_listener` sait
/// déjà récupérer un orphelin), tandis qu'un arrêt définitif veut un nettoyage.
/// Le partage est le suivant : le serveur possède la connexion, l'appelant
/// possède le chemin.
pub fn remove_socket_file(path: &Path) -> io::Result<()> {
    // `symlink_metadata` ne suit pas les liens, comme dans `bind_listener` :
    // un lien vers un fichier utilisateur ne doit pas nous faire supprimer
    // autre chose que ce qu'on croit.
    match fs::symlink_metadata(path) {
        Ok(meta) if meta.file_type().is_socket() => fs::remove_file(path),
        Ok(_) => Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!(
                "{} n'est pas un socket — refus de le supprimer",
                path.display()
            ),
        )),
        Err(ref e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}

/// Pose les délais de lecture **et** d'écriture sur une connexion.
///
/// Les deux sont nécessaires car ils couvrent deux blocages distincts : un
/// client qui n'envoie jamais rien (lecture) et un client qui n'écoute jamais
/// ses réponses (écriture). Sans le second, dès que le tampon d'émission du
/// noyau se remplit, le serveur resterait bloqué sur un client qui ne lit pas.
///
/// Sur Unix, un délai dépassé remonte `WouldBlock` (`EAGAIN`) et non `TimedOut` :
/// `SO_RCVTIMEO` et `SO_SNDTIMEO` se manifestent par la même erreur que
/// l'absence de données. `TimedOut` est la forme Windows. Les deux flux d'une
/// connexion sont en mode **bloquant** ici — le mode non bloquant ne concerne
/// que le listener, et sur Linux un flux accepté n'hérite pas de `O_NONBLOCK` —
/// donc ce `WouldBlock` ne peut venir que du délai (voir `is_timeout`).
fn set_connection_timeouts(stream: &UnixStream, timeout: Duration) -> io::Result<()> {
    stream.set_read_timeout(Some(timeout))?;
    stream.set_write_timeout(Some(timeout))
}

/// Vrai si l'erreur traduit un délai de connexion dépassé.
///
/// `WouldBlock` est la forme Unix du dépassement des délais de socket,
/// `TimedOut` la forme Windows. Les deux sont reconnues pour que le sens du
/// code ne dépende pas de la plateforme.
fn is_timeout(err: &io::Error) -> bool {
    matches!(
        err.kind(),
        io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
    )
}

/// Vrai si l'empoisonnement du verrou partagé a déjà été signalé.
///
/// Un drapeau plutôt qu'un `warn!` à chaque lecture : le journal ne doit pas
/// être noyé une fois par tick.
static LOCK_POISON_LOGGED: AtomicBool = AtomicBool::new(false);

/// Signale une seule fois l'empoisonnement du verrou partagé.
///
/// Un `AtomicBool` plutôt qu'un `warn!` à chaque accès : le journal ne doit pas
/// être noyé une fois par tick.
fn warn_lock_poisoned_once() {
    if !LOCK_POISON_LOGGED.swap(true, Ordering::Relaxed) {
        log::warn!(
            "socket: état partagé empoisonné (un thread a paniqué en l'écrivant) — \
             l'instantané reste cohérent, la publication continue"
        );
    }
}

/// Lit l'état partagé, en survivant à un verrou empoisonné.
///
/// Un panic survenu pendant une écriture empoisonne un `RwLock` : `read()`
/// renverrait `Err` **pour toujours**. Geler l'état en silence est le pire des
/// comportements — le daemon semblerait sain alors que plus rien ne serait
/// publié — d'autant que le contenu reste un instantané parfaitement cohérent
/// (une seule structure, écrite en un seul `store`). On récupère donc la valeur
/// via `into_inner()` et on signale la première occurrence. Jamais `unwrap()`.
///
/// `pub(crate)` plutôt que privé : la politique d'empoisonnement doit exister en
/// UN seul exemplaire, et `runtime` (qui publie) comme ses tests doivent pouvoir
/// relire l'état avec elle plutôt que de la réécrire.
pub(crate) fn read_shared(state: &RwLock<DaemonState>) -> DaemonState {
    match state.read() {
        Ok(guard) => guard.clone(),
        Err(poisoned) => {
            warn_lock_poisoned_once();
            poisoned.into_inner().clone()
        }
    }
}

/// Publie un nouvel instantané dans l'état partagé.
///
/// Même politique que `read_shared`, et pour la même raison : un verrou
/// empoisonné ne doit pas **non plus** empêcher la publication. Cesser d'écrire
/// serait le symétrique du gel en lecture — le daemon continuerait de tourner
/// pendant que le socket servirait un état figé. Le contenu restant cohérent,
/// on écrase sans état d'âme et on signale la première occurrence.
///
/// C'est le seul point d'écriture de l'état partagé : un `store` unique, aucun
/// appel système, aucun verrou tenu plus longtemps que cet assignement.
pub fn publish_shared(state: &RwLock<DaemonState>, snapshot: DaemonState) {
    match state.write() {
        Ok(mut guard) => *guard = snapshot,
        Err(poisoned) => {
            warn_lock_poisoned_once();
            *poisoned.into_inner() = snapshot;
        }
    }
}

/// Garde RAII du compteur de connexions actives.
///
/// L'appelant incrémente avant de construire le garde ; le garde garantit la
/// décrémentation, quoi qu'il arrive — y compris si le thread client panique.
///
/// Pourquoi le garde ne fait pas l'incrément : la décision d'accepter ou de
/// refuser dépend de la **valeur exacte** rendue par `fetch_add`. Un incrément
/// interne au garde obligerait à relire le compteur, ce qui rouvrirait une
/// course entre la lecture et la décision.
struct ConnectionGuard {
    counter: Arc<AtomicU32>,
}

impl ConnectionGuard {
    fn new(counter: Arc<AtomicU32>) -> Self {
        Self { counter }
    }
}

impl Drop for ConnectionGuard {
    fn drop(&mut self) {
        self.counter.fetch_sub(1, Ordering::SeqCst);
    }
}

/// Prend le verrou du ledger en survivant à un empoisonnement.
///
/// Même politique que `read_shared`/`publish_shared`, et pour la même raison :
/// une réservation jamais libérée gèlerait la marge pour toutes les connexions
/// suivantes — c'est pire qu'un panic déjà signalé par le journal. Le contenu du
/// ledger reste cohérent (un seul `Mutex`, des mutations courtes), donc on
/// reprend la valeur via `into_inner()`. Jamais `unwrap()`.
fn lock_ledger(ledger: &Mutex<Ledger>) -> MutexGuard<'_, Ledger> {
    ledger
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Même politique que `lock_ledger` : un panic d'un client ne doit pas rendre
/// les compteurs inutilisables pour les autres.
fn lock_refusals(refusals: &Mutex<RefusalCounters>) -> MutexGuard<'_, RefusalCounters> {
    refusals
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Garde RAII des réservations d'UNE connexion.
///
/// Libère **toutes** les allocations accordées sur cette connexion quand elle se
/// termine — retour normal, erreur d'E/S, inactivité (60 s), rate limit, ou panic
/// du handler (le déroulement lâche le garde, et `serve` rattrape le panic).
/// Motif identique à `ConnectionGuard` et `SocketFileGuard` : un `Drop`, jamais
/// un appel manuel en fin de fonction, parce qu'un chemin de sortie oublié
/// gèlerait la marge **pour toujours**.
///
/// Justification produit : un consommateur qui crashe sans libérer ne doit pas
/// garder ses ressources réservées (`docs/PLAN_CONSOMMATEUR.md` §6).
struct LedgerGuard {
    ledger: Arc<Mutex<Ledger>>,
    /// Identifiants accordés sur cette connexion, dans l'ordre d'obtention.
    held: Vec<String>,
}

impl LedgerGuard {
    fn new(ledger: Arc<Mutex<Ledger>>) -> Self {
        Self {
            ledger,
            held: Vec::new(),
        }
    }

    /// Décide via `dispatch`, puis retient l'`allocation_id` accordé : c'est ce
    /// lien entre réservation et connexion que le `Drop` exploite.
    ///
    /// Le verrou du ledger n'est tenu que pendant `dispatch` — aucun E/S, aucun
    /// verrou d'état partagé, aucune horloge n'est prise sous ce verrou.
    fn dispatch(&mut self, request: Request, state: &DaemonState) -> Reply {
        let reply = {
            let mut ledger = lock_ledger(&self.ledger);
            dispatch(request, state, &mut ledger)
        };
        if let Reply::AllocGranted { allocation_id, .. } = &reply {
            self.held.push(allocation_id.clone());
        }
        reply
    }
}

impl Drop for LedgerGuard {
    fn drop(&mut self) {
        if self.held.is_empty() {
            return; // Rien à rendre : on ne touche même pas au verrou.
        }

        let mut ledger = lock_ledger(&self.ledger);
        for allocation_id in &self.held {
            ledger.release(allocation_id);
        }
        if let Err(e) = ledger.persist() {
            eprintln!("Erreur lors de la persistance du ledger : {}", e);
        }
    }
}

/// Erreur de limitation de débit, avec l'`id` de la requête refusée.
fn rate_limited_reply(id: Option<String>) -> Reply {
    Reply::Error {
        id,
        code: ERR_RATE_LIMITED.to_string(),
        message: format!(
            "trop de requêtes (maximum {} par seconde)",
            MAX_REQUESTS_PER_SECOND
        ),
    }
}

/// Extracteur du message d'un panic rattrapé, pour le journal.
///
/// `Box<dyn Any + Send>` doit être ramené explicitement à `&dyn Any` : la
/// résolution de méthode n'enlève pas les auto-traits toute seule.
fn panic_message(payload: Box<dyn std::any::Any + Send>) -> String {
    let any: &dyn std::any::Any = payload.as_ref();
    if let Some(s) = any.downcast_ref::<&'static str>() {
        (*s).to_string()
    } else if let Some(s) = any.downcast_ref::<String>() {
        s.clone()
    } else {
        "panic sans message".to_string()
    }
}

/// Compteur de requêtes par fenêtre de temps.
///
/// Fenêtre **fixe** d'une seconde plutôt que glissante : aucune allocation,
/// aucun historique à tenir, et un coût constant par requête. Le défaut connu
/// d'une fenêtre fixe est qu'une rafale à cheval sur deux fenêtres peut laisser
/// passer jusqu'à deux fois le plafond ; pour un garde-fou d'auto-protection
/// (empêcher un client de saturer le CPU), c'est un compromis acceptable — une
/// fenêtre glissante exigerait un anneau d'horodatages pour un gain marginal.
struct RateLimiter {
    window_start: Instant,
    count: u32,
}

impl RateLimiter {
    fn new() -> Self {
        Self {
            window_start: Instant::now(),
            count: 0,
        }
    }

    /// Enregistre une requête complète ; vrai si le plafond est dépassé.
    fn exceeds(&mut self, limit: u32) -> bool {
        let now = Instant::now();
        if now.duration_since(self.window_start) >= RATE_WINDOW {
            self.window_start = now;
            self.count = 0;
        }
        self.count = self.count.saturating_add(1);
        self.count > limit
    }
}

/// Boucle d'acceptation. Un thread par connexion. Bloque jusqu'à `shutdown`.
///
/// Passe-partout du délai nominal : délègue à `serve_with_timeout`. Ne supprime
/// **pas** le fichier de socket à l'arrêt — c'est l'appelant qui sait si un
/// redémarrage est prévu (voir `remove_socket_file`).
pub fn serve(listener: UnixListener, state: Arc<RwLock<DaemonState>>, shutdown: Arc<AtomicBool>) {
    serve_with_timeout(listener, state, shutdown, CONNECTION_IDLE_TIMEOUT);
}

/// Identique à `serve`, avec un délai d'inactivité injectable.
///
/// Existe pour que les tests puissent vérifier l'expiration d'une connexion en
/// quelques centaines de millisecondes au lieu d'attendre le délai nominal.
pub fn serve_with_timeout(
    listener: UnixListener,
    state: Arc<RwLock<DaemonState>>,
    shutdown: Arc<AtomicBool>,
    idle_timeout: Duration,
) {
    serve_with_timeout_at(listener, state, shutdown, idle_timeout, ledger_path());
}

/// Démarre le serveur avec un chemin de persistance explicite. Utile pour
/// héberger plusieurs instances isolées (tests, intégration, environnements
/// dédiés) sans modifier une variable d'environnement globale.
pub fn serve_with_timeout_at(
    listener: UnixListener,
    state: Arc<RwLock<DaemonState>>,
    shutdown: Arc<AtomicBool>,
    idle_timeout: Duration,
    persistence_path: PathBuf,
) {
    if let Err(e) = listener.set_nonblocking(true) {
        log::error!("socket: listener non bloquant impossible : {}", e);
        return;
    }

    let active = Arc::new(AtomicU32::new(0));
    // Le ledger des réservations vit ICI, au niveau du serveur : il est partagé
    // par toutes les connexions (c'est tout l'intérêt) et n'entre jamais dans
    // `DaemonState`, dont `publish_shared` reste le seul écrivain.
    let ledger = Arc::new(Mutex::new(Ledger::load_from(persistence_path)));
    // Même contrainte que le ledger : un seul point d'écriture, partagé par les
    // connexions, jamais dans `DaemonState`.
    let refusals = Arc::new(Mutex::new(RefusalCounters::default()));
    // Au démarrage : donner une échéance aux baux d'ancien format, puis retirer
    // ceux déjà échus. Sans la première étape, un bail hérité d'une version
    // antérieure resterait éternel — le défaut survivrait à son correctif.
    {
        let mut guard = lock_ledger(&ledger);
        let now = now_unix();
        let adopted = guard.adopt_legacy_grants(now, lease_secs());
        let expired = guard.sweep_expired(now);
        if adopted > 0 || !expired.is_empty() {
            log::info!(
                "socket: ledger — {} bail(aux) hérité(s) mis sous échéance, {} échu(s) libéré(s)",
                adopted,
                expired.len()
            );
            if let Err(e) = guard.persist() {
                log::error!("socket: persistance du ledger après balayage : {}", e);
            }
        }
    }
    log::info!(
        "socket: serveur en écoute (plafond {} connexions, inactivité {} s)",
        MAX_CONNECTIONS,
        idle_timeout.as_secs_f64()
    );

    while !shutdown.load(Ordering::Relaxed) {
        match listener.accept() {
            Ok((stream, _addr)) => {
                let current = active.fetch_add(1, Ordering::SeqCst) + 1;
                // Le garde possède la décrémentation : plus aucun `fetch_sub`
                // manuel dans ce chemin, donc plus d'oubli possible.
                let guard = ConnectionGuard::new(Arc::clone(&active));

                if current > MAX_CONNECTIONS {
                    log::warn!(
                        "socket: plafond de {} connexions atteint — connexion refusée",
                        MAX_CONNECTIONS
                    );
                    refuse_connection(stream);
                    // `guard` retombe en fin d'itération : compteur rendu aussitôt.
                    continue;
                }

                let state = Arc::clone(&state);
                let ledger = Arc::clone(&ledger);
                let refusals = Arc::clone(&refusals);
                thread::spawn(move || {
                    // Le garde vit dans ce thread : le compteur est rendu même
                    // si `handle_client` panique (le déroulement lâche `_guard`).
                    let _guard = guard;
                    let outcome = panic::catch_unwind(AssertUnwindSafe(|| {
                        handle_client(stream, state, ledger, refusals, idle_timeout)
                    }));
                    if let Err(payload) = outcome {
                        log::error!(
                            "socket: connexion interrompue par un panic : {}",
                            panic_message(payload)
                        );
                    }
                });
            }
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                // Rien à accepter : on rend la main pour regarder `shutdown`.
                thread::sleep(ACCEPT_IDLE_SLEEP);
            }
            Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
            Err(e) => {
                log::warn!("socket: accept a échoué : {}", e);
                thread::sleep(ACCEPT_IDLE_SLEEP);
            }
        }
    }

    log::info!("socket: arrêt demandé — serveur terminé");
}

/// Répond « trop de monde » puis ferme — sans jamais garder la connexion.
fn refuse_connection(mut stream: UnixStream) {
    let reply = Reply::Error {
        id: None,
        code: ERR_RATE_LIMITED.to_string(),
        message: format!(
            "trop de connexions simultanées (maximum {})",
            MAX_CONNECTIONS
        ),
    };
    let _ = write_reply_with_version(&mut stream, &reply, MIN_PROTOCOL_VERSION);
}

/// Sert une connexion : salutation, puis lecture/décodage/réponse en boucle.
///
/// `idle_timeout` borne la durée de vie de la connexion : au-delà de ce délai
/// sans donnée entrante (ou sans écriture possible), elle est fermée.
pub fn handle_client(
    mut stream: UnixStream,
    state: Arc<RwLock<DaemonState>>,
    ledger: Arc<Mutex<Ledger>>,
    refusals: Arc<Mutex<RefusalCounters>>,
    idle_timeout: Duration,
) {
    // Posé avant la salutation : une connexion non bornée dans le temps occupe
    // un emplacement pour toujours, on ne veut pas laisser la moindre fenêtre
    // où elle échapperait au délai. Si les délais ne peuvent pas être posés, la
    // garantie est nulle : on ferme plutôt que de servir une connexion éternelle.
    if let Err(e) = set_connection_timeouts(&stream, idle_timeout) {
        log::error!("socket: délais de connexion impossibles à poser : {}", e);
        return;
    }

    // Un seul instantané pour la salutation : le verrou est relâché aussitôt.
    let hello_state = read_shared(&state);
    let hello = Reply::Hello {
        daemon_version: hello_state.daemon_version.clone(),
        platform: hello_state.platform.clone(),
        tier: hello_state.tier.clone(),
        // Le `hello` reste en v1 pour les clients déjà installés ; il annonce
        // la version la plus récente dans un champ séparé.
        protocol_version: MIN_PROTOCOL_VERSION,
        max_protocol_version: Some(PROTOCOL_VERSION),
        min_protocol_version: MIN_PROTOCOL_VERSION,
        capabilities: CAPABILITIES.iter().map(|c| (*c).to_string()).collect(),
    };
    if write_reply_with_version(&mut stream, &hello, MIN_PROTOCOL_VERSION).is_err() {
        return; // Client déjà parti : plus rien à faire.
    }

    // Version du pair pour cette connexion. Chaque réponse l'utilise : un
    // client v1 ne doit jamais recevoir un `v: 2` qu'il refuserait.
    let mut peer_version = MIN_PROTOCOL_VERSION;

    // Tampon persistant : conserve les octets reçus après un '\n' et non
    // encore traités (un client peut envoyer deux lignes d'un coup).
    let mut buffer: Vec<u8> = Vec::new();

    // Limitation de débit : elle compte les requêtes COMPLÈTES (celles qui
    // arrivent à `decode`), jamais les octets lus — une grosse requête
    // légitime ne doit pas être punie pour sa taille.
    let mut limiter = RateLimiter::new();

    // Le garde est construit AVANT la boucle : aucune allocation ne peut être
    // accordée plus tôt (la salutation ne réserve rien), donc il couvre tous les
    // chemins de sortie de la boucle, y compris le panic rattrapé par `serve`.
    let mut bookings = LedgerGuard::new(ledger);

    loop {
        // 1) Une ligne complète attend-elle déjà dans le tampon ?
        if let Some(pos) = buffer.iter().position(|b| *b == b'\n') {
            let drained: Vec<u8> = buffer.drain(..=pos).collect();
            let line = &drained[..drained.len() - 1];

            if limiter.exceeds(MAX_REQUESTS_PER_SECOND) {
                // Refus puis fermeture : la limite protège le CPU du daemon,
                // elle ne se contente pas d'avertir un client emballé.
                let reply = rate_limited_reply(salvage_id(line));
                let _ = write_reply_with_version(&mut stream, &reply, peer_version);
                return;
            }

            match decode_line_with_version(line) {
                Ok((request, version)) => {
                    peer_version = version;
                    // Verrou relâché AVANT l'écriture réseau : on lit, on clone,
                    // on lâche le garde, puis on dispatche. Le verrou n'est
                    // jamais tenu pendant une E/S.
                    // Les compteurs vivent dans le serveur : cette requête est
                    // la seule à ne pas passer par `dispatch`.
                    let reply = match request {
                        Request::Refusals { id } => Reply::Refusals {
                            id,
                            counts: lock_refusals(&refusals).snapshot(),
                        },
                        other => {
                            let snapshot = read_shared(&state);
                            let reply = bookings.dispatch(other, &snapshot);
                            // Un seul point de comptage : toute réponse
                            // `AllocDenied` passe ici, y compris `renew` d'un
                            // bail inconnu. Compter dans chaque branche de
                            // `dispatch` aurait laissé la prochaine branche
                            // ajoutée silencieusement hors du compteur.
                            if let Reply::AllocDenied { reason, .. } = &reply {
                                lock_refusals(&refusals).record(reason);
                            }
                            reply
                        }
                    };
                    if write_reply_with_version(&mut stream, &reply, peer_version).is_err() {
                        return;
                    }
                }
                Err(DecodeError::LineTooLong) => {
                    // Impossible de resynchroniser le flux : on répond et on ferme.
                    let reply = error_reply(salvage_id(line), DecodeError::LineTooLong);
                    let _ = write_reply_with_version(&mut stream, &reply, peer_version);
                    return;
                }
                Err(err) => {
                    // Tout autre refus laisse la connexion ouverte : le client
                    // peut se corriger et renvoyer une requête.
                    let reply = error_reply(salvage_id(line), err);
                    if write_reply_with_version(&mut stream, &reply, peer_version).is_err() {
                        return;
                    }
                }
            }
            continue;
        }

        // 2) Pas de ligne complète : lire un bloc, sans dépasser la borne.
        //    On s'arrête au plus tard à MAX_LINE_BYTES + 1 octets, ce qui
        //    suffit à distinguer « limite atteinte » de « ligne trop longue ».
        let remaining = (MAX_LINE_BYTES + 1).saturating_sub(buffer.len());
        let want = remaining.clamp(1, READ_CHUNK);
        let mut chunk = [0u8; READ_CHUNK];

        match stream.read(&mut chunk[..want]) {
            Ok(0) => return, // EOF : le client a fermé de son côté.
            Ok(n) => buffer.extend_from_slice(&chunk[..n]),
            Err(ref e) if e.kind() == io::ErrorKind::Interrupted => continue,
            // Inactivité : il n'y a rien à dire à un client silencieux, on
            // ferme sans message. C'est le sens exact du contrat — le délai
            // protège le daemon, il ne signale pas une faute du client.
            Err(ref e) if is_timeout(e) => {
                log::debug!("socket: connexion inactive fermée");
                return;
            }
            Err(_) => return,
        }

        if !buffer.contains(&b'\n') && buffer.len() > MAX_LINE_BYTES {
            let reply = error_reply(salvage_id(&buffer), DecodeError::LineTooLong);
            let _ = write_reply_with_version(&mut stream, &reply, peer_version);
            return;
        }
    }
}

/// Décode une ligne d'octets avec sa version : l'UTF-8 invalide est un message
/// mal formé.
fn decode_line_with_version(line: &[u8]) -> Result<(Request, u8), DecodeError> {
    let text = std::str::from_utf8(line).map_err(|_| DecodeError::MalformedJson)?;
    decode_with_version(text)
}

/// Écrit une réponse encodée puis vide le tampon.
///
/// Toute erreur — y compris un dépassement du délai d'écriture, qui remonte
/// `WouldBlock` sur Unix (voir `set_connection_timeouts`) — est rendue à
/// l'appelant, qui ferme la connexion. Il n'y a rien à dire à un client qui
/// n'écoute pas ses réponses.
/// Écrit une réponse dans la version du pair, puis vide le tampon.
fn write_reply_with_version(stream: &mut UnixStream, reply: &Reply, version: u8) -> io::Result<()> {
    stream.write_all(encode_with_version(reply, version).as_bytes())?;
    stream.flush()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader};

    /// État servant de référence : 16 Gio, palier 50 % → 8 Gio autorisés,
    /// 1 Gio déjà alloué → 7 Gio de marge. Six cœurs physiques, palier 50 %,
    /// donc **3 cœurs réservables** — le total mesuré n'est pas le pool.
    fn state() -> DaemonState {
        DaemonState {
            cpu_cores: 6,
            ..DaemonState::new("balanced")
        }
    }

    #[test]
    fn test_decode_valid_status_request() {
        let req = decode(r#"{"v":1,"id":"t1","type":"status_request"}"#)
            .expect("requête valide acceptée");
        assert_eq!(
            req,
            Request::Status {
                id: Some("t1".to_string())
            }
        );
    }

    #[test]
    fn test_decode_malformed_json() {
        assert_eq!(decode("{"), Err(DecodeError::MalformedJson));
        // JSON valide mais racine non-objet : mal formé pour ce protocole.
        assert_eq!(decode("[1,2,3]"), Err(DecodeError::MalformedJson));
        // Type absent.
        assert_eq!(decode(r#"{"v":1}"#), Err(DecodeError::MalformedJson));
        // Champ obligatoire absent d'une alloc_request.
        assert_eq!(
            decode(r#"{"v":1,"type":"alloc_request","ram_gb":1.0}"#),
            Err(DecodeError::MalformedJson)
        );
    }

    #[test]
    fn test_decode_unknown_type() {
        assert_eq!(
            decode(r#"{"v":1,"type":"nope"}"#),
            Err(DecodeError::UnknownType)
        );
    }

    #[test]
    fn test_decode_accepts_the_supported_version_window() {
        // v1 reste lisible, v2 est la version courante.
        for line in [
            r#"{"v":1,"type":"status_request"}"#,
            r#"{"v":2,"type":"status_request"}"#,
        ] {
            assert!(
                matches!(decode(line), Ok(Request::Status { .. })),
                "une version supportée doit se décoder : {line}"
            );
        }
        // v3 est futur et refusé explicitement ; l'absence de version aussi.
        assert_eq!(
            decode(r#"{"v":3,"type":"status_request"}"#),
            Err(DecodeError::UnsupportedVersion)
        );
        assert_eq!(
            decode(r#"{"type":"status_request"}"#),
            Err(DecodeError::UnsupportedVersion)
        );
    }

    #[test]
    fn test_ledger_persistence() {
        // Isole ce test dans un dossier temporaire pour éviter les collisions
        // entre tests parallèles qui partagent le même ledger_path() global.
        let dir = tempfile::tempdir().expect("dossier temporaire");
        let ledger_file = dir.path().join("ledger_persistence.json");
        // Crée un ledger, réserve des ressources, persiste, puis recharge.
        let mut ledger = Ledger::at_path(&ledger_file);
        let alloc_id = ledger.reserve("test-alloc".to_string(), 2, 1024 * 1024 * 1024, 0, None);
        // Vérifie la présence dans le ledger en mémoire.
        assert!(ledger.grants.contains_key(&alloc_id));
        // Persiste sur disque.
        ledger.persist().expect("Échec de la persistance du ledger");
        // Recharge un nouveau ledger depuis le disque.
        let loaded = Ledger::load_from(&ledger_file);
        // Le même identifiant doit être présent avec les mêmes valeurs.
        assert_eq!(loaded.grants.get(&alloc_id), ledger.grants.get(&alloc_id));
    }

    /// La persistance doit être atomique et privée : un fichier tronqué serait
    /// lu comme un ledger vide, et un ledger lisible par tous exposerait les
    /// réservations de l'utilisateur.
    #[test]
    fn test_ledger_persist_is_atomic_and_private() {
        let dir = tempfile::tempdir().expect("dossier temporaire");
        let ledger_file = dir.path().join("ledger_atomic.json");

        let mut ledger = Ledger::at_path(&ledger_file);
        ledger.reserve("alloc".to_string(), 1, 1024, 0, None);
        ledger.persist().expect("persistance");

        let mode = fs::metadata(&ledger_file)
            .expect("métadonnées du ledger")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(
            mode, 0o600,
            "un ledger doit rester privé (0600), trouvé {mode:o}"
        );

        let text = fs::read_to_string(&ledger_file).expect("relecture du ledger");
        assert!(
            serde_json::from_str::<serde_json::Value>(&text).is_ok(),
            "le fichier renommé doit toujours être un JSON complet : {text}"
        );
        assert!(
            !dir.path().join(".ledger_atomic.json.tmp").exists(),
            "aucun fichier temporaire ne doit rester après un rename réussi"
        );
    }

    /// Un ledger illisible ne doit jamais être écrasé sans trace : il est
    /// quarantiné, et le daemon repart d'un ledger vierge.
    #[test]
    fn test_corrupt_ledger_is_quarantined_not_silently_erased() {
        let dir = tempfile::tempdir().expect("dossier temporaire");
        let ledger_file = dir.path().join("ledger_corrupt.json");
        fs::write(&ledger_file, b"{ ceci n'est pas du JSON").expect("écriture du fichier corrompu");

        let ledger = Ledger::load_from(&ledger_file);
        assert!(
            ledger.is_empty(),
            "un fichier illisible ne doit pas ressusciter de fausses réservations"
        );
        assert!(
            !ledger_file.exists(),
            "l'original est déplacé, pas laissé pour être écrasé à la première écriture"
        );

        let quarantined: Vec<_> = fs::read_dir(dir.path())
            .expect("lecture du dossier")
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path())
            .filter(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.contains("corrupt-"))
            })
            .collect();
        assert_eq!(
            quarantined.len(),
            1,
            "le contenu illisible doit être conservé dans un seul fichier de quarantaine"
        );
        let preserved = fs::read(&quarantined[0]).expect("relecture de la quarantaine");
        assert_eq!(
            preserved, b"{ ceci n'est pas du JSON",
            "la quarantaine contient les octets d'origine, pas une reconstruction"
        );
    }

    #[test]
    fn test_decode_line_too_long() {
        let long = "a".repeat(MAX_LINE_BYTES + 1);
        assert_eq!(decode(&long), Err(DecodeError::LineTooLong));
        // Juste à la limite : la taille passe, c'est le contenu qui est refusé.
        let at_limit = "x".repeat(MAX_LINE_BYTES);
        assert_eq!(decode(&at_limit), Err(DecodeError::MalformedJson));
    }

    #[test]
    fn test_encode_status_response() {
        let reply = dispatch(
            Request::Status { id: None },
            &state(),
            &mut Ledger::default(),
        );
        let line = encode(&reply);

        assert!(line.ends_with('\n'), "chaque réponse est une ligne");
        assert!(
            line.contains(&format!("\"v\":{PROTOCOL_VERSION}")),
            "version courante présente : {line}"
        );
        assert!(
            line.contains("\"type\":\"status_response\""),
            "type présent : {line}"
        );
        assert!(
            line.contains("\"ram_capacity\""),
            "capacité RAM publiée : {line}"
        );
        assert!(
            line.contains("\"storage_capacity\""),
            "capacité stockage publiée : {line}"
        );
        // Le champ `id` absent ne doit pas apparaître (skip_serializing_if).
        assert!(!line.contains("\"id\""), "id omis quand absent : {line}");
    }

    #[test]
    fn test_dispatch_status_reports_ledger_capacity() {
        let state = state();
        let reply = dispatch(Request::Status { id: None }, &state, &mut Ledger::default());
        match reply {
            Reply::Status {
                ram_capacity: Some(capacity),
                storage_capacity: Some(storage),
                ..
            } => {
                assert_eq!(capacity.measured, 16 * 1024 * 1024 * 1024);
                assert_eq!(capacity.reservable, 7 * 1024 * 1024 * 1024);
                assert_eq!(capacity.enforced, 0);
                assert_eq!(
                    storage.measured, 0,
                    "le daemon n'annonce pas encore de stockage"
                );
                assert_eq!(storage.reservable, 0);
            }
            other => panic!("capacités attendues dans Status, reçu {other:?}"),
        }
    }

    #[test]
    fn test_decode_legacy_status_without_capacity() {
        let line = r#"{"v":1,"type":"status_response","tier":"balanced","seq":1,"max_cpu_percent":50,"max_ram_percent":50,"max_bandwidth_percent":60,"max_gpu_percent":40,"ram_bytes":1024,"ram_mb":0,"bandwidth_mbps":10,"cpu_cores":2,"gpu_free_vram_mb":0,"shrinking":false,"uptime_secs":1}"#;
        match decode_reply(line).expect("ancienne réponse v1 toujours décodable") {
            Reply::Status {
                ram_capacity,
                storage_capacity,
                ..
            } => {
                assert!(ram_capacity.is_none());
                assert!(storage_capacity.is_none());
            }
            other => panic!("Status attendu, reçu {other:?}"),
        }
    }

    #[test]
    fn test_dispatch_ping() {
        assert_eq!(
            dispatch(
                Request::Ping {
                    id: Some("p".to_string())
                },
                &state(),
                &mut Ledger::default()
            ),
            Reply::Pong {
                id: Some("p".to_string())
            }
        );
    }

    #[test]
    fn test_dispatch_status() {
        match dispatch(
            Request::Status { id: None },
            &state(),
            &mut Ledger::default(),
        ) {
            Reply::Status { tier, ram_mb, .. } => {
                assert!(!tier.is_empty(), "le palier est renseigné");
                assert!(ram_mb > 0, "l'allocation courante est rapportée");
            }
            other => panic!("attendu Status, obtenu {:?}", other),
        }
    }

    #[test]
    fn test_dispatch_alloc_granted() {
        let reply = dispatch(
            Request::Alloc {
                id: Some("a1".to_string()),
                client: Some("petals".to_string()),
                cpu_cores: 2,
                ram_gb: 1.0,
                gpu_vram_gb: None,
                storage_gb: None,
            },
            &state(),
            &mut Ledger::default(),
        );
        match reply {
            Reply::AllocGranted {
                allocation_id,
                cpu_cores,
                ..
            } => {
                assert!(!allocation_id.is_empty(), "identifiant non vide");
                assert_eq!(cpu_cores, 2);
            }
            other => panic!("attendu AllocGranted, obtenu {:?}", other),
        }
    }

    // ---- P1.1 : mesuré / réservable / contraint ----------------------------

    /// Une demande qui dépasse la **machine** ne doit pas être confondue avec
    /// une demande qui dépasse le **reste réservable**. Le premier refus est
    /// définitif ; le second se débloque quand un pair rend son bail.
    #[test]
    fn test_alloc_over_measured_is_not_headroom() {
        // `state()` : 16 Gio mesurés, plafond de palier à 50 %, donc 7 Gio
        // réservables. Une demande de 20 Gio dépasse la machine elle-même.
        let reply = dispatch(
            Request::Alloc {
                id: None,
                client: None,
                cpu_cores: 1,
                ram_gb: 20.0,
                gpu_vram_gb: None,
                storage_gb: None,
            },
            &state(),
            &mut Ledger::default(),
        );
        match reply {
            Reply::AllocDenied { reason, .. } => assert_eq!(
                reason, REASON_EXCEEDS_MEASURED,
                "dépasser la machine n'est pas un manque de marge"
            ),
            other => panic!("attendu AllocDenied, obtenu {:?}", other),
        }
    }

    /// Entre les deux : sous la capacité mesurée, au-dessus du réservable.
    /// C'est le refus temporaire, et il doit porter son propre motif.
    #[test]
    fn test_alloc_over_reservable_reports_headroom() {
        let reply = dispatch(
            Request::Alloc {
                id: None,
                client: None,
                cpu_cores: 1,
                ram_gb: 10.0, // < 16 Gio mesurés, > 7 Gio réservables
                gpu_vram_gb: None,
                storage_gb: None,
            },
            &state(),
            &mut Ledger::default(),
        );
        match reply {
            Reply::AllocDenied { reason, .. } => assert_eq!(
                reason, REASON_INSUFFICIENT_HEADROOM,
                "sous la machine mais au-dessus du réservable"
            ),
            other => panic!("attendu AllocDenied, obtenu {:?}", other),
        }
    }

    /// L'allocation réellement accordée porte son degré de contrainte.
    /// Tant que le daemon n'impose aucun plafond mémoire, c'est une licence.
    #[test]
    fn test_granted_says_licence_when_nothing_enforced() {
        let reply = dispatch(
            Request::Alloc {
                id: None,
                client: None,
                cpu_cores: 1,
                ram_gb: 1.0,
                gpu_vram_gb: None,
                storage_gb: None,
            },
            &state(),
            &mut Ledger::default(),
        );
        match reply {
            Reply::AllocGranted { enforced, .. } => assert!(
                !enforced,
                "rien n'est contraint par l'OS : accorder ne doit pas promettre un quota"
            ),
            other => panic!("attendu AllocGranted, obtenu {:?}", other),
        }
    }

    /// Et si l'état dit qu'une contrainte existe, le champ la reflète — la
    /// valeur est calculée, jamais écrite en dur.
    #[test]
    fn test_granted_reports_enforcement_when_state_says_so() {
        let mut st = state();
        st.enforced_ram_bytes = 2 * 1024 * 1024 * 1024;
        let reply = dispatch(
            Request::Alloc {
                id: None,
                client: None,
                cpu_cores: 1,
                ram_gb: 1.0,
                gpu_vram_gb: None,
                storage_gb: None,
            },
            &st,
            &mut Ledger::default(),
        );
        match reply {
            Reply::AllocGranted { enforced, .. } => {
                assert!(enforced, "2 Gio contraints couvrent une demande de 1 Gio")
            }
            other => panic!("attendu AllocGranted, obtenu {:?}", other),
        }
    }

    /// Invariant : on ne promet jamais plus que ce que la machine possède, et
    /// le degré de contrainte ne peut pas dépasser ce qui a été promis.
    #[test]
    fn test_capacity_invariant() {
        let st = state();
        let ledger = Ledger::default();
        let ram = ledger.ram_capacity(&st);
        assert_eq!(ram.measured, st.total_ram_bytes);
        assert!(
            ram.reservable <= ram.measured,
            "réservable {} > mesuré {}",
            ram.reservable,
            ram.measured
        );
        assert_eq!(ram.enforced, 0, "aucune contrainte OS dans état inerte");
        assert!(
            !ram.is_enforced(),
            "0 contraint ne doit jamais être présenté comme appliqué"
        );

        // Le stockage non annoncé (0) ne peut rien réserver : c'est le repli sûr.
        let storage = ledger.storage_capacity(&st);
        assert_eq!(storage.measured, 0);
        assert_eq!(storage.reservable, 0);
    }

    #[test]
    fn test_dispatch_alloc_denied_during_policy_shrink() {
        let shrinking = DaemonState {
            shrinking: true,
            ..state()
        };
        let mut ledger = Ledger::default();
        let free_before = ledger.free_ram_bytes(&shrinking);
        let reply = dispatch(
            Request::Alloc {
                id: Some("during-shrink".to_string()),
                client: Some("node".to_string()),
                cpu_cores: 1,
                ram_gb: 0.25,
                gpu_vram_gb: None,
                storage_gb: None,
            },
            &shrinking,
            &mut ledger,
        );
        assert_eq!(
            reply,
            Reply::AllocDenied {
                id: Some("during-shrink".to_string()),
                reason: REASON_POLICY_SHRINKING.to_string(),
            }
        );
        assert_eq!(ledger.free_ram_bytes(&shrinking), free_before);
        assert_eq!(
            ledger.free_cpu_cores(&shrinking),
            shrinking.allowed_cpu_cores()
        );
    }

    #[test]
    fn test_dispatch_alloc_denied_insufficient() {
        // Palier 50 % de 16 Gio = 8 Gio autorisés, déjà 8 Gio alloués → 0 de marge.
        let saturated = DaemonState {
            ram_bytes: 8 * 1024 * 1024 * 1024,
            ..state()
        };
        let reply = dispatch(
            Request::Alloc {
                id: None,
                client: None,
                cpu_cores: 1,
                ram_gb: 1.0,
                gpu_vram_gb: None,
                storage_gb: None,
            },
            &saturated,
            &mut Ledger::default(),
        );
        assert_eq!(
            reply,
            Reply::AllocDenied {
                id: None,
                reason: REASON_INSUFFICIENT_HEADROOM.to_string()
            }
        );

        // Trop de cœurs demandés : refus distinct, pas un refus de mémoire.
        let too_many_cores = dispatch(
            Request::Alloc {
                id: None,
                client: None,
                cpu_cores: 99,
                ram_gb: 1.0,
                gpu_vram_gb: None,
                storage_gb: None,
            },
            &state(),
            &mut Ledger::default(),
        );
        assert_eq!(
            too_many_cores,
            Reply::AllocDenied {
                id: None,
                reason: REASON_INSUFFICIENT_CPU.to_string()
            }
        );
    }

    /// Le pool CPU suit le palier, pas le total physique. Un nœud `eco` à
    /// 25 % de huit cœurs ne peut promettre que deux cœurs ; le cinquième
    /// refus porte le motif dédié, pas un refus de mémoire.
    #[test]
    fn test_cpu_pool_is_capped_by_the_tier() {
        let mut state = state();
        state.cpu_cores = 8;
        state.max_cpu_percent = 25;
        let mut ledger = Ledger::default();

        assert_eq!(state.allowed_cpu_cores(), 2);
        assert_eq!(ledger.free_cpu_cores(&state), 2);

        let granted = dispatch(
            Request::Alloc {
                id: None,
                client: None,
                cpu_cores: 2,
                ram_gb: 1.0,
                gpu_vram_gb: None,
                storage_gb: None,
            },
            &state,
            &mut ledger,
        );
        assert!(matches!(granted, Reply::AllocGranted { .. }));
        assert_eq!(ledger.free_cpu_cores(&state), 0);

        let denied = dispatch(
            Request::Alloc {
                id: None,
                client: None,
                cpu_cores: 1,
                ram_gb: 1.0,
                gpu_vram_gb: None,
                storage_gb: None,
            },
            &state,
            &mut ledger,
        );
        assert_eq!(
            denied,
            Reply::AllocDenied {
                id: None,
                reason: REASON_INSUFFICIENT_CPU.to_string()
            }
        );
    }

    /// Un cœur au-delà du plafond est refusé même quand la machine en a
    /// physiquement assez : c'est le contrat de palier, pas une mesure.
    #[test]
    fn test_cpu_above_the_tier_pool_is_denied() {
        let mut state = state();
        state.cpu_cores = 8;
        state.max_cpu_percent = 50;
        let reply = dispatch(
            Request::Alloc {
                id: None,
                client: None,
                cpu_cores: 5,
                ram_gb: 1.0,
                gpu_vram_gb: None,
                storage_gb: None,
            },
            &state,
            &mut Ledger::default(),
        );
        assert_eq!(
            reply,
            Reply::AllocDenied {
                id: None,
                reason: REASON_INSUFFICIENT_CPU.to_string()
            }
        );
    }

    #[test]
    fn test_ledger_reserves_and_frees() {
        // Marge de référence : 8 Gio autorisés − 1 Gio alloué = 7 Gio, 3 cœurs.
        let state = state();
        let mut ledger = Ledger::default();

        let first = ledger.reserve("alloc-a".to_string(), 1, 5 * 1024u64 * 1024 * 1024, 0, None);
        assert_eq!(first, "alloc-a", "un id libre est rendu tel quel");
        assert_eq!(ledger.free_ram_bytes(&state), 2 * 1024u64 * 1024 * 1024);
        assert_eq!(ledger.free_cpu_cores(&state), 2);

        // Même identifiant redemandé : il est départagé, et les DEUX réservations
        // comptent — c'est le bug que le ledger existe pour empêcher.
        let second = ledger.reserve("alloc-a".to_string(), 1, 1, 0, None);
        assert_ne!(
            second, "alloc-a",
            "un id déjà pris ne doit pas écraser l'autre"
        );
        assert_eq!(ledger.free_ram_bytes(&state), 2 * 1024u64 * 1024 * 1024 - 1);
        assert_eq!(ledger.free_cpu_cores(&state), 1);

        // Libération d'un id inconnu : sans effet, sans panic (idempotence).
        ledger.release("jamais-accorde");
        assert_eq!(ledger.free_cpu_cores(&state), 1);

        // Libérer les deux rend exactement la marge d'origine.
        ledger.release(&first);
        ledger.release(&second);
        assert_eq!(ledger.free_ram_bytes(&state), 7 * 1024u64 * 1024 * 1024);
        assert_eq!(ledger.free_cpu_cores(&state), 3);
    }

    #[test]
    fn test_ledger_reserves_and_frees_storage() {
        // Le stockage suit la même mécanique O(1) que la RAM : réservé,
        // agrégé, rendu à la libération. Un total de 0 refuse tout.
        let mut state = state();
        assert_eq!(
            Ledger::default().free_storage_bytes(&state),
            0,
            "sans stockage annoncé, la marge est nulle"
        );

        state.total_storage_bytes = 10 * 1024u64 * 1024 * 1024; // 10 Gio
        let mut ledger = Ledger::default();
        assert_eq!(
            ledger.free_storage_bytes(&state),
            10 * 1024u64 * 1024 * 1024
        );

        // Une réservation entame la marge du montant exact.
        let a = ledger.reserve(
            "alloc-s1".to_string(),
            1,
            0,
            4 * 1024u64 * 1024 * 1024,
            None,
        );
        assert_eq!(ledger.free_storage_bytes(&state), 6 * 1024u64 * 1024 * 1024);

        // Les réservations s'agrègent : deux clients, deux débits.
        let b = ledger.reserve(
            "alloc-s2".to_string(),
            1,
            0,
            2 * 1024u64 * 1024 * 1024,
            None,
        );
        assert_eq!(ledger.free_storage_bytes(&state), 4 * 1024u64 * 1024 * 1024);

        // Libérer un id inconnu ne rend rien (idempotence).
        ledger.release("jamais-accorde");
        assert_eq!(ledger.free_storage_bytes(&state), 4 * 1024u64 * 1024 * 1024);

        // Libérer rend exactement ce qui avait été pris — pas plus.
        ledger.release(&a);
        assert_eq!(ledger.free_storage_bytes(&state), 8 * 1024u64 * 1024 * 1024);
        ledger.release(&b);
        assert_eq!(
            ledger.free_storage_bytes(&state),
            10 * 1024u64 * 1024 * 1024
        );
    }

    #[test]
    fn test_alloc_storage_gb_absent_is_zero_not_unlimited() {
        // Compatibilité : un ancien client qui n'envoie pas `storage_gb`
        // obtient 0 octet de stockage — jamais un blanc-seing.
        let mut state = state();
        state.total_storage_bytes = 10 * 1024u64 * 1024 * 1024;

        let reply = dispatch_alloc(
            None,
            Some("vieux-client".to_string()),
            AllocRequest {
                cpu_cores: 1,
                ram_gb: 1.0,
                gpu_vram_gb: None,
                storage_gb: None,
            },
            &state,
            &mut Ledger::default(),
        );
        match reply {
            Reply::AllocGranted {
                storage_gb,
                gpu_vram_gb,
                ..
            } => {
                assert_eq!(storage_gb, None, "aucun stockage demandé → aucun accordé");
                assert_eq!(gpu_vram_gb, None);
            }
            other => panic!("allocation attendue, reçu {other:?}"),
        }
    }

    #[test]
    fn test_alloc_storage_beyond_headroom_is_denied() {
        // Demander plus que la marge annoncée est refusé, pas rogné en silence.
        let mut state = state();
        state.total_storage_bytes = 10 * 1024u64 * 1024 * 1024;

        let reply = dispatch_alloc(
            None,
            Some("gourmand".to_string()),
            AllocRequest {
                cpu_cores: 1,
                ram_gb: 1.0,
                gpu_vram_gb: None,
                storage_gb: Some(50.0), // 50 Gio pour 10 Gio annoncés
            },
            &state,
            &mut Ledger::default(),
        );
        match reply {
            Reply::AllocDenied { reason, .. } => {
                assert_eq!(reason, REASON_INSUFFICIENT_STORAGE);
            }
            other => panic!("refus attendu, reçu {other:?}"),
        }
    }

    #[test]
    fn test_alloc_old_client_json_without_storage_gb() {
        // Un client v2.0.0 (avant `storage_gb`) doit continuer à décoder.
        let json = r#"{"v":1,"id":"t1","type":"alloc_request","cpu_cores":2,"ram_gb":1.0}"#;
        let req = decode(json).expect("ancien JSON accepté");
        match req {
            Request::Alloc { storage_gb, .. } => {
                assert_eq!(storage_gb, None, "champ absent → None, pas d'erreur");
            }
            other => panic!("Alloc attendu, reçu {other:?}"),
        }
    }

    #[test]
    fn test_socket_path_honors_env() {
        // Contrat de NOM partagé avec le client Petals
        // (crates/petals/src/daemon.rs:20) : deux noms différents et les deux
        // côtés ne se parlent plus, sans que rien ne le signale.
        assert_eq!(ENV_SOCKET_PATH, "POLYGONE_DAEMON_SOCKET");

        // Décision pure : la variable est prioritaire sur le défaut.
        assert_eq!(
            socket_path_from(
                Some(OsString::from("/tmp/2b6.sock")),
                Some(PathBuf::from("/home/u"))
            ),
            PathBuf::from("/tmp/2b6.sock")
        );
        // Variable VIDE = absente : un chemin vide ne doit pas devenir le socket.
        assert_eq!(
            socket_path_from(Some(OsString::new()), Some(PathBuf::from("/home/u"))),
            PathBuf::from("/home/u/.polygone/daemon.sock")
        );
        assert_eq!(
            socket_path_from(None, Some(PathBuf::from("/home/u"))),
            PathBuf::from("/home/u/.polygone/daemon.sock")
        );
        // Sans `$HOME` exploitable, on ne rend pas un chemin vide.
        assert_eq!(
            socket_path_from(None, None),
            PathBuf::from(".").join(".polygone").join("daemon.sock")
        );

        // Le chemin RÉEL lit bien la variable. Le `OnceLock` ne s'initialise
        // qu'une fois par processus : c'est le SEUL test qui appelle
        // `socket_path()`, et il pose la variable avant l'appel — aucune autre
        // épreuve du binaire ne touche ce chemin.
        std::env::set_var(ENV_SOCKET_PATH, "/tmp/2b6-chemin-reel.sock");
        assert_eq!(socket_path(), PathBuf::from("/tmp/2b6-chemin-reel.sock"));
        std::env::remove_var(ENV_SOCKET_PATH);
    }

    // ── Symétrie du protocole (2b-7a) ────────────────────────────────────
    #[test]
    fn test_ledger_persistence_preserves_counters() {
        let dir = tempfile::tempdir().expect("dossier temporaire");
        let ledger_file = dir.path().join("ledger_counters.json");
        // Reserve some resources and persist.
        let mut ledger = Ledger::at_path(&ledger_file);
        let alloc_id = ledger.reserve(
            "test-alloc2".to_string(),
            2,
            2 * 1024 * 1024 * 1024,
            5 * 1024 * 1024 * 1024,
            None,
        );
        // Ensure counters are set.
        assert_eq!(ledger.reserved_cpu_cores, 2);
        assert_eq!(ledger.reserved_ram_bytes, 2 * 1024 * 1024 * 1024);
        assert_eq!(ledger.reserved_storage_bytes, 5 * 1024 * 1024 * 1024);
        // Persist to disk.
        ledger.persist().expect("persist failed");
        // Load anew.
        let loaded = Ledger::load_from(&ledger_file);
        // Verify counters restored.
        assert_eq!(loaded.reserved_cpu_cores, 2);
        assert_eq!(loaded.reserved_ram_bytes, 2 * 1024 * 1024 * 1024);
        assert_eq!(loaded.reserved_storage_bytes, 5 * 1024 * 1024 * 1024);
        // Verify grant entry matches.
        assert_eq!(loaded.grants.get(&alloc_id), ledger.grants.get(&alloc_id));
    }

    /// Prouve qu'un redémarrage simulé (drop du Ledger + reconstruction depuis
    /// le fichier) conserve les octets déjà réservés : la marge disponible
    /// après rechargement est réduite du montant réservé avant le drop.
    ///
    /// Sans persistance, `Ledger::load()` renverrait un ledger vierge et le
    /// deuxième accord serait accordé à tort — c'est exactement le bug visé.
    #[test]
    fn test_ledger_restart_conserves_reserved_bytes() {
        let dir = tempfile::tempdir().expect("dossier temporaire");
        let ledger_file = dir.path().join("ledger_restart.json");
        let state = state(); // 7 Gio de marge, 3 cœurs

        // --- "Premier processus" : réserve 5 Gio et persiste. ---
        let allocation_id = {
            let mut ledger = Ledger::at_path(&ledger_file);
            let id = ledger.reserve(
                "restart-alloc".to_string(),
                1,
                5 * 1024u64 * 1024 * 1024,
                0,
                None,
            );
            assert_eq!(
                ledger.free_ram_bytes(&state),
                2 * 1024u64 * 1024 * 1024,
                "avant drop : 5 Gio réservés → 2 Gio libres"
            );
            ledger.persist().expect("première persistance");
            id
            // `ledger` tombe hors de portée ici : drop sans release, simule un crash/arrêt.
        };

        // --- "Second processus" : recharge depuis le fichier. ---
        let reloaded = Ledger::load_from(&ledger_file);
        assert!(
            reloaded.grants.contains_key(&allocation_id),
            "après rechargement, la réservation doit être présente"
        );
        assert_eq!(
            reloaded.free_ram_bytes(&state),
            2 * 1024u64 * 1024 * 1024,
            "après rechargement, la marge doit toujours refléter les 5 Gio réservés"
        );
        assert_eq!(
            reloaded.reserved_cpu_cores, 1,
            "le cœur réservé doit survivre au redémarrage"
        );
    }
    /// OPS.2 : chaque motif de refus est provoqué sur un VRAI serveur, puis la
    /// répartition est relue depuis `status_response` — le chemin que la CLI
    /// affiche, pas un compteur interne inspecté en douce.
    #[test]
    fn refusals_are_counted_by_reason_through_the_server() {
        let dir = tempfile::tempdir().expect("dossier temporaire");
        let path = dir.path().join("refusals.sock");
        let listener = bind_listener(&path).expect("bind sur un socket temporaire");
        let ledger_file = dir.path().join("ledger.json");
        let state = Arc::new(RwLock::new(DaemonState::new("balanced")));
        let shutdown = Arc::new(AtomicBool::new(false));
        let serve_state = Arc::clone(&state);
        let serve_shutdown = Arc::clone(&shutdown);
        let handle = thread::spawn(move || {
            serve_with_timeout_at(
                listener,
                serve_state,
                serve_shutdown,
                Duration::from_secs(5),
                ledger_file,
            )
        });

        let mut stream = UnixStream::connect(&path).expect("connexion au serveur de test");
        {
            let mut reader = BufReader::new(stream.try_clone().expect("clone du flux"));
            let mut hello = String::new();
            reader.read_line(&mut hello).expect("salutation");
            assert!(hello.contains("hello"), "salutation reçue : {hello}");
        }

        let alloc = |ram_gb: f32,
                     cpu_cores: u32,
                     gpu_vram_gb: Option<f32>,
                     storage_gb: Option<f32>| Request::Alloc {
            id: None,
            client: None,
            cpu_cores,
            ram_gb,
            gpu_vram_gb,
            storage_gb,
        };

        let requests = [
            (alloc(0.0, 1, None, None), REASON_INVALID_REQUEST),
            (alloc(1000.0, 1, None, None), REASON_EXCEEDS_MEASURED),
            (alloc(7.5, 1, None, None), REASON_INSUFFICIENT_HEADROOM),
            (alloc(0.1, 99, None, None), REASON_INSUFFICIENT_CPU),
            (alloc(0.1, 1, Some(1.0), None), REASON_INSUFFICIENT_VRAM),
            (alloc(0.1, 1, None, Some(1.0)), REASON_INSUFFICIENT_STORAGE),
            (
                Request::Renew {
                    id: None,
                    allocation_id: "bail-inconnu".to_string(),
                },
                REASON_UNKNOWN_ALLOCATION,
            ),
        ];

        for (request, expected) in requests {
            match ask(&mut stream, &request) {
                Reply::AllocDenied { reason, .. } => assert_eq!(
                    reason, expected,
                    "le motif attendu pour {request:?} doit être nommé"
                ),
                other => panic!("refus attendu pour {request:?}, reçu {other:?}"),
            }
        }

        // `policy_shrinking` exige un état en retrait : on le pose APRÈS le
        // démarrage, comme le ferait la boucle du daemon.
        state.write().expect("verrou d'état").shrinking = true;
        match ask(&mut stream, &alloc(0.1, 1, None, None)) {
            Reply::AllocDenied { reason, .. } => {
                assert_eq!(reason, REASON_POLICY_SHRINKING)
            }
            other => panic!("refus de politique attendu, reçu {other:?}"),
        }
        state.write().expect("verrou d'état").shrinking = false;

        match ask(&mut stream, &Request::Refusals { id: None }) {
            Reply::Refusals { counts, .. } => {
                for reason in [
                    REASON_INVALID_REQUEST,
                    REASON_EXCEEDS_MEASURED,
                    REASON_INSUFFICIENT_HEADROOM,
                    REASON_INSUFFICIENT_CPU,
                    REASON_INSUFFICIENT_VRAM,
                    REASON_INSUFFICIENT_STORAGE,
                    REASON_UNKNOWN_ALLOCATION,
                    REASON_POLICY_SHRINKING,
                ] {
                    assert_eq!(
                        counts.get(reason),
                        Some(&1),
                        "motif {reason} attendu une fois dans {counts:?}"
                    );
                }
                assert_eq!(counts.len(), 8, "aucun motif inventé : {counts:?}");
                assert_eq!(counts.values().sum::<u64>(), 8);
            }
            other => panic!("Refusals attendu, reçu {other:?}"),
        }

        shutdown.store(true, Ordering::SeqCst);
        handle.join().expect("arrêt du serveur");
    }

    /// Envoie une requête sur un flux déjà salué et rend la réponse décodée.
    fn ask(stream: &mut UnixStream, request: &Request) -> Reply {
        stream
            .write_all(encode_line(request).as_bytes())
            .expect("écriture de la requête");
        stream.flush().expect("vidage du flux");
        let mut reader = BufReader::new(stream.try_clone().expect("clone du flux"));
        let mut line = String::new();
        reader.read_line(&mut line).expect("lecture de la réponse");
        decode_reply(&line).expect("réponse décodable")
    }

    /// Tous les variants de `Reply`. Existe pour que la liste blanche et le type
    /// ne puissent pas diverger sans qu'un test le dise.
    fn every_reply() -> Vec<Reply> {
        vec![
            Reply::Hello {
                daemon_version: "2.0.0".to_string(),
                platform: "linux".to_string(),
                tier: "balanced".to_string(),
                protocol_version: MIN_PROTOCOL_VERSION,
                max_protocol_version: Some(PROTOCOL_VERSION),
                min_protocol_version: MIN_PROTOCOL_VERSION,
                capabilities: CAPABILITIES.iter().map(|c| (*c).to_string()).collect(),
            },
            Reply::Status {
                id: Some("s1".to_string()),
                tier: "balanced".to_string(),
                seq: 1,
                max_cpu_percent: 50,
                max_ram_percent: 50,
                max_bandwidth_percent: 60,
                max_gpu_percent: 40,
                ram_bytes: 1024 * 1024,
                ram_mb: 1,
                bandwidth_mbps: 10,
                cpu_cores: 2,
                gpu_free_vram_mb: 0,
                shrinking: false,
                uptime_secs: 1,
                ram_capacity: Some(Box::new(Capacity {
                    measured: 16 * 1024 * 1024 * 1024,
                    reservable: 7 * 1024 * 1024 * 1024,
                    enforced: 0,
                })),
                storage_capacity: Some(Box::new(Capacity {
                    measured: 0,
                    reservable: 0,
                    enforced: 0,
                })),
            },
            Reply::AllocGranted {
                id: None,
                allocation_id: "alloc-1".to_string(),
                cpu_cores: 1,
                ram_gb: 1.0,
                gpu_vram_gb: None,
                storage_gb: None,
                expires_at: None,
                enforced: false,
            },
            Reply::AllocDenied {
                id: None,
                reason: REASON_INSUFFICIENT_CPU.to_string(),
            },
            Reply::Released {
                id: None,
                allocation_id: "alloc-1".to_string(),
            },
            Reply::Pong { id: None },
            Reply::Error {
                id: None,
                code: ERR_UNKNOWN_TYPE.to_string(),
                message: "type de message inconnu".to_string(),
            },
            Reply::Refusals {
                id: None,
                counts: BTreeMap::from([(REASON_INSUFFICIENT_CPU.to_string(), 2)]),
            },
        ]
    }

    /// Tous les variants de `Request`, même rôle que `every_reply`.
    fn every_request() -> Vec<Request> {
        vec![
            Request::Status { id: None },
            Request::Ping { id: None },
            Request::Alloc {
                id: None,
                client: None,
                cpu_cores: 1,
                ram_gb: 1.0,
                gpu_vram_gb: None,
                storage_gb: None,
            },
            Request::Release {
                id: None,
                allocation_id: "alloc-1".to_string(),
            },
            Request::Renew {
                id: None,
                allocation_id: "alloc-1".to_string(),
            },
        ]
    }

    #[test]
    fn test_known_reply_types_covers_every_variant() {
        for reply in every_reply() {
            let value = serde_json::to_value(&reply).expect("une réponse se sérialise");
            let name = value["type"]
                .as_str()
                .expect("chaque réponse porte un type")
                .to_string();

            assert!(
                KNOWN_REPLY_TYPES.contains(&name.as_str()),
                "le variant `{name}` doit être dans KNOWN_REPLY_TYPES"
            );

            // Et l'aller-retour RÉEL, pas seulement la liste blanche : c'est lui
            // qui prouve que le client peut relire ce que le serveur écrit.
            let line = encode(&reply);
            assert_eq!(
                decode_reply(&line).expect("chaque réponse encodée se relit"),
                reply,
                "aller-retour cassé pour `{name}` : {line}"
            );
        }
    }

    #[test]
    fn test_known_request_types_covers_every_variant() {
        for request in every_request() {
            let value = serde_json::to_value(&request).expect("une requête se sérialise");
            let name = value["type"]
                .as_str()
                .expect("chaque requête porte un type")
                .to_string();

            assert!(
                KNOWN_REQUEST_TYPES.contains(&name.as_str()),
                "le variant `{name}` doit être dans KNOWN_REQUEST_TYPES"
            );

            let line = encode_request(&request);
            assert_eq!(
                decode(&line).expect("chaque requête encodée se décode"),
                request,
                "aller-retour cassé pour `{name}` : {line}"
            );
        }
    }

    #[test]
    fn test_decode_reply_rejects_request_shapes() {
        // Les deux listes blanches ne se recouvrent pas : une requête présentée
        // comme une réponse est refusée AVANT toute désérialisation, et
        // réciproquement. Sans cela, `decode_reply` accepterait ce que `decode`
        // produit, et les deux moitiés pourraient dériver l'une de l'autre.
        let request_line = encode_request(&Request::Alloc {
            id: None,
            client: None,
            cpu_cores: 1,
            ram_gb: 1.0,
            gpu_vram_gb: None,
            storage_gb: None,
        });
        assert_eq!(decode_reply(&request_line), Err(DecodeError::UnknownType));

        let reply_line = encode(&Reply::Pong { id: None });
        assert_eq!(decode(&reply_line), Err(DecodeError::UnknownType));
    }

    #[test]
    fn test_encode_request_carries_version() {
        let line = encode_request(&Request::Status {
            id: Some("x".to_string()),
        });

        assert!(
            line.ends_with('\n'),
            "chaque requête est une ligne : {line}"
        );
        assert!(
            line.contains(&format!("\"v\":{PROTOCOL_VERSION}")),
            "version courante présente : {line}"
        );
        assert!(
            line.contains("\"type\":\"status_request\""),
            "type présent : {line}"
        );
        assert_eq!(
            decode(&line).expect("la requête encodée se décode"),
            Request::Status {
                id: Some("x".to_string())
            }
        );

        let legacy = encode_request_with_version(
            &Request::Status {
                id: Some("x".to_string()),
            },
            MIN_PROTOCOL_VERSION,
        );
        assert!(
            legacy.contains("\"v\":1"),
            "l'encodage négocié doit pouvoir rester en v1 : {legacy}"
        );
        assert_eq!(
            decode(&legacy).expect("la requête v1 se décode"),
            Request::Status {
                id: Some("x".to_string())
            }
        );
    }

    #[test]
    fn test_dispatch_alloc_rejects_zero_byte_ram() {
        let state = state();
        let mut ledger = Ledger::default();

        // `1e-12` Gio est fini ET strictement positif : il passe le premier
        // contrôle de validité, mais vaut ZÉRO octet une fois converti.
        let reply = dispatch(
            Request::Alloc {
                id: None,
                client: None,
                cpu_cores: 1,
                ram_gb: 1e-12,
                gpu_vram_gb: None,
                storage_gb: None,
            },
            &state,
            &mut ledger,
        );
        assert_eq!(
            reply,
            Reply::AllocDenied {
                id: None,
                reason: REASON_INVALID_REQUEST.to_string()
            },
            "un accord de zéro octet n'est pas une allocation"
        );
        assert_eq!(
            ledger.free_cpu_cores(&state),
            3,
            "un refus ne consomme aucun cœur"
        );

        // La preuve par l'absurde : trois demandes vides suffisaient à assécher
        // les 3 cœurs, après quoi toute demande réelle était refusée alors que
        // zéro octet avait été alloué.
        for _ in 0..5 {
            let denied = dispatch(
                Request::Alloc {
                    id: None,
                    client: None,
                    cpu_cores: 1,
                    ram_gb: 1e-12,
                    gpu_vram_gb: None,
                    storage_gb: None,
                },
                &state,
                &mut ledger,
            );
            assert!(matches!(denied, Reply::AllocDenied { .. }));
        }
        assert_eq!(ledger.free_cpu_cores(&state), 3, "le pool CPU est intact");

        // Et une demande RÉELLE reste servie.
        let real = dispatch(
            Request::Alloc {
                id: None,
                client: None,
                cpu_cores: 1,
                ram_gb: 1.0,
                gpu_vram_gb: None,
                storage_gb: None,
            },
            &state,
            &mut ledger,
        );
        assert!(
            matches!(real, Reply::AllocGranted { .. }),
            "une demande réelle doit rester servie, obtenu {real:?}"
        );
    }
}

#[cfg(test)]
mod server_tests {
    use super::*;
    use crate::policy::glow_up::DaemonConfig;
    use crate::resources::create_platform;
    use crate::runtime::run_tick;
    use std::io::{BufRead, BufReader};

    /// Démarre un serveur sur un socket temporaire, avec le délai nominal.
    ///
    /// Le `TempDir` renvoyé **doit** rester vivant jusqu'à la fin du test :
    /// c'est lui qui possède le dossier (et donc le socket).
    fn start_server() -> (
        PathBuf,
        Arc<AtomicBool>,
        thread::JoinHandle<()>,
        tempfile::TempDir,
    ) {
        start_server_with_timeout(CONNECTION_IDLE_TIMEOUT)
    }

    /// Un serveur de test en marche : état partagé, chemin, drapeau d'arrêt,
    /// thread serveur, et le dossier qui possède le socket.
    type TestServer = (
        Arc<RwLock<DaemonState>>,
        PathBuf,
        Arc<AtomicBool>,
        thread::JoinHandle<()>,
        tempfile::TempDir,
    );

    /// Idem, avec un délai d'inactivité court pour les tests d'expiration.
    fn start_server_with_timeout(
        idle_timeout: Duration,
    ) -> (
        PathBuf,
        Arc<AtomicBool>,
        thread::JoinHandle<()>,
        tempfile::TempDir,
    ) {
        let (_state, path, shutdown, handle, dir) = start_server_with_state(idle_timeout);
        (path, shutdown, handle, dir)
    }

    /// Idem, en rendant aussi l'état partagé — pour les tests qui doivent le
    /// modifier **après** le démarrage du serveur.
    fn start_server_with_state(idle_timeout: Duration) -> TestServer {
        let dir = tempfile::tempdir().expect("dossier temporaire");
        let path = dir.path().join("daemon.sock");
        let listener = bind_listener(&path).expect("bind sur un socket temporaire");

        // Chaque serveur possède son propre chemin de persistance. Ne pas
        // utiliser `set_var` ici : l'environnement est global au processus et
        // les tests Rust s'exécutent en parallèle.
        let ledger_file = dir.path().join("ledger.json");

        // L'état vit derrière un `RwLock` : la boucle du daemon le réécrit à
        // chaque tick, le serveur le relit à chaque requête.
        let state = Arc::new(RwLock::new(DaemonState {
            platform: "test".to_string(),
            uptime_secs: 42,
            // 6 cœurs physiques à 50 % = 3 cœurs réservables : le pool que ces
            // tests de réservation attendent depuis l'origine.
            cpu_cores: 6,
            ..DaemonState::new("balanced")
        }));
        let shutdown = Arc::new(AtomicBool::new(false));

        let serve_state = Arc::clone(&state);
        let serve_shutdown = Arc::clone(&shutdown);
        let handle = thread::spawn(move || {
            serve_with_timeout_at(
                listener,
                serve_state,
                serve_shutdown,
                idle_timeout,
                ledger_file,
            )
        });

        (state, path, shutdown, handle, dir)
    }

    /// État partagé prêt à l'emploi, pour les tests unitaires hors serveur.
    fn shared_state(state: DaemonState) -> Arc<RwLock<DaemonState>> {
        Arc::new(RwLock::new(state))
    }

    /// Moteur réel pour relier les deux moitiés : `run_tick` lit un vrai
    /// snapshot (c'est le but), mais les tests passent `apply = false`, donc
    /// rien n'est jamais appliqué à la machine.
    fn tick_engine() -> crate::GlowUpEngine {
        crate::GlowUpEngine::new(DaemonConfig::default(), create_platform())
    }

    /// Écrit dans l'état partagé, en survivant à un empoisonnement éventuel.
    fn write_shared(state: &Arc<RwLock<DaemonState>>, f: impl FnOnce(&mut DaemonState)) {
        let mut guard = state
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        f(&mut guard);
    }

    /// Arrête le serveur et attend la fin effective de sa boucle.
    fn stop_server(shutdown: &Arc<AtomicBool>, handle: thread::JoinHandle<()>) {
        shutdown.store(true, Ordering::SeqCst);
        handle.join().expect("le serveur doit s'arrêter proprement");
    }

    /// Ouvre une connexion : un flux pour écrire, un lecteur pour les réponses.
    fn connect(path: &Path) -> (UnixStream, BufReader<UnixStream>) {
        let stream = UnixStream::connect(path).expect("connexion au serveur");
        let reader = BufReader::new(stream.try_clone().expect("clone du flux"));
        (stream, reader)
    }

    /// Lit une ligne et la parse en JSON (échec explicite si le serveur ferme).
    fn read_json(reader: &mut BufReader<UnixStream>) -> serde_json::Value {
        let mut line = String::new();
        let n = reader.read_line(&mut line).expect("lecture d'une ligne");
        assert!(n > 0, "le serveur a fermé la connexion sans répondre");
        serde_json::from_str(line.trim()).expect("réponse JSON valide")
    }

    fn send(stream: &mut UnixStream, text: &str) {
        stream.write_all(text.as_bytes()).expect("envoi");
        stream.flush().expect("vidage du tampon");
    }

    #[test]
    fn test_serve_hello_on_connect() {
        let (path, shutdown, handle, _dir) = start_server();

        let (_stream, mut reader) = connect(&path);
        let hello = read_json(&mut reader);

        assert_eq!(hello["v"], 1);
        assert_eq!(hello["type"], "hello");
        assert_eq!(hello["protocol_version"], 1);
        assert_eq!(hello["max_protocol_version"], 2);
        assert_eq!(hello["min_protocol_version"], 1);
        assert!(
            hello["capabilities"]
                .as_array()
                .is_some_and(|caps| caps.iter().any(|c| c.as_str() == Some("refusals"))),
            "les capacités réelles doivent être annoncées : {hello}"
        );
        assert_eq!(hello["platform"], "test");
        assert_eq!(hello["tier"], "balanced");

        stop_server(&shutdown, handle);
    }

    #[test]
    fn test_serve_status_request() {
        let (path, shutdown, handle, _dir) = start_server();
        let (mut stream, mut reader) = connect(&path);
        let _hello = read_json(&mut reader);

        send(
            &mut stream,
            "{\"v\":1,\"id\":\"s1\",\"type\":\"status_request\"}\n",
        );
        let reply = read_json(&mut reader);

        assert_eq!(reply["v"], 1);
        assert_eq!(reply["type"], "status_response");
        assert_eq!(reply["id"], "s1");
        assert_eq!(reply["tier"], "balanced");
        assert!(
            reply["ram_mb"].as_u64().unwrap_or(0) > 0,
            "l'allocation courante est rapportée"
        );

        stop_server(&shutdown, handle);
    }

    #[test]
    fn test_serve_accepts_v1_and_refuses_v3_without_closing() {
        let (path, shutdown, handle, _dir) = start_server();
        let (mut stream, mut reader) = connect(&path);
        let _hello = read_json(&mut reader);

        // v3 est futur : refus explicite, pas d'interprétation au mieux.
        send(
            &mut stream,
            "{\"v\":3,\"id\":\"x\",\"type\":\"status_request\"}\n",
        );
        let error = read_json(&mut reader);
        assert_eq!(error["type"], "error");
        assert_eq!(error["code"], "unsupported_version");
        assert_eq!(error["v"], 1, "une erreur de version reste lisible en v1");

        // La connexion reste ouverte, et v1 est toujours servi — en v1.
        send(
            &mut stream,
            "{\"v\":1,\"id\":\"y\",\"type\":\"status_request\"}\n",
        );
        let status = read_json(&mut reader);
        assert_eq!(status["type"], "status_response");
        assert_eq!(status["id"], "y");
        assert_eq!(status["v"], 1);

        // La même requête en v2 reçoit une réponse v2.
        send(
            &mut stream,
            "{\"v\":2,\"id\":\"z\",\"type\":\"status_request\"}\n",
        );
        let status_v2 = read_json(&mut reader);
        assert_eq!(status_v2["type"], "status_response");
        assert_eq!(status_v2["id"], "z");
        assert_eq!(status_v2["v"], 2);

        stop_server(&shutdown, handle);
    }

    #[test]
    fn test_serve_ping() {
        let (path, shutdown, handle, _dir) = start_server();
        let (mut stream, mut reader) = connect(&path);
        let _hello = read_json(&mut reader);

        send(&mut stream, "{\"v\":1,\"id\":\"p42\",\"type\":\"ping\"}\n");
        let reply = read_json(&mut reader);

        assert_eq!(reply["type"], "pong");
        assert_eq!(
            reply["id"], "p42",
            "l'identifiant est recopié à l'identique"
        );

        stop_server(&shutdown, handle);
    }

    #[test]
    fn test_serve_malformed_keeps_connection() {
        let (path, shutdown, handle, _dir) = start_server();
        let (mut stream, mut reader) = connect(&path);
        let _hello = read_json(&mut reader);

        send(&mut stream, "{ ceci n'est pas du JSON\n");
        let error = read_json(&mut reader);
        assert_eq!(error["type"], "error");
        assert_eq!(error["code"], "malformed_json");

        // La connexion doit avoir survécu : la requête suivante est servie.
        send(
            &mut stream,
            "{\"v\":1,\"id\":\"after\",\"type\":\"status_request\"}\n",
        );
        let reply = read_json(&mut reader);
        assert_eq!(reply["type"], "status_response");
        assert_eq!(reply["id"], "after");

        stop_server(&shutdown, handle);
    }

    #[test]
    fn test_serve_line_too_long_closes() {
        let (path, shutdown, handle, _dir) = start_server();
        let (mut stream, mut reader) = connect(&path);
        let _hello = read_json(&mut reader);

        // MAX_LINE_BYTES + 1 octets, sans le moindre '\n'.
        let too_long = vec![b'x'; MAX_LINE_BYTES + 1];
        stream
            .write_all(&too_long)
            .expect("envoi du pavé trop long");
        stream.flush().expect("vidage du tampon");

        let error = read_json(&mut reader);
        assert_eq!(error["type"], "error");
        assert_eq!(error["code"], "line_too_long");

        // Fermeture : la lecture suivante renvoie 0 octet (fin de flux).
        let mut rest = String::new();
        let n = reader
            .read_line(&mut rest)
            .expect("lecture après fermeture");
        assert_eq!(n, 0, "la connexion doit être fermée après line_too_long");

        stop_server(&shutdown, handle);
    }

    #[test]
    fn test_serve_two_clients() {
        let (path, shutdown, handle, _dir) = start_server();

        let (mut a, mut a_reader) = connect(&path);
        let (mut b, mut b_reader) = connect(&path);
        let _hello_a = read_json(&mut a_reader);
        let _hello_b = read_json(&mut b_reader);

        send(
            &mut a,
            "{\"v\":1,\"id\":\"A\",\"type\":\"status_request\"}\n",
        );
        send(
            &mut b,
            "{\"v\":1,\"id\":\"B\",\"type\":\"status_request\"}\n",
        );

        let reply_a = read_json(&mut a_reader);
        let reply_b = read_json(&mut b_reader);

        assert_eq!(reply_a["type"], "status_response");
        assert_eq!(reply_b["type"], "status_response");
        assert_eq!(reply_a["id"], "A");
        assert_eq!(reply_b["id"], "B");

        stop_server(&shutdown, handle);
    }

    #[test]
    fn test_bind_listener_is_owner_only() {
        let dir = tempfile::tempdir().expect("dossier temporaire");
        let path = dir.path().join("private.sock");
        let listener = bind_listener(&path).expect("bind privé");
        let mode = fs::metadata(&path)
            .expect("metadata socket")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(
            mode, 0o600,
            "le socket IPC doit être réservé à son propriétaire"
        );
        drop(listener);
    }

    #[test]
    fn test_bind_listener_refuses_live_instance() {
        let dir = tempfile::tempdir().expect("dossier temporaire");
        let path = dir.path().join("daemon.sock");
        let _alive = bind_listener(&path).expect("premier bind");

        // Un premier socket écoute toujours : on refuse, on ne le tue pas.
        let second = bind_listener(&path);
        let err = second.expect_err("un second démarrage doit être refusé");
        assert_eq!(err.kind(), io::ErrorKind::AlreadyExists);
        assert!(
            err.to_string().contains("instance vivante"),
            "message explicite attendu, obtenu : {}",
            err
        );
    }

    #[test]
    fn test_bind_listener_cleans_orphan() {
        let dir = tempfile::tempdir().expect("dossier temporaire");
        let path = dir.path().join("daemon.sock");

        // Un listener lâché laisse son fichier derrière lui : orphelin type.
        drop(bind_listener(&path).expect("premier bind"));
        assert!(path.exists(), "le fichier du socket doit subsister");

        let listener = bind_listener(&path).expect("le socket orphelin est nettoyé");
        assert!(path.exists());
        drop(listener);
    }

    // ── Garde de compteur (RAII) ──────────────────────────────────────────

    #[test]
    fn test_connection_guard_decrements_on_drop() {
        // 1 = la connexion déjà comptée par `serve` avant de créer le garde.
        let counter = Arc::new(AtomicU32::new(1));

        {
            let _guard = ConnectionGuard::new(Arc::clone(&counter));
            assert_eq!(
                counter.load(Ordering::SeqCst),
                1,
                "la création du garde ne compte pas une seconde fois"
            );
        }

        assert_eq!(
            counter.load(Ordering::SeqCst),
            0,
            "le compteur est décrémenté au drop"
        );
    }

    #[test]
    fn test_connection_guard_decrements_on_panic() {
        let counter = Arc::new(AtomicU32::new(1));
        let guard = ConnectionGuard::new(Arc::clone(&counter));

        let outcome = panic::catch_unwind(AssertUnwindSafe(move || {
            // Le garde meurt pendant le déroulement, comme dans le thread client.
            let _inside = guard;
            panic!("panique volontaire du test");
        }));

        assert!(outcome.is_err(), "le panic doit être rattrapé, pas propagé");
        assert_eq!(
            counter.load(Ordering::SeqCst),
            0,
            "le garde est relâché même quand la connexion panique"
        );
    }

    // ── Limitation de débit ──────────────────────────────────────────────

    #[test]
    fn test_rate_limit_triggers_after_threshold() {
        let (path, shutdown, handle, _dir) = start_server();
        let (mut stream, mut reader) = connect(&path);
        let _hello = read_json(&mut reader);

        // 200 requêtes d'un coup, dans un seul écrit.
        let batch: String = (0..200)
            .map(|i| format!("{{\"v\":1,\"id\":\"r{i}\",\"type\":\"ping\"}}\n"))
            .collect();
        stream.write_all(batch.as_bytes()).expect("envoi du lot");
        stream.flush().expect("vidage du tampon");

        // On lit jusqu'au refus : le serveur ferme ensuite.
        // Le nombre exact de pongs dépend du moment où la fenêtre d'une seconde
        // se réarme (sur une machine très lente, elle peut basculer pendant le
        // lot) : on vérifie donc l'invariant solide — au moins une requête est
        // servie, puis le refus arrive.
        let mut pongs = 0usize;
        let mut seen_rate_limited = false;
        loop {
            let mut line = String::new();
            let n = reader.read_line(&mut line).expect("lecture");
            if n == 0 {
                break; // connexion fermée sans refus explicite
            }
            let value: serde_json::Value = serde_json::from_str(line.trim()).expect("JSON");
            match value["type"].as_str() {
                Some("pong") => pongs += 1,
                Some("error") => {
                    assert_eq!(value["code"], "rate_limited");
                    seen_rate_limited = true;
                    break;
                }
                other => panic!("type inattendu : {:?}", other),
            }
        }

        assert!(seen_rate_limited, "le refus rate_limited doit arriver");
        assert!(pongs >= 1, "les requêtes sous le plafond sont servies");
        assert!(
            pongs <= MAX_REQUESTS_PER_SECOND as usize,
            "la fenêtre n'a pas servi plus que le plafond : {pongs}"
        );

        // Puis la connexion est fermée. Deux manifestations sont correctes, et
        // une seule est possible selon ce qui reste dans la file de réception :
        //   - fin de flux propre (0 octet) si le client n'avait rien envoyé de plus ;
        //   - réinitialisation (RST) si des données reçues n'ont pas été lues.
        // Ici le client a vidé 150 requêtes d'avance : le noyau émet un RST en
        // fermant le socket. Exiger un EOF propre reviendrait à tester le noyau,
        // pas le contrat du daemon — on vérifie donc « c'est fermé, et pas autre chose ».
        let mut rest = String::new();
        match reader.read_line(&mut rest) {
            Ok(n) => assert_eq!(n, 0, "aucune réponse supplémentaire attendue"),
            Err(e) => assert_eq!(
                e.kind(),
                io::ErrorKind::ConnectionReset,
                "la fermeture doit se manifester par une fin de flux ou un RST : {e}"
            ),
        }

        stop_server(&shutdown, handle);
    }

    #[test]
    fn test_rate_limit_does_not_trigger_below_threshold() {
        let (path, shutdown, handle, _dir) = start_server();
        let (mut stream, mut reader) = connect(&path);
        let _hello = read_json(&mut reader);

        for i in 0..10 {
            send(
                &mut stream,
                &format!("{{\"v\":1,\"id\":\"n{i}\",\"type\":\"ping\"}}\n"),
            );
            let reply = read_json(&mut reader);
            assert_eq!(reply["type"], "pong");
            assert_eq!(reply["id"], format!("n{i}"));
        }

        // Onzième requête : la connexion doit toujours être ouverte et servir.
        send(&mut stream, "{\"v\":1,\"id\":\"n10\",\"type\":\"ping\"}\n");
        let reply = read_json(&mut reader);
        assert_eq!(reply["type"], "pong");
        assert_eq!(reply["id"], "n10");

        stop_server(&shutdown, handle);
    }

    // ── Durée de vie des connexions ──────────────────────────────────────
    //
    // Le délai d'écriture n'a pas de test unitaire : le déclencher demande de
    // remplir le tampon d'émission du noyau (SO_SNDBUF, ~200 Kio), ce que le
    // plafond de 50 requêtes par seconde interdit pendant la première seconde.
    // Un client ne peut donc pas faire bloquer le serveur en écriture sans
    // être d'abord refusé pour excès de débit. Le code existe (voir
    // `set_connection_timeouts`) mais reste couvert par la lecture du code, pas
    // par un test.

    #[test]
    fn test_idle_timeout_closes_connection() {
        let (path, shutdown, handle, _dir) = start_server_with_timeout(Duration::from_millis(150));
        let (_stream, mut reader) = connect(&path);
        let _hello = read_json(&mut reader);

        // Rien n'est envoyé : le serveur doit fermer de lui-même.
        thread::sleep(Duration::from_millis(400));

        let mut rest = String::new();
        let n = reader
            .read_line(&mut rest)
            .expect("lecture après expiration du délai");
        // Fin de flux PROPRE (0 octet) : le client n'a rien envoyé, donc la
        // file de réception du serveur était vide et la fermeture est un FIN,
        // pas un RST — contrairement au cas rate_limited du test précédent.
        assert_eq!(
            n, 0,
            "une connexion inactive doit être fermée par le serveur"
        );

        stop_server(&shutdown, handle);
    }

    #[test]
    fn test_idle_timeout_does_not_fire_while_active() {
        let (path, shutdown, handle, _dir) = start_server_with_timeout(Duration::from_millis(500));
        let (mut stream, mut reader) = connect(&path);
        let _hello = read_json(&mut reader);

        // Trois allers-retours espacés de 100 ms : chaque pause est très en
        // deçà du délai de 500 ms, mais la durée cumulée le dépasse. Si le
        // délai portait sur la DURÉE de la connexion et non sur l'inactivité,
        // la troisième requête ne recevrait jamais de réponse.
        for i in 0..3 {
            thread::sleep(Duration::from_millis(100));
            send(
                &mut stream,
                &format!("{{\"v\":1,\"id\":\"a{i}\",\"type\":\"ping\"}}\n"),
            );
            let reply = read_json(&mut reader);
            assert_eq!(reply["type"], "pong");
            assert_eq!(reply["id"], format!("a{i}"));
        }

        stop_server(&shutdown, handle);
    }

    // ── Nettoyage du fichier de socket ───────────────────────────────────

    #[test]
    fn test_remove_socket_file_removes_socket() {
        let dir = tempfile::tempdir().expect("dossier temporaire");
        let path = dir.path().join("daemon.sock");

        // Un listener lâché laisse son fichier derrière lui : c'est exactement
        // ce qu'un arrêt propre doit nettoyer.
        drop(bind_listener(&path).expect("bind"));
        assert!(path.exists(), "le fichier doit exister avant le nettoyage");

        remove_socket_file(&path).expect("un socket se supprime");
        assert!(!path.exists(), "le fichier de socket doit avoir disparu");
    }

    #[test]
    fn test_remove_socket_file_absent_is_ok() {
        let dir = tempfile::tempdir().expect("dossier temporaire");
        let path = dir.path().join("absent.sock");

        // Arrêt d'un daemon qui n'a jamais bindé : rien à faire, pas une erreur.
        remove_socket_file(&path).expect("un chemin absent est un succès");
    }

    #[test]
    fn test_remove_socket_file_refuses_non_socket() {
        let dir = tempfile::tempdir().expect("dossier temporaire");
        let path = dir.path().join("donnees.txt");
        fs::write(&path, b"contenu utilisateur").expect("écriture du fichier");

        let err = remove_socket_file(&path).expect_err("un fichier normal est refusé");
        assert_eq!(err.kind(), io::ErrorKind::AlreadyExists);
        assert!(
            err.to_string().contains("n'est pas un socket"),
            "message explicite attendu, obtenu : {}",
            err
        );
        assert_eq!(
            fs::read(&path).expect("lecture du fichier"),
            b"contenu utilisateur",
            "le contenu utilisateur doit être intact"
        );
    }

    // ── État partagé ─────────────────────────────────────────────────────

    #[test]
    fn test_serve_reads_shared_state() {
        let (state, path, shutdown, handle, _dir) =
            start_server_with_state(CONNECTION_IDLE_TIMEOUT);
        let (mut stream, mut reader) = connect(&path);
        let _hello = read_json(&mut reader);

        // Nouvelle valeur écrite APRÈS la construction du serveur : si
        // `handle_client` gardait une copie prise au démarrage, la réponse
        // porterait encore « balanced ».
        write_shared(&state, |s| {
            s.tier = "max".to_string();
            s.ram_bytes = 9 * 1024 * 1024 * 1024;
            s.seq = 7;
        });

        send(
            &mut stream,
            "{\"v\":1,\"id\":\"s1\",\"type\":\"status_request\"}\n",
        );
        let reply = read_json(&mut reader);

        assert_eq!(
            reply["tier"], "max",
            "l'état partagé est relu à chaque requête"
        );
        assert_eq!(
            reply["ram_mb"], 9216,
            "l'allocation publiée est la nouvelle"
        );
        assert_eq!(reply["seq"], 7);

        stop_server(&shutdown, handle);
    }

    /// Le test qui relie vraiment les deux moitiés : un tick écrit, le serveur
    /// le relit et le publie sur le fil.
    #[test]
    fn test_status_reflects_tick() {
        let (state, path, shutdown, handle, _dir) =
            start_server_with_state(CONNECTION_IDLE_TIMEOUT);
        let (mut stream, mut reader) = connect(&path);
        let _hello = read_json(&mut reader);

        // Un tick réel, sans application système.
        let mut engine = tick_engine();
        let outcome = run_tick(&mut engine, &state, 1_000_000, 5, false).expect("un tick aboutit");
        assert!(outcome.published);

        send(
            &mut stream,
            "{\"v\":1,\"id\":\"tk\",\"type\":\"status_request\"}\n",
        );
        let reply = read_json(&mut reader);

        assert_eq!(reply["type"], "status_response");
        assert_eq!(
            reply["seq"], 5,
            "le serveur publie exactement ce que le tick a écrit"
        );
        assert_eq!(
            reply["ram_bytes"], outcome.allocation.ram_bytes,
            "l'allocation vue par le client est celle du tick"
        );
        assert_eq!(
            reply["ram_mb"],
            outcome.allocation.ram_bytes / (1024 * 1024)
        );
        assert_eq!(
            reply["cpu_cores"],
            outcome.snapshot.cpu.per_core.len() as u64,
            "cpu_cores est le total physique du snapshot, pas une constante"
        );

        stop_server(&shutdown, handle);
    }

    #[test]
    fn test_status_response_contains_seq() {
        let (path, shutdown, handle, _dir) = start_server();
        let (mut stream, mut reader) = connect(&path);
        let _hello = read_json(&mut reader);

        send(
            &mut stream,
            "{\"v\":1,\"id\":\"q\",\"type\":\"status_request\"}\n",
        );
        let reply = read_json(&mut reader);

        assert_eq!(reply["type"], "status_response");
        assert!(reply["seq"].is_u64(), "le champ seq est publié : {reply}");
        assert_eq!(
            reply["seq"], 0,
            "un DaemonState neuf part de zéro : c'est l'appelant qui l'avance"
        );

        stop_server(&shutdown, handle);
    }

    #[test]
    fn test_publish_shared_survives_poisoned_lock() {
        let state = shared_state(DaemonState::new("balanced"));

        // Empoisonnement réel : un panic en tenant le verrou d'écriture.
        let writer = Arc::clone(&state);
        let outcome = panic::catch_unwind(AssertUnwindSafe(move || {
            let _guard = writer.write().expect("prise du verrou");
            panic!("panique volontaire du test");
        }));
        assert!(outcome.is_err());
        assert!(state.write().is_err(), "le verrou doit être empoisonné");

        // La publication doit passer quand même, et être relue telle quelle.
        let published = DaemonState {
            tier: "performance".to_string(),
            ram_bytes: 5 * 1024 * 1024 * 1024,
            seq: 123,
            ..DaemonState::new("balanced")
        };
        publish_shared(&state, published);

        let read_back = read_shared(&state);
        assert_eq!(
            read_back.tier, "performance",
            "un verrou empoisonné ne doit pas non plus bloquer la publication"
        );
        assert_eq!(read_back.ram_bytes, 5 * 1024 * 1024 * 1024);
        assert_eq!(read_back.seq, 123);
    }

    #[test]
    fn test_publish_shared_is_visible_to_next_read() {
        let state = shared_state(DaemonState::new("balanced"));

        publish_shared(
            &state,
            DaemonState {
                seq: 9,
                ..DaemonState::new("max")
            },
        );

        let read_back = read_shared(&state);
        assert_eq!(read_back.seq, 9);
        assert_eq!(read_back.tier, "max");
    }

    #[test]
    fn test_read_shared_survives_poisoned_lock() {
        let state = shared_state(DaemonState::new("balanced"));

        // Un panic en tenant le verrou d'écriture l'empoisonne (le thread
        // paniqué est ici isolé par `catch_unwind` pour que le test survive).
        let writer = Arc::clone(&state);
        let outcome = panic::catch_unwind(AssertUnwindSafe(move || {
            let _guard = writer.write().expect("prise du verrou");
            panic!("panique volontaire du test");
        }));
        assert!(outcome.is_err());
        assert!(state.read().is_err(), "le verrou doit être empoisonné");

        // La lecture doit quand même rendre l'instantané, sans `unwrap` fatal.
        let snapshot = read_shared(&state);
        assert_eq!(
            snapshot.tier, "balanced",
            "un verrou empoisonné ne doit pas geler l'état"
        );
        assert_eq!(snapshot.seq, 0);
        assert_eq!(snapshot.cpu_cores, 3);
    }

    // ── Réservations (ledger) ────────────────────────────────────────────
    //
    // L'état de test offre 7 Gio de marge (8 autorisés − 1 déjà alloué) et
    // 3 cœurs réservables (6 physiques à 50 %) : 5 Gio est accordable UNE
    // fois, jamais deux.

    /// Demande d'allocation de 5 Gio, 1 cœur.
    const ALLOC_5GIB: &str =
        "{\"v\":1,\"client\":\"t\",\"cpu_cores\":1,\"ram_gb\":5.0,\"type\":\"alloc_request\"}\n";

    #[test]
    fn test_alloc_second_client_denied() {
        let (path, shutdown, handle, _dir) = start_server();

        let (mut a, mut a_reader) = connect(&path);
        let _hello_a = read_json(&mut a_reader);
        send(&mut a, ALLOC_5GIB);
        let granted = read_json(&mut a_reader);
        assert_eq!(
            granted["type"], "alloc_granted",
            "le premier client est servi"
        );
        assert_eq!(granted["ram_gb"].as_f64(), Some(5.0));

        // Second client, demande identique : la même RAM ne peut pas être
        // accordée deux fois. Sans ledger, ce test échouait (deux accords).
        let (mut b, mut b_reader) = connect(&path);
        let _hello_b = read_json(&mut b_reader);
        send(&mut b, ALLOC_5GIB);
        let denied = read_json(&mut b_reader);
        assert_eq!(
            denied["type"], "alloc_denied",
            "la RAM déjà réservée ne doit pas être réaccordée"
        );
        assert_eq!(denied["reason"], "insufficient_headroom");

        // Et le refus n'est pas un blocage global : la marge NON réservée
        // (2 Gio) reste servie.
        send(
            &mut b,
            "{\"v\":1,\"client\":\"b\",\"cpu_cores\":1,\"ram_gb\":1.0,\"type\":\"alloc_request\"}\n",
        );
        let small = read_json(&mut b_reader);
        assert_eq!(
            small["type"], "alloc_granted",
            "le refus porte sur la demande, pas sur le client"
        );

        stop_server(&shutdown, handle);
    }

    #[test]
    fn test_alloc_freed_on_disconnect() {
        let (path, shutdown, handle, _dir) = start_server();

        let (mut a, mut a_reader) = connect(&path);
        let _hello_a = read_json(&mut a_reader);
        send(&mut a, ALLOC_5GIB);
        assert_eq!(read_json(&mut a_reader)["type"], "alloc_granted");

        // Fermeture BRUTALE : aucun `release` explicite — exactement ce que
        // fait un consommateur qui crashe. C'est le garde RAII qui doit rendre
        // la marge, pas la bonne volonté du client.
        drop(a);
        drop(a_reader);

        let (mut b, mut b_reader) = connect(&path);
        let _hello_b = read_json(&mut b_reader);

        // Le serveur peut mettre un instant à voir la fin de flux : on réessaie
        // jusqu'à un délai court au lieu de parier sur un `sleep` fixe, qui
        // rendrait ce test tantôt vert tantôt rouge selon la charge machine.
        let deadline = Instant::now() + Duration::from_secs(3);
        let reply = loop {
            send(&mut b, ALLOC_5GIB);
            let reply = read_json(&mut b_reader);
            if reply["type"] == "alloc_granted" || Instant::now() >= deadline {
                break reply;
            }
            thread::sleep(Duration::from_millis(25));
        };

        assert_eq!(
            reply["type"], "alloc_granted",
            "la réservation d'une connexion morte doit être libérée au drop"
        );

        stop_server(&shutdown, handle);
    }

    #[test]
    fn test_alloc_freed_on_release() {
        let (path, shutdown, handle, _dir) = start_server();

        let (mut a, mut a_reader) = connect(&path);
        let _hello_a = read_json(&mut a_reader);
        send(&mut a, ALLOC_5GIB);
        let granted = read_json(&mut a_reader);
        assert_eq!(granted["type"], "alloc_granted");
        let allocation_id = granted["allocation_id"]
            .as_str()
            .expect("un accord porte un allocation_id")
            .to_string();

        // Libération EXPLICITE, sur la même connexion : la marge doit revenir
        // tout de suite, sans attendre la fermeture.
        send(
            &mut a,
            &format!(
                "{{\"v\":1,\"id\":\"rel\",\"type\":\"release\",\"allocation_id\":\"{allocation_id}\"}}\n"
            ),
        );
        let released = read_json(&mut a_reader);
        assert_eq!(released["type"], "released");
        assert_eq!(released["allocation_id"], allocation_id.as_str());

        // La connexion `a` reste ouverte (et lâchera plus tard un release de
        // plus, sans effet) : la marge est déjà rendue.
        let (mut b, mut b_reader) = connect(&path);
        let _hello_b = read_json(&mut b_reader);
        send(&mut b, ALLOC_5GIB);
        let again = read_json(&mut b_reader);
        assert_eq!(
            again["type"], "alloc_granted",
            "release rend la marge immédiatement, sans attendre la fermeture"
        );

        stop_server(&shutdown, handle);
    }

    // ── Expiration des baux (DAEMON.1) ───────────────────────────────────
    //
    // Avant ce correctif, `expires_at` valait toujours `None` : un processus tué
    // sans rendre son bail gardait sa marge pour toujours, et le daemon
    // refusait ensuite des demandes que la machine pouvait satisfaire.

    #[test]
    fn test_expired_grant_is_swept_and_frees_its_share() {
        let mut ledger = Ledger::default();
        let id = ledger.reserve(
            "bail-echu".to_string(),
            1,
            2 * 1024 * 1024 * 1024,
            0,
            Some(1_000),
        );
        assert_eq!(ledger.reserved_ram_bytes, 2 * 1024 * 1024 * 1024);
        assert_eq!(ledger.reserved_cpu_cores, 1);

        // Une seconde avant l'échéance : rien ne bouge.
        assert!(ledger.sweep_expired(999).is_empty());
        assert_eq!(ledger.reserved_ram_bytes, 2 * 1024 * 1024 * 1024);

        // À l'échéance exacte : le bail tombe, les agrégats aussi.
        assert_eq!(ledger.sweep_expired(1_000), vec![id.clone()]);
        assert_eq!(ledger.reserved_ram_bytes, 0);
        assert_eq!(ledger.reserved_cpu_cores, 0);
        assert_eq!(ledger.len(), 0);
        assert!(!ledger.grants.contains_key(&id));
    }

    #[test]
    fn test_legacy_grant_gets_a_deadline_at_startup() {
        // Un ledger écrit avant ce correctif n'a pas d'échéance. Sans adoption,
        // le défaut survivrait exactement à la version qui le corrige.
        let mut ledger = Ledger::default();
        let id = ledger.reserve("herite".to_string(), 2, 1024 * 1024 * 1024, 0, None);
        assert_eq!(ledger.grants[&id].expires_at(), None);

        assert_eq!(ledger.adopt_legacy_grants(5_000, 60), 1);
        assert_eq!(ledger.grants[&id].expires_at(), Some(5_060));
        assert!(ledger.sweep_expired(5_059).is_empty());
        assert_eq!(ledger.sweep_expired(5_060), vec![id]);
    }

    #[test]
    fn test_legacy_ledger_file_is_still_readable() {
        // Format historique : un triplet par réservation. Refuser ce fichier
        // perdrait les baux en cours au moment de la mise à jour.
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("ledger-legacy.json");
        std::fs::write(
            &path,
            r#"{
              "grants": { "ancien": [2, 1073741824, 0] },
              "reserved_ram_bytes": 1073741824,
              "reserved_cpu_cores": 2,
              "reserved_storage_bytes": 0,
              "disambiguator": 0
            }"#,
        )
        .expect("écriture du ledger historique");

        let loaded = Ledger::load_from(&path);
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded.grants["ancien"].ram_bytes(), 1_073_741_824);
        assert_eq!(loaded.grants["ancien"].cores(), 2);
        assert_eq!(loaded.grants["ancien"].expires_at(), None);
    }

    #[test]
    fn test_alloc_reply_announces_a_real_expiry() {
        let st = DaemonState::new("balanced");
        let mut ledger = Ledger::default();
        let reply = dispatch(
            Request::Alloc {
                id: Some("expiry".to_string()),
                client: None,
                cpu_cores: 1,
                ram_gb: 0.25,
                gpu_vram_gb: None,
                storage_gb: None,
            },
            &st,
            &mut ledger,
        );

        match reply {
            Reply::AllocGranted {
                allocation_id,
                expires_at,
                ..
            } => {
                let at = expires_at.expect("une allocation doit annoncer son échéance");
                assert!(
                    at > now_unix(),
                    "l'échéance doit être dans le futur, obtenu {at} vs {}",
                    now_unix()
                );
                assert_eq!(ledger.grants[&allocation_id].expires_at(), Some(at));
            }
            other => panic!("allocation refusée alors que la marge est libre : {other:?}"),
        }
    }

    #[test]
    fn test_expiry_lets_a_new_client_take_the_freed_margin() {
        // Le refus doit être temporaire, pas définitif : après l'échéance, la
        // même demande doit passer. C'est la propriété produit de ce correctif.
        let st = DaemonState {
            // Palier 50 % de 16 Gio = 8 Gio autorisés, 7 Gio déjà alloués :
            // une demande de 1 Gio passe exactement, la suivante est refusée.
            ram_bytes: 7 * 1024 * 1024 * 1024,
            ..DaemonState::new("balanced")
        };
        let mut ledger = Ledger::default();
        let first = dispatch(
            Request::Alloc {
                id: None,
                client: Some("mort".to_string()),
                cpu_cores: 1,
                ram_gb: 1.0,
                gpu_vram_gb: None,
                storage_gb: None,
            },
            &st,
            &mut ledger,
        );
        assert!(matches!(first, Reply::AllocGranted { .. }));

        let blocked = dispatch(
            Request::Alloc {
                id: None,
                client: Some("vivant".to_string()),
                cpu_cores: 1,
                ram_gb: 1.0,
                gpu_vram_gb: None,
                storage_gb: None,
            },
            &st,
            &mut ledger,
        );
        assert_eq!(
            blocked,
            Reply::AllocDenied {
                id: None,
                reason: REASON_INSUFFICIENT_HEADROOM.to_string(),
            },
            "sans échéance respectée, le second client est refusé"
        );

        // L'horloge a avancé au-delà du bail : le balayage rend la marge.
        let horizon = now_unix() + lease_secs() as i64 + 1;
        assert_eq!(ledger.sweep_expired(horizon).len(), 1);

        let after = dispatch(
            Request::Alloc {
                id: None,
                client: Some("vivant".to_string()),
                cpu_cores: 1,
                ram_gb: 1.0,
                gpu_vram_gb: None,
                storage_gb: None,
            },
            &st,
            &mut ledger,
        );
        assert!(
            matches!(after, Reply::AllocGranted { .. }),
            "après l'échéance, la marge libérée doit être réaccordable : {after:?}"
        );
    }

    // ── Renouvellement du bail (DAEMON.3) ────────────────────────────────

    #[test]
    fn test_renew_pushes_the_deadline() {
        let mut ledger = Ledger::default();
        let id = ledger.reserve("vivant".to_string(), 1, 1024 * 1024 * 1024, 0, Some(1_000));

        // Renouvelé 100 s avant l'échéance, pour 200 s : la nouvelle échéance
        // (1 100) dépasse l'ancienne (1 000), donc le bail survit à l'instant
        // où il serait mort sans renouvellement.
        assert_eq!(ledger.renew(&id, 900, 200), Some(1_100));
        assert_eq!(ledger.grants[&id].expires_at(), Some(1_100));

        assert!(ledger.sweep_expired(1_000).is_empty());
        assert_eq!(ledger.sweep_expired(1_100), vec![id]);
    }

    #[test]
    fn test_renew_refuses_an_unknown_or_expired_lease() {
        let mut ledger = Ledger::default();
        assert_eq!(
            ledger.renew("jamais-accorde", 1_000, 60),
            None,
            "un identifiant inconnu ne doit pas créer de réservation"
        );
        assert_eq!(ledger.len(), 0);

        // Bail échu : le renouvellement ne le ressuscite pas, sinon les agrégats
        // rendus au pool seraient comptés deux fois.
        let id = ledger.reserve("mort".to_string(), 1, 1024 * 1024 * 1024, 0, Some(1_000));
        assert_eq!(ledger.sweep_expired(1_000).len(), 1);
        assert_eq!(ledger.renew(&id, 1_000, 60), None);
        assert_eq!(ledger.reserved_ram_bytes, 0);
    }

    #[test]
    fn test_dispatch_renew_replies_on_the_wire() {
        let st = DaemonState::new("balanced");
        let mut ledger = Ledger::default();
        let granted = dispatch(
            Request::Alloc {
                id: None,
                client: None,
                cpu_cores: 1,
                ram_gb: 0.25,
                gpu_vram_gb: None,
                storage_gb: None,
            },
            &st,
            &mut ledger,
        );
        let allocation_id = match granted {
            Reply::AllocGranted { allocation_id, .. } => allocation_id,
            other => panic!("allocation refusée : {other:?}"),
        };

        let renewed = dispatch(
            Request::Renew {
                id: Some("r1".to_string()),
                allocation_id: allocation_id.clone(),
            },
            &st,
            &mut ledger,
        );
        match renewed {
            Reply::Renewed { expires_at, .. } => {
                assert!(expires_at > now_unix(), "l'échéance renouvelée est future")
            }
            other => panic!("renouvellement refusé : {other:?}"),
        }

        // Le protocole refuse explicitement un bail inconnu, au lieu de répondre
        // « renewed » sur quelque chose qui n'existe pas.
        let refused = dispatch(
            Request::Renew {
                id: None,
                allocation_id: "fantome".to_string(),
            },
            &st,
            &mut ledger,
        );
        assert_eq!(
            refused,
            Reply::AllocDenied {
                id: None,
                reason: REASON_UNKNOWN_ALLOCATION.to_string(),
            }
        );
    }
}
