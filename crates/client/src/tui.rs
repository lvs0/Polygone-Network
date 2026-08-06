//! D1 — The Polygone product TUI.
//!
//! Two commands, vim-style `:` — the whole interface (Axiome 2):
//!
//! ```text
//!   :envoyer   — chiffrer + fragmenter un message pour un destinataire
//!   :quitter   — sortir proprement
//! ```
//!
//! Plus trois utilitaires, tous derrière `:` :
//!   :recevoir  — reconstruire + déchiffrer (4/7 fragments suffisent)
//!   :demo      — la démo E2E complète (relay aveugle, audit, adversaire)
//!   :clef      — afficher votre clef publique (ce qu'on partage)
//!
//! Event-driven: no polling loop. The screen redraws only on key events.

use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};

use crate::identity::LocalIdentity;
use crate::msg;

// ── Pure, testable pieces ─────────────────────────────────────────────────────

/// The result of parsing a `:` command line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Quit,
    Help,
    Status,
    Demo,
    ShowKey,
    Send,
    Receive,
    Ask(String),
    Voisins,
    Compute,
    Unknown(String),
}

/// Parse a command line (after the `:`), trimming and lowercasing.
pub fn parse_command(input: &str) -> Command {
    let raw = input.trim();
    if raw.is_empty() {
        return Command::Unknown(String::new());
    }
    let (head, rest) = raw.split_once(' ').unwrap_or((raw, ""));
    match head.to_ascii_lowercase().as_str() {
        "quitter" | "q" | "exit" => Command::Quit,
        "aide" | "help" | "?" => Command::Help,
        "statut" => Command::Status,
        "demo" => Command::Demo,
        "clef" | "cle" | "key" | "id" => Command::ShowKey,
        "envoyer" | "e" | "send" => Command::Send,
        "recevoir" | "r" | "recv" => Command::Receive,
        "ia" | "ask" | "petals" => Command::Ask(rest.trim().to_string()),
        "voisins" | "mesh" | "v" => Command::Voisins,
        "compute" | "res" => Command::Compute,
        _ => Command::Unknown(raw.to_string()),
    }
}

/// Render the home screen to a string. Pure — no terminal I/O.
pub fn render_home(identity: &LocalIdentity, uptime_secs: u64, note: &str) -> String {
    let uptime = format_uptime(uptime_secs);
    let mut s = String::new();
    s.push_str("  ╔══════════════════════════════════════════════════════════╗\n");
    s.push_str("  ║             ⬡  P O L Y G O N E   v2.0.0-rc2           ║\n");
    s.push_str("  ║   L'information n'existe pas. Elle traverse.            ║\n");
    s.push_str("  ╚══════════════════════════════════════════════════════════╝\n\n");
    s.push_str(&format!("  Identité   : {}\n", identity.pseudo));
    s.push_str(&format!(
        "  Node       : {}  (ML-KEM-1024 + ML-DSA-65)\n",
        identity.short_id()
    ));
    s.push_str("  Statut     : actif · uptime ");
    s.push_str(&uptime);
    s.push_str("\n\n");
    s.push_str("  Services   : msg ●  drive ●  mesh ○  hide ●  brain ○  petals ○\n");
    s.push_str("  Crypto     : ML-KEM-1024 · ML-DSA-65 · AES-256-GCM · BLAKE3 · Shamir 4/7\n");
    s.push_str("  Fragments  : 7 dispersés · 4 suffisent · 3 ne révèlent rien\n\n");
    if !note.is_empty() {
        s.push_str("  ");
        s.push_str(note);
        s.push_str("\n\n");
    }
    s.push_str("  Tapez  « : »  pour commander.   (:aide pour la liste)\n");
    s
}

