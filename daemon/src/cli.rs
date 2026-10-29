//! CLI du daemon — la partie qu'un humain tape.
//!
//! Ce module ne décide **rien** : il lit l'état du daemon, le rend lisible, et
//! sort un code non nul quand la demande n'a pas abouti. Une CLI n'est pas un
//! consommateur du réseau — elle regarde, et démontre que le protocole marche
//! dans les deux sens.
//!
//! Toute la logique vit ici, et non dans `main.rs` (qui n'a aucun test et n'en
//! aura pas) : `run` prend le chemin du socket **en paramètre**, exactement comme
//! `bind_listener(path)` côté serveur. Aucun JSON écrit à la main : tout passe par
//! `DaemonClient`, donc par `encode_request` / `decode_reply`.
//!
//! # Un accord ne survit pas à sa connexion
//!
//! C'est la contrainte de durée de vie à avoir en tête avant de construire quoi
//! que ce soit sur ce protocole : le serveur libère **toutes** les réservations
//! d'une connexion quand elle se ferme (garde RAII `LedgerGuard`, incrément
//! 2b-6). Un `allocation_id` n'est donc **pas un bail** : il ne se conserve pas,
//! ne se transmet pas à un autre processus, et ne survit pas au programme qui l'a
//! obtenu. Un consommateur qui veut garder ce qu'il a reçu doit **rester
//! connecté** ; se reconnecter veut dire redemander, pas reprendre.
//!
//! Conséquence assumée pour cette CLI : `polygoned alloc` prouve l'aller-retour,
//! mais sa réservation meurt en même temps que la commande. `polygoned release`
//! reste utile — elle libère un accord détenu par une AUTRE connexion encore
//! vivante (c'est ce que vérifie `test_cli_alloc_then_release_roundtrip`), et elle
//! est idempotente, donc jamais un échec.

use crate::client::{ClientError, DaemonClient};
use crate::policy::glow_up::DaemonConfig;
use crate::resources::{Platform, UserActivity};
use crate::socket::{Reply, Request};
use crate::system::SystemSnapshot;
use anyhow::Result;
use clap::Subcommand;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

/// Cadence de `watch`, en secondes.
const WATCH_INTERVAL: Duration = Duration::from_secs(1);

/// Délai de la **sonde** de diagnostic (`status`, et la reconnexion de `watch`).
///
/// Une sonde de diagnostic qui peut bloquer n'est pas un diagnostic : c'est
/// précisément quand le daemon ne répond plus qu'on a besoin de `status`, et
/// attendre une minute pour l'apprendre ne sert à rien. Ce délai est donc COURT
/// exprès — il n'a pas à attendre un consommateur.
///
/// À ne pas confondre avec `PROBE_TIMEOUT` de `socket.rs` (200 ms) : celui-là
/// sonde un **fichier** de socket pour savoir si quelqu'un écoute, sans jamais
/// parler le protocole. Ici, on parle à un daemon qui a déjà accepté la
/// connexion.
const STATUS_PROBE_TIMEOUT: Duration = Duration::from_secs(1);

/// Drapeau d'arrêt de `watch`, armé par Ctrl+C.
///
/// Le daemon a le même (`RUNNING`) : une boucle infinie doit pouvoir être
/// interrompue proprement, sans `kill -9` ni trace de pile.
static WATCH_RUNNING: AtomicBool = AtomicBool::new(true);

/// Sous-commandes de `polygoned`.
///
/// Elles vivent ici (et non dans `main.rs`) pour que la surface de commandes et
/// sa logique ne puissent pas diverger : un variant ajouté sans son traitement
/// est une erreur de compilation, pas un silence.
#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Show current allocation and system stats
    Status,
    /// One round-trip to the daemon: prints `pong` and the round-trip time
    Ping,
    /// `status` every second, on a single line (Ctrl+C to stop)
    Watch,
    /// Ask the daemon for an allocation and print the `allocation_id`
    Alloc {
        /// RAM to request, in GB
        #[arg(long, default_value_t = 1.0)]
        ram: f32,
        /// CPU cores to request
        #[arg(long, default_value_t = 1)]
        cpu: u32,
    },
    /// Release an allocation, by its `allocation_id`
    Release {
        /// Identifiant rendu par `alloc`
        allocation_id: String,
    },
    /// Not supported: protocol v1 has no stop request (stop the daemon with Ctrl+C)
    Stop,
    /// Run doctor diagnostics
    Doctor,
}

