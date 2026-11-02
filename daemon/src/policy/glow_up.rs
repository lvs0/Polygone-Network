//! Glow-Up System — Intelligent Resource Allocation Policy
//!
//! The "brain" of polygoned. Translates tier + safety + real-time state
//! into concrete allocations. Lightweight, deterministic, no ML.

use crate::allocator::Allocation;
use crate::system::SystemSnapshot;
use crate::Platform;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

// ============================================================================
// Policy types — self-contained here, re-exported via lib.rs for embedders
// ============================================================================

/// Allocation tiers — user-facing presets
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
pub enum AllocationTier {
    Eco,
    #[default]
    Balanced,
    Performance,
    Max,
    Custom,
}

impl AllocationTier {
    pub fn as_str(&self) -> &'static str {
        match self {
            AllocationTier::Eco => "eco",
            AllocationTier::Balanced => "balanced",
            AllocationTier::Performance => "performance",
            AllocationTier::Max => "max",
            AllocationTier::Custom => "custom",
        }
    }
}

impl std::fmt::Display for AllocationTier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl std::str::FromStr for AllocationTier {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "eco" => Ok(AllocationTier::Eco),
            "balanced" => Ok(AllocationTier::Balanced),
            "performance" => Ok(AllocationTier::Performance),
            "max" => Ok(AllocationTier::Max),
            "custom" => Ok(AllocationTier::Custom),
            _ => Err(format!("unknown tier: {}", s)),
        }
    }
}

/// Resource limits within a tier (max utilisation percentages)
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct ResourceLimits {
    pub max_cpu_percent: u8,
    pub max_ram_percent: u8,
    pub max_bandwidth_percent: u8,
    pub max_gpu_percent: u8,
}

/// Safety floors — minimum free resources the system must retain
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct SafetyMargins {
    pub min_free_ram_gb: f32,
    pub min_free_cpu_cores: usize,
    pub min_free_vram_mb: u32,
    pub max_cpu_percent: u8,
}

impl Default for SafetyMargins {
    /// Les garde-fous PRODUIT (pas des zéros) : une config partielle ou
    /// legacy ne doit jamais lever les planchers de sécurité.
    fn default() -> Self {
        SafetyMargins {
            min_free_ram_gb: 4.0,
            min_free_cpu_cores: 1,
            min_free_vram_mb: 512,
            max_cpu_percent: 85,
        }
    }
}

/// A named tier preset
#[derive(Debug, Clone, Copy)]
pub struct TierPreset {
    pub name: &'static str,
    pub cpu_pct: u8,
    pub ram_pct: u8,
    pub bandwidth_pct: u8,
    pub gpu_pct: u8,
}

/// All four named presets
pub const TIER_PRESETS: [(&str, TierPreset); 4] = [
    (
        "eco",
        TierPreset {
            name: "eco",
            cpu_pct: 25,
            ram_pct: 30,
            bandwidth_pct: 40,
            gpu_pct: 20,
        },
    ),
    (
        "balanced",
        TierPreset {
            name: "balanced",
            cpu_pct: 50,
            ram_pct: 50,
            bandwidth_pct: 60,
            gpu_pct: 40,
        },
    ),
    (
        "performance",
        TierPreset {
            name: "performance",
            cpu_pct: 75,
            ram_pct: 70,
            bandwidth_pct: 80,
            gpu_pct: 60,
        },
    ),
    (
        "max",
        TierPreset {
            name: "max",
            cpu_pct: 90,
            ram_pct: 85,
            bandwidth_pct: 95,
            gpu_pct: 80,
        },
    ),
];

/// Behavioural tuning — hysteresis, step sizes, user activity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BehaviorConfig {
    pub grow_step_pct: u8,
    pub shrink_step_pct: u8,
    pub shrink_hysteresis_ticks: u32,
    pub throttle_on_user_activity: bool,
    pub tick_interval_secs: u64,
}

impl Default for BehaviorConfig {
    fn default() -> Self {
        BehaviorConfig {
            grow_step_pct: 10,
            shrink_step_pct: 5,
            shrink_hysteresis_ticks: 5,
            throttle_on_user_activity: true,
            tick_interval_secs: 5,
        }
    }
}

