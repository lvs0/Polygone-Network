//! Test contrat — exécution distante via le gateway (mission, cycle 1).
//!
//! Relais RÉEL en boucle locale (`polygone_relay`) + ghost `receive_network`
//! + `TaskStore::create` avec `target_node`. Prouve le chemin complet :
//!   1. L1 : recalcul local d'une commande déterministe → `recomputed-local`,
//!      preuve persistée (ADR-0003) ;
//!   2. L0 : signature producteur seule → `producer-signed`, sans surclaim ;
//!   3. fail-closed : pair non approuvé → `failed`, jamais de faux succès ;
//!   4. validation : node_id invalide → rejet immédiat.
//!
//! Le sandbox `systemd-run --user` est requis (le ghost exécute la commande).
//! Sur un hôte incapable, le test saute avec un message — il ne simule rien.

use std::collections::HashMap;
use std::time::Duration;

use polygone_client::identity::LocalIdentity;
use polygone_client::net;
use polygone_core::task::TaskState;
use polygone_gateway::tasks::{CreateTaskRequest, TaskStore};

/// Le ghost exécute via le sandbox réel : on probe le chemin durci complet.
fn sandbox_available() -> bool {
    polygone_client::exec::run_sandboxed(
        "printf polygone-gw-probe",
        64,
        50,
        Duration::from_secs(20),
    )
    .is_ok()
}

fn request(command: &str, target: &str, relay: &str, verify_local: bool) -> CreateTaskRequest {
    CreateTaskRequest {
        owner: "gateway-contract".into(),
        kind: "echo".into(),
        input_hex: hex::encode(command),
        cpu_cores: 1,
        ram_bytes: 64 * 1024 * 1024,
        storage_bytes: 0,
        gpu_vram_bytes: 0,
        deadline_in_seconds: 120,
        max_attempts: 1,
        allow_network: false,
        allow_filesystem_write: false,
        max_output_bytes: 4096,
        target_node: Some(target.into()),
        relay: Some(relay.into()),
        verify_local: Some(verify_local),
    }
}

#[tokio::test]
async fn remote_compute_contract_through_real_relay() {
    if !sandbox_available() {
        eprintln!("sandbox systemd-run --user indisponible — test contrat sauté (honnête, pas simulé)");
        return;
    }

    let home = tempfile::tempdir().unwrap();
    // Édition 2024 : `set_var` est unsafe. Ce binaire de test est isolé par
    // processus ; HOME est fixé UNE fois avant tout spawn/sandbox, jamais muté
    // ensuite (pas de race HOME ↔ sandbox dans ce processus).
    unsafe {
        std::env::set_var("HOME", home.path());
    }

    // Ghost B + ancre TOFU : le gateway (emprunteur) approuve B dans peers.json.
    let ghost = LocalIdentity::generate();
    let ghost_node = net::node_id(&ghost);
    let mut peers = HashMap::new();
    peers.insert(ghost_node.clone(), ghost.sign_pk_hex.clone());
    net::save_peers(&peers).unwrap();

    // Le VRAI relais, port éphémère.
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let relay_addr = listener.local_addr().unwrap().to_string();
    let relay_task = tokio::spawn(async move {
        let _ = polygone_relay::relay::run_listener_with_options(listener, true).await;
    });

    // Ghost B : écoute + compute (charge ses pairs depuis HOME, signé ML-DSA).
    let ghost_relay = relay_addr.clone();
    let ghost_task = tokio::spawn(async move {
        let _ = net::receive_network(&ghost_relay, &ghost, true).await;
    });
    tokio::time::sleep(Duration::from_millis(500)).await;

    let store = TaskStore::new(Some(home.path().join("tasks.json")));
    let now = 1_900_000_000u64;

    // 1) L1 — recalcul local de la même commande déterministe.
    let task = store
        .create(request("echo hello", &ghost_node, &relay_addr, true), now)
        .await
        .expect("tâche distante L1 créée");
    assert_eq!(task.state, TaskState::Succeeded, "L1 doit réussir");
    let receipt = task.remote_receipt.expect("reçu distant présent");
    assert_eq!(receipt.producer, ghost_node, "producteur = ghost assigné");
    assert_eq!(receipt.output, "hello\n");
    assert_eq!(receipt.verification, net::VERIFICATION_RECOMPUTED);
    assert!(receipt.recomputed, "L1 doit reproduire la sortie");
    assert!(receipt.evidence_path.is_some(), "preuve L1 persistée (ADR-0003)");
    assert!(!receipt.verification_limit.is_empty(), "limite honnête portée");

    // 2) L0 — signature producteur seule, pas de surclaim « vérifié ».
    let task0 = store
        .create(request("echo world", &ghost_node, &relay_addr, false), now)
        .await
        .expect("tâche distante L0 créée");
    assert_eq!(task0.state, TaskState::Succeeded);
    let receipt0 = task0.remote_receipt.expect("reçu L0 présent");
    assert_eq!(receipt0.output, "world\n");
    assert_eq!(receipt0.verification, net::VERIFICATION_PRODUCER_SIGNED);
    assert!(!receipt0.recomputed, "L0 ne recompte pas");
    assert!(receipt0.evidence_path.is_none(), "pas de preuve L1 en L0");

    // 3) Fail-closed — pair non approuvé : `failed`, aucun faux succès.
    let unknown = "f".repeat(32);
    let task_bad = store
        .create(request("echo nope", &unknown, &relay_addr, true), now)
        .await
        .expect("création acceptée puis échec d'exécution");
    assert_eq!(task_bad.state, TaskState::Failed, "pair inconnu doit échouer");
    assert!(task_bad.remote_receipt.is_none(), "aucun reçu de succès");
    assert!(task_bad.last_error.is_some(), "raison d'échec journalisée");

    // 4) Validation — node_id invalide : rejet immédiat, avant tout réseau.
    let invalid = request("echo x", "zzz", &relay_addr, false);
    assert!(
        store.create(invalid, now).await.is_err(),
        "node_id invalide rejeté"
    );

    relay_task.abort();
    ghost_task.abort();
}