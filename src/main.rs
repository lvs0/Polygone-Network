//! polygone — Post-quantum ephemeral privacy network.
//! One command. Arrow-key dashboard. Zero config.
#![forbid(unsafe_code)]

use std::io;
use std::sync::Arc;

use clap::{Parser, Subcommand};
use tokio::sync::RwLock;

use polygone::web::{self as webmod, NodeState, WebConfig};
use polygone::tui;
use polygone::crypto::kem;
use polygone::msg;

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Parser)]
#[command(
    name = "polygone",
    version = VERSION,
    about = "⬡ POLYGONE — L'information n'existe pas. Elle traverse.",
    long_about = concat!(
        "Post-quantum ephemeral transit network.\n\n",
        "ML-KEM-1024 key exchange · AES-256-GCM encryption\n",
        "Shamir 4-of-7 fragmentation · BLAKE3 domain-separated KDF\n\n",
        "No server sees the message. No observer can prove a message existed.\n",
        "Source: https://github.com/lvs0/Polygone-Network\n",
        "License: MIT — No investors. No token. No telemetry.",
    ),
)]
struct Cli {
    #[command(subcommand)]
    cmd: Option<Cmd>,
}

#[derive(Subcommand)]
enum Cmd {
    /// Launch the interactive TUI dashboard (default)
    Menu {
        #[arg(long, default_value = "1")]
        tab: usize,
    },
    /// Show system status (non-interactive)
    Status,
    /// Run the real crypto self-test suite
    Test,
    /// Start the web dashboard on :8080
    Serve {
        #[arg(long, default_value = "127.0.0.1:8080")]
        bind: String,
    },
    /// Encrypt and fragment a message (Alice → Bob)
    Send {
        /// The plaintext message to send
        message: String,
        /// Recipient's ML-KEM-1024 public key (hex)
        #[arg(long, short = 'k')]
        key: Option<String>,
        /// Generate a new keypair first
        #[arg(long, short = 'g')]
        generate: bool,
        /// Output file (default: stdout)
        #[arg(long, short = 'o')]
        output: Option<String>,
    },
    /// Reconstruct and decrypt a message from fragments
    Receive {
        /// Path to file containing fragments + KEM_CT (or stdin with '-')
        #[arg(default_value = "-")]
        input: String,
        /// Recipient's ML-KEM-1024 secret key (hex)
        #[arg(long, short = 'k')]
        key: Option<String>,
        /// Path to secret key file (~/.polygone/keys/secret.hex)
        #[arg(long)]
        key_file: Option<String>,
    },
}

fn print_status() {
    println!("⬡ POLYGONE — Statut du réseau");
    println!();
    println!("  Version    : {VERSION}");
    println!("  Crypto     : ML-KEM-1024, AES-256-GCM, Shamir 4-of-7");
    println!("  Hash       : BLAKE3 (domain-separated)");
    println!("  Nœuds      : 0 (local)");
    println!("  Sessions   : 0 actives");
    println!("  Statut     : Opérationnel");
    println!();
    println!("  Source: https://github.com/lvs0/Polygone-Network");
    println!("  License: MIT");
}

