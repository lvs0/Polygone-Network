//! Modèle de nœud et enveloppe de tâche.
//!
//! Deux périmètres distincts, et aucun partage global automatique :
//!
//! - [`Scope::Local`] : le nœud n'accepte que les tâches de son propre
//!   `local_fabric` (un identifiant de réseau local, pas une promesse de
//!   sécurité) ;
//! - [`Scope::Global`] : le nœud doit avoir `global_consent = true` **et** la
//!   requête doit porter `global_consent = true`. Deux oublis valent un refus.
//!
//! Les ressources sont ce que le nœud **offre**, pas ce qu'il possède. Un
//! appelant qui veut connaître la machine lit `cpu_cores` / `ram_bytes` comme
//! une capacité offerte ; le daemon reste seul juge de ce qu'il prête.

use polygone_core::task::TaskSpec;
use serde::{Deserialize, Serialize};

/// Longueur maximale d'un identifiant de nœud.
pub const MAX_NODE_ID_LEN: usize = 128;
/// Longueur maximale d'un nom de réseau local.
pub const MAX_FABRIC_NAME_LEN: usize = 64;
/// Batterie sous laquelle un nœud refuse les tâches distantes.
pub const MIN_BATTERY_PERCENT: u8 = 20;
/// Température au-dessus de laquelle un nœud refuse les tâches distantes.
pub const MAX_THERMAL_CELSIUS: u8 = 85;
/// Charge au-dessus de laquelle un nœud refuse les nouvelles tâches distantes.
pub const MAX_REMOTE_LOAD_PERMILLE: u16 = 950;

/// Version du protocole de fil. Une version inconnue est refusée.
pub const WIRE_VERSION: u8 = 1;

/// Périmètre dans lequel une tâche peut s'exécuter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Scope {
    /// Réseau privé : même `local_fabric`, aucune économie de crédits.
    Local,
    /// Réseau global : consentement explicite des deux côtés.
    Global,
}

impl Scope {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Local => "local",
            Self::Global => "global",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "local" => Some(Self::Local),
            "global" => Some(Self::Global),
            _ => None,
        }
    }
}

/// Ce qu'un nœud offre au réseau, à un instant donné.
///
/// `load_permille` et `failure_rate_permille` sont des mesures ou des
/// déclarations **bornées** ; une valeur absente doit rester absente
/// (`Option`), jamais devenir un zéro silencieux qui ferait croire à une
/// mesure.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeCapabilities {
    pub node_id: String,
    pub scope: Scope,
    /// Consentement explicite au périmètre global. `false` par défaut.
    pub global_consent: bool,
    /// Identifiant du réseau local (ex. `maison`), pas un secret.
    pub local_fabric: String,
    /// Cœurs **offerts** aux tâches.
    pub cpu_cores: u32,
    /// RAM **offerte** aux tâches.
    pub ram_bytes: u64,
    /// Stockage **offert** aux tâches.
    pub storage_bytes: u64,
    /// VRAM **offerte** aux tâches.
    pub gpu_vram_bytes: u64,
    /// Charge observée, en millièmes (0 = inactif, 1000 = saturé).
    pub load_permille: u16,
    /// Batterie, si un capteur existe. `None` = inconnue, pas « pleine ».
    pub battery_percent: Option<u8>,
    /// Température, si un capteur existe. `None` = inconnue.
    pub thermal_celsius: Option<u8>,
    /// L'utilisateur travaille : la priorité absolue lui revient.
    pub user_active: bool,
    /// Latence estimée vers ce nœud, en millisecondes.
    pub rtt_ms: u32,
    /// Débit utile estimé, en kbit/s.
    pub bandwidth_kbps: u32,
    /// Taux d'échec observé ou déclaré, en millièmes.
    pub failure_rate_permille: u16,
    /// Le nœud accepte-t-il de nouvelles tâches ?
    pub available: bool,
}

