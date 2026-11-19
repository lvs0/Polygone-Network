//! Device Reuse — proposer un rôle à un appareil à partir de **mesures**.
//!
//! Rien ici n'active un rôle, ne réserve une ressource ni ne contacte un pair.
//! La fonction rend une proposition motivée : chaque rôle est soit éligible,
//! soit refusé avec la mesure qui l'a refusé. Une mesure absente
//! (`Option::None`) n'est jamais interprétée comme favorable : elle est
//! signalée comme non évaluée, ce qui interdit de lire « aucune contrainte
//! connue » comme « appareil adapté ».
//!
//! Les seuils sont des choix produit, pas des mesures : ils sont publics et
//! regroupés ici pour qu'une modification se voie en revue.

use serde::{Deserialize, Serialize};

/// Un mébioctet, pour exprimer les seuils sans facteur recopié.
pub const MIB: u64 = 1024 * 1024;
/// Un gibioctet.
pub const GIB: u64 = 1024 * MIB;

/// Mesures d'un appareil candidat à un rôle.
///
/// Chaque `Option` signifie « non mesuré », jamais « zéro » : un stockage
/// inconnu ne doit pas produire un refus, ni une acceptation.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct DeviceCapabilities {
    /// Cœurs physiques mesurés.
    pub cpu_cores: u32,
    /// RAM totale mesurée.
    pub ram_bytes: u64,
    /// Espace libre mesuré sur le volume de données.
    pub free_storage_bytes: Option<u64>,
    /// VRAM libre mesurée, si un GPU est présent.
    pub free_vram_bytes: Option<u64>,
    /// Niveau de batterie mesuré, si l'appareil en a une.
    pub battery_percent: Option<u8>,
    /// Température mesurée, si un capteur est lisible.
    pub thermal_celsius: Option<u8>,
    /// `Some(true)` : quelqu'un utilise la machine maintenant.
    pub user_active: Option<bool>,
}

/// Rôles qu'un appareil peut rendre au réseau.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeviceRole {
    /// Relais aveugle : faible RAM, aucun stockage durable.
    Relay,
    /// Supervision en lecture seule : le rôle le plus léger.
    Supervision,
    /// Conservation de blocs : demande de l'espace libre stable.
    Storage,
    /// Calcul déterministe de petite taille : ne doit pas dégrader la machine.
    LightCompute,
}

impl DeviceRole {
    /// Tous les rôles, dans l'ordre d'affichage d'un diagnostic.
    pub const ALL: [Self; 4] = [
        Self::Relay,
        Self::Supervision,
        Self::Storage,
        Self::LightCompute,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Relay => "relais",
            Self::Supervision => "supervision",
            Self::Storage => "stockage",
            Self::LightCompute => "calcul léger",
        }
    }
}

/// Proposition motivée pour un rôle.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RoleSuggestion {
    pub role: DeviceRole,
    pub eligible: bool,
    /// Motifs, dans l'ordre : les refus d'abord, puis les mesures absentes.
    pub reasons: Vec<String>,
}

/// RAM minimale pour servir de relais (transport seul).
pub const MIN_RELAY_RAM: u64 = 256 * MIB;
/// RAM minimale pour un superviseur en lecture seule.
pub const MIN_SUPERVISION_RAM: u64 = 128 * MIB;
/// Espace libre minimal pour proposer la conservation de blocs.
pub const MIN_STORAGE_FREE: u64 = 20 * GIB;
/// Cœurs physiques minimaux pour proposer du calcul.
pub const MIN_COMPUTE_CORES: u32 = 2;
/// RAM minimale pour proposer du calcul.
pub const MIN_COMPUTE_RAM: u64 = 2 * GIB;
/// Batterie en dessous de laquelle le calcul est refusé.
pub const MIN_BATTERY_PERCENT: u8 = 20;
/// Température à partir de laquelle le calcul est refusé.
pub const MAX_THERMAL_CELSIUS: u8 = 80;

/// Propose un rôle pour chaque [`DeviceRole`], sans en activer aucun.
///
/// L'ordre est stable (`DeviceRole::ALL`) pour qu'un rapport se lise toujours
/// de la même façon.
pub fn suggest_roles(capabilities: &DeviceCapabilities) -> Vec<RoleSuggestion> {
    DeviceRole::ALL
        .into_iter()
        .map(|role| match role {
            DeviceRole::Relay => relay(capabilities),
            DeviceRole::Supervision => supervision(capabilities),
            DeviceRole::Storage => storage(capabilities),
            DeviceRole::LightCompute => light_compute(capabilities),
        })
        .collect()
}