/// Désérialise `tier` en acceptant les DEUX formes historiques :
/// - forme actuelle (scalaire) : `tier = "Balanced"`
/// - forme legacy (table) : `[tier] tier = "Balanced"` — écrite par les
///   versions antérieures (dont l'ancien `--gen-config` qui produisait
///   `[tier] name = "…"`).
///
/// D8 : un daemon doit lire la config d'une version précédente, pas exiger
/// la régénération (zéro friction, robustesse à la frontière).
fn deserialize_tier<'de, D>(deserializer: D) -> Result<AllocationTier, D::Error>
where
    D: serde::Deserializer<'de>,
{
    struct TierVisitor;

    impl<'de> serde::de::Visitor<'de> for TierVisitor {
        type Value = AllocationTier;

        fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(
                f,
                "a tier name (\"Balanced\", \"Eco\", …) or a legacy `[tier]` table"
            )
        }

        fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<Self::Value, E> {
            v.parse().map_err(E::custom)
        }

        fn visit_string<E: serde::de::Error>(self, v: String) -> Result<Self::Value, E> {
            v.parse().map_err(E::custom)
        }

        fn visit_map<A: serde::de::MapAccess<'de>>(
            self,
            mut map: A,
        ) -> Result<Self::Value, A::Error> {
            let mut value: Option<String> = None;
            while let Some(key) = map.next_key::<String>()? {
                match key.as_str() {
                    "tier" | "name" => value = Some(map.next_value()?),
                    _ => {
                        map.next_value::<serde::de::IgnoredAny>()?;
                    }
                }
            }
            let Some(value) = value else {
                return Err(serde::de::Error::custom("missing `tier` in [tier] table"));
            };
            value.parse().map_err(serde::de::Error::custom)
        }
    }

    deserializer.deserialize_any(TierVisitor)
}

/// Daemon configuration — tier, limits, safety, behaviour
///
/// Rétro-compat D8 : les champs absents du format legacy (écrit par les
/// versions antérieures) se replient sur les defaults PRODUIT — jamais
/// des zéros — pour qu'une config partielle ne lève ni les garde-fous
/// de sécurité ni les fonctionnalités.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonConfig {
    #[serde(deserialize_with = "deserialize_tier")]
    pub tier: AllocationTier,
    pub custom_limits: Option<ResourceLimits>,
    #[serde(default)]
    pub behavior: BehaviorConfig,
    #[serde(default)]
    pub safety: SafetyMargins,
    /// Platform-specific feature toggles
    #[serde(default = "default_true")]
    pub memory_limit_enabled: bool,
    #[serde(default = "default_true")]
    pub bandwidth_shaping: bool,
    #[serde(default = "default_true")]
    pub gpu_allocation_enabled: bool,
    #[serde(default = "default_true")]
    pub service_integration: bool,
}

/// Les toggles produit sont ACTIVÉS par défaut (une config legacy qui ne
/// les mentionne pas ne doit pas désactiver silencieusement des
/// fonctionnalités).
fn default_true() -> bool {
    true
}

impl Default for DaemonConfig {
    fn default() -> Self {
        DaemonConfig {
            tier: AllocationTier::Balanced,
            custom_limits: None,
            behavior: BehaviorConfig::default(),
            safety: SafetyMargins::default(),
            memory_limit_enabled: true,
            bandwidth_shaping: true,
            gpu_allocation_enabled: true,
            service_integration: true,
        }
    }
}

impl DaemonConfig {
    /// Resolve effective limits (custom overrides tier defaults)
    pub fn effective_limits(&self, _snap: &SystemSnapshot) -> ResourceLimits {
        self.custom_limits.unwrap_or_else(|| {
            let tier_str = self.tier.to_string().to_lowercase();
            TIER_PRESETS
                .iter()
                .find(|(k, _)| *k == tier_str)
                .map(|(_, p)| ResourceLimits {
                    max_cpu_percent: p.cpu_pct,
                    max_ram_percent: p.ram_pct,
                    max_bandwidth_percent: p.bandwidth_pct,
                    max_gpu_percent: p.gpu_pct,
                })
                .unwrap_or_else(|| ResourceLimits {
                    max_cpu_percent: 50,
                    max_ram_percent: 50,
                    max_bandwidth_percent: 60,
                    max_gpu_percent: 40,
                })
        })
    }