fn run_self_test() {
    println!("⬡ POLYGONE — Self-test\n");

    let mut passed = 0;
    let total = 5;

    // 1. ML-KEM-1024 round-trip
    match kem::generate_keypair() {
        Ok((pk, sk)) => {
            match kem::encapsulate(&pk) {
                Ok((ct, ss1)) => {
                    match kem::decapsulate(&sk, &ct) {
                        Ok(ss2) if ss2 == ss1 => {
                            println!("  [1/5] ML-KEM-1024 round-trip ........... ✔");
                            passed += 1;
                        }
                        _ => println!("  [1/5] ML-KEM-1024 round-trip ........... ✖ (shared secret mismatch)"),
                    }
                }
                Err(e) => println!("  [1/5] ML-KEM-1024 round-trip ........... ✖ ({e})"),
            }
        }
        Err(e) => println!("  [1/5] ML-KEM-1024 round-trip ........... ✖ ({e})"),
    }

    // 2. AES-256-GCM encrypt/decrypt
    match kem::generate_keypair() {
        Ok((pk, _)) => {
            match kem::encapsulate(&pk) {
                Ok((_, ss)) => {
                    let sk = polygone::crypto::symmetric::SessionKey::derive_from_secret(&ss);
                    let plaintext = b"POLYGONE AES-256-GCM test vector";
                    match polygone::crypto::symmetric::encrypt(plaintext, &sk) {
                        Ok(encrypted) => {
                            match polygone::crypto::symmetric::decrypt(&encrypted, &sk) {
                                Ok(decrypted) if decrypted == plaintext => {
                                    println!("  [2/5] AES-256-GCM encrypt/decrypt ...... ✔");
                                    passed += 1;
                                }
                                _ => println!("  [2/5] AES-256-GCM encrypt/decrypt ...... ✖ (decrypt mismatch)"),
                            }
                        }
                        Err(e) => println!("  [2/5] AES-256-GCM encrypt/decrypt ...... ✖ ({e})"),
                    }
                }
                Err(e) => println!("  [2/5] AES-256-GCM encrypt/decrypt ...... ✖ ({e})"),
            }
        }
        Err(e) => println!("  [2/5] AES-256-GCM encrypt/decrypt ...... ✖ ({e})"),
    }

    // 3. Shamir 4-of-7
    let secret = b"shamir-4-of-7-test-secret-32bytes!";
    match polygone::crypto::shamir::split(secret, 4, 7) {
        Ok(frags) => {
            // Test all 35 combinations
            let mut all_ok = true;
            for i in 0..7 {
                for j in (i+1)..7 {
                    for k in (j+1)..7 {
                        for l in (k+1)..7 {
                            let subset = vec![frags[i].clone(), frags[j].clone(), frags[k].clone(), frags[l].clone()];
                            match polygone::crypto::shamir::reconstruct(&subset, 4) {
                                Ok(rec) if rec == secret => {},
                                _ => { all_ok = false; break; }
                            }
                        }
                        if !all_ok { break; }
                    }
                    if !all_ok { break; }
                }
                if !all_ok { break; }
            }
            if all_ok {
                println!("  [3/5] Shamir 4-of-7 (35 combinaisons) .. ✔");
                passed += 1;
            } else {
                println!("  [3/5] Shamir 4-of-7 (35 combinaisons) .. ✖");
            }

            // Test insufficient fragments
            match polygone::crypto::shamir::reconstruct(&frags[..3], 4) {
                Err(_) => {
                    println!("  [4/5] Fragments insuffisants → rejeté ... ✔");
                    passed += 1;
                }
                Ok(_) => println!("  [4/5] Fragments insuffisants → rejeté ... ✖"),
            }
        }
        Err(e) => println!("  [3/5] Shamir 4-of-7 ...................... ✖ ({e})"),
    }

    // 5. Full msg send/receive round-trip
    match kem::generate_keypair() {
        Ok((recipient_pk, recipient_sk)) => {
            let message = "⬡ Polygone v1.0 — L'information n'existe pas. Elle traverse.";
            match msg::send(message, &recipient_pk) {
                Ok(output) => {
                    match msg::receive(&output, &recipient_sk) {
                        Ok(decrypted) if decrypted == message => {
                            println!("  [5/5] Session round-trip (Alice → Bob) .. ✔");
                            passed += 1;
                        }
                        Ok(_) => println!("  [5/5] Session round-trip (Alice → Bob) .. ✖ (decrypted != original)"),
                        Err(e) => println!("  [5/5] Session round-trip (Alice → Bob) .. ✖ ({e})"),
                    }
                }
                Err(e) => println!("  [5/5] Session round-trip (Alice → Bob) .. ✖ ({e})"),
            }
        }
        Err(e) => println!("  [5/5] Session round-trip (Alice → Bob) .. ✖ ({e})"),
    }

    println!();
    if passed == total {
        println!("  ✔ Tous les tests passent. Polygone est opérationnel.");
    } else {
        println!("  ⚠ {passed}/{total} tests passés. Vérifie les erreurs ci-dessus.");
    }
}

