//! Polygone — the unified product command.
//!
//! ```text
//! polygone                  → la TUI (2 commandes : envoyer / quitter)
//! polygone demo             → démo E2E post-quantique complète
//! polygone envoyer <clef> <message> → chiffrer + fragmenter (wire text)
//! polygone recevoir [fichier]       → reconstruire + déchiffrer (4/7)
//! polygone clef                     → votre clef publique (à partager)
//! polygone id                       → identité nœud
//! polygone doctor                   → mesure réelle du daemon et de ses capacités
//! ```
//!
//! « On voit rien. Et c'est comme ça que ça devrait être. »

mod demo;
mod duress;
mod exec;
mod hide;
mod identity;
mod mesh;
mod msg;

// `drive` et `brain` ne sont PAS montés : placeholders du commit 060c11d,
// jamais branchés (`DriveFile` a zéro appelant — décision 3 documentée dans
// `blocks.rs`, qui prévoyait déjà la suppression de `drive.rs`). Les monter
// faisait échouer `cargo clippy -D warnings` sur du code mort
// (never constructed / never used) et bloquait la CI pour tout le monde.
// Les fichiers restent sur disque, prêts à être branchés le jour où ils
// auront un appelant. Les monter de nouveau exige d'abord de les câbler.
mod net;
mod petals;
mod product;
mod reputation;
mod self_test;
mod serverless;
#[cfg(test)]
mod testutil;
mod tui;

use anyhow::Result;
use clap::{Parser, Subcommand};
use polygone_client::{node, node_isolation, pause, uninstall};
use polygone_core::reuse;
use polygoned::socket::{self, Reply};
use polygoned::DaemonClient;
use std::path::Path;
use std::time::Duration;

#[derive(Parser, Debug)]
#[command(
    name = "polygone",
    version = env!("CARGO_PKG_VERSION"),
    about = "⬡ POLYGONE — \"On voit rien. Et c'est comme ça que ça devrait être.\"",
    long_about = concat!(
        "Post-quantum ephemeral transit network.\n\n",
        "ML-KEM-1024 · ML-DSA-65 · AES-256-GCM · BLAKE3 · Shamir 4-of-7\n\n",
        "Sans commande : la TUI. Sinon : demo, envoyer, recevoir, clef, id.\n",
        "Aucun compte obligatoire. Relais facultatifs. Aucune télémétrie.",
    ),
)]
struct Args {
    #[arg(long)]
    verbose: bool,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// État réel de l'écosystème local : PWA, Hide, Petals, Compute.
    Ecosysteme {
        /// Adresse du relais utilisé par Hide.
        #[arg(long, default_value = "127.0.0.1:7000")]
        relais: String,
        /// Adresse d'Ollama utilisée par Petals.
        #[arg(long, default_value = "127.0.0.1:11434")]
        ollama: String,
        /// Répertoire du site PWA (défaut : ./web).
        #[arg(long)]
        site: Option<String>,
        /// Sortie 1 si un service est absent.
        #[arg(long)]
        strict: bool,
    },
    /// Diagnostic des capacités réellement mesurées, réservables et contraintes.
    Doctor,
    /// Choisir un profil d'usage (sobre|equilibre|genereux) et écrire les seuils.
    Profil {
        /// Profil à écrire ; sans argument, affiche le profil courant.
        nom: Option<String>,
        /// Chemin de config explicite (défaut : ~/.config/polygone/daemon.toml).
        #[arg(long)]
        config: Option<String>,
    },
    /// Suspendre le service : libérer le bail et écrire une intention bornée.
    Pause {
        /// Durée de la pause, entre 10 min et 4 h (`30m`, `2h`).
        #[arg(long, default_value = "30m")]
        duree: String,
    },
    /// Consommer la pause et redemander un bail au daemon.
    Reprendre,
    /// Énumérer ce qui sera supprimé, puis supprimer après confirmation.
    Desinstaller {
        /// Supprimer réellement. Sans ce drapeau, seule l'énumération est faite.
        #[arg(long)]
        confirmer: bool,
    },
    /// Launch the TUI (default) — :envoyer / :quitter
    Tui,
    /// Run the flagship E2E demo: Alice → blind relay → Bob + relay audit
    Demo {
        /// Relay address (reserved for future TCP mode)
        #[arg(long)]
        relay: Option<String>,
    },
    /// Encrypt + fragment a message or a file for a recipient
    Envoyer {
        /// Recipient's ML-KEM-1024 public key (hex) — omit for a self-demo
        #[arg(long, short = 'd')]
        dest: Option<String>,
        /// Route through a relay: --via <relay:port> --a <dest_node_id>
        #[arg(long)]
        via: Option<String>,
        /// Destination node id on the relay (with --via)
        #[arg(long, short = 'a')]
        a: Option<String>,
        /// Send a file (with --via: received by the peer's `ecouter`)
        #[arg(long)]
        fichier: Option<String>,
        /// The message to send (everything after the flags)
        #[arg(required = false)]
        message: Vec<String>,
        /// Read the message from stdin — nothing appears in the shell history
        #[arg(long)]
        stdin: bool,
    },
    /// Reconstruct + decrypt from wire text (file path, or stdin if '-')
    Recevoir {
        /// File containing wire text, or '-' for stdin
        #[arg(default_value = "-")]
        input: String,
    },
    /// Listen for messages through a relay (plane 2 — real network)
    Ecouter {
        /// Relay address
        #[arg(long, default_value = "127.0.0.1:7000")]
        relay: String,
        /// Also announce this node + relay on the LAN mesh (Phase 4)
        #[arg(long)]
        annoncer: bool,
        /// Act as a RES ghost node — grant compute requests
        #[arg(long)]
        compute: bool,
        /// Act as a Polygone Hide exit node — open real connections for
        /// tunnel requests (HIDE-SPEC.md Phase 1)
        #[arg(long)]
        hide: bool,
    },
    /// Scan the LAN for announcing Polygone nodes (mesh, Phase 4)
    Voisins {
        /// Scan duration in seconds
        #[arg(long, default_value_t = 2)]
        duree: u64,
    },
    /// Announce this node + its relay on the LAN (mesh, Phase 4)
    Annoncer {
        /// The relay address peers should use to reach you
        #[arg(long, default_value = "127.0.0.1:7000")]
        relay: String,
    },
    /// RES — your free compute + the ghost nodes on the LAN (notes Bear)
    Compute {
        /// Scan duration in seconds for LAN peers
        #[arg(long, default_value_t = 3)]
        duree: u64,
        /// Borrow compute from a ghost node
        #[arg(long)]
        emprunter: Option<String>,
        /// Execute a task on the ghost node (sandboxed, RES execution)
        #[arg(long)]
        executer: Option<String>,
        /// Execute a WASM module on the ghost node (wasmi sandbox)
        #[arg(long)]
        wasm: Option<String>,
        /// Relay to route the borrow/exec request through
        #[arg(long, default_value = "127.0.0.1:7000")]
        via: String,
        /// Recalculer localement la tâche et comparer au grant (vérification L1)
        #[arg(long)]
        verifier_local: bool,
    },
    /// Nœud de stockage : demande un bail au daemon avant d'ouvrir le port.
    Node {
        #[command(subcommand)]
        action: NodeAction,
    },
    /// Polygone Serverless — prépare une exécution en quorum (SERVERLESS-SPEC.md)
    Serverless {
        /// Commande déterministe que chaque nœud exécuterait
        #[arg(long, default_value = "true")]
        commande: String,
        /// Seuil du quorum (nœuds distincts, même résultat)
        #[arg(long, default_value_t = 4)]
        seuil: u8,
        /// Nombre de parts à produire (le protocole plafonne à 7)
        #[arg(long, default_value_t = 7)]
        parts: u8,
        /// Charge utile remise aux nœuds
        payload: Vec<String>,
    },
    /// Show your ML-KEM-1024 public key (what you share to receive)
    Clef,
    /// Rotate the ML-DSA signing key; the old key is revoked in the identity file
    #[command(name = "clef-tourner")]
    ClefTourner,
    /// Print this node's relay identifier (BLAKE3 of its ML-DSA signing key, 128 bits)
    Id,
    /// Run the real crypto self-test suite (exit 0 = all green)
    Test,
    /// Forensique locale — « voici ce que j'ai de toi : rien », énuméré
    Verite,
    /// Scénario guidé de 5 minutes — le message meurt, regardez
    PremierSoir {
        /// Durée de vie réelle des fragments (défaut : 30 s)
        #[arg(long, default_value_t = 30)]
        ttl: u64,
    },
    /// La clé comme objet social — à montrer et échanger en personne
    Carte,
    /// Local AI service (D4 pilot) — talks to local Ollama, no cloud
    Petals {
        #[command(subcommand)]
        action: PetalsAction,
    },
    /// Mode duress — destroy local identity + received files (Axiome 5).
    /// Requires --confirmer (explicit signal, irreversible).
    Duress {
        /// Explicit confirmation that the destruction is intended
        #[arg(long)]
        confirmer: bool,
    },
    /// Polygone Hide — SOCKS5 proxy through the blind relay (HIDE-SPEC.md)
    Hide {
        /// Relay address
        #[arg(long, default_value = "127.0.0.1:7000")]
        via: String,
        /// The exit node (node_id) that opens the real connection
        #[arg(long)]
        sortie: Option<String>,
        /// ML-KEM public key of the exit node (hex) — the destination is
        /// encrypted with it, the relay never sees it
        #[arg(long, short = 'd')]
        dest: Option<String>,
        /// SOCKS5 listen address
        #[arg(long, default_value = hide::DEFAULT_LISTEN)]
        ecoute: String,
        /// Planifier un chemin multi-hop (dry-run) : node_id:clef,node_id:clef
        /// — valide le circuit, n'ouvre aucune connexion
        #[arg(long)]
        circuit: Option<String>,
    },
}

