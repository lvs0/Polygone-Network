//! Preuve de bout en bout du gateway sur un vrai socket TCP.
//!
//! Ces tests utilisent uniquement std::net::TcpStream côté client : aucune
//! dépendance de test supplémentaire et aucune route simulée en mémoire.

use std::{
    io::{Read, Write},
    net::{Shutdown, SocketAddr, TcpStream},
    path::PathBuf,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use polygone_gateway::Gateway;

fn send_raw_http(address: SocketAddr, method: &str, target: &str, body: &[u8]) -> String {
    let mut stream =
        TcpStream::connect_timeout(&address, Duration::from_secs(2)).expect("connexion TCP");
    stream
        .set_read_timeout(Some(Duration::from_secs(3)))
        .expect("configurer le timeout de lecture");

    let headers = format!(
        "{method} {target} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\nContent-Length: {}\r\nContent-Type: application/json\r\n\r\n",
        body.len()
    );
    stream
        .write_all(headers.as_bytes())
        .expect("écrire les en-têtes");
    stream.write_all(body).expect("écrire le corps");

    let mut response = Vec::new();
    stream
        .read_to_end(&mut response)
        .expect("lire la réponse complète");
    String::from_utf8_lossy(&response).into_owned()
}

fn send_raw_bytes(address: SocketAddr, request: &[u8]) -> String {
    let mut stream =
        TcpStream::connect_timeout(&address, Duration::from_secs(2)).expect("connexion TCP");
    stream
        .set_read_timeout(Some(Duration::from_secs(3)))
        .expect("configurer le timeout de lecture");
    stream.write_all(request).expect("écrire la requête brute");
    stream
        .shutdown(Shutdown::Write)
        .expect("fermer le côté écriture");
    let mut response = Vec::new();
    stream
        .read_to_end(&mut response)
        .expect("lire la réponse complète");
    String::from_utf8_lossy(&response).into_owned()
}

fn status_code(response: &str) -> u16 {
    response
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|value| value.parse().ok())
        .unwrap_or_else(|| panic!("ligne de statut HTTP invalide : {response:?}"))
}

fn response_body(response: &str) -> &str {
    response
        .split_once("\r\n\r\n")
        .map(|(_, body)| body)
        .unwrap_or_else(|| panic!("réponse HTTP sans séparateur d'en-têtes : {response:?}"))
}

async fn start_gateway() -> (
    SocketAddr,
    tokio::task::JoinHandle<anyhow::Result<()>>,
    PathBuf,
) {
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
        .await
        .expect("bind sur un port éphémère");
    let address = listener.local_addr().expect("lire le port lié");
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("horloge après epoch")
        .as_nanos();
    let site_dir = std::env::temp_dir().join(format!(
        "polygone-gateway-serve-{}-{unique}",
        std::process::id()
    ));
    std::fs::create_dir_all(&site_dir).expect("créer le site de test");
    std::fs::write(site_dir.join("index.html"), b"gateway test").expect("écrire l'index de test");

    let server_site = site_dir.clone();
    let server = tokio::spawn(async move { Gateway::serve(listener, server_site).await });

    (address, server, site_dir)
}

