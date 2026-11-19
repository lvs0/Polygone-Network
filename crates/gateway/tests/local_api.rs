//! Tests de contrat des domaines locaux F1-API-01 (identité, daemon,
//! ressources, réglages, usage, événements).
//!
//! Chaque route est exercée via `Gateway::route` sur des chemins temporaires
//! injectés (`with_local`) — jamais `$HOME`. Les relais sont comparés à un
//! VRAI daemon `polygoned` sur un socket temporaire : la parité HTTP/socket
//! est vérifiée champ par champ, omissions comprises. Un daemon absent, un
//! daemon ancien (sans capacité `ledger`) et un protocole inattendu ont
//! chacun leur test : l'absence ne se déguise jamais en zéro.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;

use polygone_gateway::Gateway;
use polygone_gateway::events::EVENT_CAPACITY;
use polygone_gateway::http::{HttpRequest, HttpResponse};
use polygone_gateway::local::LocalPaths;
use serde_json::{Value, json};

fn get(target: &str) -> HttpRequest {
    match target.split_once('?') {
        Some((path, query)) => HttpRequest::new("GET", path, Vec::new()).with_query(query),
        None => HttpRequest::new("GET", target, Vec::new()),
    }
}

fn post(path: &str, body: &str) -> HttpRequest {
    HttpRequest::new("POST", path, body.as_bytes().to_vec())
}

fn body_json(response: &HttpResponse) -> Value {
    serde_json::from_slice(&response.body).expect("réponse JSON")
}

fn local_paths(dir: &tempfile::TempDir) -> LocalPaths {
    LocalPaths {
        identity_file: dir.path().join("identity.json"),
        config_file: dir.path().join("daemon.toml"),
        daemon_socket: dir.path().join("daemon.sock"),
    }
}

fn gateway_with(dir: &tempfile::TempDir) -> Gateway {
    Gateway::new(dir.path().join("site")).with_local(local_paths(dir))
}

/// Identité complète telle que le client l'écrit, avec une VRAIE clé publique
/// ML-DSA-65 (pour que la dérivation du node_id réussisse) et des secrets
/// factices qu'aucune réponse ne doit jamais contenir.
fn write_identity(dir: &tempfile::TempDir) -> String {
    let keypair = polygone_core::sign::generate_keypair().expect("keygen ML-DSA-65");
    let sign_pk_hex = keypair.verifier.public_key().to_hex();
    let identity = json!({
        "pseudo": "vox-contrat",
        "kem_pk_hex": "aa".repeat(32),
        "kem_sk_hex": "SECRET-KEM-NE-JAMAIS-PUBLIER",
        "sign_pk_hex": sign_pk_hex,
        "sign_sk_hex": "SECRET-SIGN-NE-JAMAIS-PUBLIER",
        "revoked_sign_pk_hex": [],
        "rotated_at_unix": null
    });
    std::fs::write(dir.path().join("identity.json"), identity.to_string()).unwrap();
    sign_pk_hex
}

#[tokio::test]
async fn identity_publique_derive_le_node_id_et_ne_fuite_aucun_secret() {
    let dir = tempfile::tempdir().unwrap();
    let sign_pk_hex = write_identity(&dir);
    let gateway = gateway_with(&dir);

    let response = gateway.route(get("/api/v1/identity")).await;
    assert_eq!(response.status, 200);
    let body = body_json(&response);
    assert_eq!(body["pseudo"], "vox-contrat");
    assert_eq!(
        body["node_id"],
        polygone_core::sign::node_id_from_sign_pk_hex(&sign_pk_hex).unwrap(),
        "le node_id doit être la dérivation ADR-0002 de la clé publique"
    );
    let text = String::from_utf8(response.body.clone()).unwrap();
    assert!(!text.contains("SECRET-KEM"), "clé secrète KEM publiée");
    assert!(
        !text.contains("SECRET-SIGN"),
        "clé secrète de signature publiée"
    );
    assert!(!text.contains("sk_hex"), "un champ secret est publié");
}