fn relay(capabilities: &DeviceCapabilities) -> RoleSuggestion {
    let mut reasons = Vec::new();
    let mut eligible = true;
    if capabilities.cpu_cores == 0 {
        eligible = false;
        reasons.push("aucun cœur CPU mesuré".to_string());
    }
    if capabilities.ram_bytes < MIN_RELAY_RAM {
        eligible = false;
        reasons.push(format!(
            "RAM mesurée insuffisante : {} Mio < {} Mio",
            capabilities.ram_bytes / MIB,
            MIN_RELAY_RAM / MIB
        ));
    }
    if eligible {
        reasons.push(format!(
            "{} cœur(s) et {} Mio de RAM mesurés",
            capabilities.cpu_cores,
            capabilities.ram_bytes / MIB
        ));
    }
    RoleSuggestion {
        role: DeviceRole::Relay,
        eligible,
        reasons,
    }
}

fn supervision(capabilities: &DeviceCapabilities) -> RoleSuggestion {
    let mut reasons = Vec::new();
    let mut eligible = true;
    if capabilities.ram_bytes < MIN_SUPERVISION_RAM {
        eligible = false;
        reasons.push(format!(
            "RAM mesurée insuffisante : {} Mio < {} Mio",
            capabilities.ram_bytes / MIB,
            MIN_SUPERVISION_RAM / MIB
        ));
    }
    if eligible {
        reasons.push(format!(
            "lecture seule, {} Mio de RAM mesurés",
            capabilities.ram_bytes / MIB
        ));
    }
    RoleSuggestion {
        role: DeviceRole::Supervision,
        eligible,
        reasons,
    }
}

fn storage(capabilities: &DeviceCapabilities) -> RoleSuggestion {
    match capabilities.free_storage_bytes {
        None => RoleSuggestion {
            role: DeviceRole::Storage,
            eligible: false,
            reasons: vec!["stockage libre non mesuré — contrainte non évaluée".to_string()],
        },
        Some(free) if free < MIN_STORAGE_FREE => RoleSuggestion {
            role: DeviceRole::Storage,
            eligible: false,
            reasons: vec![format!(
                "espace libre mesuré insuffisant : {} Gio < {} Gio",
                free / GIB,
                MIN_STORAGE_FREE / GIB
            )],
        },
        Some(free) => RoleSuggestion {
            role: DeviceRole::Storage,
            eligible: true,
            reasons: vec![format!(
                "{} Gio libres mesurés (seuil {} Gio)",
                free / GIB,
                MIN_STORAGE_FREE / GIB
            )],
        },
    }
}

