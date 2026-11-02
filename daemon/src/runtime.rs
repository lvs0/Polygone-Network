//! Conversion de l'état réel du daemon vers l'instantané publié sur le socket.
//!
//! Cette fonction est **pure** : aucun appel système, aucun verrou, aucune E/S.
//! Elle est la seule chose qui décide ce que le socket raconte, donc la seule
//! qui puisse mentir sans qu'aucun test existant ne s'en aperçoive — d'où son
//! isolement dans son propre module, avec ses propres tests.
//!
//! Conséquence directe de la pureté : l'heure courante n'est **pas** lue ici,
//! mais prise dans `snap.timestamp`. C'est le même instant que le reste des
//! données publiées, et cela rend la fonction reproductible en test.

use crate::allocator::Allocation;
use crate::socket::{publish_shared, DaemonState};
use crate::system::SystemSnapshot;
use crate::GlowUpEngine;
use anyhow::Result;
use std::sync::RwLock;

/// Construit l'instantané du protocole depuis l'état du moteur et un tick.
///
/// `seq` est fourni par l'appelant : c'est un compteur de tick, il ne se déduit
/// pas de l'état du moteur.
pub fn runtime_state(
    engine: &GlowUpEngine,
    snap: &SystemSnapshot,
    started_at_secs: u64,
    seq: u64,
) -> DaemonState {
    // Les plafonds du palier, marges de sécurité appliquées — exactement ce que
    // `GlowUpEngine::tick` utilise pour décider, donc ce que le client doit voir.
    let limits = engine
        .config
        .apply_safety(engine.config.effective_limits(snap), snap);

    // ⚠️ PLAFOND, PAS UNE DISPONIBILITÉ.
    //
    // Le moteur n'alloue **aucun** CPU aujourd'hui : `GlowUpEngine::apply` ne
    // pose qu'une priorité `nice` ; il n'applique ni quota CPU ni limite mémoire,
    // et `set_cpu_affinity` n'a aucun appelant. Il n'existe donc aucune
    // « disponibilité CPU » à publier. On
    // publie le total physique, seule grandeur honnête et vérifiable, et on le
    // dit ici pour que personne ne lise ce champ comme « cœurs libres ».
    let cpu_cores = snap.cpu.per_core.len() as u32;

    // VRAM libre réelle, en saturant : un pilote peut rapporter un « utilisé »
    // supérieur au « total » (ou le total peut changer en cours de route).
    let gpu_free_vram_mb = snap
        .gpu
        .iter()
        .map(|g| g.vram_total_mb.saturating_sub(g.vram_used_mb))
        .fold(0u32, u32::saturating_add);

    // Cf. en-tête : on date la durée de vie avec l'horloge du snapshot, pas avec
    // celle du système, pour rester pur.
    //
    // Deux pièges dans cette soustraction, tous deux corrigés ici :
    //   - `saturating_sub` sur `i64` sature à `i64::MIN`, et le cast en `u64`
    //     replierait alors la valeur sur un nombre astronomique : il faut
    //     borner à 0 AVANT le cast, pas compter sur la saturation ;
    //   - `as i64` sur un `u64` est un cast enveloppant ; `try_from` évite de
    //     publier une durée de vie absurde si l'entrée est aberrante.
    let started_at = i64::try_from(started_at_secs).unwrap_or(i64::MAX);
    let uptime_secs = snap.timestamp.saturating_sub(started_at).max(0) as u64;

    let alloc = &engine.current;

    DaemonState {
        seq,
        tier: engine.config.tier.as_str().to_string(),
        max_cpu_percent: limits.max_cpu_percent,
        max_ram_percent: limits.max_ram_percent,
        max_bandwidth_percent: limits.max_bandwidth_percent,
        max_gpu_percent: limits.max_gpu_percent,
        // Le total de la machine, jamais une constante.
        total_ram_bytes: snap.memory.total_bytes,
        ram_bytes: alloc.ram_bytes,
        bandwidth_mbps: alloc.bandwidth_mbps,
        cpu_cores,
        gpu_free_vram_mb,
        shrinking: alloc.shrinking,
        uptime_secs,
        daemon_version: env!("CARGO_PKG_VERSION").to_string(),
        platform: engine.platform.name().to_string(),
        // Pour l'instant, aucune capacité de stockage n'est exposée via le
        // snapshot système — le champ restera à 0 tant qu'aucune source
        // fiable de quota disque n'est branchée. Toute demande de stockage
        // sera refusée (0 = rien d'allouable).
        total_storage_bytes: 0,
    }
}