    /// Apply safety floors to limits given current system state
    pub fn apply_safety(&self, limits: ResourceLimits, _snap: &SystemSnapshot) -> ResourceLimits {
        let mut out = limits;
        if out.max_cpu_percent > self.safety.max_cpu_percent {
            out.max_cpu_percent = self.safety.max_cpu_percent;
        }
        out
    }
}

// ============================================================================
// Glow-Up allocation engine
// ============================================================================

/// Glow-Up allocation engine
pub struct GlowUpEngine {
    pub config: DaemonConfig,
    pub current: Allocation,
    pub history: AllocationHistory,
    pub platform: Box<dyn Platform>,
}

#[derive(Debug, Clone)]
pub struct AllocationHistory {
    /// RAM allocations in bytes
    pub ram_allocations: VecDeque<u64>,
    /// CPU core allocations
    pub cpu_allocations: VecDeque<usize>,
    /// Bandwidth allocations in Mbps
    pub bw_allocations: VecDeque<u32>,
    /// GPU allocations in MB
    pub gpu_allocations: VecDeque<u32>,
    max_len: usize,
}

impl AllocationHistory {
    fn new(max_len: usize) -> Self {
        Self {
            ram_allocations: VecDeque::with_capacity(max_len),
            cpu_allocations: VecDeque::with_capacity(max_len),
            bw_allocations: VecDeque::with_capacity(max_len),
            gpu_allocations: VecDeque::with_capacity(max_len),
            max_len,
        }
    }

    fn push(&mut self, alloc: &Allocation) {
        self.ram_allocations.push_back(alloc.ram_bytes);
        if self.ram_allocations.len() > self.max_len {
            self.ram_allocations.pop_front();
        }
    }

    fn len(&self) -> usize {
        self.ram_allocations.len()
    }
}

impl GlowUpEngine {
    pub fn new(config: DaemonConfig, platform: Box<dyn Platform>) -> Self {
        let tier = config.tier;
        let preset = TIER_PRESETS
            .iter()
            .find(|(k, _)| *k == tier.to_string().to_lowercase())
            .map(|(_, p)| *p)
            .unwrap_or(TIER_PRESETS[1].1);

        // L'état initial est publié avant le premier tick. Il doit donc déjà
        // respecter le plafond configuré, sinon `ram_headroom_bytes()`
        // pourrait devenir négatif (saturé à zéro) pendant toute l'hystérésis.
        // En l'absence de mesure mémoire fiable, démarrer à zéro est plus sûr
        // que d'inventer une capacité initiale.
        let configured_ram_percent = config
            .custom_limits
            .map(|limits| limits.max_ram_percent)
            .unwrap_or(preset.ram_pct);
        let measured_total_ram = platform
            .memory_info()
            .map(|memory| memory.total_bytes)
            .unwrap_or(0);
        let configured_ram_cap = (measured_total_ram as u128 * configured_ram_percent as u128 / 100)
            .min(u64::MAX as u128) as u64;
        let bootstrap_ram = (preset.ram_pct as u64 * 1024 * 1024 * 1024 / 100)
            .min(2 * 1024 * 1024 * 1024)
            .min(configured_ram_cap);

        let initial = Allocation {
            ram_bytes: bootstrap_ram,
            bandwidth_mbps: 10,
            shrink_streak: 0,
            free_mem_avg_bytes: 0,
            shrinking: false,
        };

        Self {
            config,
            current: initial,
            history: AllocationHistory::new(100),
            platform,
        }
    }

    /// Main tick: compute new allocation from system snapshot
    pub fn tick(&mut self, snap: &SystemSnapshot) -> Result<Allocation> {
        let limits = self.config.effective_limits(snap);
        let safe_limits = self.config.apply_safety(limits, snap);
        let ideal = self.compute_ideal(snap, &safe_limits);
        let constrained = self.apply_constraints(ideal, snap);
        let smoothed = self.smooth_transition(constrained);

        self.current = smoothed;
        self.history.push(&smoothed);
        Ok(smoothed)
    }

    fn compute_ideal(&self, snap: &SystemSnapshot, limits: &ResourceLimits) -> Allocation {
        let total_ram = snap.memory.total_bytes;
        let bw_estimate = snap.bandwidth.total_mbps.max(10.0) as u32;

        let ram_bytes = (total_ram as f32 * limits.max_ram_percent as f32 / 100.0) as u64;
        let bandwidth_mbps =
            (bw_estimate as f32 * limits.max_bandwidth_percent as f32 / 100.0) as u32;

        Allocation {
            ram_bytes,
            bandwidth_mbps,
            shrink_streak: 0,
            free_mem_avg_bytes: snap.memory.available_bytes,
            shrinking: false,
        }
    }

