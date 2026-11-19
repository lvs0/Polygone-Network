//! `polygone-fabric` — protocole de calcul bidirectionnel (prototype).
//!
//! Deux usages :
//!
//! ```text
//! polygone-fabric serve --bind 127.0.0.1:7650 --fabric maison --cpu 8 --ram-mb 8192
//! polygone-fabric run --peer 127.0.0.1:7650 --kind blake3 --input "bonjour"
//! polygone-fabric demo
//! ```
//!
//! `serve` est un nœud qui exécute des tâches déterministes et signe ses
//! résultats. `run` planifie local/distant, exécute, vérifie la signature et
//! affiche le résultat en JSON. `demo` fait le parcours complet sur loopback
//! avec des nœuds en mémoire, sans réseau.
//!
//! Aucune promesse de vitesse : les estimations du planificateur sont un
//! modèle, et la démonstration affiche la latence réellement observée.

use std::collections::HashMap;
use std::net::{SocketAddr, TcpListener};
use std::sync::Arc;
use std::time::Instant;

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use polygone_core::sign::{generate_keypair, node_id_from_sign_pk_hex};
use polygone_core::task::{
    TaskConstraints, TaskId, TaskKind, TaskPermissions, TaskRequirements, TaskSpec,
};
use polygone_fabric::{
    chunk_data, now_unix, reconstruct, replica_plan, FabricEngine, MemoryNode, MemoryTransport,
    NodeCapabilities, Placement, PlacementPolicy, Scope, ServerNode, TaskEnvelope, TcpTransport,
    WIRE_VERSION,
};

#[derive(Parser, Debug)]
#[command(
    name = "polygone-fabric",
    about = "Polygone Fabric — calcul bidirectionnel, placement et résultats signés (prototype)"
)]
struct Args {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Servir des tâches déterministes sur un socket TCP.
    Serve {
        #[arg(long, default_value = "127.0.0.1:7650")]
        bind: String,
        /// Identifiant du réseau local (doit correspondre au demandeur).
        #[arg(long, default_value = "local")]
        fabric: String,
        /// Cœurs offerts.
        #[arg(long, default_value_t = 4)]
        cpu: u32,
        /// RAM offerte, en Mio.
        #[arg(long, default_value_t = 2048)]
        ram_mb: u64,
        /// Stockage offert, en Mio.
        #[arg(long, default_value_t = 1024)]
        storage_mb: u64,
        /// Périmètre global.
        #[arg(long)]
        global: bool,
        /// Consentement explicite au périmètre global.
        #[arg(long)]
        allow_global: bool,
        /// Charge simulée, en millièmes (0 = inactif).
        #[arg(long, default_value_t = 0)]
        load: u16,
    },
    /// Planifier, exécuter et vérifier une tâche (local ou distant).
    Run {
        /// Pair TCP à contacter (répétable).
        #[arg(long)]
        peer: Vec<String>,
        #[arg(long, default_value = "blake3")]
        kind: String,
        #[arg(long, default_value = "")]
        input: String,
        #[arg(long, default_value = "local")]
        scope: String,
        #[arg(long)]
        allow_global: bool,
        #[arg(long, default_value = "local")]
        fabric: String,
        /// Charge locale simulée, en millièmes.
        #[arg(long, default_value_t = 0)]
        load: u16,
        /// Cœurs locaux offerts.
        #[arg(long, default_value_t = 4)]
        cpu: u32,
        /// RAM locale offerte, en Mio.
        #[arg(long, default_value_t = 2048)]
        ram_mb: u64,
    },
    /// Parcours complet sans réseau : placement, exécution signée, réplication.
    Demo,
}

fn main() -> Result<()> {
    let args = Args::parse();
    match args.command {
        Command::Serve {
            bind,
            fabric,
            cpu,
            ram_mb,
            storage_mb,
            global,
            allow_global,
            load,
        } => serve(
            &bind,
            &fabric,
            cpu,
            ram_mb,
            storage_mb,
            global,
            allow_global,
            load,
        ),
        Command::Run {
            peer,
            kind,
            input,
            scope,
            allow_global,
            fabric,
            load,
            cpu,
            ram_mb,
        } => run(
            &peer,
            &kind,
            &input,
            &scope,
            allow_global,
            &fabric,
            load,
            cpu,
            ram_mb,
        ),
        Command::Demo => demo(),
    }
}