/// Render the help screen.
pub fn render_help() -> String {
    let mut s = String::new();
    s.push_str("  ⬡ POLYGONE — aide\n\n");
    s.push_str("  :envoyer            chiffrer + fragmenter un message\n");
    s.push_str("                      (sans argument : destinataire fictif)\n");
    s.push_str("  :recevoir           reconstruire + déchiffrer (4/7)\n");
    s.push_str("  :ia <question>      l'IA locale répond (petals, zéro cloud)\n");
    s.push_str("  :voisins            scanner le LAN (mesh, Phase 4)\n");
    s.push_str("  :demo               démo E2E — relay aveugle + audit\n");
    s.push_str("  :clef               afficher votre clef publique\n");
    s.push_str("  :statut             rafraîchir\n");
    s.push_str("  :quitter            sortir\n\n");
    s.push_str("  Échap annule. Chaque commande se tape après « : ».\n");
    s
}

/// Format seconds as h:mm:ss.
pub fn format_uptime(secs: u64) -> String {
    let h = secs / 3600;
    let m = (secs % 3600) / 60;
    let s = secs % 60;
    format!("{h}:{m:02}:{s:02}")
}

// ── Terminal loop ─────────────────────────────────────────────────────────────

/// The interactive TUI. Blocks until `:quitter`.
pub fn run(identity: LocalIdentity) -> anyhow::Result<()> {
    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen)?;

    let result = run_inner(&identity);

    disable_raw_mode()?;
    execute!(stdout, LeaveAlternateScreen)?;
    result
}

/// Interactive state machine — extracted so it can be tested/driven.
struct Session {
    view: View,
    command_buffer: String,
    input_buffer: String,
    input_prompt: String,
    note: String,
    started: std::time::Instant,
}

#[derive(PartialEq)]
enum View {
    Home,
    Help,
    Output,
    SendPk,
    SendMsg,
    ReceivePaste,
}

fn run_inner(identity: &LocalIdentity) -> anyhow::Result<()> {
    let mut session = Session {
        view: View::Home,
        command_buffer: String::new(),
        input_buffer: String::new(),
        input_prompt: String::new(),
        note: String::new(),
        started: std::time::Instant::now(),
    };

    draw(identity, &session)?;

    loop {
        let event = event::read()?;
        if let Event::Key(key) = event {
            if key.kind != KeyEventKind::Press {
                continue;
            }
            if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
                return Ok(());
            }
            match session.view {
                View::Home => handle_home_key(identity, &mut session, key)?,
                View::Help | View::Output => {
                    if key.code == KeyCode::Esc {
                        session.view = View::Home;
                        session.command_buffer.clear();
                        draw(identity, &session)?;
                    }
                }
                View::SendPk | View::SendMsg | View::ReceivePaste => {
                    handle_input_key(identity, &mut session, key)?;
                }
            }
        }
    }
}

fn handle_home_key(
    identity: &LocalIdentity,
    session: &mut Session,
    key: crossterm::event::KeyEvent,
) -> anyhow::Result<()> {
    match key.code {
        KeyCode::Char(':') if !session.command_buffer.starts_with(':') => {
            // Start command mode (vim-style). A ':' typed mid-command is a
            // literal character — handled by the arm below.
            session.command_buffer.clear();
            session.command_buffer.push(':');
            draw_with_prompt(identity, session)?;
        }
        KeyCode::Char(c) if session.command_buffer.starts_with(':') => {
            session.command_buffer.push(c);
            draw_with_prompt(identity, session)?;
        }
        KeyCode::Backspace => {
            if session.command_buffer.starts_with(':') {
                session.command_buffer.pop();
                draw_with_prompt(identity, session)?;
            }
        }
        KeyCode::Enter => {
            if session.command_buffer.starts_with(':') {
                let line = session.command_buffer[1..].to_string();
                execute_command(identity, session, &line)?;
            }
        }
        KeyCode::Esc => {
            session.command_buffer.clear();
            draw(identity, session)?;
        }
        _ => {}
    }
    Ok(())
}