#[tokio::test]
async fn identity_absente_repond_404_explicite() {
    let dir = tempfile::tempdir().unwrap();
    let gateway = gateway_with(&dir);
    let response = gateway.route(get("/api/v1/identity")).await;
    assert_eq!(response.status, 404);
    assert_eq!(body_json(&response)["error"], "identity_absent");
}

#[tokio::test]
async fn identity_illisible_repond_500_explicite() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("identity.json"), "{ cassé").unwrap();
    let gateway = gateway_with(&dir);
    let response = gateway.route(get("/api/v1/identity")).await;
    assert_eq!(response.status, 500);
    assert_eq!(body_json(&response)["error"], "identity_unreadable");
}

#[tokio::test]
async fn settings_absentes_publient_les_defaults_produit() {
    let dir = tempfile::tempdir().unwrap();
    let gateway = gateway_with(&dir);
    let response = gateway.route(get("/api/v1/settings")).await;
    assert_eq!(response.status, 200);
    let body = body_json(&response);
    assert_eq!(body["source"], "defaults");
    assert_eq!(body["config"]["tier"], "balanced");
    assert_eq!(body["config"]["memory_limit_enabled"], true);
}

#[tokio::test]
async fn settings_lues_depuis_le_fichier() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("daemon.toml"),
        "tier = \"performance\"\nmemory_limit_enabled = false\n",
    )
    .unwrap();
    let gateway = gateway_with(&dir);
    let response = gateway.route(get("/api/v1/settings")).await;
    assert_eq!(response.status, 200);
    let body = body_json(&response);
    assert_eq!(body["source"], "file");
    assert_eq!(body["config"]["tier"], "performance");
    assert_eq!(body["config"]["memory_limit_enabled"], false);
}

#[tokio::test]
async fn settings_invalides_ne_sont_jamais_masquees_par_des_defaults() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("daemon.toml"), "tier = \"inventé\"\n").unwrap();
    let gateway = gateway_with(&dir);
    let response = gateway.route(get("/api/v1/settings")).await;
    assert_eq!(response.status, 500);
    assert_eq!(body_json(&response)["error"], "config_invalid");
}

#[tokio::test]
async fn events_ordre_curseur_et_rejeu() {
    let dir = tempfile::tempdir().unwrap();
    let gateway = gateway_with(&dir);
    for i in 0..5 {
        gateway.events.emit("test", json!({ "i": i }));
    }
    let response = gateway.route(get("/api/v1/events?since=0")).await;
    assert_eq!(response.status, 200);
    let page = body_json(&response);
    let events = page["events"].as_array().unwrap();
    assert_eq!(events.len(), 5);
    assert!(
        events
            .windows(2)
            .all(|w| w[0]["id"].as_u64() < w[1]["id"].as_u64())
    );
    assert_eq!(page["gap"], false);
    let cursor = page["next_since"].as_u64().unwrap();

    let response = gateway
        .route(get(&format!("/api/v1/events?since={cursor}")))
        .await;
    let page = body_json(&response);
    assert!(page["events"].as_array().unwrap().is_empty());
    assert_eq!(page["gap"], false);
}

#[tokio::test]
async fn events_debordement_signale_le_gap() {
    let dir = tempfile::tempdir().unwrap();
    let gateway = gateway_with(&dir);
    for i in 0..(EVENT_CAPACITY + 40) {
        gateway.events.emit("overflow", json!({ "i": i }));
    }
    let response = gateway.route(get("/api/v1/events?since=0&limit=500")).await;
    let page = body_json(&response);
    assert_eq!(page["gap"], true, "la perte doit être signalée");
    assert_eq!(page["dropped_total"], 40);
    // Page plafonnée à 500 ; l'anneau en retient 512.
    assert_eq!(page["events"].as_array().unwrap().len(), 500);
}

#[tokio::test]
async fn events_filtre_par_kind() {
    let dir = tempfile::tempdir().unwrap();
    let gateway = gateway_with(&dir);
    gateway
        .events
        .emit("task_created", json!({ "id": "task_x" }));
    gateway.events.emit("node_seen", json!({ "node_id": "n" }));
    let response = gateway
        .route(get("/api/v1/events?since=0&kind=task_created"))
        .await;
    let page = body_json(&response);
    let events = page["events"].as_array().unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0]["kind"], "task_created");
}