fn cmd_send(message: &str, key_hex: Option<&str>, generate: bool, output_path: Option<&str>) {
    let (recipient_pk, recipient_sk_hex) = if generate {
        let (pk, sk) = kem::generate_keypair().expect("KEM keygen");
        println!("🔑 Nouvelle paire de clés générée :");
        println!("   Clé publique (à partager) : {}", pk.to_hex());
        println!("   Clé secrète  (à garder)   : {}", sk.to_hex());
        (pk, Some(sk.to_hex()))
    } else if let Some(kh) = key_hex {
        let pk = kem::KemPublicKey::from_hex(kh)
            .expect("Clé publique invalide (mauvais hex ?)");
        (pk, None)
    } else {
        eprintln!("❌ Il faut soit --generate, soit --key <hex>");
        eprintln!("   Exemple : polygone send \"Salut\" --generate");
        eprintln!("          ou polygone send \"Salut\" --key <hex de la clé publique du destinataire>");
        std::process::exit(1);
    };

    match msg::send(message, &recipient_pk) {
        Ok(output) => {
            let text = output.display();
            if let Some(path) = output_path {
                std::fs::write(path, &text).expect("écriture fichier");
                println!("✅ Message fragmenté sauvegardé dans : {path}");
                println!("   {} fragments (4 nécessaires pour reconstruire)", output.fragments.len());
            } else {
                println!("✅ Message fragmenté (7 fragments, seuil 4) :\n");
                print!("{}", text);
                println!("─── Fin du message fragmenté ───");
                println!("Envoyez ces fragments au destinataire.");
            }
            if let Some(sk_hex) = recipient_sk_hex {
                println!("\n⚠️  Gardez cette clé secrète précieusement :");
                println!("   {}", sk_hex);
            }
        }
        Err(e) => {
            eprintln!("❌ Erreur d'envoi : {e}");
            std::process::exit(1);
        }
    }
}

fn cmd_receive(input_path: &str, key_hex: Option<&str>, key_file: Option<&str>) {
    // Read input
    let input = if input_path == "-" {
        std::io::read_to_string(std::io::stdin()).expect("lecture stdin")
    } else {
        std::fs::read_to_string(input_path)
            .unwrap_or_else(|e| {
                eprintln!("❌ Impossible de lire {} : {e}", input_path);
                std::process::exit(1);
            })
    };

    // Parse the send output
    let output = msg::SendOutput::parse(&input)
        .unwrap_or_else(|e| {
            eprintln!("❌ Format invalide : {e}");
            eprintln!("   Le fichier doit contenir des lignes KEM_CT:, SENDER_PK:, FRAG:");
            std::process::exit(1);
        });

    println!("📨 Message reçu : {} fragments", output.fragments.len());

    // Load secret key
    let sk = if let Some(kh) = key_hex {
        kem::KemSecretKey::from_hex(kh)
            .unwrap_or_else(|e| {
                eprintln!("❌ Clé secrète invalide : {e}");
                std::process::exit(1);
            })
    } else if let Some(kf) = key_file {
        let hex = std::fs::read_to_string(&kf)
            .unwrap_or_else(|e| {
                eprintln!("❌ Impossible de lire {} : {e}", kf);
                std::process::exit(1);
            });
        kem::KemSecretKey::from_hex(hex.trim())
            .unwrap_or_else(|e| {
                eprintln!("❌ Clé secrète invalide dans {} : {e}", kf);
                std::process::exit(1);
            })
    } else {
        // Try default key location
        let default_path = dirs::home_dir()
            .unwrap_or_default()
            .join(".polygone")
            .join("keys")
            .join("secret.hex");
        if default_path.exists() {
            let hex = std::fs::read_to_string(&default_path)
                .expect("lecture clé par défaut");
            kem::KemSecretKey::from_hex(hex.trim())
                .unwrap_or_else(|e| {
                    eprintln!("❌ Clé par défaut invalide : {e}");
                    std::process::exit(1);
                })
        } else {
            eprintln!("❌ Aucune clé secrète fournie.");
            eprintln!("   Utilise --key <hex> ou --key-file <chemin>");
            eprintln!("   La clé par défaut (~/.polygone/keys/secret.hex) n'existe pas.");
            std::process::exit(1);
        }
    };

    // Decrypt
    match msg::receive(&output, &sk) {
        Ok(plaintext) => {
            println!("\n✅ Message déchiffré :\n");
            println!("{}", plaintext);
        }
        Err(e) => {
            eprintln!("❌ Échec du déchiffrement : {e}");
            eprintln!("   Vérifiez que vous utilisez la bonne clé secrète.");
            std::process::exit(1);
        }
    }
}