fn handle_input_key(
    identity: &LocalIdentity,
    session: &mut Session,
    key: crossterm::event::KeyEvent,
) -> anyhow::Result<()> {
    match key.code {
        KeyCode::Esc => {
            session.view = View::Home;
            session.input_buffer.clear();
            session.command_buffer.clear();
            draw(identity, session)?;
        }
        KeyCode::Char(c) => {
            session.input_buffer.push(c);
            draw_with_prompt(identity, session)?;
        }
        KeyCode::Backspace => {
            session.input_buffer.pop();
            draw_with_prompt(identity, session)?;
        }
        KeyCode::Enter => match session.view {
            View::SendPk => {
                let pk_hex = session.input_buffer.trim().to_string();
                session.input_buffer.clear();
                session.view = View::SendMsg;
                session.input_prompt = "message :".to_string();
                session.note = format!("destinataire : {:.16}… (hex)", pk_hex);
                draw_with_prompt(identity, session)?;
            }
            View::SendMsg => {
                let message = session.input_buffer.clone();
                session.input_buffer.clear();
                let pk_hex = session
                    .note
                    .split("(hex)")
                    .next()
                    .unwrap_or("")
                    .trim()
                    .to_string();
                let pk = match kem_pk_from_note(&pk_hex) {
                    Some(pk) => pk,
                    None => {
                        // No recipient given — generate a fictive one (self-demo).
                        let (pk, _sk) = polygone_core::crypto::kem::generate_keypair()?;
                        pk
                    }
                };
                let output = msg::send(&message, &pk)?;
                session.view = View::Output;
                session.command_buffer.clear();
                session.note = format!(
                    "Envoyé à {:.16}… — {} fragments, 4 suffisent pour lire.",
                    pk.to_hex(),
                    output.fragments.len()
                );
                session.input_prompt = output.display();
                draw(identity, session)?;
            }
            View::ReceivePaste => {
                let text = session.input_buffer.clone();
                session.input_buffer.clear();
                match msg::SendOutput::parse(&text) {
                    Ok(out) => match msg::receive(&out, &identity.kem_secret_key()?) {
                        Ok(plain) => {
                            session.view = View::Output;
                            session.command_buffer.clear();
                            session.note = "Message reçu, déchiffré et vérifié.".to_string();
                            session.input_prompt = plain;
                            draw(identity, session)?;
                        }
                        Err(e) => {
                            session.note = format!("échec déchiffrement : {e}");
                            session.view = View::Home;
                            draw(identity, session)?;
                        }
                    },
                    Err(e) => {
                        session.note = format!("format invalide : {e}");
                        session.view = View::Home;
                        draw(identity, session)?;
                    }
                }
            }
            _ => {}
        },
        _ => {}
    }
    Ok(())
}

/// Extract the ML-KEM public key hex from the note we stored during SendPk.
fn kem_pk_from_note(note: &str) -> Option<polygone_core::crypto::kem::KemPublicKey> {
    let hex_part = note.trim();
    if hex_part.is_empty() {
        return None;
    }
    polygone_core::crypto::kem::KemPublicKey::from_hex(hex_part).ok()
}