#[allow(clippy::too_many_arguments)]
fn serve(
    bind: &str,
    fabric: &str,
    cpu: u32,
    ram_mb: u64,
    storage_mb: u64,
    global: bool,
    allow_global: bool,
    load: u16,
) -> Result<()> {
    let keypair = generate_keypair().context("génération de la clé ML-DSA-65")?;
    let node_id = node_id_from_sign_pk_hex(&keypair.verifier.public_key().to_hex())
        .context("dérivation du NodeId")?;
    let capabilities = NodeCapabilities {
        node_id: node_id.clone(),
        scope: if global { Scope::Global } else { Scope::Local },
        global_consent: allow_global,
        local_fabric: fabric.to_string(),
        cpu_cores: cpu,
        ram_bytes: ram_mb.saturating_mul(1024 * 1024),
        storage_bytes: storage_mb.saturating_mul(1024 * 1024),
        gpu_vram_bytes: 0,
        load_permille: load.min(1000),
        battery_percent: None,
        thermal_celsius: None,
        user_active: false,
        rtt_ms: 0,
        bandwidth_kbps: 1_000_000,
        failure_rate_permille: 0,
        available: true,
    };
    let node = Arc::new(ServerNode::new(capabilities, keypair).map_err(anyhow::Error::msg)?);
    let listener = TcpListener::bind(bind).with_context(|| format!("bind {bind}"))?;
    println!("⬡ polygone-fabric — nœud {node_id}");
    println!(
        "  écoute {} · périmètre {} · consentement global {}",
        listener.local_addr()?,
        if global { "global" } else { "local" },
        allow_global
    );
    println!("  tâches déterministes echo/blake3, résultats signés ML-DSA-65");
    polygone_fabric::serve(listener, node).context("boucle serveur")
}

#[allow(clippy::too_many_arguments)]
fn run(
    peers: &[String],
    kind: &str,
    input: &str,
    scope: &str,
    allow_global: bool,
    fabric: &str,
    load: u16,
    cpu: u32,
    ram_mb: u64,
) -> Result<()> {
    let kind = TaskKind::parse(kind)
        .ok_or_else(|| anyhow::anyhow!("type inconnu : {kind} (attendu : echo|blake3)"))?;
    let scope = Scope::parse(scope)
        .ok_or_else(|| anyhow::anyhow!("périmètre inconnu : {scope} (attendu : local|global)"))?;
    let keypair = generate_keypair().context("génération de la clé ML-DSA-65")?;
    let local_id = node_id_from_sign_pk_hex(&keypair.verifier.public_key().to_hex())
        .context("dérivation du NodeId")?;
    let local = NodeCapabilities {
        node_id: local_id,
        scope: Scope::Local,
        global_consent: false,
        local_fabric: fabric.to_string(),
        cpu_cores: cpu,
        ram_bytes: ram_mb.saturating_mul(1024 * 1024),
        storage_bytes: 0,
        gpu_vram_bytes: 0,
        load_permille: load.min(1000),
        battery_percent: None,
        thermal_celsius: None,
        user_active: false,
        rtt_ms: 0,
        bandwidth_kbps: 1_000_000,
        failure_rate_permille: 0,
        available: true,
    };
    let mut engine = FabricEngine::new(local.clone(), keypair, PlacementPolicy::default())
        .map_err(anyhow::Error::msg)?;

    let addresses: Vec<SocketAddr> = peers
        .iter()
        .map(|peer| peer.parse::<SocketAddr>())
        .collect::<std::result::Result<_, _>>()
        .context("adresse de pair invalide")?;
    let transport = TcpTransport::new(addresses);

    let now = now_unix();
    let task = TaskSpec {
        id: new_task_id(),
        owner: "cli".to_string(),
        kind,
        input: input.as_bytes().to_vec(),
        requirements: TaskRequirements {
            cpu_cores: 1,
            ram_bytes: 1024 * 1024,
            storage_bytes: 0,
            gpu_vram_bytes: 0,
        },
        constraints: TaskConstraints {
            deadline_unix: now + 300,
            max_attempts: 3,
            permissions: TaskPermissions::default(),
        },
    };
    let envelope = TaskEnvelope {
        v: WIRE_VERSION,
        scope,
        global_consent: allow_global,
        fabric: fabric.to_string(),
        requester: local.node_id.clone(),
        task,
    };
    let outcome = engine.execute(&envelope, now, Some(&transport));
    println!("{}", serde_json::to_string_pretty(&outcome)?);
    if !outcome.verified {
        std::process::exit(2);
    }
    Ok(())
}

