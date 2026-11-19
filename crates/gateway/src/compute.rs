//! compute.rs — pont d'exécution distante du gateway (mission, cycle 1).
//!
//! Le gateway réutilise le chemin PROUVÉ sur deux hôtes physiques
//! (P0-DEUX-HOTES-2026-11-19) : `polygone_client::net::borrow_compute`
//! (tâche chiffrée ML-KEM-768, grant ML-DSA-65 vérifié contre l'ancre TOFU
//! `peers.json`, recomputation locale L1, ADR-0003). Aucune crypto n'est
//! réécrite ici. Ce module ajoute :
//!   - le garde-fou fail-closed (pair non approuvé → AUCUNE exécution) ;
//!   - le recalcul local L1 optionnel d'une commande déterministe ;
//!   - un [`RemoteReceipt`] honnête (`producer-signed` ≠ `recomputed-local`).
//!
//! Ce n'est PAS du calcul distribué : un producteur, une recomputation locale
//! du demandeur (L1). Le multi-producteur / quorum L2 reste différé (ADR-0003).
//! Le transport est en mode RELAIS — pas de P2P direct, LAN/NAT non prouvé.

use std::time::Duration;

use anyhow::{Context, Result, bail};
use polygone_client::identity::LocalIdentity;
use polygone_client::{exec, net};
use polygone_core::task::RemoteReceipt;

/// Cible d'exécution distante, validée en amont par le store de tâches.
pub struct RemoteTarget {
    /// node_id du producteur approuvé (ancre TOFU dans `peers.json`).
    pub node: String,
    /// Adresse du relais (host:port).
    pub relay: String,
    /// Commande déterministe à exécuter à distance (et à recalculer en L1).
    pub command: String,
    /// Recalcul local L1 demandé (sinon L0 : signature producteur seule).
    pub verify_local: bool,
}

/// Bornes identiques au chemin CLI prouvé (`compute --emprunter`).
const COMPUTE_TIMEOUT: Duration = Duration::from_secs(40);
const RECOMPUTE_MEM_MB: u32 = 256;
const RECOMPUTE_CPU_PCT: u32 = 50;
const RECOMPUTE_TIMEOUT: Duration = Duration::from_secs(30);

/// Emprunte du calcul à un pair approuvé via le relais, puis recompte
/// localement si demandé (L1).
///
/// Fail-closed : pair inconnu/non approuvé, refus du producteur, timeout,
/// grant illisible ou échec du recalcul local → `Err` ou reçu L0 honnête.
/// Aucun reçu de succès n'est publié sans grant vérifié en amont.
pub async fn borrow_remote(target: &RemoteTarget) -> Result<RemoteReceipt> {
    // 1. Ancre de confiance : le pair doit être approuvé (TOFU, peers.json).
    let mut peers = net::load_peers().context("registre de pairs du gateway")?;
    if !peers.contains_key(&target.node) {
        bail!("pair non approuvé ou inconnu : exécution distante refusée (fail-closed)");
    }

    // 2. Identité locale du demandeur : le gateway emprunte comme un client.
    let identity = LocalIdentity::load_or_create().context("identité locale du gateway")?;

    // 3. Chemin prouvé : tâche chiffrée, grant signé ML-DSA vérifié TOFU avant
    //    déchiffrement. `None` = refus ou timeout.
    let grant = net::borrow_compute(
        &target.relay,
        &target.node,
        &target.command,
        &identity,
        &mut peers,
        COMPUTE_TIMEOUT,
    )
    .await
    .context("emprunt de calcul distant")?;
    let grant = grant.ok_or_else(|| anyhow::anyhow!("aucun grant reçu (refus ou timeout)"))?;

    // 4. Persister les pairs appris (TOFU) — comme le CLI. Non fatal.
    let _ = net::save_peers(&peers);

    // 5. Parser le grant vérifié.
    let body: serde_json::Value = serde_json::from_str(&grant).context("grant illisible")?;
    if body.get("ok").and_then(serde_json::Value::as_bool) != Some(true) {
        let reason = body
            .get("refus")
            .or_else(|| body.get("error"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or("refus du producteur");
        bail!("exécution distante refusée : {reason}");
    }
    let producer = body
        .get("node")
        .and_then(serde_json::Value::as_str)
        .unwrap_or(&target.node)
        .to_string();
    let output = body
        .get("output")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_string();
    let l0_level = body
        .get("verification")
        .and_then(serde_json::Value::as_str)
        .unwrap_or(net::VERIFICATION_PRODUCER_SIGNED)
        .to_string();
    let l0_limit = body
        .get("verification_limit")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("la signature prouve l'auteur, pas la justesse du résultat")
        .to_string();

    // 6. L1 optionnel : recalcul local de la MÊME commande déterministe.
    if !target.verify_local {
        return Ok(RemoteReceipt {
            producer,
            relay: target.relay.clone(),
            command: target.command.clone(),
            output,
            verification: l0_level,
            verification_limit: l0_limit,
            recomputed: false,
            evidence_path: None,
        });
    }

    match exec::run_sandboxed(
        &target.command,
        RECOMPUTE_MEM_MB,
        RECOMPUTE_CPU_PCT,
        RECOMPUTE_TIMEOUT,
    ) {
        Ok(local) => {
            let (annotated, reproduced) =
                net::annotate_local_recomputation(&grant, &local).context("annotation L1")?;
            // La persistance de la preuve ne invalide jamais un recalcul réussi.
            let evidence = net::persist_l1_evidence(
                &annotated,
                &target.command,
                &target.node,
                &net::node_id(&identity),
            )
            .ok()
            .flatten()
            .map(|path| path.display().to_string());
            let annotated_body: serde_json::Value =
                serde_json::from_str(&annotated).unwrap_or(body);
            let verification = annotated_body
                .get("verification")
                .and_then(serde_json::Value::as_str)
                .unwrap_or(net::VERIFICATION_PRODUCER_SIGNED)
                .to_string();
            let verification_limit = annotated_body
                .get("verification_limit")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("")
                .to_string();
            Ok(RemoteReceipt {
                producer,
                relay: target.relay.clone(),
                command: target.command.clone(),
                output,
                verification,
                verification_limit,
                recomputed: reproduced,
                evidence_path: evidence,
            })
        }
        // Recalcul local impossible : on ne prétend pas vérifier. Reste L0.
        Err(error) => Ok(RemoteReceipt {
            producer,
            relay: target.relay.clone(),
            command: target.command.clone(),
            output,
            verification: l0_level,
            verification_limit: format!("recalcul local indisponible : {error}"),
            recomputed: false,
            evidence_path: None,
        }),
    }
}