fn execute_command(
    identity: &LocalIdentity,
    session: &mut Session,
    line: &str,
) -> anyhow::Result<()> {
    match parse_command(line) {
        Command::Quit => {
            disable_raw_mode()?;
            let mut stdout = std::io::stdout();
            execute!(stdout, LeaveAlternateScreen)?;
            std::process::exit(0);
        }
        Command::Help => {
            session.view = View::Help;
            session.command_buffer.clear();
            draw(identity, session)?;
        }
        Command::Status => {
            session.command_buffer.clear();
            session.note = String::new();
            draw(identity, session)?;
        }
        Command::Demo => {
            session.command_buffer.clear();
            session.note = String::new();
            let report = crate::demo::build()?;
            session.view = View::Output;
            session.input_prompt = format!(
                "Démo terminée — signature {} · relay: {} · adversaire 3/7: {} · message: {}",
                if report.signature_valid {
                    "VALIDE"
                } else {
                    "INVALIDE"
                },
                if report.relay_saw_plaintext {
                    "a vu du clair !"
                } else {
                    "n'a rien vu"
                },
                if report.adversary_3_reconstructed {
                    "a reconstruit !"
                } else {
                    "ne peut pas reconstruire"
                },
                report.recovered,
            );
            draw(identity, session)?;
        }
        Command::ShowKey => {
            session.command_buffer.clear();
            session.view = View::Output;
            session.input_prompt = format!(
                "Votre clef publique ML-KEM-1024 (à partager) :\n\n{}\n\nFichier identité : {}",
                identity.kem_pk_hex,
                LocalIdentity::path().display()
            );
            draw(identity, session)?;
        }
        Command::Send => {
            session.command_buffer.clear();
            session.view = View::SendPk;
            session.input_buffer.clear();
            session.input_prompt =
                "clef publique ML-KEM du destinataire (hex, Entrée vide = démo) :".to_string();
            session.note = String::new();
            draw_with_prompt(identity, session)?;
        }
        Command::Receive => {
            session.command_buffer.clear();
            session.view = View::ReceivePaste;
            session.input_buffer.clear();
            session.input_prompt =
                "collez le texte reçu (KEM_CT / SENDER_PK / FRAG…) puis Entrée :".to_string();
            session.note = String::new();
            draw_with_prompt(identity, session)?;
        }
        Command::Ask(question) => {
            session.command_buffer.clear();
            session.note = String::new();
            if question.is_empty() {
                session.note =
                    "utilisez « :ia <question> » — l'IA locale répond (petals, zéro cloud)."
                        .to_string();
                draw(identity, session)?;
                return Ok(());
            }
            session.view = View::Output;
            session.input_prompt =
                format!("⬡ Petals — question : {question}\n\n…réflexion locale…");
            draw(identity, session)?;
            match crate::petals::ask(&question, None) {
                Ok(answer) => {
                    session.input_prompt = format!(
                        "⬡ Petals — question : {question}\n\n{answer}\n\n(réponse du modèle local, rien ne quitte la machine)"
                    );
                }
                Err(e) => {
                    session.input_prompt = format!("⬡ Petals — erreur : {e}");
                }
            }
            draw(identity, session)?;
        }
        Command::Voisins => {
            session.command_buffer.clear();
            session.note = String::new();
            session.view = View::Output;
            session.input_prompt = "⬡ Mesh — scan du LAN…".to_string();
            draw(identity, session)?;
            match crate::mesh::discover(std::time::Duration::from_secs(3)) {
                Ok(peers) => {
                    let mut out = String::from("⬡ Mesh — nœuds du LAN\n");
                    if peers.is_empty() {
                        out.push_str(
                            "\n  aucun nœud trouvé.\n  (lancez « polygone ecouter --annoncer » sur un autre poste)",
                        );
                    } else {
                        for p in &peers {
                            out.push_str(&format!("\n  · {}  →  relay {}", p.node_id, p.relay));
                        }
                    }
                    session.input_prompt = out;
                }
                Err(e) => {
                    session.input_prompt = format!("⬡ Mesh — erreur : {e}");
                }
            }
            draw(identity, session)?;
        }
        Command::Compute => {
            session.command_buffer.clear();
            session.note = String::new();
            session.view = View::Output;
            session.input_prompt = "⬡ RES — scan des nœuds fantômes…".to_string();
            draw(identity, session)?;
            let mut out = String::from("⬡ RES — ressources du LAN\n");
            match crate::mesh::free_ram_mb() {
                Some(ram) => out.push_str(&format!("\n  ce nœud : {ram} Mo de RAM libre")),
                None => out.push_str("\n  ce nœud : RAM libre inconnue"),
            }
            match crate::mesh::discover(std::time::Duration::from_secs(3)) {
                Ok(peers) => {
                    if peers.is_empty() {
                        out.push_str("\n\n  aucun nœud fantôme.");
                    } else {
                        out.push_str("\n\n  nœuds fantômes (prêts à prêter) :");
                        for p in &peers {
                            match p.free_ram_mb {
                                Some(ram) => out.push_str(&format!(
                                    "\n  · {} → {} · {ram} Mo libres",
                                    p.node_id, p.relay
                                )),
                                None => out.push_str(&format!("\n  · {} → {}", p.node_id, p.relay)),
                            }
                        }
                    }
                }
                Err(e) => out.push_str(&format!("\n\n  erreur scan : {e}")),
            }
            session.input_prompt = out;
            draw(identity, session)?;
        }
        Command::Unknown(what) => {
            session.command_buffer.clear();
            session.note = format!("commande inconnue : « {} » — tapez :aide", what);
            draw(identity, session)?;
        }
    }
    Ok(())
}