/// Ce que `run` reçoit : de quoi lire le local, et où parler au daemon.
///
/// Le `socket` est un paramètre, pas un appel à `socket_path()` : c'est ce qui
/// rend ce module testable sur un socket temporaire (même raison que
/// `bind_listener(path)`).
pub struct CliArgs<'a> {
    /// Chemin du socket à interroger.
    pub socket: PathBuf,
    pub command: Commands,
    /// Config déjà chargée par `main` : la CLI n'invente pas les valeurs locales.
    pub config: &'a DaemonConfig,
    /// Plateforme déjà initialisée par `main`.
    pub platform: &'a dyn Platform,
}

/// Point d'entrée de la CLI : une sous-commande, un résultat.
pub fn run(args: CliArgs<'_>) -> Result<()> {
    match &args.command {
        Commands::Status => {
            print!("{}", status_report(&args));
            Ok(())
        }
        Commands::Doctor => {
            print!("{}", doctor_report(&args));
            Ok(())
        }
        Commands::Ping => cmd_ping(&args.socket),
        Commands::Watch => cmd_watch(&args.socket),
        Commands::Alloc { ram, cpu } => cmd_alloc(&args.socket, *ram, *cpu),
        Commands::Release { allocation_id } => cmd_release(&args.socket, allocation_id),
        Commands::Stop => {
            // Le daemon SERT ce socket au lieu de s'y connecter, et le protocole
            // v1 n'expose aucune requête d'arrêt : annoncer « signal envoyé »
            // serait un mensonge. On le dit, et on échoue.
            anyhow::bail!(
                "arrêt à distance non pris en charge : le protocole v1 n'expose pas de requête \
                 d'arrêt — arrêtez le daemon par Ctrl+C"
            )
        }
    }
}

/// Libellé humain de l'état d'activité, partagé par `status`, `doctor` et le
/// journal d'un tick.
///
/// « inconnu » est un état de plein droit : c'est aujourd'hui le seul que les
/// plateformes savent produire, donc l'afficher est la moitié du correctif.
pub fn activity_label(activity: UserActivity) -> &'static str {
    match activity {
        UserActivity::Active => "actif",
        UserActivity::Idle => "inactif",
        UserActivity::Unknown => "inconnu",
    }
}

fn cmd_ping(socket: &Path) -> Result<()> {
    let mut client = connect(socket)?;

    let started = Instant::now();
    let reply = client.request(Request::Ping { id: None })?;
    let elapsed = started.elapsed();

    match reply {
        Reply::Pong { .. } => {
            println!("pong — {:.2} ms", elapsed.as_secs_f64() * 1000.0);
            Ok(())
        }
        other => anyhow::bail!("réponse inattendue au ping : {other:?}"),
    }
}

fn cmd_watch(socket: &Path) -> Result<()> {
    // Un seul handler Ctrl+C par processus : `main` n'en pose pas pour une
    // sous-commande, et si un autre en a déjà posé un, on continue sans lui.
    let _ = ctrlc::set_handler(|| WATCH_RUNNING.store(false, Ordering::SeqCst));

    // Aucune connexion n'est ouverte ICI : c'est `watch_step` qui la crée, et qui
    // la recrée si le daemon est mort. `watch` démarre donc même quand le daemon
    // n'est pas encore là — il attendra, au lieu de mourir au premier essai.
    let mut state = WatchState { client: None };
    let mut first = true;

    while WATCH_RUNNING.load(Ordering::SeqCst) {
        // Réaffichage SUR PLACE (retour chariot + effacement de la ligne) : un
        // `watch` qui défile une fois par seconde noie le reste du terminal.
        let line = watch_step(&mut state, socket);
        if first {
            first = false;
        } else {
            print!("\r\x1b[2K");
        }
        print!("{line}");
        std::io::stdout().flush().ok();

        std::thread::sleep(WATCH_INTERVAL);
    }

    // La dernière ligne reste lisible : on ne laisse pas le curseur au milieu.
    println!();
    Ok(())
}

