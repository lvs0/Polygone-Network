//! Polygone — the unified product command.
//!
//! ```text
//! polygone                  → la TUI (2 commandes : envoyer / quitter)
//! polygone demo             → démo E2E post-quantique complète
//! polygone envoyer <clef> <message> → chiffrer + fragmenter (wire text)
//! polygone recevoir [fichier]       → reconstruire + déchiffrer (4/7)
//! polygone clef                     → votre clef publique (à partager)
//! polygone id                       → identité nœud
//! ```
//!
//! « On voit rien. Et c'est comme ça que ça devrait être. »

mod demo;
mod duress;
mod exec;
mod identity;
mod mesh;
mod msg;
mod net;
mod petals;
mod product;
mod reputation;
mod self_test;
#[cfg(test)]
mod testutil;
mod tui;

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(
    name = "polygone",
    version = env!("CARGO_PKG_VERSION"),
    about = "⬡ POLYGONE — \"On voit rien. Et c'est comme ça que ça devrait être.\"",
    long_about = concat!(
        "Post-quantum ephemeral transit network.\n\n",
        "ML-KEM-1024 · ML-DSA-65 · AES-256-GCM · BLAKE3 · Shamir 4-of-7\n\n",
        "Sans commande : la TUI. Sinon : demo, envoyer, recevoir, clef, id.\n",
        "Aucun compte. Aucun serveur. Aucune télémétrie.",
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
    },
    /// Show your ML-KEM-1024 public key (what you share to receive)
    Clef,
    /// Print this node's random NodeId
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

    // First run: materialise the local identity (keys + pseudo).
    let identity = identity::LocalIdentity::load_or_create()?;

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
        }) => {
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
                let mut peers = net::load_peers();
                match net::borrow_wasm(
                    &via,
                    &ghost,
                    &wasm_bytes,
                    &identity,
                    &mut peers,
                    std::time::Duration::from_secs(40),
                )
                .await?
                {
                    Some(grant) => {
                        let body: serde_json::Value = serde_json::from_str(&grant)?;
                        println!(
                            "  ⬡ SORTIE WASM (nœud {}) :",
                            body["node"].as_str().unwrap_or("?")
                        );
                        println!("{}", body["output"].as_str().unwrap_or("(pas de sortie)"));
                        reputation::ReputationTable::load().record(&ghost, true);
                    }
                    None => {
                        println!("  ✖ aucune réponse — le nœud est-il en `ecouter --compute` ?");
                        reputation::ReputationTable::load().record(&ghost, false);
                    }
                }
                net::save_peers(&peers);
                return Ok(());
            }

            // Execution mode: send a task to the ghost, get the sandboxed output.
            if let Some(command) = executer {
                let ghost = emprunter.ok_or_else(|| {
                    anyhow::anyhow!("--executer demande --emprunter <node> (le nœud fantôme)")
                })?;
                println!("⬡ RES — exécution sandboxée → {ghost} (via {via})");
                println!("  tâche : {command}");
                let mut peers = net::load_peers();
                match net::borrow_compute(
                    &via,
                    &ghost,
                    &command,
                    &identity,
                    &mut peers,
                    std::time::Duration::from_secs(40),
                )
                .await?
                {
                    Some(grant) => {
                        let body: serde_json::Value = serde_json::from_str(&grant)?;
                        println!(
                            "  ⬡ RÉSULTAT (nœud {}) :",
                            body["node"].as_str().unwrap_or("?")
                        );
                        println!("{}", body["output"].as_str().unwrap_or("(pas de sortie)"));
                        reputation::ReputationTable::load().record(&ghost, true);
                    }
                    None => {
                        println!("  ✖ aucune réponse — le nœud est-il en `ecouter --compute` ?");
                        reputation::ReputationTable::load().record(&ghost, false);
                    }
                }
                net::save_peers(&peers);
                return Ok(());
            }

            // Borrow mode: send a compute request to a ghost node.
            if let Some(ghost) = emprunter {
                println!("⬡ RES — demande de compute → {ghost} (via {via})");
                let task = "bench calcul Polygone (réservé au RES)".to_string();
                let mut peers = net::load_peers();
                match net::borrow_compute(
                    &via,
                    &ghost,
                    &task,
                    &identity,
                    &mut peers,
                    std::time::Duration::from_secs(6),
                )
                .await?
                {
                    Some(grant) => {
                        println!("  ⬡ COMPUTE ACCORDÉ :");
                        reputation::ReputationTable::load().record(&ghost, true);
                        println!("  {grant}");
                    }
                    None => {
                        println!("  ✖ aucun grant reçu — le nœud est-il en `ecouter --compute` ?");
                        reputation::ReputationTable::load().record(&ghost, false);
                    }
                }
                net::save_peers(&peers);
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
            println!("  (la couche de prêt P2P arrive — staging `compute`)");
        }
        Some(Commands::Clef) => {
            println!("{}", identity.kem_pk_hex);
            println!(
                "# fichier identité : {}",
                identity::LocalIdentity::path().display()
            );
        }
        Some(Commands::Id) => {
            use polygone_core::NodeId;
            let id = NodeId::random();
            println!("{id}");
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
        Some(Commands::Duress { .. }) => {
            unreachable!("handled before identity load")
        }
    }
    Ok(())
}