fn demo() -> Result<()> {
    println!("⬡ polygone-fabric — démonstration loopback, sans réseau");
    println!("  (les latences affichées sont mesurées, pas simulées)");

    let local_key = MemoryNode::keypair();
    let local_id = node_id_from_sign_pk_hex(&local_key.verifier.public_key().to_hex())
        .context("NodeId local")?;
    let local = NodeCapabilities {
        node_id: local_id,
        scope: Scope::Local,
        global_consent: false,
        local_fabric: "demo".to_string(),
        cpu_cores: 1,
        ram_bytes: 512 * 1024 * 1024,
        storage_bytes: 0,
        gpu_vram_bytes: 0,
        load_permille: 980,
        battery_percent: None,
        thermal_celsius: None,
        user_active: true,
        rtt_ms: 0,
        bandwidth_kbps: 1_000_000,
        failure_rate_permille: 0,
        available: true,
    };
    let mut engine = FabricEngine::new(local, local_key, PlacementPolicy::default())
        .map_err(anyhow::Error::msg)?;

    let remote_key = MemoryNode::keypair();
    let remote_id = node_id_from_sign_pk_hex(&remote_key.verifier.public_key().to_hex())
        .context("NodeId distant")?;
    let remote = NodeCapabilities {
        node_id: remote_id.clone(),
        scope: Scope::Local,
        global_consent: false,
        local_fabric: "demo".to_string(),
        cpu_cores: 16,
        ram_bytes: 16 * 1024 * 1024 * 1024,
        storage_bytes: 0,
        gpu_vram_bytes: 0,
        load_permille: 50,
        battery_percent: None,
        thermal_celsius: None,
        user_active: false,
        rtt_ms: 2,
        bandwidth_kbps: 1_000_000,
        failure_rate_permille: 0,
        available: true,
    };
    let transport = MemoryTransport::new(vec![
        MemoryNode::new(remote, remote_key).map_err(anyhow::Error::msg)?
    ]);

    let now = now_unix();
    let task = TaskSpec {
        id: new_task_id(),
        owner: "demo".to_string(),
        kind: TaskKind::Blake3,
        input: b"le calcul quitte la machine occupee".to_vec(),
        requirements: TaskRequirements {
            cpu_cores: 2,
            ram_bytes: 1024 * 1024,
            storage_bytes: 0,
            gpu_vram_bytes: 0,
        },
        constraints: TaskConstraints {
            deadline_unix: now + 300,
            max_attempts: 2,
            permissions: TaskPermissions::default(),
        },
    };
    let envelope = TaskEnvelope {
        v: WIRE_VERSION,
        scope: Scope::Local,
        global_consent: false,
        fabric: "demo".to_string(),
        requester: engine.local.node_id.clone(),
        task,
    };
    let started = Instant::now();
    let outcome = engine.execute(&envelope, now, Some(&transport));
    let elapsed = started.elapsed();
    println!("\n1. placement et exécution");
    println!("   placement : {:?}", outcome.placement);
    println!("   vérifié   : {}", outcome.verified);
    println!("   tentatives: {}", outcome.attempts);
    println!("   durée     : {} ms", elapsed.as_millis());
    if let Some(result) = &outcome.result {
        println!("   producteur: {}", result.producer);
        println!("   hash      : {}", result.output_hash);
    }
    if !outcome.verified {
        bail!(
            "la démonstration n'a pas produit de résultat vérifié : {:?}",
            outcome.error
        );
    }

    println!("\n2. stockage adressé par contenu + réplication (hors Babel)");
    let data = vec![42u8; 12_000];
    let (manifest, payloads) = chunk_data(&data, 4096).context("découpage")?;
    let nodes = vec![
        ("node-a".to_string(), 1_000_000u64),
        ("node-b".to_string(), 1_000_000u64),
        ("node-c".to_string(), 1_000_000u64),
    ];
    let plan = replica_plan(&manifest, &nodes, 2).context("plan de réplication")?;
    println!("   blocs     : {}", manifest.chunks.len());
    println!("   racine    : {}", manifest.root);
    println!(
        "   disponible avec node-b + node-c : {}",
        plan.is_available(&["node-b".to_string(), "node-c".to_string()])
    );
    let map: HashMap<_, _> = payloads.into_iter().collect();
    let rebuilt = reconstruct(&manifest, &map).context("reconstruction")?;
    println!("   reconstruction identique : {}", rebuilt == data);

    println!("\nLimites : aucun TLS, demande non authentifiée, deux nœuds sur une machine.");
    println!("Le nom « Babel » reste ouvert (ADR-014) : ce module ne le revendique pas.");
    Ok(())
}

fn new_task_id() -> TaskId {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    TaskId::parse(format!("task_{}_{}", std::process::id(), nanos))
        .expect("identifiant de tâche valide")
}

#[allow(dead_code)]
fn placement_label(placement: &Placement) -> &'static str {
    match placement {
        Placement::Local => "local",
        Placement::Remote { .. } => "distant",
        Placement::Refused { .. } => "refusé",
    }
}