fn light_compute(capabilities: &DeviceCapabilities) -> RoleSuggestion {
    let mut reasons = Vec::new();
    let mut eligible = true;

    if capabilities.cpu_cores < MIN_COMPUTE_CORES {
        eligible = false;
        reasons.push(format!(
            "cœurs mesurés insuffisants : {} < {}",
            capabilities.cpu_cores, MIN_COMPUTE_CORES
        ));
    }
    if capabilities.ram_bytes < MIN_COMPUTE_RAM {
        eligible = false;
        reasons.push(format!(
            "RAM mesurée insuffisante : {} Mio < {} Mio",
            capabilities.ram_bytes / MIB,
            MIN_COMPUTE_RAM / MIB
        ));
    }
    match capabilities.user_active {
        Some(true) => {
            eligible = false;
            reasons.push("activité utilisateur mesurée — le calcul dégraderait la machine".into());
        }
        Some(false) => {}
        None => reasons.push("activité utilisateur non mesurée — contrainte non évaluée".into()),
    }
    match capabilities.battery_percent {
        Some(percent) if percent < MIN_BATTERY_PERCENT => {
            eligible = false;
            reasons.push(format!(
                "batterie mesurée basse : {percent} % < {MIN_BATTERY_PERCENT} %"
            ));
        }
        _ => {}
    }
    match capabilities.thermal_celsius {
        Some(celsius) if celsius >= MAX_THERMAL_CELSIUS => {
            eligible = false;
            reasons.push(format!(
                "température mesurée élevée : {celsius} °C ≥ {MAX_THERMAL_CELSIUS} °C"
            ));
        }
        _ => {}
    }
    if capabilities.free_vram_bytes == Some(0) {
        reasons.push("VRAM libre mesurée nulle — calcul GPU non proposé".into());
    }
    if eligible {
        reasons.push(format!(
            "{} cœur(s) et {} Mio de RAM mesurés",
            capabilities.cpu_cores,
            capabilities.ram_bytes / MIB
        ));
    }

    RoleSuggestion {
        role: DeviceRole::LightCompute,
        eligible,
        reasons,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn machine(cores: u32, ram_gib: u64) -> DeviceCapabilities {
        DeviceCapabilities {
            cpu_cores: cores,
            ram_bytes: ram_gib * GIB,
            ..Default::default()
        }
    }

    /// Un appareil correct n'est pas refusé pour du calcul seulement parce que
    /// la batterie ou la température ne sont pas lisibles ; le rapport le dit.
    #[test]
    fn unmeasured_constraints_do_not_claim_a_refusal() {
        let roles = suggest_roles(&machine(4, 8));
        let compute = roles
            .iter()
            .find(|suggestion| suggestion.role == DeviceRole::LightCompute)
            .expect("le rôle de calcul est toujours évalué");
        assert!(compute.eligible);
        assert!(
            compute
                .reasons
                .iter()
                .any(|reason| reason.contains("non mesurée")),
            "{:?}",
            compute.reasons
        );
    }

    /// Un stockage non mesuré ne devient jamais un rôle de stockage : une
    /// promesse sans mesure n'est pas une proposition.
    #[test]
    fn storage_without_a_measurement_is_refused() {
        let roles = suggest_roles(&machine(4, 8));
        let storage = roles
            .iter()
            .find(|suggestion| suggestion.role == DeviceRole::Storage)
            .expect("le rôle de stockage est toujours évalué");
        assert!(!storage.eligible);
        assert!(
            storage.reasons[0].contains("non mesuré"),
            "{:?}",
            storage.reasons
        );
    }

    /// L'espace mesuré sous le seuil refuse le rôle, avec la valeur.
    #[test]
    fn measured_storage_below_the_threshold_is_refused() {
        let capabilities = DeviceCapabilities {
            free_storage_bytes: Some(5 * GIB),
            ..machine(4, 8)
        };
        let roles = suggest_roles(&capabilities);
        let storage = roles
            .iter()
            .find(|suggestion| suggestion.role == DeviceRole::Storage)
            .unwrap();
        assert!(!storage.eligible);
        assert!(
            storage.reasons[0].contains("5 Gio"),
            "{:?}",
            storage.reasons
        );
    }

    /// Un appareil ancien à deux cœurs reste un relais et un superviseur,
    /// mais pas un nœud de calcul.
    #[test]
    fn a_small_device_is_only_a_relay_and_a_supervisor() {
        let roles = suggest_roles(&machine(1, 1));
        let eligible: Vec<DeviceRole> = roles
            .iter()
            .filter(|suggestion| suggestion.eligible)
            .map(|suggestion| suggestion.role)
            .collect();
        assert_eq!(eligible, vec![DeviceRole::Relay, DeviceRole::Supervision]);
        let compute = roles
            .iter()
            .find(|suggestion| suggestion.role == DeviceRole::LightCompute)
            .unwrap();
        assert!(compute
            .reasons
            .iter()
            .any(|reason| reason.contains("cœurs")));
        assert!(compute.reasons.iter().any(|reason| reason.contains("RAM")));
    }

    /// Deux contraintes dynamiques mesurées refusent le calcul, séparément.
    #[test]
    fn battery_and_heat_refuse_compute() {
        let capabilities = DeviceCapabilities {
            battery_percent: Some(7),
            thermal_celsius: Some(91),
            ..machine(8, 16)
        };
        let roles = suggest_roles(&capabilities);
        let compute = roles
            .iter()
            .find(|suggestion| suggestion.role == DeviceRole::LightCompute)
            .unwrap();
        assert!(!compute.eligible);
        assert!(compute
            .reasons
            .iter()
            .any(|reason| reason.contains("batterie")));
        assert!(compute
            .reasons
            .iter()
            .any(|reason| reason.contains("température")));
    }

    /// Une machine utilisée maintenant n'est pas proposée pour du calcul,
    /// même si ses ressources statiques le permettraient.
    #[test]
    fn user_activity_refuses_compute() {
        let capabilities = DeviceCapabilities {
            user_active: Some(true),
            ..machine(8, 16)
        };
        let roles = suggest_roles(&capabilities);
        let compute = roles
            .iter()
            .find(|suggestion| suggestion.role == DeviceRole::LightCompute)
            .unwrap();
        assert!(!compute.eligible);
        assert!(compute.reasons[0].contains("activité utilisateur"));
    }

    /// Un appareil inconnu (aucune mesure) n'est proposé pour rien, et le
    /// rapport ne prétend pas le contraire.
    #[test]
    fn a_default_device_has_no_eligible_role() {
        let roles = suggest_roles(&DeviceCapabilities::default());
        assert!(roles.iter().all(|suggestion| !suggestion.eligible));
        assert_eq!(roles.len(), DeviceRole::ALL.len());
    }

    /// L'ordre ne dépend pas des mesures : un rapport se lit toujours pareil.
    #[test]
    fn role_order_is_stable() {
        let first: Vec<DeviceRole> = suggest_roles(&machine(8, 16))
            .into_iter()
            .map(|suggestion| suggestion.role)
            .collect();
        let second: Vec<DeviceRole> = suggest_roles(&DeviceCapabilities::default())
            .into_iter()
            .map(|suggestion| suggestion.role)
            .collect();
        assert_eq!(first, second);
        assert_eq!(first, DeviceRole::ALL.to_vec());
    }
}