/// Affiche un grant vérifié et renvoie son résultat métier. Une réponse
/// signée avec ok=false est un refus/échec, jamais une réussite de réputation.
fn display_compute_grant(grant: &str, label: &str) -> Result<bool> {
    let body: serde_json::Value = serde_json::from_str(grant)?;
    if body.get("ok").and_then(serde_json::Value::as_bool) != Some(true) {
        let reason = body
            .get("error")
            .or_else(|| body.get("refus"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or("refus sans motif fourni");
        println!("  ✖ {label} refusé/échoué : {reason}");
        return Ok(false);
    }

    println!(
        "  ⬡ {label} (nœud {}) :",
        body.get("node")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("inconnu")
    );
    if let Some(level) = body.get("verification").and_then(serde_json::Value::as_str) {
        let limit = body
            .get("verification_limit")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("signature de l'auteur, pas preuve de justesse");
        println!("  vérification : {level} — {limit}");
    }
    if body
        .get("recompute")
        .and_then(|recompute| recompute.get("match"))
        .and_then(serde_json::Value::as_bool)
        == Some(false)
    {
        println!("  ✖ recalcul local DIVERGENT — la sortie du producteur n'a pas été reproduite");
    }
    if let Some(output) = body.get("output").and_then(serde_json::Value::as_str) {
        println!("{output}");
    }
    Ok(true)
}

#[cfg(test)]
mod compute_grant_tests {
    use super::display_compute_grant;

    #[test]
    fn signed_refusal_is_not_a_success() {
        let grant = r#"{"ok":false,"error":"sandbox refusée"}"#;
        assert!(!display_compute_grant(grant, "RÉSULTAT").unwrap());
    }

    #[test]
    fn signed_success_is_accepted() {
        let grant = r#"{"ok":true,"node":"ghost","output":"42"}"#;
        assert!(display_compute_grant(grant, "RÉSULTAT").unwrap());
    }
}

/// Demande un statut au daemon avec les délais courts d'une sonde.
/// L'absence du daemon produit une erreur unique, jamais un rapport rempli de zéros.
fn doctor_status(socket_path: &Path) -> Result<Reply> {
    let mut client = DaemonClient::connect_with_timeout(socket_path, Duration::from_secs(2))
        .map_err(|error| {
            anyhow::anyhow!(
                "daemon indisponible sur {} : {error}",
                socket_path.display()
            )
        })?;
    client
        .status()
        .map_err(|error| anyhow::anyhow!("lecture du statut daemon impossible : {error}"))
}

fn gibibytes(bytes: u64) -> f64 {
    bytes as f64 / 1_073_741_824.0
}

/// Formate les valeurs publiées par le ledger du daemon, puis l'isolation que
/// le noyau applique réellement à ce processus — mesurée, jamais déclarée.
fn format_doctor_report(
    reply: &Reply,
    isolation: Result<node_isolation::OsIsolation, String>,
) -> Result<String> {
    let (tier, seq, capacity, cpu_cores, free_vram_mb, storage) = match reply {
        Reply::Status {
            tier,
            seq,
            ram_capacity: Some(capacity),
            cpu_cores,
            gpu_free_vram_mb,
            storage_capacity,
            ..
        } => (
            tier,
            seq,
            capacity,
            *cpu_cores,
            *gpu_free_vram_mb,
            storage_capacity.as_deref(),
        ),
        Reply::Status {
            ram_capacity: None, ..
        } => {
            anyhow::bail!("le daemon répond, mais ne publie pas Capacity ; mettez-le à jour avant de conclure")
        }
        other => anyhow::bail!("réponse inattendue du daemon : {other:?}"),
    };

    if capacity.reservable > capacity.measured || capacity.enforced > capacity.reservable {
        anyhow::bail!("le daemon a publié une Capacity incohérente ; aucune conclusion n'est sûre");
    }

    let conclusion = if capacity.is_enforced() {
        "contraint par l'OS"
    } else if capacity.enforced == 0 {
        "licence à respecter — aucune limite OS confirmée"
    } else {
        "licence à respecter — la limite OS ne couvre qu'une partie du réservable"
    };

    let mut report = format!(
        "⬡ Polygone doctor\n  Daemon : joignable · palier {tier} · séquence {seq}\n\n  Capacité RAM\n    Mesurée   : {:.2} Gio\n    Réservable: {:.2} Gio\n    Contrainte: {:.2} Gio\n\n",
        gibibytes(capacity.measured),
        gibibytes(capacity.reservable),
        gibibytes(capacity.enforced),
    );

    report.push_str("  Isolation OS (ce processus)\n");
    match isolation {
        Ok(state) => {
            let memory = match state.memory_max {
                Some(bytes) => format!("{:.2} Gio", gibibytes(bytes)),
                None => "illimitée (memory.max = max)".to_string(),
            };
            let cpu = match state.cpu_quota {
                Some((quota, period)) => format!(
                    "{:.2} % ({:.2} cœur(s))",
                    quota as f64 * 100.0 / period as f64,
                    quota as f64 / period as f64
                ),
                None => "illimitée (cpu.max = max)".to_string(),
            };
            report.push_str(&format!(
                "    Cgroup    : {} (scope Polygone : {})\n    MemoryMax : {memory}\n    CPUQuota  : {cpu}\n",
                state.cgroup_path,
                if state.dedicated_scope { "oui" } else { "non" },
            ));
        }
        Err(reason) => report.push_str(&format!("    Non mesurée : {reason}\n")),
    }

    // Device Reuse : les entrées sont les mesures publiées par le daemon, pas
    // une déclaration de l'utilisateur. Batterie, température et activité
    // restent `None` : le rapport dit alors que la contrainte n'a pas été
    // évaluée, au lieu de la supposer favorable.
    let capabilities = reuse::DeviceCapabilities {
        cpu_cores,
        ram_bytes: capacity.measured,
        free_storage_bytes: storage.map(|capacity| capacity.measured),
        free_vram_bytes: Some(u64::from(free_vram_mb) * reuse::MIB),
        battery_percent: None,
        thermal_celsius: None,
        user_active: None,
    };
    report.push_str("\n  Rôles proposés (Device Reuse — proposition, aucune activation)\n");
    for suggestion in reuse::suggest_roles(&capabilities) {
        let verdict = if suggestion.eligible {
            "éligible"
        } else {
            "non éligible"
        };
        report.push_str(&format!(
            "    {:<13}: {verdict} — {}\n",
            suggestion.role.as_str(),
            suggestion.reasons.join(" ; ")
        ));
    }

    report.push_str(&format!("\n  Conclusion : {conclusion}\n"));
    Ok(report)
}

fn doctor_report(socket_path: &Path) -> Result<String> {
    let reply = doctor_status(socket_path)?;
    format_doctor_report(&reply, node_isolation::measure_current())
}

/// Arrête le service qui tient un bail, s'il y en a un de vivant.
///
/// Le bail ne se « libère » pas par un message : il meurt avec la connexion du
/// processus qui le détient (`LedgerGuard` côté daemon). Arrêter ce processus
/// est donc la seule libération honnête depuis un autre processus.
fn stop_serving_node() -> String {
    let path = node::default_state_path();
    let (state, age) = match node::read_state(&path) {
        Ok(read) => read,
        Err(_) => return "aucun service en cours".to_string(),
    };
    if state.status != "serving" || age > node::STATE_STALE_AFTER {
        return format!(
            "aucun service vivant (dernier état : {} · {} s)",
            state.status,
            age.as_secs()
        );
    }
    #[cfg(unix)]
    {
        let killed = std::process::Command::new("kill")
            .args(["-TERM", &state.pid.to_string()])
            .status()
            .map(|status| status.success())
            .unwrap_or(false);
        if killed {
            format!("service pid {} arrêté (SIGTERM), bail rendu", state.pid)
        } else {
            format!(
                "service pid {} non arrêté — arrêtez-le à la main avant la reprise",
                state.pid
            )
        }
    }
    #[cfg(not(unix))]
    {
        format!(
            "service pid {} : arrêtez-le à la main (aucun SIGTERM sur cette plateforme)",
            state.pid
        )
    }
}

fn run_reprendre() -> Result<()> {
    let path = pause::default_intent_path();
    let message = pause::resume(&path, pause::now_unix(), || {
        let socket = socket::socket_path();
        let mut client = DaemonClient::connect_with_timeout(&socket, Duration::from_secs(2))
            .map_err(|error| format!("daemon indisponible sur {} : {error}", socket.display()))?;
        let reply = client
            .allocate(node::DEFAULT_BAIL_RAM_GB, node::DEFAULT_BAIL_CPU_CORES)
            .map_err(|error| format!("allocation refusée : {error}"))?;
        match reply {
            Reply::AllocGranted {
                allocation_id,
                cpu_cores,
                ram_gb,
                enforced,
                ..
            } => {
                // Rendu aussitôt : `reprendre` prouve que le bail est de nouveau
                // accordable ; c'est `node servir` qui le tiendra ensuite.
                let released = client.release(&allocation_id).is_ok();
                Ok(format!(
                    "{ram_gb:.2} Gio · {cpu_cores} cœur(s){} — rendu aussitôt : {}",
                    if enforced {
                        " · contraint par l'OS"
                    } else {
                        ""
                    },
                    if released { "oui" } else { "non" }
                ))
            }
            other => Err(format!("réponse inattendue du daemon : {other:?}")),
        }
    })
    .map_err(anyhow::Error::msg)?;
    println!("⬡ {message}");
    Ok(())
}

fn run_desinstaller(confirmer: bool) -> Result<()> {
    let home = uninstall::home_dir().map_err(anyhow::Error::msg)?;
    let plan = uninstall::plan(&home);
    println!(
        "⬡ Désinstallation — {} chemin(s) existant(s) sur {} énuméré(s)",
        plan.existing_count(),
        plan.entries.len() + plan.binaries.len()
    );
    for entry in &plan.entries {
        println!("  {} {}", if entry.exists { "•" } else { "·" }, entry.label);
    }
    for binary in &plan.binaries {
        println!(
            "  {} {}",
            if binary.exists() { "•" } else { "·" },
            binary.display()
        );
    }

    if !confirmer {
        anyhow::bail!("confirmation requise : rien n'a été supprimé (relancez avec --confirmer)");
    }

    let socket = socket::socket_path();
    if DaemonClient::connect_with_timeout(&socket, Duration::from_millis(300)).is_ok() {
        anyhow::bail!(
            "le daemon répond sur {} : arrêtez-le avant de désinstaller",
            socket.display()
        );
    }

    let removed = uninstall::remove(&plan).map_err(anyhow::Error::msg)?;
    for path in &removed {
        println!("  ✓ supprimé : {path}");
    }
    println!(
        "\n⬡ {} chemin(s) supprimé(s). Retirez la ligne PATH de votre rc shell si l'installateur en a ajouté une.",
        removed.len()
    );
    Ok(())
}

fn run_pause(duree: &str) -> Result<()> {
    let duration = pause::parse_duration(duree).map_err(anyhow::Error::msg)?;
    let intent = pause::intent(pause::now_unix(), duration).map_err(anyhow::Error::msg)?;
    let stopped = stop_serving_node();
    let path = pause::default_intent_path();
    pause::write_intent(&path, &intent).map_err(anyhow::Error::msg)?;
    println!("⬡ Pause enregistrée : {}", pause::describe(&intent));
    println!("  Bail : {stopped}");
    println!("  Intention : {}", path.display());
    println!("  Reprise : `polygone reprendre` (refusée une fois la pause expirée)");
    Ok(())
}

#[cfg(test)]
mod doctor_tests {
    #[cfg(target_os = "linux")]
    use super::node_isolation::measure_from;
    use super::node_isolation::OsIsolation;
    use super::{doctor_report, format_doctor_report};
    use polygoned::socket::{Capacity, Reply};
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};
    const GIO: u64 = 1_073_741_824;

    /// Isolation neutre : cgroup de session, aucune limite. Les tests qui ne
    /// parlent pas d'isolation passent celle-ci et n'en disent rien.
    fn unlimited() -> Result<OsIsolation, String> {
        Ok(OsIsolation {
            cgroup_path: "/user.slice/user-1000.slice/session-1.scope".into(),
            dedicated_scope: false,
            memory_max: None,
            cpu_quota: None,
        })
    }

    fn status(capacity: Option<Capacity>) -> Reply {
        Reply::Status {
            id: None,
            tier: "balanced".into(),
            seq: 7,
            max_cpu_percent: 50,
            max_ram_percent: 50,
            max_bandwidth_percent: 60,
            max_gpu_percent: 40,
            ram_bytes: 1_073_741_824,
            ram_mb: 1024,
            bandwidth_mbps: 10,
            cpu_cores: 2,
            gpu_free_vram_mb: 0,
            shrinking: false,
            uptime_secs: 60,
            ram_capacity: capacity.map(Box::new),
            storage_capacity: None,
        }
    }

    /// Les rôles Device Reuse ne sont proposés qu'à partir des mesures du
    /// daemon ; un stockage non mesuré est refusé, pas supposé disponible.
    #[test]
    fn doctor_proposes_device_reuse_roles_from_measured_capacity() {
        let reply = status(Some(Capacity {
            measured: 8 * GIO,
            reservable: 3 * GIO,
            enforced: 0,
        }));
        let report = format_doctor_report(&reply, unlimited()).unwrap();
        assert!(report.contains("Rôles proposés (Device Reuse"), "{report}");
        assert!(report.contains("relais"), "{report}");
        assert!(
            report.contains("non éligible — stockage libre non mesuré"),
            "{report}"
        );
        assert!(report.contains("aucune activation"), "{report}");
    }

    #[test]
    fn doctor_reports_licence_when_nothing_enforced() {
        let reply = status(Some(Capacity {
            measured: 8 * 1_073_741_824,
            reservable: 3 * 1_073_741_824,
            enforced: 0,
        }));
        let report = format_doctor_report(&reply, unlimited()).expect("capacités publiées");
        assert!(report.contains("Mesurée   : 8.00 Gio"), "{report}");
        assert!(report.contains("Réservable: 3.00 Gio"), "{report}");
        assert!(report.contains("Contrainte: 0.00 Gio"), "{report}");
        assert!(report.contains("licence à respecter"), "{report}");
        assert!(!report.contains("contraint par l'OS"), "{report}");
    }

    #[test]
    fn doctor_fails_with_one_cause_when_daemon_absent() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let socket = PathBuf::from(format!(
            "/tmp/polygone-doctor-{}-{nonce}.sock",
            std::process::id()
        ));
        let error = doctor_report(&socket)
            .expect_err("daemon absent : le doctor doit échouer")
            .to_string();
        assert!(error.starts_with("daemon indisponible sur "), "{error}");
        assert_eq!(
            error.lines().count(),
            1,
            "une seule cause, pas un tableau vide : {error}"
        );
    }

    #[test]
    fn doctor_refuses_a_legacy_status_without_capacity() {
        let error = format_doctor_report(&status(None), unlimited())
            .expect_err("Capacity absente")
            .to_string();
        assert!(error.contains("ne publie pas Capacity"), "{error}");
    }

    #[test]
    fn doctor_only_claims_os_enforcement_when_it_covers_reservable_capacity() {
        let reply = status(Some(Capacity {
            measured: 8 * 1_073_741_824,
            reservable: 3 * 1_073_741_824,
            enforced: 3 * 1_073_741_824,
        }));
        let report = format_doctor_report(&reply, unlimited()).unwrap();
        assert!(
            report.contains("Conclusion : contraint par l'OS"),
            "{report}"
        );
    }

    /// La preuve CLI.1 : le chiffre affiché est celui du fichier cgroup, relu
    /// dans le même test — pas une valeur recopiée depuis la configuration.
    #[cfg(target_os = "linux")]
    #[test]
    fn doctor_prints_the_cgroup_limit_it_read() {
        let reply = status(Some(Capacity {
            measured: 8 * GIO,
            reservable: 3 * GIO,
            enforced: 0,
        }));
        let memory_file = "4294967296\n";
        let cpu_file = "200000 100000\n";
        let isolation = measure_from("0::/user.slice/session-1.scope\n", |path| {
            if path.ends_with("memory.max") {
                Ok(memory_file.to_string())
            } else {
                Ok(cpu_file.to_string())
            }
        })
        .expect("cgroup lisible");

        let expected_gio = memory_file.trim().parse::<u64>().unwrap() as f64 / GIO as f64;
        let report = format_doctor_report(&reply, Ok(isolation)).unwrap();
        assert!(
            report.contains(&format!("MemoryMax : {expected_gio:.2} Gio")),
            "{report}"
        );
        assert!(
            report.contains("CPUQuota  : 200.00 % (2.00 cœur(s))"),
            "{report}"
        );
        assert!(report.contains("scope Polygone : non"), "{report}");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn doctor_says_unlimited_instead_of_zero_when_the_kernel_says_max() {
        let reply = status(Some(Capacity {
            measured: 8 * GIO,
            reservable: 3 * GIO,
            enforced: 0,
        }));
        let isolation = measure_from("0::/user.slice/session-1.scope\n", |path| {
            if path.ends_with("memory.max") {
                Ok("max\n".to_string())
            } else {
                Ok("max 100000\n".to_string())
            }
        })
        .unwrap();
        let report = format_doctor_report(&reply, Ok(isolation)).unwrap();
        assert!(report.contains("MemoryMax : illimitée"), "{report}");
        assert!(report.contains("CPUQuota  : illimitée"), "{report}");
        assert!(!report.contains("MemoryMax : 0.00 Gio"), "{report}");
    }

    #[test]
    fn doctor_reports_a_failed_measurement_instead_of_inventing_a_limit() {
        let reply = status(Some(Capacity {
            measured: 8 * GIO,
            reservable: 3 * GIO,
            enforced: 0,
        }));
        let report = format_doctor_report(
            &reply,
            Err("lecture de /sys/fs/cgroup/x/memory.max impossible".into()),
        )
        .unwrap();
        assert!(report.contains("Non mesurée : lecture de"), "{report}");
        assert!(!report.contains("MemoryMax :"), "{report}");
    }
}

async fn run_node(action: NodeAction) -> Result<()> {
    use std::net::SocketAddr;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    match action {
        NodeAction::Status => {
            println!(
                "{}",
                node::status_report(
                    &node::default_state_path(),
                    std::time::Duration::from_secs(2)
                )
            );
        }
        NodeAction::Servir {
            ecoute,
            ram_gb,
            coeurs,
            isoler,
        } => {
            let listen: SocketAddr = ecoute.parse().map_err(|error| {
                anyhow::anyhow!("adresse d'écoute invalide « {ecoute} » : {error}")
            })?;
            if !ram_gb.is_finite() || ram_gb <= 0.0 {
                anyhow::bail!("--ram-gb doit être un nombre fini strictement positif");
            }
            if coeurs == 0 {
                anyhow::bail!("--coeurs doit être supérieur à zéro");
            }

            #[cfg(target_os = "linux")]
            if isoler {
                let memory_bytes =
                    node_isolation::requested_memory_bytes(ram_gb).map_err(anyhow::Error::msg)?;
                if !node_isolation::current_scope_matches(ram_gb, coeurs)
                    .map_err(anyhow::Error::msg)?
                {
                    let executable = std::env::current_exe()?;
                    let timestamp = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)?
                        .as_nanos();
                    let unit = format!("polygone-node-{}-{timestamp}", std::process::id());
                    let memory_property = format!("MemoryMax={memory_bytes}");
                    let cpu_property = format!("CPUQuota={}%", u64::from(coeurs) * 100);
                    let mut command = tokio::process::Command::new("systemd-run");
                    command
                        .args([
                            "--user",
                            "--scope",
                            "--slice=app.slice",
                            "--unit",
                            &unit,
                            "--property",
                            &memory_property,
                            "--property",
                            &cpu_property,
                            "--property",
                            "TasksMax=64",
                            "--collect",
                            "--quiet",
                            "--",
                        ])
                        .arg(executable)
                        .args(std::env::args_os().skip(1));
                    let status = command.status().await.map_err(|error| {
                        anyhow::anyhow!(
                            "impossible de lancer systemd-run --user : {error}. Vérifiez le gestionnaire systemd utilisateur et cgroup v2"
                        )
                    })?;
                    if !status.success() {
                        anyhow::bail!(
                            "systemd n'a pas pu créer le scope isolé (code {status}). Aucun nœud n'a été démarré."
                        );
                    }
                    return Ok(());
                }
                println!(
                    "  Isolation OS vérifiée : MemoryMax={memory_bytes} octets · CPUQuota={} % · cgroup v2",
                    u64::from(coeurs) * 100
                );
            }

            #[cfg(not(target_os = "linux"))]
            if isoler {
                anyhow::bail!("--isoler est actuellement disponible uniquement sous Linux avec systemd et cgroup v2");
            }

            let config = node::NodeConfig {
                listen,
                bail_ram_gb: ram_gb,
                bail_cpu_cores: coeurs,
                ..node::NodeConfig::default()
            };
            let socket = polygoned::socket::socket_path();
            let node = node::Node::acquire(&socket, config).map_err(anyhow::Error::new)?;

            // Le protocole v1 accorde exactement la demande ou la refuse ;
            // vérifier tout de même cette propriété avant de servir sous un
            // plafond calculé à partir de la demande. Si un futur daemon répond
            // par une allocation partielle, fermer le nœud rend le bail via la
            // fermeture de sa connexion (LedgerGuard).
            #[cfg(target_os = "linux")]
            if isoler {
                let requested =
                    node_isolation::requested_memory_bytes(ram_gb).map_err(anyhow::Error::msg)?;
                let granted = node_isolation::requested_memory_bytes(node.lease().ram_gb)
                    .map_err(anyhow::Error::msg)?;
                if requested != granted || node.lease().cpu_cores != coeurs {
                    drop(node);
                    anyhow::bail!(
                        "le daemon a accordé une ressource différente du scope OS : bail rendu, aucun service lancé"
                    );
                }
            }

            let addr = node.local_addr()?;
            println!(
                "⬡ Bail accordé : {:.2} Gio · {} cœur(s)",
                node.lease().ram_gb,
                node.lease().cpu_cores
            );
            println!(
                "  Service de blocs sur {addr} · plafond auto-imposé : {} transfert(s)",
                node.max_transfers()
            );
            if isoler {
                println!("  Ctrl+C pour arrêter, rendre le bail et détruire le scope transitoire.");
            } else {
                println!("  ATTENTION : aucune limite CPU/RAM OS n'est appliquée. Le plafond de transferts est seulement auto-imposé.");
            }

            let stop = Arc::new(AtomicBool::new(false));
            let signal_stop = Arc::clone(&stop);
            let signal_task = tokio::spawn(async move {
                if tokio::signal::ctrl_c().await.is_ok() {
                    signal_stop.store(true, Ordering::SeqCst);
                }
            });
            let outcome = tokio::task::spawn_blocking(move || node.serve(stop)).await??;
            signal_task.abort();
            println!(
                "⬡ Nœud arrêté : {} · {} bloc(s) · {} octet(s) · bail rendu : {}",
                outcome.reason.label(),
                outcome.served_blocks,
                outcome.served_bytes,
                if outcome.released {
                    "oui"
                } else {
                    "non (daemon perdu)"
                }
            );
        }
    }
    Ok(())
}

