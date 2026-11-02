//! `polygone serverless` — la commande utilisateur de Polygone Serverless.
//!
//! Elle s'appuie sur le crate `polygone-serverless`, qui porte la seule
//! mécanique honnête du service :
//!
//! - la charge utile est découpée en **vraies parts Shamir 4-of-7** pour le
//!   transit (`request::shards`) ;
//! - le résultat, lui, ne se « reconstitue » pas à partir des parts : il est
//!   accepté par **quorum**, c'est-à-dire quand `seuil` nœuds distincts
//!   renvoient le même hash de sortie (`quorum::tally`).
//!
//! État : la commande prépare et vérifie la demande localement. **Aucun nœud
//! n'est contacté** — le transport vers les nœuds fantômes (`ecouter
//! --compute`) n'est pas branché sur ce chemin. Le dire à l'écran fait partie
//! du contrat de la maison : on n'affiche pas un résultat qui n'existe pas.

use anyhow::Result;

use polygone_serverless::{reassemble, FunctionKind, ServerlessRequest};

/// Construit la demande, vérifie la fragmentation, et rend un rapport lisible.
///
/// Retourne `true` si la vérification locale (4 parts sur 7 → même charge
/// utile) a réussi.
pub fn run(commande: &str, payload: &str, seuil: u8, parts: u8) -> Result<bool> {
    let function = FunctionKind::Shell {
        command: commande.to_string(),
    };
    let mut request = ServerlessRequest::new(function, payload.as_bytes().to_vec());
    request.threshold = seuil;
    request.total = parts;
    request.validate()?;

    let shards = request.shards()?;
    let hash = request.request_hash();

    println!("⬡ Serverless — demande {}", request.id);
    println!("  fonction      : shell `{commande}`");
    println!(
        "  charge utile  : {} octets · hash {}",
        payload.len(),
        hex::encode(&hash[..8])
    );
    println!(
        "  politique     : {seuil}-of-{parts} — Shamir pour le transit, quorum pour le résultat"
    );
    println!("  parts :");
    for shard in &shards {
        println!(
            "    · part {}  {} octets  hash {}",
            shard.index,
            shard.payload.len(),
            hex::encode(&shard.content_hash[..8])
        );
    }

    // Vérification locale réelle : on ne l'affirme pas, on la calcule.
    let threshold = usize::from(seuil);
    let sample = &shards[..threshold.min(shards.len())];
    let restored = reassemble(sample, seuil)?;
    let ok = restored == payload.as_bytes();
    println!(
        "  vérification  : {} parts sur {} → {} ",
        threshold.min(shards.len()),
        shards.len(),
        if ok {
            "charge utile identique"
        } else {
            "ÉCHEC — les parts ne se reconstituent pas"
        }
    );

    println!();
    println!("  Aucun nœud n'a été contacté : le transport vers les nœuds fantômes");
    println!("  n'est pas branché (état « partiel » dans STAGING.md). La demande");
    println!("  et sa politique sont prêtes ; l'envoi reste à câbler.");

    Ok(ok)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_and_reassemble_roundtrip() {
        let ok = run("echo", "salut", 4, 7).expect("la demande doit être valide");
        assert!(ok, "4 parts sur 7 doivent reconstituer la charge utile");
    }

    #[test]
    fn rejects_policy_below_two() {
        // Un seuil de 1 ne protège rien : le crate doit refuser.
        assert!(run("echo", "salut", 1, 7).is_err());
    }

    #[test]
    fn rejects_more_parts_than_protocol_allows() {
        assert!(run("echo", "salut", 4, 9).is_err());
    }
}