    fn apply_constraints(&self, mut alloc: Allocation, snap: &SystemSnapshot) -> Allocation {
        let behavior = &self.config.behavior;
        let safety = &self.config.safety;

        // CPU ceiling
        if snap.cpu.usage_percent > safety.max_cpu_percent as f32 {
            let factor = (safety.max_cpu_percent as f32 / snap.cpu.usage_percent).min(1.0);
            alloc.ram_bytes = (alloc.ram_bytes as f32 * factor) as u64;
            alloc.bandwidth_mbps = (alloc.bandwidth_mbps as f32 * factor) as u32;
        }

        // User activity throttle
        if behavior.throttle_on_user_activity && snap.user_active.is_active() {
            alloc.ram_bytes = (alloc.ram_bytes as f32 * 0.5) as u64;
            alloc.bandwidth_mbps = (alloc.bandwidth_mbps as f32 * 0.5) as u32;
        }

        // Hard safety floors
        let min_ram = (safety.min_free_ram_gb * 1_073_741_824.0) as u64;
        if snap.memory.available_bytes < min_ram + alloc.ram_bytes {
            alloc.ram_bytes = snap.memory.available_bytes.saturating_sub(min_ram);
        }

        // Minimum floor
        alloc.ram_bytes = alloc.ram_bytes.max(64 * 1024 * 1024);
        alloc.bandwidth_mbps = alloc.bandwidth_mbps.max(1);
        alloc
    }

    fn smooth_transition(&self, target: Allocation) -> Allocation {
        let behavior = &self.config.behavior;
        let mut result = target;
        let current = &self.current;

        // L'hystérésis doit mesurer la pression de la cible, pas la pente de
        // l'allocation déjà lissée. L'ancienne condition (historique décroissant)
        // était auto-bloquante : tant que le daemon retenait son allocation,
        // l'historique restait plat et le premier retrait ne pouvait jamais partir.
        let ram_under_pressure = target.ram_bytes < current.ram_bytes;
        result.shrink_streak = if ram_under_pressure {
            current.shrink_streak.saturating_add(1)
        } else {
            0
        };
        let allow_ram_shrink = result.shrink_streak >= behavior.shrink_hysteresis_ticks.max(1);
        result.shrinking = false;

        // RAM : croissance rapide, retrait progressif après N cycles de pression
        // consécutifs. Une fois le seuil atteint, `shrinking` reste vrai pendant
        // toute la contraction, pas seulement sur un tick : les consommateurs
        // qui interrogent le socket peuvent ainsi observer le signal sans course
        // contre l'intervalle de polling.
        if target.ram_bytes > current.ram_bytes {
            let step =
                (current.ram_bytes * behavior.grow_step_pct as u64 / 100).max(64 * 1024 * 1024);
            result.ram_bytes = (current.ram_bytes + step).min(target.ram_bytes);
        } else if ram_under_pressure {
            if allow_ram_shrink {
                let step = (current.ram_bytes * behavior.shrink_step_pct as u64 / 100)
                    .max(64 * 1024 * 1024);
                result.ram_bytes = current.ram_bytes.saturating_sub(step).max(target.ram_bytes);
            } else {
                result.ram_bytes = current.ram_bytes; // Hystérésis : maintien
            }
        }
        result.shrinking = ram_under_pressure && allow_ram_shrink;

        // Bandwidth
        if target.bandwidth_mbps > current.bandwidth_mbps {
            let step = ((current.bandwidth_mbps as f32 * behavior.grow_step_pct as f32 / 100.0)
                as u32)
                .max(5);
            result.bandwidth_mbps = (current.bandwidth_mbps + step).min(target.bandwidth_mbps);
        } else if target.bandwidth_mbps < current.bandwidth_mbps {
            if self.history.len() >= behavior.shrink_hysteresis_ticks as usize {
                let step = ((current.bandwidth_mbps as f32 * behavior.shrink_step_pct as f32
                    / 100.0) as u32)
                    .max(5);
                result.bandwidth_mbps = current
                    .bandwidth_mbps
                    .saturating_sub(step)
                    .max(target.bandwidth_mbps);
            } else {
                result.bandwidth_mbps = current.bandwidth_mbps;
            }
        }

        result
    }