// ============================================================================
// Un tick complet, extrait de la boucle de main.rs
// ============================================================================
//
// Ce bloc vivait dans `main.rs`, donc dans un binaire sans le moindre test :
// « un tick produit-il une publication observable ? » n'était vérifiable qu'à la
// main, en lançant le daemon et en lisant le socket. Il vit maintenant ici, où
// un test peut l'appeler directement.

/// Ce qu'un tick a produit, rendu à l'appelant.
///
/// L'appelant en a besoin pour journaliser, rafraîchir son propre statut et
/// avancer son compteur sans relire ce que le tick vient de faire.
#[derive(Debug, Clone)]
pub struct TickOutcome {
    /// La décision du moteur pour ce tick.
    pub allocation: Allocation,
    /// Le snapshot qui a servi à décider — le même que celui publié.
    pub snapshot: SystemSnapshot,
    /// Vrai quand l'instantané a été écrit dans l'état partagé.
    ///
    /// Toujours vrai sur le chemin `Ok` : un échec de décision rend `Err`
    /// **avant** la publication, donc `run_tick` ne peut pas réussir sans
    /// publier. Le champ évite à l'appelant de le déduire — ce n'est **pas** un
    /// détecteur de changement, et il ne faut pas le lire comme tel : un état
    /// identique à l'octet près ferait quand même avancer `seq`.
    pub published: bool,
}

/// Un tick complet : capture → décision → application éventuelle → publication.
///
/// C'est le corps de la boucle de `main.rs`, extrait pour être testable. La
/// fonction ne touche ni le réseau ni le disque : elle ne parle au système que
/// par `engine.apply()`, et uniquement quand `apply` est vrai. Les tests passent
/// donc `false` et ne modifient jamais la machine.
///
/// `seq` est fourni par l'appelant, qui seul sait combien de publications ont
/// déjà eu lieu.
/// Publie un instantané final sans créer un second chemin d'écriture hors
/// de ce module. Toute publication du daemon passe par la même frontière.
pub fn publish_runtime_state(shared: &RwLock<DaemonState>, snapshot: DaemonState) {
    publish_shared(shared, snapshot);
}