impl NodeCapabilities {
    /// Valide les bornes. Une capacité hors bornes est refusée, jamais corrigée
    /// silencieusement.
    pub fn validate(&self) -> Result<(), String> {
        if self.node_id.trim().is_empty() || self.node_id.len() > MAX_NODE_ID_LEN {
            return Err("node_id vide ou trop long".to_string());
        }
        if self.local_fabric.trim().is_empty() || self.local_fabric.len() > MAX_FABRIC_NAME_LEN {
            return Err("local_fabric vide ou trop long".to_string());
        }
        if self.load_permille > 1000 {
            return Err("load_permille hors bornes".to_string());
        }
        if self.failure_rate_permille > 1000 {
            return Err("failure_rate_permille hors bornes".to_string());
        }
        if let Some(battery) = self.battery_percent {
            if battery > 100 {
                return Err("battery_percent hors bornes".to_string());
            }
        }
        if let Some(thermal) = self.thermal_celsius {
            if thermal > 150 {
                return Err("thermal_celsius hors bornes".to_string());
            }
        }
        Ok(())
    }

    /// Millièmes de cœur réellement disponibles.
    ///
    /// 1 cœur à 0 % de charge = 1000 millièmes ; 8 cœurs à 50 % = 4000.
    pub fn available_cpu_millicores(&self) -> u32 {
        let free = 1000u32.saturating_sub(u32::from(self.load_permille.min(1000)));
        self.cpu_cores.saturating_mul(free)
    }

    /// Ressources suffisantes pour la tâche. Ne juge ni l'activité utilisateur
    /// ni la batterie : c'est la partie commune aux exécutions locale et
    /// distante.
    fn resources_allow(&self, task: &TaskSpec) -> Result<(), String> {
        if !self.available {
            return Err("nœud indisponible".to_string());
        }
        let required_millicores = task.requirements.cpu_cores.saturating_mul(1000);
        if self.available_cpu_millicores() < required_millicores {
            return Err(format!(
                "CPU insuffisant : {} millièmes disponibles, {} requis",
                self.available_cpu_millicores(),
                required_millicores
            ));
        }
        if self.ram_bytes < task.requirements.ram_bytes {
            return Err("RAM insuffisante".to_string());
        }
        if self.storage_bytes < task.requirements.storage_bytes {
            return Err("stockage insuffisant".to_string());
        }
        if self.gpu_vram_bytes < task.requirements.gpu_vram_bytes {
            return Err("VRAM insuffisante".to_string());
        }
        Ok(())
    }

    /// Une tâche demandée par l'utilisateur de cette machine peut s'exécuter
    /// localement dès que les ressources le permettent : l'activité de
    /// l'utilisateur ne bloque pas sa propre demande.
    pub fn can_run_locally(&self, task: &TaskSpec) -> Result<(), String> {
        self.resources_allow(task)
    }

    /// Une tâche **distante** est refusée si l'utilisateur travaille, si la
    /// batterie est basse, si la machine chauffe ou si la charge est extrême.
    /// C'est le contrat « la priorité absolue est l'utilisation personnelle ».
    pub fn can_accept_remote(&self, task: &TaskSpec) -> Result<(), String> {
        if self.user_active {
            return Err("utilisateur actif : priorité absolue".to_string());
        }
        if let Some(battery) = self.battery_percent {
            if battery < MIN_BATTERY_PERCENT {
                return Err(format!("batterie basse ({battery} %)"));
            }
        }
        if let Some(thermal) = self.thermal_celsius {
            if thermal > MAX_THERMAL_CELSIUS {
                return Err(format!("température élevée ({thermal} °C)"));
            }
        }
        if self.load_permille > MAX_REMOTE_LOAD_PERMILLE {
            return Err(format!("charge trop élevée ({} ‰)", self.load_permille));
        }
        self.resources_allow(task)
    }
}