    /// Apply allocation to platform (CPU priority).
    ///
    /// Le plafond mémoire n'est **plus** posé ici, et c'est une décision, pas un
    /// oubli : le daemon n'écrit jamais le cgroup qui le contient. Ce fichier
    /// appartient à `systemd` (`MemoryMax=` de l'unité `--user`) ; un daemon qui
    /// le réécrit à chaque tick se bat contre son gestionnaire et plafonne le
    /// cgroup de son lanceur — terminal, navigateur, session graphique — pas
    /// lui-même. Voir `docs/PLAN_PRIORITES_2B5.md` §1/§4.
    pub fn apply(&self, alloc: &Allocation) -> Result<()> {
        // CPU priority via nice value
        let nice = if alloc.ram_bytes > 512 * 1024 * 1024 {
            0
        } else {
            5
        };
        self.platform.set_cpu_priority(nice)?;
        Ok(())
    }

    pub fn current(&self) -> &Allocation {
        &self.current
    }

    pub fn set_tier(&mut self, tier: AllocationTier) {
        self.config.tier = tier;
        self.config.custom_limits = None;
    }

    pub fn set_custom_limits(&mut self, limits: ResourceLimits) {
        self.config.tier = AllocationTier::Custom;
        self.config.custom_limits = Some(limits);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CpuTopology;

    fn test_snapshot(total_ram: u64, available_ram: u64) -> SystemSnapshot {
        SystemSnapshot {
            cpu: crate::CpuSnapshot {
                usage_percent: 10.0,
                per_core: vec![10.0; 4],
                load_average: [0.1, 0.1, 0.1],
                frequency_mhz: 2_000.0,
                topology: CpuTopology::default(),
            },
            memory: crate::MemorySnapshot {
                total_bytes: total_ram,
                used_bytes: total_ram.saturating_sub(available_ram),
                free_bytes: available_ram,
                available_bytes: available_ram,
                swap_used_bytes: 0,
                swap_total_bytes: 0,
            },
            bandwidth: crate::BandwidthSnapshot {
                interface: "test0".into(),
                rx_mbps: 5.0,
                tx_mbps: 5.0,
                total_mbps: 10.0,
            },
            gpu: Vec::new(),
            timestamp: 1,
            user_active: crate::UserActivity::Idle,
        }
    }

    #[test]
    fn test_ram_shrink_starts_after_consecutive_pressure_and_is_reported() {
        let config = DaemonConfig {
            tier: AllocationTier::Balanced,
            custom_limits: Some(ResourceLimits {
                max_cpu_percent: 85,
                max_ram_percent: 10,
                max_bandwidth_percent: 60,
                max_gpu_percent: 40,
            }),
            behavior: BehaviorConfig {
                grow_step_pct: 10,
                shrink_step_pct: 20,
                shrink_hysteresis_ticks: 3,
                throttle_on_user_activity: false,
                tick_interval_secs: 1,
            },
            safety: SafetyMargins {
                min_free_ram_gb: 15.92,
                ..SafetyMargins::default()
            },
            ..DaemonConfig::default()
        };
        let mut engine = GlowUpEngine::new(config, crate::create_platform());
        let initial_ram = engine.current.ram_bytes;
        let snap = test_snapshot(16 * 1024 * 1024 * 1024, 15 * 1024 * 1024 * 1024);

        // Les deux premiers ticks confirment la pression mais ne retirent rien.
        for expected_streak in 1..3 {
            let allocation = engine.tick(&snap).expect("tick sous pression");
            assert_eq!(allocation.ram_bytes, initial_ram);
            assert_eq!(allocation.shrink_streak, expected_streak);
            assert!(!allocation.shrinking, "pas de signal avant le vrai retrait");
        }

        // Au troisième tick, la baisse est effective et le consommateur peut
        // voir le signal dans le même état que l'allocation publiée.
        let allocation = engine.tick(&snap).expect("tick de retrait");
        assert!(allocation.ram_bytes < initial_ram);
        assert_eq!(allocation.shrink_streak, 3);
        assert!(allocation.shrinking, "un retrait engagé doit être annoncé");

        // Le signal reste visible à chaque tick pendant que la cible n'est pas
        // encore atteinte, afin qu'un client ne le rate pas entre deux polls.
        let mut last = allocation;
        for _ in 0..10 {
            if !last.shrinking {
                break;
            }
            last = engine.tick(&snap).expect("tick pendant la contraction");
            if last.ram_bytes > (16 * 1024 * 1024 * 1024) / 100 {
                assert!(last.shrinking, "signal maintenu avant la cible");
            }
        }
        assert!(
            !last.shrinking,
            "le signal s'efface une fois la cible atteinte"
        );
        assert_eq!(last.ram_bytes, 64 * 1024 * 1024);
    }

    #[test]
    fn test_ram_shrink_streak_resets_when_pressure_disappears() {
        let config = DaemonConfig {
            tier: AllocationTier::Balanced,
            custom_limits: Some(ResourceLimits {
                max_cpu_percent: 85,
                max_ram_percent: 10,
                max_bandwidth_percent: 60,
                max_gpu_percent: 40,
            }),
            behavior: BehaviorConfig {
                shrink_hysteresis_ticks: 3,
                throttle_on_user_activity: false,
                ..BehaviorConfig::default()
            },
            safety: SafetyMargins {
                min_free_ram_gb: 15.92,
                ..SafetyMargins::default()
            },
            ..DaemonConfig::default()
        };
        let mut engine = GlowUpEngine::new(config, crate::create_platform());
        let low = test_snapshot(16 * 1024 * 1024 * 1024, 15 * 1024 * 1024 * 1024);
        engine.tick(&low).expect("premier tick sous pression");
        engine.tick(&low).expect("deuxième tick sous pression");
        assert_eq!(engine.current.shrink_streak, 2);

        engine.set_custom_limits(ResourceLimits {
            max_cpu_percent: 85,
            max_ram_percent: 80,
            max_bandwidth_percent: 60,
            max_gpu_percent: 40,
        });
        // La marge de sûreté revient à zéro dans cette contre-épreuve : la
        // cible doit redevenir supérieure à l'allocation courante.
        engine.config.safety.min_free_ram_gb = 0.0;
        let before_recovery = engine.current.ram_bytes;
        let high = test_snapshot(16 * 1024 * 1024 * 1024, 15 * 1024 * 1024 * 1024);
        let allocation = engine
            .tick(&high)
            .expect("tick après disparition de la pression");
        assert_eq!(allocation.shrink_streak, 0);
        assert!(!allocation.shrinking);
        assert!(
            allocation.ram_bytes > before_recovery,
            "la croissance doit reprendre"
        );
    }

    #[test]
    fn test_initial_allocation_respects_custom_ram_cap() {
        let platform = crate::create_platform();
        let total_ram = platform.memory_info().expect("mesure RAM").total_bytes;
        let config = DaemonConfig {
            tier: AllocationTier::Balanced,
            custom_limits: Some(ResourceLimits {
                max_cpu_percent: 85,
                max_ram_percent: 1,
                max_bandwidth_percent: 60,
                max_gpu_percent: 40,
            }),
            ..DaemonConfig::default()
        };
        let engine = GlowUpEngine::new(config, platform);
        let allowed = (total_ram as u128 / 100) as u64;
        assert!(
            engine.current.ram_bytes <= allowed,
            "l'état initial ne doit pas dépasser le plafond personnalisé : {} > {allowed}",
            engine.current.ram_bytes
        );
    }

    #[test]
    fn test_tier_presets() {
        assert_eq!(TIER_PRESETS[0].0, "eco");
        assert_eq!(TIER_PRESETS[1].0, "balanced");
        assert_eq!(TIER_PRESETS[2].0, "performance");
        assert_eq!(TIER_PRESETS[3].0, "max");
    }

    #[test]
    fn test_safety_application() {
        let config = DaemonConfig::default();
        let snap = SystemSnapshot {
            cpu: crate::CpuSnapshot {
                usage_percent: 90.0,
                per_core: vec![90.0; 8],
                load_average: [1.0, 1.0, 1.0],
                frequency_mhz: 3000.0,
                topology: CpuTopology {
                    sockets: 1,
                    cores_per_socket: 8,
                    threads_per_core: 1,
                },
            },
            memory: crate::MemorySnapshot {
                total_bytes: 16 * 1024 * 1024 * 1024,
                used_bytes: 8 * 1024 * 1024 * 1024,
                free_bytes: 8 * 1024 * 1024 * 1024,
                available_bytes: 8 * 1024 * 1024 * 1024,
                swap_used_bytes: 0,
                swap_total_bytes: 0,
            },
            bandwidth: crate::BandwidthSnapshot {
                interface: "eth0".into(),
                rx_mbps: 10.0,
                tx_mbps: 10.0,
                total_mbps: 20.0,
            },
            gpu: vec![crate::GpuSnapshot {
                device_id: 0,
                name: "RTX".into(),
                vram_total_mb: 8192,
                vram_used_mb: 2048,
                vram_free_mb: 6144,
                utilization_pct: 10,
                temperature_c: 40,
                power_watts: 50,
            }],
            timestamp: 0,
            user_active: crate::UserActivity::Idle,
        };
        let limits = ResourceLimits {
            max_cpu_percent: 60,
            max_ram_percent: 45,
            max_bandwidth_percent: 70,
            max_gpu_percent: 40,
        };
        let safe = config.apply_safety(limits, &snap);
        assert!(safe.max_cpu_percent <= 85);
    }

    // ── D8 : rétro-compat des formats de config ──────────────────────────

    #[test]
    fn test_config_current_format_roundtrip() {
        // Format actuel (écrit par --gen-config depuis it.16) : tier scalaire.
        let toml_str = r#"
tier = "Balanced"
memory_limit_enabled = true
bandwidth_shaping = true
gpu_allocation_enabled = true
service_integration = true

[behavior]
grow_step_pct = 10
shrink_step_pct = 5
shrink_hysteresis_ticks = 5
throttle_on_user_activity = true
tick_interval_secs = 5

[safety]
min_free_ram_gb = 4.0
min_free_cpu_cores = 1
min_free_vram_mb = 512
max_cpu_percent = 85
"#;
        let config: DaemonConfig = toml::from_str(toml_str).expect("format actuel lisible");
        assert_eq!(config.tier, AllocationTier::Balanced);
        assert_eq!(config.safety.min_free_ram_gb, 4.0);

        // Round-trip : ce que serde écrit doit se relire tel quel.
        let written = toml::to_string_pretty(&config).unwrap();
        let reread: DaemonConfig = toml::from_str(&written).unwrap();
        assert_eq!(reread.tier, config.tier);
        assert_eq!(reread.safety.max_cpu_percent, config.safety.max_cpu_percent);
        assert_eq!(reread.behavior.tick_interval_secs, 5);
    }

    #[test]
    fn test_config_legacy_tier_table() {
        // Format legacy (config réelle écrite le 2026-07-20) : `[tier]`
        // est une TABLE, et une section `[platform]` inconnue est ignorée.
        let toml_str = r#"
[tier]
tier = "Balanced"

[safety]
min_free_ram_gb = 4.0
min_free_cpu_cores = 1
min_free_vram_mb = 512
max_cpu_percent = 85

[behavior]
grow_step_pct = 10
shrink_step_pct = 5
shrink_hysteresis_ticks = 5
throttle_on_user_activity = true
tick_interval_secs = 5

[platform]
mode = "linux"
"#;
        let config: DaemonConfig =
            toml::from_str(toml_str).expect("config legacy (table [tier]) lisible");
        assert_eq!(config.tier, AllocationTier::Balanced);
        // Champs absents du format legacy → defaults, jamais d'erreur.
        assert!(config.memory_limit_enabled);
        assert!(config.service_integration);
    }

    #[test]
    fn test_config_legacy_name_key() {
        // L'ancien --gen-config cassé écrivait `[tier] name = "…"`.
        let toml_str = r#"
[tier]
name = "Performance"
"#;
        let config: DaemonConfig =
            toml::from_str(toml_str).expect("forme legacy [tier] name lisible");
        assert_eq!(config.tier, AllocationTier::Performance);
        assert!(config.service_integration); // default
    }

    #[test]
    fn test_config_minimal_defaults() {
        // Même une config réduite à `tier = "Eco"` doit charger.
        let config: DaemonConfig =
            toml::from_str("tier = \"Eco\"").expect("config minimale lisible");
        assert_eq!(config.tier, AllocationTier::Eco);
        assert_eq!(config.behavior.tick_interval_secs, 5); // default
        assert_eq!(config.safety.max_cpu_percent, 85); // default
    }
}
