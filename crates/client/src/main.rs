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
mod identity;
mod msg;
mod net;
mod tui;

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(
    name = "polygone",
    version = "2.0.0-rc2",
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
    },
    /// Show your ML-KEM-1024 public key (what you share to receive)
    Clef,
    /// Print this node's random NodeId
    Id,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    let level = if args.verbose { "debug" } else { "info" };
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(level)).init();

    // First run: materialise the local identity (keys + pseudo).
    let identity = identity::LocalIdentity::load_or_create()?;

    match args.command {
        None | Some(Commands::Tui) => tui::run(identity)?,
        Some(Commands::Demo { relay: _ }) => {
            demo::run()?;
        }
        Some(Commands::Envoyer { dest, via, a, fichier, message }) => {
            let message = message.join(" ");

            // File mode: read the file, send its bytes.
            let payload: Vec<u8> = match &fichier {
                Some(path) => std::fs::read(path)?,
                None => message.as_bytes().to_vec(),
            };
            if fichier.is_none() && message.is_empty() {
                anyhow::bail!("rien à envoyer : passez un message ou --fichier <chemin>");
            }

            // Network mode: route the fragments through a blind relay.
            if let Some(relay) = via {
                let dest_node = a.ok_or_else(|| {
                    anyhow::anyhow!("mode réseau : précisez le nœud destinataire avec --a <node_id>")
                })?;
                let recipient = match dest {
                    Some(hex) => polygone_core::crypto::kem::KemPublicKey::from_hex(&hex)?,
                    None => anyhow::bail!("mode réseau : précisez la clef du destinataire avec -d <clef>"),
                };
                let name = fichier.as_ref().map(|p| {
                    std::path::Path::new(p)
                        .file_name()
                        .map(|s| s.to_string_lossy().to_string())
                        .unwrap_or_else(|| p.clone())
                });
                let session = net::send_network(&relay, &dest_node, &payload, name.as_deref(), &recipient, &identity).await?;
                match &fichier {
                    Some(_) => println!("⬡ fichier envoyé via relay {relay} → {dest_node}"),
                    None => println!("⬡ message envoyé via relay {relay} → {dest_node}"),
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
        Some(Commands::Ecouter { relay }) => {
            net::receive_network(&relay, &identity).await?;
        }
        Some(Commands::Clef) => {
            println!("{}", identity.kem_pk_hex);
            println!("# fichier identité : {}", identity::LocalIdentity::path().display());
        }
        Some(Commands::Id) => {
            use polygone_core::NodeId;
            let id = NodeId::random();
            println!("{id}");
        }
    }
    Ok(())
}