/// L'état de la boucle de `watch` : un client, ou **aucun**.
///
/// Un `Option` et non un `DaemonClient` : c'est le `None` qui distingue « on est
/// branché » de « on a perdu le daemon, il faut en rouvrir un ». Un client gardé
/// après la mort du daemon pointerait un descripteur sans destinataire.
struct WatchState {
    client: Option<DaemonClient>,
}

/// UN tour de `watch` : rend la ligne à afficher.
///
/// Ne fait AUCUN affichage et ne dort PAS : c'est ce qui rend la reconnexion
/// testable sans horloge — un test appelé deux fois vérifie le réabonnement,
/// alors qu'il ne peut pas attendre une boucle qui dort une seconde par tour.
///
/// Règle de vie du client : **un client mort se jette, il ne se répare pas.**
/// Toute panne de transport remet `client` à `None`, et le tour suivant en ouvre
/// un neuf. C'est ce `None` qui fait que `watch` se réabonne.
fn watch_step(state: &mut WatchState, socket: &Path) -> String {
    if state.client.is_none() {
        // Une sonde, pas un consommateur : même délai court que `status`, sinon la
        // reconnexion attendrait une minute avant d'avouer qu'elle a échoué.
        match DaemonClient::connect_with_timeout(socket, STATUS_PROBE_TIMEOUT) {
            Ok(client) => state.client = Some(client),
            Err(e) => return format!("⬡ daemon non joignable ({e})"),
        }
    }

    // Ici le client est forcément `Some` : le bloc ci-dessus vient de le remplir,
    // ou a rendu la main. On sort proprement plutôt que d'`unwrap` : dans une
    // surveillance, une ligne « injoignable » vaut mieux qu'un déroulement de pile.
    let Some(client) = state.client.as_mut() else {
        return "⬡ daemon non joignable".to_string();
    };

    match client.status() {
        Ok(reply) => watch_line(reply),
        // Refus DÉCIDÉ par le serveur : la connexion est saine, on la GARDE.
        // Jeter le client ici transformerait un refus lisible en panne de réseau.
        Err(e @ ClientError::Server { .. }) => format!("⬡ refus du serveur ({e})"),
        // Panne de TRANSPORT (E/S, protocole) : la connexion est perdue. C'est le
        // cas du daemon redémarré — l'ancien descripteur pointe un inode mort.
        Err(e) => {
            state.client = None;
            format!("⬡ daemon non joignable ({e})")
        }
    }
}

/// Rend une réponse `status` en une ligne pour `watch`.
///
/// Pure : elle ne parle pas au daemon et ne décide de rien. Séparée de
/// `watch_step` pour que le texte affiché ne dépende pas de l'état de connexion.
fn watch_line(reply: Reply) -> String {
    match reply {
        Reply::Status {
            tier,
            seq,
            ram_mb,
            cpu_cores,
            uptime_secs,
            shrinking,
            ..
        } => format!(
            "⬡ {tier} seq={seq} | RAM {:.2} GB | CPU {cpu_cores} cœurs | uptime {uptime_secs} s{}",
            ram_mb as f64 / 1024.0,
            if shrinking { " | rétrécit" } else { "" }
        ),
        other => format!("⬡ réponse inattendue : {other:?}"),
    }
}

fn cmd_alloc(socket: &Path, ram_gb: f32, cpu_cores: u32) -> Result<()> {
    let mut client = connect(socket)?;

    match client.allocate(ram_gb, cpu_cores)? {
        Reply::AllocGranted {
            allocation_id,
            ram_gb,
            cpu_cores,
            ..
        } => {
            println!("allocation accordée : {allocation_id} ({ram_gb} GB, {cpu_cores} cœurs)");
            // Répété à l'usage, pas seulement en documentation : c'est ici que la
            // confusion coûterait cher (croire que l'identifiant est un bail).
            println!(
                "  ⚠ licence, pas contrainte — et elle meurt avec cette commande : \
                 pour la garder, il faut rester connecté"
            );
            Ok(())
        }
        Reply::AllocDenied { reason, .. } => anyhow::bail!("allocation refusée : {reason}"),
        other => anyhow::bail!("réponse inattendue : {other:?}"),
    }
}

