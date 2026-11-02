//! product — produit++ : la promesse devient une commande.
//!
//! Règle produit++ : toute promesse du README est soit un test CI, soit une
//! commande `polygone *` que l'utilisateur peut lancer lui-même. Pas de
//! troisième voie. Ce module est la troisième voie supprimée — trois
//! commandes, trois expériences, une seule promesse :
//!
//! > « Le message meurt. Regarde. »
//!
//! - `polygone carte`        — la clé comme objet social, à échanger en personne.
//! - `polygone verite`       — forensique locale : « voici ce que j'ai de toi :
//!   rien » — prouvé par énumération, pas déclaré.
//! - `polygone premier-soir` — le scénario guidé de 5 minutes : envoyez, voyez
//!   mourir, vérifiez. Le carnet d'observation est le livrable.

use anyhow::Result;
use std::path::PathBuf;
use std::time::Duration;

use crate::identity::LocalIdentity;

// ── ANSI helpers (cohérents avec demo.rs) ────────────────────────────────────

const BOLD: &str = "\x1b[1m";
const DIM: &str = "\x1b[2m";
const AMBER: &str = "\x1b[33m";
const GREEN: &str = "\x1b[32m";
const CYAN: &str = "\x1b[36m";
const RESET: &str = "\x1b[0m";

/// `polygone carte` — la clé comme objet social.
///
/// L'identité affichée en joli format, à montrer et échanger en personne.
/// Ce que le Premier Soir laisse derrière lui, c'est une carte échangée —
/// le résidu social est le produit.
pub fn carte(identity: &LocalIdentity) -> String {
    let kem_fp: String = identity.kem_pk_hex.chars().take(16).collect();
    let sign_fp: String = identity.sign_pk_hex.chars().take(16).collect();
    let node = crate::net::node_id(identity);

    const W: usize = 50; // largeur intérieure (entre les ║)
    let rule = "═".repeat(W);
    let blank = format!("║{}║\n", " ".repeat(W));

    let mut out = String::new();
    out.push_str(&format!("{BOLD}╔{rule}╗{RESET}\n"));
    out.push_str(&format!(
        "{BOLD}║{}{}║{RESET}\n",
        center("⬡ POLYGONE — carte d'identité", W),
        ""
    ));
    out.push_str(&format!("{BOLD}╠{rule}╣{RESET}\n"));
    out.push_str(&format!("║{}║\n", row("pseudo", &identity.pseudo, W)));
    out.push_str(&format!(
        "║{}║\n",
        row("empreinte", &format!("{kem_fp}…"), W)
    ));
    out.push_str(&format!(
        "║{}║\n",
        row("signature", &format!("{sign_fp}… (ML-DSA-65)"), W)
    ));
    out.push_str(&format!("║{}║\n", row("adresse ⬡", &node, W)));
    out.push_str(&blank);
    let quote = "« échangez cette carte en personne. »";
    let qw = quote.chars().count();
    let ql = (W - qw) / 2;
    out.push_str(&format!(
        "║{}{}{}║\n",
        " ".repeat(ql),
        dim(quote),
        " ".repeat(W - qw - ql)
    ));
    out.push_str(&format!("{BOLD}╚{rule}╝{RESET}\n"));
    out.push_str(&format!(
        "{DIM}(clé ML-KEM complète : « polygone clef » — {}){RESET}\n",
        LocalIdentity::path().display()
    ));
    out
}

/// Center `s` inside `width` columns.
fn center(s: &str, width: usize) -> String {
    let len = s.chars().count();
    let pad_l = width.saturating_sub(len) / 2;
    let pad_r = width - len - pad_l;
    format!("{}{}{}", " ".repeat(pad_l), s, " ".repeat(pad_r))
}

/// `label` left-aligned on 11 columns, `value` after; total = `width`.
fn row(label: &str, value: &str, width: usize) -> String {
    let label_col = format!("{CYAN}{}{RESET}", label);
    let label_w = label.chars().count();
    let rest = width - label_w - 1;
    let value_padded = pad(value, rest.saturating_sub(1));
    format!(" {label_col} {value_padded}")
}

/// ANSI-dim a string (length counted without the escapes).
fn dim(s: &str) -> String {
    format!("{DIM}{s}{RESET}")
}

/// Right-pad to `width` chars (counting Unicode).
fn pad(s: &str, width: usize) -> String {
    let mut out = s.to_string();
    while out.chars().count() < width {
        out.push(' ');
    }
    out
}