#[tokio::test]
async fn events_parametres_invalides_refuses_franchement() {
    let dir = tempfile::tempdir().unwrap();
    let gateway = gateway_with(&dir);
    for query in ["since=abc", "limit=abc", "since=-1", "kind=", "inconnu=1"] {
        let response = gateway.route(get(&format!("/api/v1/events?{query}"))).await;
        assert_eq!(response.status, 400, "query acceptée à tort : {query}");
    }
}

#[tokio::test]
async fn creation_de_tache_publie_un_evenement_reel() {
    let dir = tempfile::tempdir().unwrap();
    let tasks_file = dir.path().join("tasks.json");
    let gateway = Gateway::with_state_files(dir.path().join("site"), None, Some(tasks_file))
        .with_local(local_paths(&dir));

    let body = json!({
        "owner": "ui-contrat",
        "kind": "echo",
        "input_hex": "68656c6c6f",
        "deadline_in_seconds": 300
    })
    .to_string();
    let response = gateway.route(post("/api/v1/tasks", &body)).await;
    assert_eq!(response.status, 201);
    let task_id = body_json(&response)["task"]["spec"]["id"]
        .as_str()
        .unwrap()
        .to_string();

    let response = gateway
        .route(get("/api/v1/events?since=0&kind=task_created"))
        .await;
    let page = body_json(&response);
    let events = page["events"].as_array().unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0]["data"]["id"], task_id);
    // L'exécuteur local est attaché : la tâche a réellement été exécutée.
    assert!(matches!(
        events[0]["data"]["state"].as_str(),
        Some("succeeded") | Some("failed")
    ));
}

/// Vrai daemon `polygoned` sur un socket temporaire, ledger isolé.
fn start_daemon(dir: &tempfile::TempDir) -> (PathBuf, Arc<AtomicBool>, thread::JoinHandle<()>) {
    let path = dir.path().join("daemon.sock");
    let listener = polygoned::socket::bind_listener(&path).expect("bind socket temporaire");
    // SAFETY : édition 2024 — `set_var` est unsafe. Ce test est le seul de ce
    // binaire à toucher cette variable, et le serveur la lit au démarrage,
    // avant toute autre connexion : pas de course avec un autre thread ici.
    unsafe {
        std::env::set_var(
            polygoned::socket::ENV_LEDGER_PATH,
            dir.path().join("ledger.json"),
        );
    }
    let state = Arc::new(std::sync::RwLock::new(polygoned::socket::DaemonState::new(
        "balanced",
    )));
    let shutdown = Arc::new(AtomicBool::new(false));
    let serve_shutdown = Arc::clone(&shutdown);
    let handle = thread::spawn(move || {
        polygoned::socket::serve_with_timeout(
            listener,
            state,
            serve_shutdown,
            polygoned::socket::CONNECTION_IDLE_TIMEOUT,
        )
    });
    (path, shutdown, handle)
}

/// Champs variant entre deux lectures ou deux connexions : exclus de la
/// comparaison de parité. `seq`/`uptime_secs`/`timestamp` bougent avec le
/// temps ; `id` est l'écho de corrélation propre à CHAQUE connexion client
/// (le relais et le client de test ont leurs compteurs). Tout le reste,
/// y compris les omissions et l'enveloppe `type`, doit être identique.
fn strip_volatile(value: &mut Value) {
    if let Some(map) = value.as_object_mut() {
        map.remove("seq");
        map.remove("uptime_secs");
        map.remove("timestamp");
        map.remove("id");
    }
}