fn cmd_release(socket: &Path, allocation_id: &str) -> Result<()> {
    let mut client = connect(socket)?;

    match client.release(allocation_id)? {
        Reply::Released { allocation_id, .. } => {
            println!("libéré : {allocation_id}");
            // Le daemon ne dit pas si l'identifiant existait : c'est voulu
            // (libération idempotente), mais il faut le dire plutôt que de
            // laisser croire à une confirmation de vie.
            println!(
                "  (idempotent : le daemon répond `released` même pour un identifiant inconnu)"
            );
            Ok(())
        }
        other => anyhow::bail!("réponse inattendue : {other:?}"),
    }
}

/// Se connecte, en disant clairement OÙ et POURQUOI ça a échoué.
///
/// `ClientError` distingue déjà transport / protocole / refus ; on ajoute le
/// chemin, sans quoi « connexion impossible » ne dit pas à quel daemon.
fn connect(socket: &Path) -> Result<DaemonClient> {
    DaemonClient::connect(socket)
        .map_err(|e| anyhow::anyhow!("connexion à {} impossible : {e}", socket.display()))
}

/// Résumé d'une ligne de l'état du daemon, ou la raison de son absence.
///
/// Ne rend JAMAIS d'erreur : un daemon éteint n'est pas une anomalie, c'est un
/// état normal de la machine. `status` doit le DIRE, pas échouer.
fn daemon_summary(socket: &Path) -> String {
    if !socket.exists() {
        return format!(
            "daemon non joignable (socket absent : {})",
            socket.display()
        );
    }

    match DaemonClient::connect_with_timeout(socket, STATUS_PROBE_TIMEOUT)
        .and_then(|mut client| client.status())
    {
        Ok(Reply::Status {
            tier,
            seq,
            ram_mb,
            cpu_cores,
            uptime_secs,
            shrinking,
            ..
        }) => format!(
            "tier={tier} seq={seq} | RAM {:.1} GB allouée | CPU {cpu_cores} cœurs | \
             uptime {uptime_secs} s{}",
            ram_mb as f64 / 1024.0,
            if shrinking { " | rétrécit" } else { "" }
        ),
        Ok(other) => format!("réponse inattendue : {other:?}"),
        Err(e) => format!("daemon non joignable ({e})"),
    }
}

/// Le rapport de `status`, rendu à part pour être testable sans capturer stdout.
///
/// `run` imprime EXACTEMENT cette chaîne : il n'existe pas deux textes parallèles
/// qui pourraient diverger entre ce qu'on affiche et ce qu'on teste.
///
/// `status` lit la config et la machine, PAS le daemon : c'est un état local. La
/// dernière ligne, elle, interroge le socket.
fn status_report(args: &CliArgs<'_>) -> String {
    use std::fmt::Write as _;

    let snap = SystemSnapshot::capture(args.platform);
    let limits = args.config.effective_limits(&snap);
    let safe_limits = args.config.apply_safety(limits, &snap);

    let mut out = String::new();
    let _ = writeln!(
        out,
        "\n  ⬡ polygoned v{} — Status",
        env!("CARGO_PKG_VERSION")
    );
    let _ = writeln!(out, "  ──────────────────────────────────────────");
    let _ = writeln!(out, "  Platform    : {}", args.platform.name());
    let _ = writeln!(out, "  Tier        : {}", args.config.tier);
    let _ = writeln!(
        out,
        "  Limits      : CPU {}% | RAM {}% | BW {}% | GPU {}%",
        safe_limits.max_cpu_percent,
        safe_limits.max_ram_percent,
        safe_limits.max_bandwidth_percent,
        safe_limits.max_gpu_percent
    );
    let _ = writeln!(out);
    let _ = writeln!(
        out,
        "  System RAM  : {:.1} GB total | {:.1} GB free | {:.1} GB used",
        snap.memory.total_bytes as f64 / 1_073_741_824.0,
        snap.memory.available_bytes as f64 / 1_073_741_824.0,
        snap.memory.used_bytes as f64 / 1_073_741_824.0
    );
    let _ = writeln!(
        out,
        "  CPU         : {} cores | {:.0}% usage | load {:.2}",
        snap.cpu.per_core.len(),
        snap.cpu.usage_percent,
        snap.cpu.load_average[0]
    );
    let _ = writeln!(out, "  User active : {}", activity_label(snap.user_active));
    let _ = writeln!(out);

    for gpu in &snap.gpu {
        let _ = writeln!(
            out,
            "  GPU {}       : {} | {:.1} GB VRAM | {} MB used",
            gpu.device_id,
            gpu.name,
            gpu.vram_total_mb as f64 / 1024.0,
            gpu.vram_used_mb
        );
    }

    let _ = writeln!(out);
    // Ligne conservée à l'identique : `scripts/smoke-commands.sh` la vérifie
    // (socket absent → ❌, présent → ✅).
    let _ = writeln!(
        out,
        "  Socket      : {} {}",
        args.socket.display(),
        if args.socket.exists() { "✅" } else { "❌" }
    );
    // Nouvelle ligne (2b-7b) : l'état RÉEL du daemon s'il écoute, sinon pourquoi.
    let _ = writeln!(out, "  Daemon      : {}", daemon_summary(&args.socket));
    let _ = writeln!(out);

    out
}

