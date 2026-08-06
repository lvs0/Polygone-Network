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
    /// Encrypt + fragment a message for a recipient (prints wire text)
    Envoyer {
        /// Recipient's ML-KEM-1024 public key (hex) — omit for a self-demo
        #[arg(long, short = 'd')]
        dest: Option<String>,
        /// The message to send (everything after the flags)
        #[arg(required = true)]
        message: Vec<String>,
    },
    /// Reconstruct + decrypt from wire text (file path, or stdin if '-')
    Recevoir {
        /// File containing wire text, or '-' for stdin
        #[arg(default_value = "-")]
        input: String,
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
        Some(Commands::Envoyer { dest, message }) => {
            let message = message.join(" ");
            let recipient = match dest {
                Some(hex) => polygone_core::crypto::kem::KemPublicKey::from_hex(&hex)?,
                None => {
                    // No recipient: self-demo against a fresh keypair.
                    let (pk, _sk) = polygone_core::crypto::kem::generate_keypair()?;
                    println!("# pas de destinataire — envoi auto (démo) vers une clef fraîche");
                    pk
                }
            };
            let output = msg::send(&message, &recipient)?;
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