fn stop_gateway(server: tokio::task::JoinHandle<anyhow::Result<()>>, site_dir: &PathBuf) {
    server.abort();
    let _ = std::fs::remove_dir_all(site_dir);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn gateway_answers_real_http_requests_and_rejects_path_traversal() {
    let (address, server, site_dir) = start_gateway().await;

    let network = send_raw_http(address, "GET", "/api/v1/network/status", &[]);
    assert_eq!(status_code(&network), 200, "{network}");
    let network_json: serde_json::Value =
        serde_json::from_str(response_body(&network)).expect("statut réseau JSON");
    assert!(network_json["nodes"].is_number());
    assert!(network_json["active"].is_number());
    assert!(network_json["uptime_seconds"].is_number());

    let services = send_raw_http(address, "GET", "/api/v1/services", &[]);
    assert_eq!(status_code(&services), 200, "{services}");
    let services_json: serde_json::Value =
        serde_json::from_str(response_body(&services)).expect("services JSON");
    let services = services_json["services"]
        .as_array()
        .expect("tableau services");
    assert!(!services.is_empty());
    for service in services {
        assert!(service["name"].is_string(), "champ name absent : {service}");
        assert!(
            service["state"].is_string(),
            "champ state absent : {service}"
        );
        assert!(
            service["since"].is_string(),
            "champ since absent : {service}"
        );
    }

    let unknown = send_raw_http(address, "GET", "/api/v1/inconnu", &[]);
    assert_eq!(status_code(&unknown), 404, "{unknown}");

    for path in ["/../Cargo.toml", "/%2e%2e/Cargo.toml"] {
        let traversal = send_raw_http(address, "GET", path, &[]);
        assert_ne!(
            status_code(&traversal),
            200,
            "traversée acceptée : {traversal}"
        );
        assert!(
            !response_body(&traversal).contains("polygone-gateway"),
            "Cargo.toml a été exposé : {traversal}"
        );
    }

    let (kem_public_key, _) = polygone_core::crypto::kem::generate_keypair().unwrap();
    let sign_public_key = polygone_core::sign::generate_keypair()
        .unwrap()
        .verifier
        .public_key()
        .to_hex();
    let valid_pair = serde_json::json!({
        "device_name": "integration-test",
        "kem_public_key_hex": kem_public_key.to_hex(),
        "sign_public_key_hex": sign_public_key
    });
    let pair = send_raw_http(
        address,
        "POST",
        "/api/v1/pair/request",
        valid_pair.to_string().as_bytes(),
    );
    assert_eq!(status_code(&pair), 200, "{pair}");
    let pair_json: serde_json::Value =
        serde_json::from_str(response_body(&pair)).expect("appairage JSON");
    let token = pair_json["token"].as_str().expect("jeton 256 bits");
    assert_eq!(token.len(), 69); // "poly_" + 64 hex characters

    let invalid_pair = send_raw_http(address, "POST", "/api/v1/pair/request", b"{not-json");
    assert_eq!(status_code(&invalid_pair), 400, "{invalid_pair}");

    let unsupported = send_raw_http(address, "DELETE", "/api/v1/services", &[]);
    assert_eq!(status_code(&unsupported), 405, "{unsupported}");

    stop_gateway(server, &site_dir);
}

/// GW.1 par le vrai socket : appairage HTTP, heartbeat signé, `last_seen`
/// visible dans `/status`, fichier `nodes.json` écrit, puis rechargé par une
/// nouvelle instance du gateway (le redémarrage du process, ici, c'est la
/// reconstruction complète de l'état depuis le disque).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn heartbeat_round_trip_over_real_http_is_persisted() {
    use polygone_gateway::routes::{heartbeat_message, ping_message, pong_message};

    let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
        .await
        .expect("bind éphémère");
    let address = listener.local_addr().expect("port lié");
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("horloge")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "polygone-gateway-heartbeat-http-{}-{unique}",
        std::process::id()
    ));
    let site_dir = root.join("site");
    std::fs::create_dir_all(&site_dir).expect("site de test");
    std::fs::write(site_dir.join("index.html"), b"gateway test").expect("index");
    let nodes_file = root.join("nodes.json");

    let server_site = site_dir.clone();
    let server_nodes = nodes_file.clone();
    let server = tokio::spawn(async move {
        Gateway::serve_with_nodes(listener, server_site, Some(server_nodes)).await
    });

    let (kem_public, _) = polygone_core::crypto::kem::generate_keypair().unwrap();
    let keys = polygone_core::sign::generate_keypair().unwrap();
    let kem_hex = kem_public.to_hex();
    let node_id =
        polygone_core::sign::node_id_from_sign_pk_hex(&keys.verifier.public_key().to_hex())
            .expect("valid signing key");
    let request = serde_json::json!({
        "device_name": "heartbeat-http",
        "kem_public_key_hex": kem_hex,
        "sign_public_key_hex": keys.verifier.public_key().to_hex(),
    });
    let pair = send_raw_http(
        address,
        "POST",
        "/api/v1/pair/request",
        request.to_string().as_bytes(),
    );
    assert_eq!(status_code(&pair), 200, "{pair}");
    let token = serde_json::from_str::<serde_json::Value>(response_body(&pair))
        .expect("JSON appairage")["token"]
        .as_str()
        .expect("jeton")
        .to_string();
    let accept = send_raw_http(
        address,
        "POST",
        "/api/v1/pair/accept",
        serde_json::json!({ "token": token }).to_string().as_bytes(),
    );
    assert_eq!(status_code(&accept), 200, "{accept}");

    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let signature = keys
        .signer
        .sign(heartbeat_message(&node_id, stamp).as_bytes());
    let heartbeat = serde_json::json!({
        "node_id": node_id,
        "timestamp": stamp,
        "signature_hex": hex::encode(signature.as_bytes()),
    });
    let response = send_raw_http(
        address,
        "POST",
        "/api/v1/nodes/heartbeat",
        heartbeat.to_string().as_bytes(),
    );
    assert_eq!(status_code(&response), 200, "{response}");

    // Le heartbeat est horodaté par la gateway, pas par le client. Comparer
    // `last_seen` au `stamp` envoyé ne passe que si les deux tombent dans la
    // même seconde : c'est un test qui échoue au hasard d'une frontière de
    // seconde, pas un défaut du produit. On relit la valeur réellement
    // persistée depuis la réponse.
    let heartbeat_last_seen = serde_json::from_str::<serde_json::Value>(response_body(&response))
        .expect("JSON heartbeat")["last_seen"]
        .as_u64()
        .expect("last_seen numérique");
    assert!(
        heartbeat_last_seen >= stamp,
        "la gateway ne peut pas dater un heartbeat avant le client"
    );

    // Mesure réelle : ping signé → challenge, pong signé → RTT enregistré.
    let ping_signature = keys.signer.sign(ping_message(&node_id, stamp).as_bytes());
    let ping = send_raw_http(
        address,
        "POST",
        "/api/v1/nodes/ping",
        serde_json::json!({
            "node_id": node_id,
            "timestamp": stamp,
            "signature_hex": hex::encode(ping_signature.as_bytes()),
        })
        .to_string()
        .as_bytes(),
    );
    assert_eq!(status_code(&ping), 200, "{ping}");
    let challenge = serde_json::from_str::<serde_json::Value>(response_body(&ping))
        .expect("JSON ping")["challenge"]
        .as_str()
        .expect("challenge")
        .to_string();
    let pong_signature = keys
        .signer
        .sign(pong_message(&node_id, stamp, &challenge, 42).as_bytes());
    let pong = send_raw_http(
        address,
        "POST",
        "/api/v1/nodes/pong",
        serde_json::json!({
            "node_id": node_id,
            "timestamp": stamp,
            "challenge": challenge,
            "rtt_ms": 42,
            "signature_hex": hex::encode(pong_signature.as_bytes()),
        })
        .to_string()
        .as_bytes(),
    );
    assert_eq!(status_code(&pong), 200, "{pong}");

    let network = send_raw_http(address, "GET", "/api/v1/network/status", &[]);
    let status: serde_json::Value =
        serde_json::from_str(response_body(&network)).expect("statut JSON");
    assert_eq!(status["nodes"], 1);
    assert_eq!(
        status["active"], 1,
        "le heartbeat doit rendre le nœud actif"
    );
    assert_eq!(
        status["latency_ms_median"], 42,
        "la mesure doit apparaître dans le statut réseau"
    );

    // Le ping/pong qui suit le heartbeat avance légitimement `last_seen`. On
    // compare donc le disque au **dernier** état servi, pas au premier.
    let node_path = format!("/api/v1/node/{node_id}");
    let node_info = send_raw_http(address, "GET", &node_path, &[]);
    assert_eq!(status_code(&node_info), 200, "{node_info}");
    let node_last_seen = serde_json::from_str::<serde_json::Value>(response_body(&node_info))
        .expect("JSON nœud")["last_seen"]
        .as_u64()
        .expect("last_seen numérique");
    assert!(node_last_seen >= heartbeat_last_seen);

    server.abort();
    let _ = server.await;

    assert!(
        nodes_file.is_file(),
        "nodes.json doit exister après heartbeat"
    );
    let reloaded = Gateway::with_nodes_file(site_dir.clone(), Some(nodes_file.clone()));
    let nodes = reloaded.nodes.read().await;
    let node = nodes.get(&node_id).expect("nœud rechargé depuis le disque");
    assert_eq!(
        node.last_seen, node_last_seen,
        "l'état persisté doit être le dernier état servi"
    );
    drop(nodes);
    std::fs::remove_dir_all(root).ok();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ambiguous_http_framing_is_rejected_before_routing() {
    let (address, server, site_dir) = start_gateway().await;
    let requests: &[&[u8]] = &[
        b"POST /api/v1/pair/request HTTP/1.1\r\nHost: localhost\r\nContent-Length: 0\r\nContent-Length: 0\r\n\r\n",
        b"POST /api/v1/pair/request HTTP/1.1\r\nHost: localhost\r\nTransfer-Encoding: chunked\r\nContent-Length: 0\r\n\r\n",
        b"POST /api/v1/pair/request HTTP/1.1\r\nHost: localhost\r\nContent-Length: nope\r\n\r\n",
        b"POST /api/v1/pair/request HTTP/1.1\r\nContent-Length: 0\r\n\r\n",
        b"GET http://example.invalid/ HTTP/1.1\r\nHost: localhost\r\n\r\n",
    ];
    for request in requests {
        let response = send_raw_bytes(address, request);
        assert_eq!(
            status_code(&response),
            400,
            "request={request:?} response={response}"
        );
    }
    stop_gateway(server, &site_dir);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn incomplete_body_is_rejected_and_oversized_headers_return_431() {
    let (address, server, site_dir) = start_gateway().await;
    let incomplete = send_raw_bytes(
        address,
        b"POST /api/v1/pair/request HTTP/1.1\r\nHost: localhost\r\nContent-Length: 4\r\n\r\na",
    );
    assert_eq!(status_code(&incomplete), 400, "{incomplete}");

    let mut oversized = b"GET / HTTP/1.1\r\nHost: localhost\r\nX-Fill: ".to_vec();
    oversized.extend(std::iter::repeat_n(b'a', 17 * 1024));
    oversized.extend_from_slice(b"\r\n\r\n");
    let response = send_raw_bytes(address, &oversized);
    assert_eq!(status_code(&response), 431, "{response}");
    stop_gateway(server, &site_dir);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn declared_body_over_one_mib_returns_413_without_reading_it() {
    let (address, server, site_dir) = start_gateway().await;
    let mut stream =
        TcpStream::connect_timeout(&address, Duration::from_secs(2)).expect("connexion TCP");
    stream
        .set_read_timeout(Some(Duration::from_secs(3)))
        .expect("configurer le timeout de lecture");
    let headers = format!(
        "POST /api/v1/pair/request HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\nContent-Length: {}\r\nContent-Type: application/json\r\n\r\n",
        1024 * 1024 + 1
    );
    stream
        .write_all(headers.as_bytes())
        .expect("écrire les en-têtes");
    // Le serveur doit pouvoir refuser sur Content-Length, sans attendre le corps.
    stream
        .shutdown(Shutdown::Write)
        .expect("fermer le côté écriture");
    let mut response = Vec::new();
    stream.read_to_end(&mut response).expect("lire le 413");
    let response = String::from_utf8_lossy(&response).into_owned();
    assert_eq!(status_code(&response), 413, "{response}");
    stop_gateway(server, &site_dir);
}