fn uptime(session: &Session) -> u64 {
    session.started.elapsed().as_secs()
}

/// Draw the current view, no command prompt.
fn draw(identity: &LocalIdentity, session: &Session) -> anyhow::Result<()> {
    draw_inner(identity, session, false, "")
}

/// Draw with the `:` command prompt line active.
fn draw_with_prompt(identity: &LocalIdentity, session: &Session) -> anyhow::Result<()> {
    draw_inner(identity, session, true, &session.command_buffer)
}

fn draw_inner(
    identity: &LocalIdentity,
    session: &Session,
    show_cmd: bool,
    cmd_line: &str,
) -> anyhow::Result<()> {
    let mut out = String::new();
    out.push_str("\x1b[2J\x1b[H"); // clear screen + home
    match session.view {
        View::Home => out.push_str(&render_home(identity, uptime(session), &session.note)),
        View::Help => out.push_str(&render_help()),
        View::Output => {
            out.push_str("  ── Polygone · sortie ───────────────────────────────────────\n\n");
            out.push_str(&session.input_prompt);
            out.push_str("\n\n  (Échap pour revenir)\n");
        }
        View::SendPk | View::SendMsg | View::ReceivePaste => {
            out.push_str("  ── Polygone · saisie ────────────────────────────────────────\n\n");
            if !session.note.is_empty() {
                out.push_str(&format!("  {}\n\n", session.note));
            }
            out.push_str(&format!("  {}\n", session.input_prompt));
        }
    }
    if show_cmd {
        out.push_str("\n  :");
        out.push_str(&cmd_line[1.min(cmd_line.len())..]);
        out.push(' ');
    } else if session.view == View::Home {
        out.push_str("\n  :");
    }
    print!("{out}");
    std::io::Write::flush(&mut std::io::stdout())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_known_commands() {
        assert_eq!(parse_command("quitter"), Command::Quit);
        assert_eq!(parse_command("q"), Command::Quit);
        assert_eq!(parse_command("exit"), Command::Quit);
        assert_eq!(parse_command("aide"), Command::Help);
        assert_eq!(parse_command("?"), Command::Help);
        assert_eq!(parse_command("envoyer"), Command::Send);
        assert_eq!(parse_command("e alice"), Command::Send);
        assert_eq!(parse_command("recevoir"), Command::Receive);
        assert_eq!(parse_command("clef"), Command::ShowKey);
        assert_eq!(parse_command("demo"), Command::Demo);
        assert_eq!(parse_command("statut"), Command::Status);
        assert_eq!(
            parse_command("ia quelle heure est-il"),
            Command::Ask("quelle heure est-il".into())
        );
        assert_eq!(
            parse_command("petals explique l'ephemerite"),
            Command::Ask("explique l'ephemerite".into())
        );
        assert_eq!(parse_command("voisins"), Command::Voisins);
        assert_eq!(parse_command("mesh"), Command::Voisins);
        assert_eq!(parse_command("compute"), Command::Compute);
        assert_eq!(parse_command("res"), Command::Compute);
        assert_eq!(parse_command("  aide  "), Command::Help);
    }

    #[test]
    fn parse_unknown_command() {
        assert_eq!(parse_command("planter"), Command::Unknown("planter".into()));
        assert_eq!(parse_command(""), Command::Unknown(String::new()));
    }

    #[test]
    fn home_screen_renders_identity() {
        let id = LocalIdentity::generate();
        let home = render_home(&id, 125, "note test");
        assert!(home.contains(&id.pseudo));
        assert!(home.contains(&id.short_id()));
        assert!(home.contains("0:02:05"));
        assert!(home.contains("note test"));
        assert!(home.contains("L'information n'existe pas."));
    }

    #[test]
    fn uptime_formatting() {
        assert_eq!(format_uptime(0), "0:00:00");
        assert_eq!(format_uptime(125), "0:02:05");
        assert_eq!(format_uptime(3661), "1:01:01");
    }
}