/// Le rapport de `doctor` — même raison d'être séparé que `status_report`.
fn doctor_report(args: &CliArgs<'_>) -> String {
    use std::fmt::Write as _;

    let platform = args.platform;
    let mut out = String::new();

    let _ = writeln!(out, "\n  ⬡ polygoned doctor");
    let _ = writeln!(out, "  ──────────────────────────────────────────");
    let _ = writeln!(out, "  ✅ Platform: {}", platform.name());
    let _ = writeln!(out, "  Capabilities:");

    let caps = platform.capabilities();
    for (label, value) in [
        ("CPU affinity     ", caps.cpu_affinity),
        ("CPU priority     ", caps.cpu_priority),
        ("Memory limit     ", caps.memory_limit),
        ("Bandwidth monitor", caps.bandwidth_monitor),
        ("GPU monitor      ", caps.gpu_monitor),
        ("Unix sockets     ", caps.unix_sockets),
        ("Named pipes      ", caps.named_pipes),
    ] {
        let _ = writeln!(out, "    {label}: {}", if value { "yes" } else { "no" });
    }

    // `doctor` est la commande où l'on vient vérifier : l'état d'activité doit y
    // être lisible, y compris — et surtout — quand il vaut `inconnu`. Sans ça, on
    // continuera de croire que le frein fonctionne (docs/PLAN_FREIN.md §1).
    // Une erreur de lecture devient `inconnu`, jamais `inactif`.
    let _ = writeln!(
        out,
        "    User activity    : {}",
        activity_label(platform.user_active().unwrap_or(UserActivity::Unknown))
    );

    if let Ok(cpu) = platform.cpu_info() {
        let _ = writeln!(
            out,
            "\n  CPU: {} cores ({} sockets)",
            cpu.cores, cpu.topology.sockets
        );
    }

    if let Ok(mem) = platform.memory_info() {
        let _ = writeln!(
            out,
            "\n  Memory: {:.1} GB total | {:.1} GB available",
            mem.total_bytes as f64 / 1_073_741_824.0,
            mem.available_bytes as f64 / 1_073_741_824.0
        );
    }

    if let Ok(bw) = platform.bandwidth_info() {
        let _ = writeln!(out, "\n  Network interfaces:");
        for iface in &bw.interfaces {
            if !iface.name.starts_with("lo") {
                let _ = writeln!(
                    out,
                    "    {} : RX {} MB | TX {} MB",
                    iface.name,
                    iface.rx_bytes / 1_000_000,
                    iface.tx_bytes / 1_000_000
                );
            }
        }
    }

    if let Ok(gpus) = platform.gpu_info() {
        if gpus.is_empty() {
            let _ = writeln!(out, "\n  GPUs: none detected");
        } else {
            let _ = writeln!(out, "\n  GPUs:");
            for gpu in &gpus {
                let _ = writeln!(
                    out,
                    "    {} [{}]: {} GB | {} MB used | {}°C | {}W",
                    gpu.device_id,
                    gpu.name,
                    gpu.total_vram_mb as f64 / 1024.0,
                    gpu.used_vram_mb,
                    gpu.temperature_c,
                    gpu.power_watts
                );
            }
        }
    }

    let _ = writeln!(
        out,
        "\n  IPC socket: {} {}",
        args.socket.display(),
        if args.socket.exists() { "✅" } else { "❌" }
    );
    let _ = writeln!(out, "\n  ✅ All checks complete");

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resources::create_platform;
    use crate::socket::{bind_listener, serve_with_timeout, DaemonState, CONNECTION_IDLE_TIMEOUT};
    use std::os::unix::net::UnixListener;
    use std::sync::{Arc, RwLock};
    use std::thread;

    /// Un VRAI serveur sur un socket temporaire, avec le palier demandé (pour
    /// distinguer le palier PUBLIÉ par le daemon de celui de la config locale).
    fn start_server(
        tier: &str,
    ) -> (
        PathBuf,
        Arc<AtomicBool>,
        thread::JoinHandle<()>,
        tempfile::TempDir,
    ) {
        let dir = tempfile::tempdir().expect("dossier temporaire");
        let path = dir.path().join("daemon.sock");
        let (shutdown, handle) = start_server_at(&path, tier, CONNECTION_IDLE_TIMEOUT);
        (path, shutdown, handle, dir)
    }

    /// Vrai serveur sur le chemin IMPOSÉ, avec son délai d'inactivité.
    ///
    /// Le chemin est un paramètre pour qu'un test puisse arrêter un serveur et en
    /// relancer un AUTRE sur le même fichier : c'est exactement la situation du
    /// daemon qui redémarre, celle que `watch` doit survivre.
    fn start_server_at(
        path: &Path,
        tier: &str,
        idle_timeout: Duration,
    ) -> (Arc<AtomicBool>, thread::JoinHandle<()>) {
        let listener = bind_listener(path).expect("bind sur un socket temporaire");
        // Le ledger est isolé par test, comme le socket : sans ça, un test hérite
        // des réservations laissées par un test précédent (`insufficient_headroom`).
        if let Some(parent) = path.parent() {
            std::env::set_var(crate::socket::ENV_LEDGER_PATH, parent.join("ledger.json"));
        }
        let state = Arc::new(RwLock::new(DaemonState::new(tier)));
        let shutdown = Arc::new(AtomicBool::new(false));

        let serve_shutdown = Arc::clone(&shutdown);
        let handle = thread::spawn(move || {
            serve_with_timeout(listener, state, serve_shutdown, idle_timeout)
        });

        (shutdown, handle)
    }

    /// Faux serveur MUET : il accepte la connexion et n'écrit **jamais**.
    ///
    /// Celui de `client.rs` est privé à son module de tests : on le réécrit ici,
    /// en quelques lignes, plutôt que d'ouvrir un jeu d'utilitaires partagés pour
    /// deux tests. `UnixListener` brut : aucun serveur du daemon n'est impliqué,
    /// donc rien du protocole ne peut « sauver » le client par accident.
    fn start_silent_server() -> (PathBuf, tempfile::TempDir) {
        let dir = tempfile::tempdir().expect("dossier temporaire");
        let path = dir.path().join("muet.sock");
        let listener = UnixListener::bind(&path).expect("bind du serveur muet");

        thread::spawn(move || {
            // Tolérant : le client qui renonce ferme la connexion, et le thread
            // ne doit pas s'en plaindre. `join` est ignoré côté test.
            if let Ok((stream, _)) = listener.accept() {
                thread::sleep(Duration::from_secs(5));
                drop(stream);
            }
        });

        (path, dir)
    }

    fn stop_server(shutdown: &Arc<AtomicBool>, handle: thread::JoinHandle<()>) {
        shutdown.store(true, Ordering::SeqCst);
        handle.join().expect("le serveur doit s'arrêter proprement");
    }

    #[test]
    fn test_cli_status_reads_real_daemon() {
        let (path, shutdown, handle, _dir) = start_server("palier-publie-2b7b");
        let platform = create_platform();
        let config = DaemonConfig::default();

        // `run` est réellement exécuté (il ne doit ni paniquer ni échouer)…
        run(CliArgs {
            socket: path.clone(),
            command: Commands::Status,
            config: &config,
            platform: &*platform,
        })
        .expect("status ne doit jamais échouer");

        // …et on vérifie le texte qu'il a imprimé, via la MÊME fonction de rendu
        // (`std` ne permet pas de capturer stdout en cours de test : passer par
        // `status_report` évite d'avoir deux textes qui divergeraient).
        let report = status_report(&CliArgs {
            socket: path.clone(),
            command: Commands::Status,
            config: &config,
            platform: &*platform,
        });

        assert!(
            report.contains("palier-publie-2b7b"),
            "le palier publié par le daemon doit apparaître :\n{report}"
        );
        assert!(
            report.contains("Daemon      :"),
            "la ligne « Daemon : … » doit exister :\n{report}"
        );
        assert!(
            report.contains("daemon.sock"),
            "le chemin du socket reste affiché (contrat du smoke test) :\n{report}"
        );

        stop_server(&shutdown, handle);
    }

    #[test]
    fn test_cli_alloc_then_release_roundtrip() {
        let (path, shutdown, handle, _dir) = start_server("balanced");
        let platform = create_platform();
        let config = DaemonConfig::default();

        // 1) Un client DIRECT garde sa connexion ouverte : sa réservation vit
        //    (c'est la seule façon d'avoir un `allocation_id` encore valide, la
        //    réservation de la CLI mourant avec la commande).
        let mut direct = DaemonClient::connect(&path).expect("connexion directe");
        let allocation_id = match direct.allocate(5.0, 1).expect("allocation directe") {
            Reply::AllocGranted { allocation_id, .. } => allocation_id,
            other => panic!("attendu AllocGranted, obtenu {other:?}"),
        };

        // 2) La CLI libère CET identifiant : le ledger est global, pas par
        //    connexion — libérer depuis un autre processus doit marcher.
        run(CliArgs {
            socket: path.clone(),
            command: Commands::Release {
                allocation_id: allocation_id.clone(),
            },
            config: &config,
            platform: &*platform,
        })
        .expect("release doit réussir");

        // 3) La preuve que la CLI a VRAIMENT libéré : la même demande repasse.
        assert!(
            matches!(
                direct.allocate(5.0, 1).expect("seconde allocation"),
                Reply::AllocGranted { .. }
            ),
            "la marge devait être rendue par la CLI"
        );

        // 4) Et `alloc` par la CLI accorde bien quelque chose (il reste 2 GB de
        //    marge : les 5 GB du client direct, moins les 7 GB disponibles).
        run(CliArgs {
            socket: path.clone(),
            command: Commands::Alloc { ram: 1.0, cpu: 1 },
            config: &config,
            platform: &*platform,
        })
        .expect("alloc doit réussir");

        stop_server(&shutdown, handle);
    }

    #[test]
    fn test_cli_reports_daemon_absent_without_panicking() {
        let dir = tempfile::tempdir().expect("dossier temporaire");
        let missing = dir.path().join("personne.sock");
        let platform = create_platform();
        let config = DaemonConfig::default();

        // Toute commande QUI PARLE AU DAEMON doit rendre une erreur propre (donc
        // un code de sortie non nul), jamais une panique ni un `unwrap`.
        for command in [
            Commands::Ping,
            Commands::Alloc { ram: 1.0, cpu: 1 },
            Commands::Release {
                allocation_id: "alloc-inexistant".to_string(),
            },
        ] {
            let err = run(CliArgs {
                socket: missing.clone(),
                command,
                config: &config,
                platform: &*platform,
            })
            .expect_err("un socket absent doit être une erreur");

            let message = format!("{err}");
            assert!(
                message.contains("connexion"),
                "le message doit dire la connexion : {message}"
            );
            assert!(
                message.contains("personne.sock"),
                "le message doit citer le chemin visé : {message}"
            );
        }

        // `status`, lui, ne doit PAS échouer : un daemon éteint n'est pas une
        // anomalie, c'est un état qu'il faut savoir afficher.
        run(CliArgs {
            socket: missing.clone(),
            command: Commands::Status,
            config: &config,
            platform: &*platform,
        })
        .expect("status tolère un daemon éteint");

        let report = status_report(&CliArgs {
            socket: missing.clone(),
            command: Commands::Status,
            config: &config,
            platform: &*platform,
        });
        assert!(
            report.contains("daemon non joignable"),
            "l'absence de daemon doit être dite :\n{report}"
        );
    }

    /// Régression du défaut 1 : un daemon qui a le socket OUVERT mais ne répond
    /// plus ne doit pas faire pendre `status` une minute.
    #[test]
    fn test_status_probe_does_not_hang_on_silent_daemon() {
        let (path, _dir) = start_silent_server();
        let platform = create_platform();
        let config = DaemonConfig::default();

        let started = Instant::now();
        let report = status_report(&CliArgs {
            socket: path.clone(),
            command: Commands::Status,
            config: &config,
            platform: &*platform,
        });
        let elapsed = started.elapsed();

        assert!(
            report.contains("non joignable"),
            "un daemon muet doit être dit injoignable :\n{report}"
        );

        // Borne LARGE (2 s pour une sonde de 1 s) : ce test doit rester vert sur
        // une machine chargée, où la capture de l'instantané système coûte déjà
        // quelque chose. Ce qu'il interdit, c'est d'attendre les 60 s d'un
        // consommateur — c'est tout le défaut qu'il verrouille.
        assert!(
            elapsed < Duration::from_secs(2),
            "la sonde devait renoncer en ~1 s, pas en {elapsed:?} :\n{report}"
        );
    }

    /// Régression du défaut 2 : `watch` doit se RÉABONNER quand le daemon meurt
    /// puis revient sur le MÊME chemin.
    #[test]
    fn test_watch_step_reports_and_recovers() {
        let dir = tempfile::tempdir().expect("dossier temporaire");
        let path = dir.path().join("daemon.sock");
        let mut state = WatchState { client: None };

        // Premier daemon : délai d'inactivité COURT, exprès. `socket.rs` est clos
        // (interdit de le toucher) et `handle_client` y vit dans son PROPRE
        // thread, qu'aucun drapeau d'arrêt n'interrompt : le seul moyen, ici, de
        // faire vraiment mourir la connexion du client est de laisser le serveur
        // fermer une connexion inactive. C'est le même événement observable côté
        // client — une fin de flux — qu'un daemon qui disparaît.
        let (shutdown, handle) = start_server_at(&path, "palier-un", Duration::from_millis(300));

        let line = watch_step(&mut state, &path);
        assert!(
            line.contains("palier-un"),
            "le palier publié doit apparaître : {line}"
        );
        assert!(
            state.client.is_some(),
            "un tour réussi GARDE le client — on ne se reconnecte pas à chaque tour"
        );

        // Le serveur ferme la connexion devenue inactive…
        thread::sleep(Duration::from_millis(900));

        let line = watch_step(&mut state, &path);
        assert!(
            line.contains("non joignable"),
            "une connexion morte doit être dite injoignable : {line}"
        );
        assert!(
            state.client.is_none(),
            "un client mort est JETÉ, pas gardé — sinon `watch` surveillerait un fantôme"
        );

        // …et un nouveau daemon écoute sur le MÊME chemin.
        stop_server(&shutdown, handle);
        let (shutdown, handle) = start_server_at(&path, "palier-deux", CONNECTION_IDLE_TIMEOUT);

        let line = watch_step(&mut state, &path);
        assert!(
            line.contains("palier-deux"),
            "watch doit se RÉABONNER au daemon suivant : {line}"
        );
        assert!(state.client.is_some(), "le client neuf est gardé");

        stop_server(&shutdown, handle);
    }
}