fn print_splash() {
    use std::thread;
    use std::time::Duration;

    let logo = [
        r"    ⬡ ⬡ ⬡ ⬡ ⬡ ⬡ ⬡ ⬡ ⬡ ⬡ ⬡ ⬡ ⬡ ⬡ ⬡ ⬡ ⬡ ⬡ ⬡ ⬡ ⬡ ⬡ ⬡",
        r"    ⬡                                 ⬡",
        r"    ⬡   ░██████╗░██████╗██╗░░░░░██╗░░░██╗   ⬡",
        r"    ⬡   ██╔════╝██╔════╝██║░░░░░╚██╗░██╔╝   ⬡",
        r"    ⬡   ╚█████╗░╚█████╗░██║░░░░░░╚████╔╝░   ⬡",
        r"    ⬡   ░╚═══██╗░╚═══██╗██║░░░░░░░╚██╔╝░░   ⬡",
        r"    ⬡   ██████╔╝██████╔╝███████╗░░░██║░░░   ⬡",
        r"    ⬡   ╚═════╝░╚═════╝░╚══════╝░░░╚═╝░░░   ⬡",
        r"    ⬡                                 ⬡",
        r"    ⬡ ⬡ ⬡ ⬡ ⬡ ⬡ ⬡ ⬡ ⬡ ⬡ ⬡ ⬡ ⬡ ⬡ ⬡ ⬡ ⬡ ⬡ ⬡ ⬡ ⬡ ⬡ ⬡",
    ];

    let stages = [
        "Initialisation du noyau crypto...",
        "Génération des clés ML-KEM-1024...",
        "Chiffrement AES-256-GCM...",
        "Fragmentation Shamir 4-of-7...",
        "Démarrage du réseau P2P...",
        "Prêt.",
    ];

    print!("\x1B[2J\x1B[1;1H");
    for line in &logo {
        println!("\x1B[36m{}\x1B[0m", line);
        thread::sleep(Duration::from_millis(30));
    }
    println!();

    for (i, stage) in stages.iter().enumerate() {
        let bar_width = 30;
        let filled = (i + 1) * bar_width / stages.len();
        let bar: String = (0..bar_width)
            .map(|j| if j < filled { '█' } else { '░' })
            .collect();
        print!("\r  \x1B[33m{}\x1B[0m \x1B[36m{}\x1B[0m", bar, stage);
        std::io::Write::flush(&mut std::io::stdout()).ok();
        thread::sleep(Duration::from_millis(200));
    }
    println!();
    println!();
    thread::sleep(Duration::from_millis(300));
    print!("\x1B[2J\x1B[1;1H");
}

fn main() -> io::Result<()> {
    let cli = Cli::parse();
    let cmd = cli.cmd.unwrap_or(Cmd::Menu { tab: 1 });

    match cmd {
        Cmd::Status => {
            print_status();
            Ok(())
        }
        Cmd::Test => {
            run_self_test();
            Ok(())
        }
        Cmd::Menu { tab } => {
            print_splash();
            // Default landing on `polygone` with no `--tab` is now the
            // master menu (Phase 3). `--tab 1..=4` jumps straight to a
            // dashboard tab. `--tab 0` (or unset) lands on the menu.
            let initial_view = match tab {
                1 => tui::View::Dashboard,
                2 => tui::View::Favorites,
                3 => tui::View::Services,
                4 => tui::View::Settings,
                _ => tui::View::Menu,
            };
            tui::run_tui(initial_view)
        }
        Cmd::Serve { bind } => {
            let rt = tokio::runtime::Runtime::new()
                .expect("tokio runtime");
            rt.block_on(async {
                let addr: std::net::SocketAddr = bind
                    .parse()
                    .expect("invalid --bind address (e.g. 127.0.0.1:8080)");
                let cfg = WebConfig { bind: addr };
                let state = Arc::new(RwLock::new(NodeState::fresh()));
                eprintln!("⬡ POLYGONE v{VERSION} — web dashboard");
                eprintln!("  → http://{addr}");
                webmod::serve(cfg, state).await
            })
        }
        Cmd::Send { message, key, generate, output } => {
            cmd_send(&message, key.as_deref(), generate, output.as_deref());
            Ok(())
        }
        Cmd::Receive { input, key, key_file } => {
            cmd_receive(&input, key.as_deref(), key_file.as_deref());
            Ok(())
        }
    }
}