#[tokio::test]
async fn relais_daemon_parite_complete_sur_un_vrai_daemon() {
    let dir = tempfile::tempdir().unwrap();
    let (socket, shutdown, handle) = start_daemon(&dir);
    let gateway = gateway_with(&dir);

    // --- ressources : le HTTP relaie le status_response TEL QUEL ---
    let mut client = polygoned::client::DaemonClient::connect(&socket).expect("connexion daemon");
    let socket_status = client.status().expect("status socket");
    let mut socket_json = serde_json::to_value(&socket_status).unwrap();
    strip_volatile(&mut socket_json);

    let response = gateway.route(get("/api/v1/resources")).await;
    assert_eq!(response.status, 200);
    let mut http_json = body_json(&response);
    strip_volatile(&mut http_json);
    assert_eq!(
        http_json, socket_json,
        "le relais HTTP doit être identique au socket, omissions comprises"
    );
    assert!(http_json.get("ram_mb").is_some());
    assert!(http_json.get("tier").is_some());

    // --- daemon : le hello est résumé fidèlement ---
    let response = gateway.route(get("/api/v1/daemon")).await;
    assert_eq!(response.status, 200);
    let hello = body_json(&response);
    assert_eq!(hello["daemon_version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(hello["protocol_version"], 2);
    assert!(
        hello["capabilities"]
            .as_array()
            .unwrap()
            .iter()
            .any(|c| c == "ledger")
    );

    // --- usage : une allocation réelle apparaît dans le ledger relayé ---
    let granted = client.allocate(1.0, 1).expect("allocation de test");
    let allocation_id = match &granted {
        polygoned::socket::Reply::AllocGranted { allocation_id, .. } => allocation_id.clone(),
        other => panic!("allocation refusée sur un daemon de test : {other:?}"),
    };
    let socket_ledger = client.ledger().expect("ledger socket");
    let mut socket_ledger_json = serde_json::to_value(&socket_ledger).unwrap();
    strip_volatile(&mut socket_ledger_json);

    let response = gateway.route(get("/api/v1/usage")).await;
    assert_eq!(response.status, 200);
    let mut http_ledger = body_json(&response);
    strip_volatile(&mut http_ledger);
    assert_eq!(
        http_ledger, socket_ledger_json,
        "le relais usage doit être identique au ledger du socket"
    );
    assert!(
        http_ledger["grants"]
            .as_array()
            .unwrap()
            .iter()
            .any(|g| g["allocation_id"] == allocation_id)
    );
    assert!(http_ledger["reserved_ram_bytes"].as_u64().unwrap() > 0);

    // --- libération : le ledger se vide, le relais suit ---
    client.release(&allocation_id).expect("release");
    let response = gateway.route(get("/api/v1/usage")).await;
    let http_ledger = body_json(&response);
    assert!(http_ledger["grants"].as_array().unwrap().is_empty());
    assert_eq!(http_ledger["reserved_ram_bytes"], 0);

    shutdown.store(true, Ordering::SeqCst);
    handle.join().expect("arrêt propre du daemon");
}

#[tokio::test]
async fn daemon_absent_repond_503_sur_les_trois_relais() {
    let dir = tempfile::tempdir().unwrap();
    let gateway = gateway_with(&dir);
    for path in ["/api/v1/resources", "/api/v1/usage", "/api/v1/daemon"] {
        let response = gateway.route(get(path)).await;
        assert_eq!(response.status, 503, "{path} devrait dire daemon absent");
        assert_eq!(body_json(&response)["error"], "daemon_absent");
    }
}

#[tokio::test]
async fn daemon_sans_capacite_ledger_repond_501_pas_un_ledger_vide() {
    let dir = tempfile::tempdir().unwrap();
    let socket = dir.path().join("daemon.sock");
    let listener = std::os::unix::net::UnixListener::bind(&socket).unwrap();
    let handle = thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            use std::io::Write;
            // Daemon v2 SANS la capacité ledger : il ne l'annonce pas.
            let hello = "{\"v\":1,\"type\":\"hello\",\"daemon_version\":\"2.0.0\",\
                         \"platform\":\"test\",\"tier\":\"balanced\",\"protocol_version\":2,\
                         \"capabilities\":[\"refusals\",\"renew\",\"storage_capacity\",\"cpu_tier_cap\"]}\n";
            let _ = stream.write_all(hello.as_bytes());
            // Laisse le client lire puis fermer ; aucun autre échange n'est dû.
            let mut buf = [0u8; 256];
            use std::io::Read;
            let _ = stream.read(&mut buf);
        }
    });
    let gateway = gateway_with(&dir);
    let response = gateway.route(get("/api/v1/usage")).await;
    assert_eq!(response.status, 501);
    assert_eq!(body_json(&response)["error"], "ledger_unsupported");
    handle.join().unwrap();
}