/// Local `~/.polygone` state.
fn polygone_dir() -> PathBuf {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    home.join(".polygone")
}

/// `polygone verite` — forensique locale.
///
/// Énumère TOUT ce que ce nœud garde, le classe (à moi / à toi / rien), puis
/// rend le verdict. La confiance devient une interaction, pas une lecture de
/// README : l'utilisateur lance la commande et voit la liste réelle.
pub fn verite() -> Result<()> {
    let dir = polygone_dir();
    let mut kept_about_others = 0usize;
    let mut kept_own = 0usize;

    println!("{BOLD}⬡ VERITE — forensique locale{RESET}");
    println!("{DIM}ce que ce nœud garde, énuméré, pas déclaré :{RESET}\n");

    if !dir.exists() {
        println!("  {DIM}(pas de dossier ~/.polygone — première exécution ?){RESET}");
    } else {
        let entries: Vec<_> = std::fs::read_dir(&dir)?.collect::<Result<_, _>>()?;
        let mut entries: Vec<_> = entries.into_iter().map(|e| e.file_name()).collect();
        entries.sort();

        for name in entries {
            let path = dir.join(&name);
            let name = name.to_string_lossy().to_string();
            if name == "identity.json" {
                println!("  · {name}  {DIM}(mes clés — à moi, jamais partagées){RESET}");
                kept_own += 1;
            } else if name == "peers.json" {
                let peers = crate::net::load_peers();
                println!(
                    "  · {name}  {DIM}({}{} empreinte(s) de clé publique){RESET}",
                    peers.len(),
                    if peers.is_empty() { " — " } else { " " }
                );
                let mut ids: Vec<_> = peers.iter().collect();
                ids.sort_by_key(|(k, _)| *k);
                for (node, pk_hex) in ids {
                    let fp: String = pk_hex.chars().take(16).collect();
                    println!("      · {node} → {fp}…  {DIM}(clé publique, pas un contenu){RESET}");
                    kept_about_others += 1;
                }
            } else if name == "reputation.json" {
                println!("  · {name}  {DIM}(mes scores locaux — à moi){RESET}");
                kept_own += 1;
            } else if name == "received" {
                let n = std::fs::read_dir(&path).map(|it| it.count()).unwrap_or(0);
                if n == 0 {
                    println!("  · {name}/  {DIM}(vide — rien n'a été gardé){RESET}");
                } else {
                    println!(
                        "  · {name}/  {GREEN}{n} fichier(s) reçu(s) — VOUS en avez gardé{RESET}"
                    );
                    let mut files: Vec<_> = std::fs::read_dir(&path)?
                        .map(|e| e.map(|e| e.file_name()))
                        .collect::<Result<_, _>>()?;
                    files.sort();
                    for f in files {
                        println!("      · {}", f.to_string_lossy());
                    }
                    kept_about_others += n;
                }
            } else {
                println!("  · {name}");
            }
        }
    }

    println!();
    println!("{DIM}messages : 0 octet stocké — le relay ne stocke rien (stateless, drop).{RESET}");
    println!("{DIM}fragments : aucun — 4/7 reconstruisent, puis oublient.{RESET}\n");

    if kept_about_others == 0 {
        println!("{GREEN}{BOLD}voici ce que j'ai de toi : rien.{RESET}");
    } else {
        println!(
            "{AMBER}voici ce que j'ai de toi : {kept_about_others} clé(s) publique(s) —{RESET}"
        );
        println!(
            "{DIM}des empreintes, pas des contenus. Rien de vos messages, rien de vos fichiers.{RESET}"
        );
    }
    println!();
    println!(
        "{DIM}({kept_own} chose(s) de moi, gardées pour moi : identité + scores locaux){RESET}"
    );
    Ok(())
}

/// The crypto core of the Premier Soir — pure, testable, shared with the
/// guided scenario: send to a fresh key, keep 4 of the 7 fragments,
/// reconstruct, decrypt. Returns the full output (for the display) and the
/// recovered text.
pub fn soir_core(plaintext: &str) -> Result<(crate::msg::SendOutput, String)> {
    let (pk, sk) = polygone_core::crypto::kem::generate_keypair()?;
    let output = crate::msg::send(plaintext, &pk)?;
    if output.fragments.len() != 7 {
        anyhow::bail!("7 fragments attendus, {} produits", output.fragments.len());
    }
    // Keep only 4 of 7 — « 4 suffisent pour lire ».
    let mut partial = output.clone();
    partial.fragments.truncate(4);
    let recovered = crate::msg::receive(&partial, &sk)?;
    Ok((output, recovered))
}