/// Tout ce qu'un nœud reçoit pour exécuter une tâche.
///
/// `requester` est informatif : il n'authentifie rien. L'authentification du
/// producteur se fait sur le **résultat** signé, pas sur la demande. C'est une
/// limite assumée du prototype (voir `docs/prototypes/FABRIC-2026-11-14.md`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskEnvelope {
    pub v: u8,
    pub scope: Scope,
    /// Consentement du demandeur au périmètre global.
    pub global_consent: bool,
    /// Réseau local attendu quand `scope == Local`.
    pub fabric: String,
    pub requester: String,
    pub task: TaskSpec,
}

impl TaskEnvelope {
    pub fn validate(&self) -> Result<(), String> {
        if self.v != WIRE_VERSION {
            return Err(format!("version de protocole inconnue : {}", self.v));
        }
        if self.fabric.trim().is_empty() || self.fabric.len() > MAX_FABRIC_NAME_LEN {
            return Err("fabric vide ou trop long".to_string());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use polygone_core::task::{
        TaskConstraints, TaskId, TaskKind, TaskPermissions, TaskRequirements,
    };

    const NOW: u64 = 1_800_000_000;

    fn task(cores: u32, ram: u64) -> TaskSpec {
        TaskSpec {
            id: TaskId::parse("task_model_1").unwrap(),
            owner: "owner".to_string(),
            kind: TaskKind::Echo,
            input: b"hello".to_vec(),
            requirements: TaskRequirements {
                cpu_cores: cores,
                ram_bytes: ram,
                storage_bytes: 0,
                gpu_vram_bytes: 0,
            },
            constraints: TaskConstraints {
                deadline_unix: NOW + 300,
                max_attempts: 2,
                permissions: TaskPermissions::default(),
            },
        }
    }

    fn node() -> NodeCapabilities {
        NodeCapabilities {
            node_id: "node-a".to_string(),
            scope: Scope::Local,
            global_consent: false,
            local_fabric: "maison".to_string(),
            cpu_cores: 4,
            ram_bytes: 1024 * 1024 * 1024,
            storage_bytes: 0,
            gpu_vram_bytes: 0,
            load_permille: 500,
            battery_percent: None,
            thermal_celsius: None,
            user_active: false,
            rtt_ms: 5,
            bandwidth_kbps: 100_000,
            failure_rate_permille: 0,
            available: true,
        }
    }

    #[test]
    fn available_cpu_is_a_measure_not_a_claim() {
        let mut n = node();
        assert_eq!(n.available_cpu_millicores(), 2000);
        n.load_permille = 1000;
        assert_eq!(n.available_cpu_millicores(), 0);
        assert!(n.can_run_locally(&task(1, 1)).is_err());
    }

    #[test]
    fn user_activity_blocks_remote_but_not_local() {
        let mut n = node();
        n.user_active = true;
        let t = task(1, 1024);
        assert!(
            n.can_run_locally(&t).is_ok(),
            "la demande locale reste permise"
        );
        assert!(
            n.can_accept_remote(&t).is_err(),
            "une tâche distante cède la priorité à l'utilisateur"
        );
    }

    #[test]
    fn low_battery_and_heat_refuse_remote_tasks() {
        let mut n = node();
        n.battery_percent = Some(19);
        assert!(n.can_accept_remote(&task(1, 1024)).is_err());
        n.battery_percent = Some(80);
        n.thermal_celsius = Some(86);
        assert!(n.can_accept_remote(&task(1, 1024)).is_err());
        n.thermal_celsius = Some(60);
        assert!(n.can_accept_remote(&task(1, 1024)).is_ok());
    }

    #[test]
    fn out_of_range_capabilities_are_rejected_not_clamped() {
        let mut n = node();
        n.load_permille = 1001;
        assert!(n.validate().is_err());
        n.load_permille = 0;
        n.global_consent = true;
        n.scope = Scope::Global;
        n.failure_rate_permille = 1001;
        assert!(n.validate().is_err());
    }

    #[test]
    fn wire_version_is_checked() {
        let envelope = TaskEnvelope {
            v: 2,
            scope: Scope::Local,
            global_consent: false,
            fabric: "maison".to_string(),
            requester: "node-a".to_string(),
            task: task(1, 1024),
        };
        assert!(envelope.validate().is_err());
    }
}