#[derive(Subcommand, Debug)]
enum PetalsAction {
    /// List installed models
    Models,
    /// Show Ollama status (models + count)
    Status,
    /// Ask the local model a question
    Ask {
        /// Question
        #[arg(required = true)]
        question: Vec<String>,
        /// Model to use (default: first installed)
        #[arg(long)]
        model: Option<String>,
    },
}

#[derive(Subcommand, Debug, Clone)]
enum NodeAction {
    /// Obtenir un bail puis servir le magasin de blocs jusqu'à Ctrl+C.
    Servir {
        /// Adresse d'écoute TCP (0.0.0.0 expose le service sur les interfaces réseau).
        #[arg(long, default_value = "0.0.0.0:7643")]
        ecoute: String,
        /// RAM demandée au daemon, en Gio.
        #[arg(long, default_value_t = 0.25)]
        ram_gb: f32,
        /// Cœurs demandés au daemon.
        #[arg(long, default_value_t = 1)]
        coeurs: u32,
        /// Sous Linux, relancer dans un scope systemd cgroup v2 avec MemoryMax/CPUQuota réels.
        #[arg(long)]
        isoler: bool,
    },
    /// Afficher le dernier état local et mesurer le mesh maintenant.
    Status,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    let level = if args.verbose { "debug" } else { "info" };
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(level)).init();

    // Duress runs before any identity materialization — destroying must not
    // first create.
    if let Some(Commands::Duress { confirmer }) = &args.command {
        if !confirmer {
            anyhow::bail!("mode duress : confirmez avec --confirmer (irréversible)");
        }
        println!("{}", duress::plan());
        println!();
        for line in duress::execute()? {
            println!("  ✓ {line}");
        }
        println!();
        println!("L'information n'existait pas. Elle ne traversera plus.");
        return Ok(());
    }

    // Le service de blocs et le diagnostic n'ont pas besoin de créer une identité.
    if let Some(Commands::Node { action }) = args.command.as_ref() {
        return run_node(action.clone()).await;
    }
    if let Some(Commands::Ecosysteme {
        relais,
        ollama,
        site,
        strict,
    }) = args.command.as_ref()
    {
        let site_dir = site
            .as_deref()
            .map(Path::new)
            .unwrap_or_else(|| Path::new("web"));
        let probes =
            polygone_client::ecosystem::probes(site_dir, relais, ollama, &socket::socket_path());
        let all_ready = polygone_client::ecosystem::print(&probes);
        if *strict && !all_ready {
            anyhow::bail!("écosystème incomplet (--strict)");
        }
        return Ok(());
    }
    if matches!(args.command.as_ref(), Some(Commands::Doctor)) {
        let report = doctor_report(&socket::socket_path())?;
        print!("{report}");
        return Ok(());
    }
    // Un profil écrit une config relue par polygoned : aucune identité, aucun
    // bail, aucun accès réseau.
    if let Some(Commands::Profil { nom, config }) = args.command.as_ref() {
        polygone_client::profile::run(nom.as_deref(), config.as_deref().map(std::path::Path::new))?;
        return Ok(());
    }
    // Pause et reprise ne créent pas d'identité : elles parlent au daemon et au
    // fichier d'intention, rien d'autre.
    if let Some(Commands::Pause { duree }) = args.command.as_ref() {
        run_pause(duree)?;
        return Ok(());
    }
    if matches!(args.command.as_ref(), Some(Commands::Reprendre)) {
        run_reprendre()?;
        return Ok(());
    }
    if let Some(Commands::Desinstaller { confirmer }) = args.command.as_ref() {
        run_desinstaller(*confirmer)?;
        return Ok(());
    }

    // First run: materialise the local identity (keys + pseudo).
    let mut identity = identity::LocalIdentity::load_or_create()?;

    match args.command {
        None | Some(Commands::Tui) => tui::run(identity).await?,
        Some(Commands::Demo { relay: _ }) => {
            demo::run()?;
        }
        Some(Commands::Envoyer {
            dest,
            via,
            a,
            fichier,
            message,
            stdin,
        }) => {
            let message = message.join(" ");

            // stdin mode: the message never appears in the shell history.
            // Mutually exclusive with --fichier.
            if stdin && fichier.is_some() {
                anyhow::bail!("--stdin et --fichier sont exclusifs");
            }
            // File mode: read the file, send its bytes.
            let payload: Vec<u8> = if stdin {
                std::io::read_to_string(std::io::stdin())?
                    .as_bytes()
                    .to_vec()
            } else {
                match &fichier {
                    Some(path) => std::fs::read(path)?,
                    None => message.as_bytes().to_vec(),
                }
            };
            if fichier.is_none() && message.is_empty() && !stdin {
                anyhow::bail!("rien à envoyer : passez un message, --fichier <chemin> ou --stdin");
            }

            // Network mode: route the fragments through a blind relay.
            if let Some(relay) = via {
                let dest_node = a.ok_or_else(|| {
                    anyhow::anyhow!(
                        "mode réseau : précisez le nœud destinataire avec --a <node_id>"
                    )
                })?;
                let recipient = match dest {
                    Some(hex) => polygone_core::crypto::kem::KemPublicKey::from_hex(&hex)?,
                    None => anyhow::bail!(
                        "mode réseau : précisez la clef du destinataire avec -d <clef>"
                    ),
                };
                let name = fichier.as_ref().map(|p| {
                    std::path::Path::new(p)
                        .file_name()
                        .map(|s| s.to_string_lossy().to_string())
                        .unwrap_or_else(|| p.clone())
                });
                let session = net::send_network(
                    &relay,
                    &dest_node,
                    &payload,
                    name.as_deref(),
                    &recipient,
                    &identity,
                )
                .await?;
                match &fichier {
                    Some(_) => println!("⬡ fichier envoyé via relay {relay} → {dest_node}"),
                    None => println!("⬡ message envoyé via relay {relay} → {dest_node}"),
                }
                println!("  session {session} · 7 fragments + KEM_CT (4 suffisent pour lire)");
                return Ok(());
            }

            // Mesh mode: --a <node> without --via → find the peer's relay on
            // the LAN (Phase 4), then route through it. Zero configuration.
            if let Some(dest_node) = a {
                let recipient = match dest {
                    Some(hex) => polygone_core::crypto::kem::KemPublicKey::from_hex(&hex)?,
                    None => anyhow::bail!("précisez la clef du destinataire avec -d <clef>"),
                };
                let peers = mesh::discover(std::time::Duration::from_secs(3))?;
                let peer = peers.iter().find(|p| p.node_id == dest_node).cloned().ok_or_else(|| {
                    anyhow::anyhow!(
                        "nœud {dest_node} introuvable sur le LAN — l'annonce-t-il (polygone ecouter --annoncer) ?"
                    )
                })?;
                let name = fichier.as_ref().map(|p| {
                    std::path::Path::new(p)
                        .file_name()
                        .map(|s| s.to_string_lossy().to_string())
                        .unwrap_or_else(|| p.clone())
                });
                let session = net::send_network(
                    &peer.relay,
                    &dest_node,
                    &payload,
                    name.as_deref(),
                    &recipient,
                    &identity,
                )
                .await?;
                match &fichier {
                    Some(_) => {
                        println!(
                            "⬡ fichier envoyé via mesh → {dest_node} (relay {})",
                            peer.relay
                        )
                    }
                    None => {
                        println!(
                            "⬡ message envoyé via mesh → {dest_node} (relay {})",
                            peer.relay
                        )
                    }
                }
                println!("  session {session} · 7 fragments + KEM_CT (4 suffisent pour lire)");
                return Ok(());
            }

            let recipient = match dest {
                Some(hex) => polygone_core::crypto::kem::KemPublicKey::from_hex(&hex)?,
                None => {
                    // No recipient: self-demo against a fresh keypair.
                    let (pk, _sk) = polygone_core::crypto::kem::generate_keypair()?;
                    println!("# pas de destinataire — envoi auto (démo) vers une clef fraîche");
                    pk
                }
            };
            let output = msg::send_bytes(&payload, &recipient)?;
            print!("{}", output.display());
        }
        Some(Commands::Recevoir { input }) => {
            let text = if input == "-" {
                std::io::read_to_string(std::io::stdin())?
            } else {
                std::fs::read_to_string(&input)?
            };
            let output = msg::SendOutput::parse(&text)?;
            let plain = msg::receive(&output, &identity.kem_secret_key()?)?;
            println!("{plain}");
        }
        Some(Commands::Ecouter {
            relay,
            annoncer,
            compute,
            hide,
        }) => {
            // Optionally announce on the LAN so peers can find us without a
            // hardcoded address (Phase 4 mesh).
            if annoncer {
                let node = net::node_id(&identity);
                let relay = relay.clone();
                tokio::spawn(async move {
                    let _ = mesh::announce(&node, &relay).await;
                });
            }
            if hide {
                return hide::exit_listen(&relay, &identity).await;
            }
            net::receive_network(&relay, &identity, compute).await?;
        }
        Some(Commands::Voisins { duree }) => {
            let peers = mesh::discover(std::time::Duration::from_secs(duree))?;
            mesh::print_peers(&peers);
        }
        Some(Commands::Annoncer { relay }) => {
            mesh::announce(&net::node_id(&identity), &relay).await?;
        }
        Some(Commands::Compute {
            duree,
            emprunter,
            executer,
            wasm,
            via,
            verifier_local,
        }) => {
            if verifier_local && executer.is_none() {
                anyhow::bail!(
                    "--verifier-local demande --executer <commande> : le recalcul L1 ne s'applique qu'à une tâche déterministe déclarée"
                );
            }
            // WASM mode: send a wasm module for sandboxed execution.
            if let Some(path) = wasm {
                let ghost = emprunter.ok_or_else(|| {
                    anyhow::anyhow!("--wasm demande --emprunter <node> (le nœud fantôme)")
                })?;
                let wasm_bytes = std::fs::read(&path)?;
                println!("⬡ RES — exécution WASM → {ghost} (via {via})");
                println!(
                    "  module : {path} ({} octets, wasmi sandbox)",
                    wasm_bytes.len()
                );
                let mut peers = net::load_peers()?;
                let success = match net::borrow_wasm(
                    &via,
                    &ghost,
                    &wasm_bytes,
                    &identity,
                    &mut peers,
                    std::time::Duration::from_secs(40),
                )
                .await?
                {
                    Some(grant) => display_compute_grant(&grant, "SORTIE WASM")?,
                    None => {
                        println!("  ✖ aucune réponse vérifiable — le nœud est-il en `ecouter --compute` ?");
                        false
                    }
                };
                reputation::ReputationTable::load().record(&ghost, success);
                net::save_peers(&peers)?;
                if !success {
                    anyhow::bail!(
                        "l'exécution WASM distante a échoué ou n'a pas fourni de grant vérifiable"
                    );
                }
                return Ok(());
            }

            // Execution mode: send a task to the ghost, get the sandboxed output.
            if let Some(command) = executer {
                let ghost = emprunter.ok_or_else(|| {
                    anyhow::anyhow!("--executer demande --emprunter <node> (le nœud fantôme)")
                })?;
                println!("⬡ RES — exécution sandboxée → {ghost} (via {via})");
                println!("  tâche : {command}");
                let mut peers = net::load_peers()?;
                let (shown, reproduced) = match net::borrow_compute(
                    &via,
                    &ghost,
                    &command,
                    &identity,
                    &mut peers,
                    std::time::Duration::from_secs(40),
                )
                .await?
                {
                    Some(grant) if verifier_local => {
                        // L1: run the same deterministic command here, in the
                        // same sandbox, and compare both outputs.
                        println!("  vérification L1 : recalcul local dans le même sandbox…");
                        match crate::exec::run_sandboxed(
                            &command,
                            256,
                            50,
                            std::time::Duration::from_secs(30),
                        ) {
                            Ok(local) => {
                                let (annotated, reproduced) =
                                    net::annotate_local_recomputation(&grant, &local)?;
                                // ADR-0003 : la vérification L1 laisse une preuve
                                // canonique persistée, accord comme divergence.
                                // Un échec d'écriture n'invalide pas le résultat.
                                match net::persist_l1_evidence(
                                    &annotated,
                                    &command,
                                    &ghost,
                                    &net::node_id(&identity),
                                ) {
                                    Ok(Some(path)) => {
                                        println!("  preuve L1 : {}", path.display())
                                    }
                                    Ok(None) => {}
                                    Err(error) => {
                                        println!("  ⚠ preuve L1 non persistée : {error}")
                                    }
                                }
                                (annotated, reproduced)
                            }
                            Err(error) => {
                                println!("  ⚠ recalcul local impossible : {error}");
                                (grant, false)
                            }
                        }
                    }
                    Some(grant) => (grant, true),
                    None => {
                        println!("  ✖ aucune réponse vérifiable — le nœud est-il en `ecouter --compute` ?");
                        (String::new(), false)
                    }
                };
                let accepted = if shown.is_empty() {
                    false
                } else {
                    display_compute_grant(&shown, "RÉSULTAT")?
                };
                // A result that was not reproduced is a failure for reputation:
                // the ghost did not deliver what an independent run produces.
                reputation::ReputationTable::load().record(&ghost, accepted && reproduced);
                net::save_peers(&peers)?;
                if !accepted {
                    anyhow::bail!("l'exécution compute distante a échoué ou n'a pas fourni de grant vérifiable");
                }
                if !reproduced {
                    anyhow::bail!("vérification L1 : le recalcul local ne reproduit pas la sortie du producteur");
                }
                return Ok(());
            }

            // Borrow mode: send a compute request to a ghost node.
            if let Some(ghost) = emprunter {
                println!("⬡ RES — demande de compute → {ghost} (via {via})");
                let task = "bench calcul Polygone (réservé au RES)".to_string();
                let mut peers = net::load_peers()?;
                let success = match net::borrow_compute(
                    &via,
                    &ghost,
                    &task,
                    &identity,
                    &mut peers,
                    std::time::Duration::from_secs(6),
                )
                .await?
                {
                    Some(grant) => display_compute_grant(&grant, "COMPUTE ACCORDÉ")?,
                    None => {
                        println!(
                            "  ✖ aucun grant vérifiable — le nœud est-il en `ecouter --compute` ?"
                        );
                        false
                    }
                };
                reputation::ReputationTable::load().record(&ghost, success);
                net::save_peers(&peers)?;
                if !success {
                    anyhow::bail!(
                        "le prêt compute a été refusé ou n'a pas fourni de grant vérifiable"
                    );
                }
                return Ok(());
            }
            println!("⬡ RES — ressources et nœuds fantômes");
            println!();
            match mesh::free_ram_mb() {
                Some(ram) => println!("  ce nœud : {ram} Mo de RAM libre"),
                None => println!("  ce nœud : RAM libre inconnue"),
            }
            let peers = mesh::discover(std::time::Duration::from_secs(duree))?;
            println!();
            if peers.is_empty() {
                println!("  aucun nœud fantôme sur le LAN (lancez « polygone annoncer » ailleurs)");
            } else {
                println!("  nœuds fantômes trouvés (prêts à prêter du compute) :");
                for p in &peers {
                    let rep = reputation::ReputationTable::load();
                    match p.free_ram_mb {
                        Some(ram) => println!(
                            "    · {}  →  relay {}  · {ram} Mo libres  · réputation {}%",
                            p.node_id,
                            p.relay,
                            rep.score_of(&p.node_id)
                        ),
                        None => println!(
                            "    · {}  →  relay {}  · réputation {}%",
                            p.node_id,
                            p.relay,
                            rep.score_of(&p.node_id)
                        ),
                    }
                }
            }
            println!();
            println!("  (prêt et exécution disponibles avec --emprunter <node> --via <relay> dès qu'un pair est en écoute)");
        }
        Some(Commands::Serverless {
            commande,
            seuil,
            parts,
            payload,
        }) => {
            let ok = serverless::run(&commande, &payload.join(" "), seuil, parts)?;
            if !ok {
                anyhow::bail!("vérification locale de la fragmentation échouée");
            }
        }
        Some(Commands::Clef) => {
            println!("{}", identity.kem_pk_hex);
            println!(
                "# fichier identité : {}",
                identity::LocalIdentity::path().display()
            );
        }
        Some(Commands::ClefTourner) => {
            let outcome = identity.rotate_sign_key()?;
            identity.save()?;
            let old_accepted = identity.accepts_signature_from(&outcome.revoked_pk_hex);
            let new_accepted = identity.accepts_signature_from(&outcome.new_pk_hex);
            println!("✔ nouvelle clef de signature : {}", outcome.new_pk_hex);
            println!("  ancienne clef révoquée : {}", outcome.revoked_pk_hex);
            println!(
                "  révocation effective : {} · nouvelle clef active : {}",
                !old_accepted, new_accepted
            );
            println!(
                "  rotation enregistrée à {} (unix)",
                outcome.rotated_at_unix
            );
            println!(
                "  preuve de rotation : {} octets, vérifiable avec l'ancienne clef publique",
                outcome.proof_hex.len() / 2
            );
            println!(
                "  identité KEM inchangée : {}",
                identity::LocalIdentity::path().display()
            );
        }
        Some(Commands::Id) => {
            // The relay identity is derived from the persistent local KEM key.
            // A fresh NodeId::random() here would print an unrelated ID on
            // every invocation, impossible for peers to use for routing.
            println!("{}", net::node_id(&identity));
        }
        Some(Commands::Test) => {
            self_test::run()?;
        }
        Some(Commands::Verite) => {
            product::verite()?;
        }
        Some(Commands::PremierSoir { ttl }) => {
            let carnet = product::premier_soir(&identity, ttl)?;
            // Le carnet est la sortie honnête : le scénario est rejouable.
            println!("\n\x1b[1m── Carnet d'observation ──────────────────────────────\x1b[0m");
            println!("{carnet}");
            println!("\x1b[1m─────────────────────────────────────────────────────────\x1b[0m");
        }
        Some(Commands::Carte) => {
            print!("{}", product::carte(&identity));
        }
        Some(Commands::Petals { action }) => match action {
            PetalsAction::Models => {
                for m in petals::models()? {
                    println!("{m}");
                }
            }
            PetalsAction::Status => {
                let models = petals::models()?;
                println!("⬡ Petals — IA locale via {}", petals::ollama_url());
                println!("  {} modèles installés :", models.len());
                for m in models {
                    println!("    · {m}");
                }
                println!("  (aucun cloud, aucun compte, aucune télémétrie)");
            }
            PetalsAction::Ask { question, model } => {
                let q = question.join(" ");
                let response = petals::ask(&q, model.as_deref())?;
                println!("{response}");
            }
        },
        Some(Commands::Node { .. }) => {
            unreachable!("handled before identity load")
        }
        Some(Commands::Doctor) => {
            unreachable!("handled before identity load")
        }
        Some(Commands::Ecosysteme { .. }) => {
            unreachable!("handled before identity load")
        }
        Some(Commands::Profil { .. }) => {
            unreachable!("handled before identity load")
        }
        Some(Commands::Pause { .. }) | Some(Commands::Reprendre) => {
            unreachable!("handled before identity load")
        }
        Some(Commands::Desinstaller { .. }) => {
            unreachable!("handled before identity load")
        }
        Some(Commands::Duress { .. }) => {
            unreachable!("handled before identity load")
        }
        Some(Commands::Hide {
            via,
            sortie,
            dest,
            ecoute,
            circuit,
        }) => {
            if let Some(spec) = circuit {
                hide::plan_circuit(&spec)?;
                return Ok(());
            }
            let sortie = sortie.ok_or_else(|| {
                anyhow::anyhow!("--sortie requis (ou --circuit pour planifier un chemin)")
            })?;
            let dest = dest.ok_or_else(|| {
                anyhow::anyhow!("--dest requis (ou --circuit pour planifier un chemin)")
            })?;
            hide::serve(&via, &sortie, &dest, &ecoute, identity).await?;
        }
    }
    Ok(())
}