pub fn run_tick(
    engine: &mut GlowUpEngine,
    shared: &RwLock<DaemonState>,
    started_at_secs: u64,
    seq: u64,
    apply: bool,
) -> Result<TickOutcome> {
    let snap = SystemSnapshot::capture(&*engine.platform);
    let alloc = engine.tick(&snap)?;

    if apply {
        // Un échec d'application n'annule pas la décision : le tour se poursuit
        // et l'instantané est publié quand même. C'est le comportement d'origine
        // de la boucle, conservé à l'identique — un `?` ici changerait ce que
        // voit un client dès que le noyau refuse une limite.
        if let Err(e) = engine.apply(&alloc) {
            log::warn!("apply failed: {}", e);
        }
    }

    // Le verrou n'est pris que pour ce `store`, et `apply()` est déjà terminé :
    // jamais de verrou tenu pendant un appel système, ni pendant une E/S.
    publish_runtime_state(shared, runtime_state(engine, &snap, started_at_secs, seq));

    Ok(TickOutcome {
        allocation: alloc,
        snapshot: snap,
        published: true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::policy::glow_up::{AllocationTier, DaemonConfig};
    use crate::resources::create_platform;
    use crate::socket::read_shared;
    use crate::system::{BandwidthSnapshot, CpuSnapshot, GpuSnapshot, MemorySnapshot};
    use std::path::{Path, PathBuf};
    use std::time::Duration;

    /// Moteur réel, palier `eco` : `runtime_state` ne lit que `config` et le nom
    /// de la plateforme, donc une plateforme réelle convient parfaitement — et
    /// elle évite d'inventer un double de test.
    fn engine() -> GlowUpEngine {
        let config = DaemonConfig {
            tier: AllocationTier::Eco,
            ..DaemonConfig::default()
        };
        GlowUpEngine::new(config, create_platform())
    }

    /// Snapshot de référence : des valeurs volontairement distinctives, pour
    /// qu'aucune ne puisse coïncider par accident avec un défaut codé en dur.
    fn snapshot() -> SystemSnapshot {
        SystemSnapshot {
            cpu: CpuSnapshot {
                usage_percent: 12.5,
                // 6 cœurs → `cpu_cores` doit valoir 6, pas les 3 de `DaemonState::new`.
                per_core: vec![2400.0; 6],
                load_average: [0.1, 0.2, 0.3],
                frequency_mhz: 2400.0,
                topology: Default::default(),
            },
            memory: MemorySnapshot {
                // 7 Gio : distinct des 16 Gio codés en dur dans `DaemonState::new`.
                total_bytes: 7 * 1024 * 1024 * 1024,
                used_bytes: 3 * 1024 * 1024 * 1024,
                free_bytes: 4 * 1024 * 1024 * 1024,
                available_bytes: 4 * 1024 * 1024 * 1024,
                swap_used_bytes: 0,
                swap_total_bytes: 0,
            },
            bandwidth: BandwidthSnapshot {
                interface: "eth0".to_string(),
                rx_mbps: 10.0,
                tx_mbps: 5.0,
                total_mbps: 15.0,
            },
            gpu: vec![
                GpuSnapshot {
                    device_id: 0,
                    name: "gpu0".to_string(),
                    vram_total_mb: 8192,
                    vram_used_mb: 2692,
                    vram_free_mb: 5500,
                    utilization_pct: 10,
                    temperature_c: 40,
                    power_watts: 50,
                },
                GpuSnapshot {
                    device_id: 1,
                    name: "gpu1".to_string(),
                    vram_total_mb: 1000,
                    // Utilisé > total : ce pilote ment, la soustraction doit
                    // saturer à zéro et JAMAIS déclencher un panic de débordement.
                    vram_used_mb: 1500,
                    vram_free_mb: 0,
                    utilization_pct: 0,
                    temperature_c: 30,
                    power_watts: 10,
                },
            ],
            timestamp: 1_000_050,
            user_active: crate::resources::UserActivity::Idle,
        }
    }

    #[test]
    fn test_runtime_state_maps_snapshot() {
        let engine = engine();
        let snap = snapshot();
        let state = runtime_state(&engine, &snap, 1_000_000, 42);

        assert_eq!(state.seq, 42, "le compteur de tick est recopié tel quel");
        assert_eq!(state.tier, "eco", "le palier vient de la configuration");
        assert_eq!(
            state.total_ram_bytes, snap.memory.total_bytes,
            "le total est celui de la machine, pas une constante"
        );
        assert_eq!(
            state.total_ram_bytes,
            7 * 1024 * 1024 * 1024,
            "et surtout pas les 16 Gio de DaemonState::new"
        );
        assert_eq!(
            state.ram_bytes, engine.current.ram_bytes,
            "l'allocation publiée est celle du moteur"
        );
        assert_eq!(state.bandwidth_mbps, engine.current.bandwidth_mbps);
        assert_eq!(state.shrinking, engine.current.shrinking);
        assert_eq!(
            state.cpu_cores,
            snap.cpu.per_core.len() as u32,
            "cpu_cores publie le total physique (un plafond)"
        );
        assert_eq!(
            state.gpu_free_vram_mb, 5500,
            "somme saturante : 8192-2692 = 5500, et 1000-1500 sature à 0"
        );
        assert_eq!(
            state.uptime_secs, 50,
            "durée de vie calculée sur l'horloge du snapshot"
        );
        assert_eq!(
            state.platform,
            engine.platform.name(),
            "la plateforme est celle du moteur"
        );
        assert_eq!(state.daemon_version, env!("CARGO_PKG_VERSION"));

        // Les plafonds viennent de `effective_limits` + `apply_safety` : on
        // vérifie l'invariant qui compte (la marge de sécurité est respectée)
        // plutôt qu'une constante de preset qui bougera avec la politique.
        assert!(
            state.max_cpu_percent <= engine.config.safety.max_cpu_percent,
            "la marge de sécurité CPU doit avoir été appliquée"
        );
        assert!(state.max_ram_percent > 0);
        assert!(state.max_bandwidth_percent > 0);
    }

    #[test]
    fn test_tick_publishes_new_seq() {
        // L'état partagé démarre à seq = 0, comme celui construit par main.rs
        // avant la boucle.
        let shared = RwLock::new(DaemonState::new("balanced"));
        let mut engine = engine();

        let outcome = run_tick(&mut engine, &shared, 1_000_000, 1, false).expect("tick");

        let published = read_shared(&shared);
        assert_eq!(published.seq, 1, "le tick publie le nouveau compteur");
        assert_eq!(
            published.ram_bytes, outcome.allocation.ram_bytes,
            "la RAM publiée est celle de l'allocation retournée"
        );
        assert_eq!(published.bandwidth_mbps, outcome.allocation.bandwidth_mbps);
        assert_eq!(
            published.total_ram_bytes, outcome.snapshot.memory.total_bytes,
            "le snapshot publié est celui du tick, pas un état fabriqué"
        );
        assert_eq!(published.platform, engine.platform.name());
        assert!(outcome.published);
        assert!(
            published.total_ram_bytes != DaemonState::new("balanced").total_ram_bytes,
            "l'état fabriqué (16 Gio) ne doit pas se retrouver publié"
        );
    }

    #[test]
    fn test_tick_publishes_each_time_a_new_seq() {
        let shared = RwLock::new(DaemonState::new("balanced"));
        let mut engine = engine();

        let mut seen = vec![read_shared(&shared).seq];
        for seq in 1..=3 {
            run_tick(&mut engine, &shared, 1_000_000, seq, false).expect("tick");
            seen.push(read_shared(&shared).seq);
        }

        assert_eq!(seen, vec![0, 1, 2, 3], "seq avance d'exactement 1 par tick");
    }

    #[test]
    fn test_tick_without_call_does_not_change_seq() {
        let shared = RwLock::new(DaemonState::new("balanced"));
        let before = read_shared(&shared);

        // Contre-épreuve : rien ne publie tout seul. Le temps passe, le daemon
        // dort, l'état ne bouge pas.
        std::thread::sleep(Duration::from_millis(50));

        let after = read_shared(&shared);
        assert_eq!(after.seq, before.seq, "aucune publication sans run_tick");
        assert_eq!(after.seq, 0);
        assert_eq!(after.ram_bytes, before.ram_bytes);
        assert_eq!(after.total_ram_bytes, before.total_ram_bytes);
        assert_eq!(after.uptime_secs, before.uptime_secs);
        assert_eq!(after.tier, before.tier);
    }

    /// Motif cherché, reconstruit pour que ce test ne se compte pas lui-même :
    /// écrit d'un seul tenant, son propre littéral contiendrait la chaîne.
    const NEEDLE: &str = concat!("publish_", "shared(");

    /// Garde-fou structurel : `run_tick` doit rester le SEUL point de publication.
    ///
    /// Aucun type ne peut l'exprimer, puisque la publication est une fonction
    /// publique du module socket. On inspecte donc la source : c'est peu
    /// élégant, et c'est le seul contrôle qui fonctionne sur un invariant de
    /// structure dans un projet dont le binaire n'a pas de test.
    ///
    /// Limite assumée : vérification de TEXTE, pas de sémantique. Elle détecte
    /// l'apparition d'un second appelant, pas un contournement indirect (une
    /// future `fn publier_autrement()` dans `runtime.rs` lui échapperait).
    #[test]
    fn test_tick_is_the_only_writer() {
        fn scan(dir: &Path, hits: &mut Vec<PathBuf>) {
            for entry in std::fs::read_dir(dir).expect("lecture du dossier src") {
                let path = entry.expect("entrée de dossier").path();
                if path.is_dir() {
                    scan(&path, hits);
                } else if path.extension().is_some_and(|ext| ext == "rs") {
                    let text = std::fs::read_to_string(&path).expect("lecture d'un fichier .rs");
                    if text.contains(NEEDLE) {
                        hits.push(path);
                    }
                }
            }
        }

        let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut hits = Vec::new();
        scan(&src, &mut hits);

        let mut runtime_calls = 0usize;
        let mut offenders: Vec<String> = Vec::new();
        for path in &hits {
            let name = path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            match name.as_str() {
                // socket.rs porte la définition et ses propres tests.
                "socket.rs" => {}
                "runtime.rs" => {
                    let text = std::fs::read_to_string(path).expect("relecture de runtime.rs");
                    runtime_calls = text.matches(NEEDLE).count();
                }
                other => offenders.push(other.to_string()),
            }
        }

        assert!(
            !hits.is_empty(),
            "le test serait vide : aucun appel trouvé, le motif a peut-être changé"
        );
        assert!(
            offenders.is_empty(),
            "publication hors de runtime.rs : {offenders:?}"
        );
        assert_eq!(
            runtime_calls, 1,
            "run_tick doit rester le seul appelant dans runtime.rs"
        );
    }

    /// Motif cherché, reconstruit pour que ce test ne se compte pas lui-même :
    /// écrit d'un seul tenant, son propre littéral contiendrait la chaîne.
    const CGROUP_LIMIT_FILE: &str = concat!("memory", ".max");

    /// Garde-fou structurel : **aucun** fichier de `daemon/src/` ne doit nommer
    /// le fichier de plafond mémoire d'un cgroup, encore moins l'écrire.
    ///
    /// La décision est actée dans `docs/PLAN_PRIORITES_2B5.md` §4 : la limite
    /// mémoire vit dans l'unité `systemd --user` (`MemoryMax=`), pas dans le
    /// daemon. L'écriture retirée en 2b-5b ne limitait pas le daemon : elle
    /// plafonnait le cgroup du LANCEUR (terminal, navigateur, session), avec une
    /// valeur inventée de 307 Mio. C'est le genre d'effet qui ne se voit ni dans
    /// un `status` ni dans un test de comportement : seul un scan de source le
    /// rend impossible à réintroduire sans casser la suite.
    ///
    /// Limite assumée, identique à celle de `test_tick_is_the_only_writer` :
    /// c'est une vérification de TEXTE. Un chemin construit autrement (par
    /// concaténation d'octets, par exemple) lui échapperait. Sa contre-mesure
    /// réelle est le test de non-modification dans `glow_up.rs`, qui observe
    /// l'effet au lieu de la forme.
    #[test]
    fn test_no_cgroup_write_anywhere() {
        fn scan(dir: &Path, hits: &mut Vec<PathBuf>) -> usize {
            let mut visited = 0usize;
            for entry in std::fs::read_dir(dir).expect("lecture du dossier src") {
                let path = entry.expect("entrée de dossier").path();
                if path.is_dir() {
                    visited += scan(&path, hits);
                } else if path.extension().is_some_and(|ext| ext == "rs") {
                    visited += 1;
                    let text = std::fs::read_to_string(&path).expect("lecture d'un fichier .rs");
                    if text.contains(CGROUP_LIMIT_FILE) {
                        hits.push(path);
                    }
                }
            }
            visited
        }

        let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut hits = Vec::new();
        let visited = scan(&src, &mut hits);

        // Un scan qui n'a rien lu passerait en vert sans rien vérifier : on
        // refuse explicitement ce faux négatif.
        assert!(
            visited > 0,
            "scan vide : aucun fichier .rs lu sous {} — le garde serait aveugle",
            src.display()
        );

        let offenders: Vec<String> = hits
            .iter()
            .map(|p| p.strip_prefix(&src).unwrap_or(p).display().to_string())
            .collect();

        assert!(
            offenders.is_empty(),
            "le daemon ne doit plus toucher au plafond mémoire du cgroup ; trouvé dans \
             {offenders:?} — voir docs/PLAN_PRIORITES_2B5.md §4 (la limite vit dans \
             l'unité systemd, pas dans le binaire)"
        );
    }

    #[test]
    fn test_runtime_state_uptime_saturates_on_clock_skew() {
        let engine = engine();
        let snap = snapshot();

        // Snapshot antérieur au démarrage (horloge qui recule) : on publie 0,
        // jamais un `u64` replié sur une valeur astronomique.
        let state = runtime_state(&engine, &snap, 2_000_000, 0);
        assert_eq!(
            state.uptime_secs, 0,
            "un timestamp antérieur au démarrage donne 0, pas un repli"
        );
    }
}