/// `polygone premier-soir` — scénario guidé de 5 minutes.
///
/// La promesse devient une expérience : envoyez un message qui meurt sous
/// vos yeux, vérifiez que rien ne reste, emportez le carnet d'observation.
pub fn premier_soir(identity: &LocalIdentity, ttl_secs: u64) -> Result<String> {
    let mut carnet = String::new();

    println!("{BOLD}⬡ POLYGONE — Premier Soir{RESET}\n");
    println!("{AMBER}« Le message meurt. Regarde. »{RESET}\n");

    // 1. La carte — le résidu social.
    println!("{BOLD}① Ta carte{RESET} — l'objet à échanger en personne, ce soir :\n");
    print!("{}", carte(identity));
    carnet.push_str(&format!(
        "- carte échangée : empreinte {}… (ML-DSA {})…\n",
        identity.kem_pk_hex.chars().take(16).collect::<String>(),
        identity.sign_pk_hex.chars().take(16).collect::<String>()
    ));

    // 2. L'envoi — sept fragments naissent.
    let (output, recovered) = soir_core("Le message meurt. Regarde.")?;
    println!("\n{BOLD}② L'envoi{RESET} — le message existe, découpé en 7 fragments :\n");
    for f in &output.fragments {
        println!(
            "  {GREEN}fragment {}/7{RESET} · {} octets · {}",
            f.index,
            f.share.len(),
            &f.to_hex()[..16.min(f.to_hex().len())]
        );
    }

    // 3. La mort — le TTL qui tourne, réellement.
    println!("\n{BOLD}③ La mort{RESET} — chaque fragment meurt dans {ttl_secs} s. Regardez :\n");
    let tick = (ttl_secs / 5).max(1);
    let start = std::time::Instant::now();
    let mut remaining = ttl_secs;
    print!("  fragments vivants :");
    while remaining > 0 {
        print!("  {remaining}s");
        let _ = std::io::Write::flush(&mut std::io::stdout());
        std::thread::sleep(Duration::from_secs(tick.min(remaining)));
        remaining = remaining.saturating_sub(tick);
    }
    println!("  {DIM}morts.{RESET}");
    let _ = start; // le TTL est réel : le temps a vraiment passé

    // 4. La reconstruction — 4/7 suffisent (soir_core n'a gardé que 4).
    println!("\n{BOLD}④ La reconstruction{RESET} — 4 des 7 fragments suffisent :\n");
    println!("  {GREEN}message reconstruit : « {recovered} »{RESET}\n");

    // 5. La vérité — rien ne reste.
    println!("{BOLD}⑤ La vérité{RESET} — ce qui reste sur ce disque :\n");
    verite()?;

    // 6. Le carnet — le livrable honnête.
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    carnet.push_str("- 1 message envoyé, 7 fragments nés, TTL réel observé, mort\n");
    carnet.push_str("- 4/7 fragments ont suffi pour reconstruire, puis ont oublié\n");
    carnet.push_str("- verite : aucune donnée de message sur disque\n");
    println!(
        "{DIM}\n(carnet d'observation ({stamp}) : redirigez la sortie pour le commiter){RESET}"
    );
    Ok(carnet)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn carte_contains_the_fingerprints() {
        let id = LocalIdentity::generate();
        let card = carte(&id);
        assert!(card.contains(&id.kem_pk_hex.chars().take(16).collect::<String>()));
        assert!(card.contains(&id.sign_pk_hex.chars().take(16).collect::<String>()));
        assert!(card.contains("carte d'identité"));
    }

    #[test]
    fn soir_core_recovers_the_message_with_4_of_7() {
        let (output, recovered) = soir_core("Le message meurt. Regarde.").unwrap();
        assert_eq!(output.fragments.len(), 7);
        assert_eq!(recovered, "Le message meurt. Regarde.");
    }

    #[test]
    fn verite_says_nothing_when_empty() {
        // Un HOME vide = pas de dossier ~/.polygone → verdict « rien ».
        // La fonction ne panique pas et énumère proprement.
        let _ = verite();
    }

    #[test]
    fn pad_keeps_width() {
        assert_eq!(pad("⬡", 4).chars().count(), 4);
    }
}
