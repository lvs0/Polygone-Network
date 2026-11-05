//! Routage pur : entrée [`HttpRequest`], sortie [`HttpResponse`].
//!
//! Rien ici n'ouvre de socket, ce qui permet de tester chaque route en
//! mémoire. La couche réseau de `lib.rs` ne fait que lire puis écrire.

use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::http::{HttpRequest, HttpResponse};
use crate::static_files;
use crate::{
    Gateway, MAX_PENDING_PAIRINGS, NodeInfo, PAIR_TOKEN_TTL_SECS, PairAcceptRequest, PairRequest,
    PairToken, now_secs, uuid_short,
};

/// État déclaré d'un service. C'est une affirmation sur le code, pas un
/// objectif : `partial` veut dire qu'une partie manque, et le commentaire de la
/// table dit laquelle.
#[derive(Clone, Copy, Serialize)]
pub struct ServiceStatus {
    pub name: &'static str,
    pub state: &'static str,
    /// Version depuis laquelle cet état est vrai.
    pub since: &'static str,
}

/// Table des services, alignée sur `STAGING.md`.
///
/// - `msg`, `drive`, `hide` : livrés et testés dans le binaire `polygone` ;
/// - `petals` : inférence locale livrée (Ollama), plan de pipeline distribué
///   présent, inférence distribuée **non** branchée ;
/// - `serverless` : crate et CLI présents (quorum + Shamir de transit),
///   dispatch réel sur le relay **non** branché.
pub const SERVICES: &[ServiceStatus] = &[
    ServiceStatus {
        name: "msg",
        state: "live",
        since: "2.0.0",
    },
    ServiceStatus {
        name: "drive",
        state: "live",
        since: "2.0.0",
    },
    ServiceStatus {
        name: "hide",
        state: "live",
        since: "2.0.0",
    },
    ServiceStatus {
        name: "petals",
        state: "partial",
        since: "2.1.0-dev",
    },
    ServiceStatus {
        name: "serverless",
        state: "partial",
        since: "2.1.0-dev",
    },
];

/// Fenêtre anti-rejeu d'un heartbeat : ±5 minutes, comme les sessions.
pub const HEARTBEAT_WINDOW_SECS: u64 = 300;

/// Message canonique signé par le nœud. On signe ce triplet, pas le JSON :
/// une resérialisation différente ne change pas la signature.
pub fn heartbeat_message(node_id: &str, timestamp: u64) -> String {
    format!("polygone-heartbeat-v1\n{node_id}\n{timestamp}")
}

/// Heartbeat poussé par le nœud. La clé publique n'est **pas** dans le corps :
/// elle est celle épinglée à l'appairage, donc un heartbeat ne peut pas
/// substituer une autre identité.
#[derive(Debug, Deserialize)]
pub struct HeartbeatRequest {
    pub node_id: String,
    pub timestamp: u64,
    pub signature_hex: String,
}

fn valid_pair_token(token: &str) -> bool {
    let Some(secret) = token.strip_prefix("poly_") else {
        return false;
    };
    secret.len() == 64 && secret.bytes().all(|byte| byte.is_ascii_hexdigit())
}

impl Gateway {
    /// Route une requête déjà lue. Toute réponse de l'API porte les en-têtes
    /// CORS, y compris les erreurs : un client qui échoue doit pouvoir lire
    /// pourquoi.
    pub async fn route(&self, request: HttpRequest) -> HttpResponse {
        let method = request.method.to_ascii_uppercase();
        if method == "OPTIONS" {
            return HttpResponse::empty(204).cors();
        }
        if method != "GET" && method != "POST" {
            return HttpResponse::text(405, "méthode non autorisée").cors();
        }
        if request.body.len() > crate::MAX_BODY_BYTES {
            return HttpResponse::text(413, "corps trop volumineux").cors();
        }

        let path = request.path.as_str();
        let response = match (method.as_str(), path) {
            ("GET", "/api/v1/network/status") => {
                let status = self.snapshot().await;
                HttpResponse::json(200, &json!(status))
            }
            ("GET", "/api/v1/services") => {
                HttpResponse::json(200, &json!({ "services": SERVICES }))
            }
            ("POST", "/api/v1/pair/request") => self.pair_request(&request.body).await,
            ("POST", "/api/v1/pair/accept") => self.pair_accept(&request.body).await,
            ("POST", "/api/v1/nodes/heartbeat") => self.heartbeat(&request.body).await,
            ("GET", path) if path.starts_with("/api/v1/pair/qr/") => {
                let token = path.trim_start_matches("/api/v1/pair/qr/");
                self.pair_qr(token).await
            }
            ("GET", path) if path.starts_with("/api/v1/node/") => {
                let id = path.trim_start_matches("/api/v1/node/");
                self.node_info(id).await
            }
            // Une route d'API inconnue est une erreur de client, pas un fichier
            // du site : elle ne doit jamais retomber sur le disque.
            ("GET", path) if path.starts_with("/api/") => {
                HttpResponse::json(404, &json!({ "error": "route inconnue" }))
            }
            ("GET", path) => static_files::serve(&self.site_dir, path).await,
            _ => HttpResponse::json(404, &json!({ "error": "not_found" })),
        };

        response.cors()
    }

    async fn pair_request(&self, body: &[u8]) -> HttpResponse {
        let request: PairRequest = match serde_json::from_slice(body) {
            Ok(value) => value,
            Err(_) => return HttpResponse::text(400, "corps JSON invalide"),
        };
        let device_name = request.device_name.trim();
        let kem_public_key_hex = request.kem_public_key_hex.trim();
        let sign_public_key_hex = request.sign_public_key_hex.trim();
        if device_name.is_empty() || kem_public_key_hex.is_empty() || sign_public_key_hex.is_empty()
        {
            return HttpResponse::text(
                400,
                "device_name, kem_public_key_hex et sign_public_key_hex sont requis",
            );
        }
        if device_name.len() > 128 || device_name.chars().any(char::is_control) {
            return HttpResponse::text(400, "device_name invalide");
        }
        // Pin both halves of the actual network identity, not arbitrary hex.
        if polygone_core::crypto::kem::KemPublicKey::from_hex(kem_public_key_hex).is_err() {
            return HttpResponse::text(400, "clé publique ML-KEM-1024 invalide");
        }
        if polygone_core::sign::PublicKey::from_hex(sign_public_key_hex).is_err() {
            return HttpResponse::text(400, "clé publique ML-DSA-65 invalide");
        }
        let now = now_secs();
        let mut pairs = self.pairs.write().await;
        pairs.retain(|_, pair| now.saturating_sub(pair.created_at) <= PAIR_TOKEN_TTL_SECS);
        if pairs.len() >= MAX_PENDING_PAIRINGS {
            return HttpResponse::text(429, "trop de demandes d'appairage en attente");
        }
        let token = loop {
            let token = format!("poly_{}", uuid_short());
            if !pairs.contains_key(&token) {
                break token;
            }
        };
        let pair = PairToken {
            token: token.clone(),
            device_name: device_name.to_string(),
            kem_public_key_hex: kem_public_key_hex.to_string(),
            sign_public_key_hex: sign_public_key_hex.to_string(),
            created_at: now,
        };
        pairs.insert(token.clone(), pair);

        HttpResponse::json(
            200,
            &json!({ "token": token, "expires_in_seconds": PAIR_TOKEN_TTL_SECS }),
        )
    }

    async fn pair_accept(&self, body: &[u8]) -> HttpResponse {
        let request: PairAcceptRequest = match serde_json::from_slice(body) {
            Ok(value) => value,
            Err(_) => return HttpResponse::text(400, "un token d'appairage est requis"),
        };
        if !valid_pair_token(&request.token) {
            return HttpResponse::text(400, "token d'appairage invalide");
        }

        // Remove under the write lock: concurrent accept requests cannot use
        // the same bearer token twice. Unknown and expired tokens are uniform.
        let now = now_secs();
        let pair = self.pairs.write().await.remove(&request.token);
        let Some(pair) =
            pair.filter(|pair| now.saturating_sub(pair.created_at) <= PAIR_TOKEN_TTL_SECS)
        else {
            return HttpResponse::json(
                404,
                &json!({ "error": "token inconnu, expiré ou déjà utilisé" }),
            );
        };

        let node_id: String = pair.kem_public_key_hex.chars().take(16).collect();
        self.nodes.write().await.insert(
            node_id.clone(),
            NodeInfo {
                id: node_id,
                services: vec!["msg".into()],
                device_name: Some(pair.device_name),
                signing_key_hex: Some(pair.sign_public_key_hex),
                last_seen: now,
                latency_ms: None,
            },
        );

        HttpResponse::json(200, &json!({ "ok": true }))
    }

    /// Met à jour `last_seen` d'un nœud **appairé**, après vérification de sa
    /// signature ML-DSA-65 avec la clé épinglée. Un heartbeat inconnu ou mal
    /// signé ne touche ni la table ni le fichier.
    async fn heartbeat(&self, body: &[u8]) -> HttpResponse {
        let request: HeartbeatRequest = match serde_json::from_slice(body) {
            Ok(value) => value,
            Err(_) => return HttpResponse::text(400, "corps JSON invalide"),
        };
        let now = now_secs();
        if now.abs_diff(request.timestamp) > HEARTBEAT_WINDOW_SECS {
            return HttpResponse::text(400, "horodatage hors fenêtre (±300 s)");
        }

        let signing_key_hex = {
            let nodes = self.nodes.read().await;
            match nodes.get(&request.node_id) {
                Some(node) => match node.signing_key_hex.clone() {
                    Some(key) => key,
                    None => {
                        return HttpResponse::text(403, "nœud sans clé de signature épinglée");
                    }
                },
                None => {
                    return HttpResponse::json(
                        404,
                        &json!({ "error": "nœud inconnu — appairage requis" }),
                    );
                }
            }
        };

        let signature_bytes = match hex::decode(request.signature_hex.trim()) {
            Ok(bytes) => bytes,
            Err(_) => return HttpResponse::text(400, "signature_hex invalide"),
        };
        if signature_bytes.len() != polygone_core::sign::SIGNATURE_SIZE {
            return HttpResponse::text(403, "signature ML-DSA-65 invalide");
        }
        let public_key = match polygone_core::sign::PublicKey::from_hex(&signing_key_hex) {
            Ok(key) => key,
            Err(_) => return HttpResponse::text(403, "clé de signature épinglée illisible"),
        };
        let verifier = polygone_core::sign::Verifier::from_public(public_key);
        let signature = polygone_core::sign::Signature::from_bytes(&signature_bytes);
        if !verifier.verify(
            heartbeat_message(&request.node_id, request.timestamp).as_bytes(),
            &signature,
        ) {
            return HttpResponse::text(403, "signature ML-DSA-65 invalide");
        }

        let snapshot = {
            let mut nodes = self.nodes.write().await;
            let Some(node) = nodes.get_mut(&request.node_id) else {
                return HttpResponse::json(
                    404,
                    &json!({ "error": "nœud inconnu — appairage requis" }),
                );
            };
            node.last_seen = now;
            nodes.clone()
        };
        self.persist_nodes(&snapshot).await;

        HttpResponse::json(
            200,
            &json!({ "ok": true, "node_id": request.node_id, "last_seen": now }),
        )
    }

    async fn pair_qr(&self, token: &str) -> HttpResponse {
        if !valid_pair_token(token) {
            return HttpResponse::json(404, &json!({ "error": "token inconnu" }));
        }
        let now = now_secs();
        let mut pairs = self.pairs.write().await;
        let Some(pair) = pairs.get(token) else {
            return HttpResponse::json(404, &json!({ "error": "token inconnu" }));
        };
        if now.saturating_sub(pair.created_at) > PAIR_TOKEN_TTL_SECS {
            pairs.remove(token);
            return HttpResponse::json(404, &json!({ "error": "token expiré" }));
        }
        let deep_link = format!("polygone://pair/{}", pair.token);
        HttpResponse::json(200, &json!({ "deep_link": deep_link }))
    }

    async fn node_info(&self, id: &str) -> HttpResponse {
        let nodes = self.nodes.read().await;
        match nodes.get(id) {
            Some(node) => HttpResponse::json(200, &json!(node)),
            None => HttpResponse::json(404, &json!({ "error": "nœud inconnu" })),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn public_keys() -> (String, String) {
        let (kem_public, _) = polygone_core::crypto::kem::generate_keypair().unwrap();
        let sign_public = polygone_core::sign::generate_keypair()
            .unwrap()
            .verifier
            .public_key()
            .to_hex();
        (kem_public.to_hex(), sign_public)
    }

    fn gateway() -> Gateway {
        Gateway::new(PathBuf::from("."))
    }

    fn get(path: &str) -> HttpRequest {
        HttpRequest {
            method: "GET".into(),
            path: path.into(),
            body: Vec::new(),
        }
    }

    fn post(path: &str, body: &str) -> HttpRequest {
        HttpRequest {
            method: "POST".into(),
            path: path.into(),
            body: body.as_bytes().to_vec(),
        }
    }

    fn body_json(response: &HttpResponse) -> serde_json::Value {
        serde_json::from_slice(&response.body).unwrap()
    }

    #[tokio::test]
    async fn options_is_204_with_cors() {
        let response = gateway()
            .route(HttpRequest {
                method: "OPTIONS".into(),
                path: "/api/v1/services".into(),
                body: Vec::new(),
            })
            .await;

        assert_eq!(response.status, 204);
        assert!(
            response
                .extra_headers
                .iter()
                .any(|(name, value)| name == "Access-Control-Allow-Origin" && value == "*")
        );
    }

    fn temp_nodes_path(label: &str) -> (std::path::PathBuf, std::path::PathBuf) {
        use std::time::{SystemTime, UNIX_EPOCH};
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "polygone-gateway-heartbeat-{label}-{}-{nonce}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("nodes.json");
        (dir, path)
    }

    async fn seed_node(gateway: &Gateway, id: &str, signing_key_hex: &str) {
        gateway.nodes.write().await.insert(
            id.to_string(),
            NodeInfo {
                id: id.to_string(),
                services: vec![],
                device_name: Some("heartbeat-test".into()),
                signing_key_hex: Some(signing_key_hex.to_string()),
                last_seen: 1,
                latency_ms: None,
            },
        );
    }

    fn heartbeat_body(keys: &polygone_core::sign::KeyPair, id: &str, timestamp: u64) -> String {
        let signature = keys
            .signer
            .sign(heartbeat_message(id, timestamp).as_bytes());
        json!({
            "node_id": id,
            "timestamp": timestamp,
            "signature_hex": hex::encode(signature.as_bytes()),
        })
        .to_string()
    }

    #[tokio::test]
    async fn heartbeat_survives_process_restart() {
        let (dir, path) = temp_nodes_path("restart");
        let keys = polygone_core::sign::generate_keypair().unwrap();
        let node_id = "0123456789abcdef";
        let sign_public = keys.verifier.public_key().to_hex();

        let first = Gateway::with_nodes_file(PathBuf::from("."), Some(path.clone()));
        seed_node(&first, node_id, &sign_public).await;
        let stamp = now_secs();
        let response = first
            .route(post(
                "/api/v1/nodes/heartbeat",
                &heartbeat_body(&keys, node_id, stamp),
            ))
            .await;
        assert_eq!(response.status, 200, "heartbeat valide accepté");
        assert!(path.is_file(), "le heartbeat doit être écrit sur disque");
        drop(first);

        // Nouveau process : le nœud est rechargé depuis le fichier, pas d'une
        // table mémoire qui aurait disparu.
        let second = Gateway::with_nodes_file(PathBuf::from("."), Some(path.clone()));
        let nodes = second.nodes.read().await;
        let node = nodes.get(node_id).expect("nœud rechargé au démarrage");
        assert_eq!(node.last_seen, stamp);
        assert_eq!(node.signing_key_hex.as_deref(), Some(sign_public.as_str()));
        drop(nodes);
        std::fs::remove_dir_all(dir).ok();
    }

    #[tokio::test]
    async fn heartbeat_requires_valid_signature() {
        let (dir, path) = temp_nodes_path("signature");
        let keys = polygone_core::sign::generate_keypair().unwrap();
        let attacker = polygone_core::sign::generate_keypair().unwrap();
        let node_id = "fedcba9876543210";

        let gateway = Gateway::with_nodes_file(PathBuf::from("."), Some(path.clone()));
        seed_node(&gateway, node_id, &keys.verifier.public_key().to_hex()).await;

        let response = gateway
            .route(post(
                "/api/v1/nodes/heartbeat",
                &heartbeat_body(&attacker, node_id, now_secs()),
            ))
            .await;
        assert_eq!(response.status, 403);
        let nodes = gateway.nodes.read().await;
        assert_eq!(nodes[node_id].last_seen, 1, "last_seen ne doit pas bouger");
        drop(nodes);
        assert!(!path.exists(), "un refus ne doit rien persister");

        // Un heartbeat d'un nœud jamais appairé est inconnu, pas « accepté ».
        let response = gateway
            .route(post(
                "/api/v1/nodes/heartbeat",
                &heartbeat_body(&attacker, "0000000000000000", now_secs()),
            ))
            .await;
        assert_eq!(response.status, 404);
        std::fs::remove_dir_all(dir).ok();
    }

    #[tokio::test]
    async fn heartbeat_outside_replay_window_is_refused() {
        let (dir, path) = temp_nodes_path("replay");
        let keys = polygone_core::sign::generate_keypair().unwrap();
        let node_id = "0011223344556677";
        let gateway = Gateway::with_nodes_file(PathBuf::from("."), Some(path.clone()));
        seed_node(&gateway, node_id, &keys.verifier.public_key().to_hex()).await;

        let stale = now_secs().saturating_sub(HEARTBEAT_WINDOW_SECS + 1);
        let response = gateway
            .route(post(
                "/api/v1/nodes/heartbeat",
                &heartbeat_body(&keys, node_id, stale),
            ))
            .await;
        assert_eq!(response.status, 400);
        assert_eq!(gateway.nodes.read().await[node_id].last_seen, 1);
        std::fs::remove_dir_all(dir).ok();
    }

    #[tokio::test]
    async fn unsupported_method_is_405() {
        let response = gateway()
            .route(HttpRequest {
                method: "PUT".into(),
                path: "/api/v1/services".into(),
                body: Vec::new(),
            })
            .await;

        assert_eq!(response.status, 405);
    }

    #[tokio::test]
    async fn unknown_api_route_is_404_json() {
        let response = gateway().route(get("/api/v1/nope")).await;
        assert_eq!(response.status, 404);
        assert_eq!(response.content_type, "application/json");
    }

    #[tokio::test]
    async fn services_table_is_honest() {
        let response = gateway().route(get("/api/v1/services")).await;
        assert_eq!(response.status, 200);

        let services = body_json(&response)["services"].clone();
        let services = services.as_array().unwrap();
        assert_eq!(services.len(), 5);

        let by_name = |name: &str| {
            services
                .iter()
                .find(|service| service["name"] == name)
                .cloned()
                .unwrap()
        };
        assert_eq!(by_name("msg")["state"], "live");
        assert_eq!(by_name("hide")["state"], "live");
        assert_eq!(by_name("petals")["state"], "partial");
        assert_eq!(by_name("serverless")["state"], "partial");
    }

    #[tokio::test]
    async fn pair_flow_registers_a_fresh_node() {
        let gateway = gateway();
        let (kem_public_key_hex, sign_public_key_hex) = public_keys();
        let expected_node_id: String = kem_public_key_hex.chars().take(16).collect();
        let payload = json!({
            "device_name":"tv",
            "kem_public_key_hex": kem_public_key_hex,
            "sign_public_key_hex": sign_public_key_hex
        })
        .to_string();

        let response = gateway.route(post("/api/v1/pair/request", &payload)).await;
        assert_eq!(response.status, 200);
        let token = body_json(&response)["token"].as_str().unwrap().to_string();

        let response = gateway
            .route(get(&format!("/api/v1/pair/qr/{token}")))
            .await;
        assert_eq!(response.status, 200);
        assert_eq!(
            body_json(&response)["deep_link"],
            format!("polygone://pair/{token}")
        );

        let accept_payload = json!({ "token": token }).to_string();
        let response = gateway
            .route(post("/api/v1/pair/accept", &accept_payload))
            .await;
        assert_eq!(response.status, 200);

        // The bearer token is consumed and cannot be replayed.
        let response = gateway
            .route(post("/api/v1/pair/accept", &accept_payload))
            .await;
        assert_eq!(response.status, 404);
        let response = gateway
            .route(get(&format!("/api/v1/pair/qr/{token}")))
            .await;
        assert_eq!(response.status, 404);

        let status = body_json(&gateway.route(get("/api/v1/network/status")).await);
        assert_eq!(status["nodes"], 1);
        let nodes = gateway.nodes.read().await;
        assert!(nodes.contains_key(&expected_node_id));
        assert!(nodes[&expected_node_id].signing_key_hex.is_some());
        assert_eq!(nodes[&expected_node_id].device_name.as_deref(), Some("tv"));
        assert_eq!(status["active"], 1);
        // Aucune latence mesurée pour ce nœud : la médiane est `null`, pas 0.
        assert!(status["latency_ms_median"].is_null());
    }

    #[tokio::test]
    async fn pair_accept_requires_an_existing_unexpired_token() {
        let gateway = gateway();
        let response = gateway
            .route(post("/api/v1/pair/accept", r#"{"token":"poly_inconnu"}"#))
            .await;
        assert_eq!(response.status, 400);

        let response = gateway
            .route(post("/api/v1/pair/accept", r#"{"token":"poly_0000000000000000000000000000000000000000000000000000000000000000"}"#))
            .await;
        assert_eq!(response.status, 404);
        assert_eq!(
            body_json(&gateway.route(get("/api/v1/network/status")).await)["nodes"],
            0
        );
    }

    #[tokio::test]
    async fn expired_pair_token_is_removed_and_cannot_be_accepted() {
        let gateway = gateway();
        let token = format!("poly_{}", "a".repeat(64));
        gateway.pairs.write().await.insert(
            token.clone(),
            PairToken {
                token: token.clone(),
                device_name: "old-device".into(),
                kem_public_key_hex: public_keys().0,
                sign_public_key_hex: public_keys().1,
                created_at: now_secs().saturating_sub(PAIR_TOKEN_TTL_SECS + 1),
            },
        );
        let response = gateway
            .route(get(&format!("/api/v1/pair/qr/{token}")))
            .await;
        assert_eq!(response.status, 404);
        assert!(!gateway.pairs.read().await.contains_key(&token));
    }

    #[tokio::test]
    async fn concurrent_pair_acceptance_consumes_a_bearer_token_once() {
        let gateway = gateway();
        let (kem_public_key_hex, sign_public_key_hex) = public_keys();
        let request = json!({
            "device_name":"phone",
            "kem_public_key_hex": kem_public_key_hex,
            "sign_public_key_hex": sign_public_key_hex
        })
        .to_string();
        let created = gateway.route(post("/api/v1/pair/request", &request)).await;
        let token = body_json(&created)["token"].as_str().unwrap().to_string();
        let body = format!("{{\"token\":\"{token}\"}}");
        let accept = |gateway: Gateway, body: String| async move {
            gateway
                .route(post("/api/v1/pair/accept", &body))
                .await
                .status
        };
        let (first, second) = tokio::join!(
            accept(gateway.clone(), body.clone()),
            accept(gateway.clone(), body.clone()),
        );
        let statuses = [first, second];
        assert_eq!(statuses.iter().filter(|status| **status == 200).count(), 1);
        assert_eq!(statuses.iter().filter(|status| **status == 404).count(), 1);
        assert_eq!(gateway.nodes.read().await.len(), 1);
        let node = gateway.nodes.read().await.values().next().unwrap().clone();
        assert_eq!(node.id.len(), 16);
        assert!(node.signing_key_hex.is_some());
    }

    #[tokio::test]
    async fn unknown_pair_token_is_404() {
        let response = gateway().route(get("/api/v1/pair/qr/poly_inconnu")).await;
        assert_eq!(response.status, 404);
    }

    #[tokio::test]
    async fn invalid_json_is_400() {
        let response = gateway()
            .route(post("/api/v1/pair/request", "{pas du json"))
            .await;
        assert_eq!(response.status, 400);
    }

    #[tokio::test]
    async fn malformed_kem_or_signing_public_key_is_refused() {
        let (_kem, sign) = public_keys();
        let bad_kem = json!({
            "device_name": "tv",
            "kem_public_key_hex": "ab12",
            "sign_public_key_hex": sign
        })
        .to_string();
        let response = gateway()
            .route(post("/api/v1/pair/request", &bad_kem))
            .await;
        assert_eq!(response.status, 400);

        let (kem, _sign) = public_keys();
        let bad_sign = json!({
            "device_name": "tv",
            "kem_public_key_hex": kem,
            "sign_public_key_hex": "ab12"
        })
        .to_string();
        let response = gateway()
            .route(post("/api/v1/pair/request", &bad_sign))
            .await;
        assert_eq!(response.status, 400);
    }

    #[tokio::test]
    async fn empty_key_is_refused() {
        let payload =
            r#"{"device_name":"tv","kem_public_key_hex":"  ","sign_public_key_hex":"  "}"#;
        let response = gateway().route(post("/api/v1/pair/request", payload)).await;
        assert_eq!(response.status, 400);
    }

    #[tokio::test]
    async fn unknown_node_is_404() {
        let response = gateway().route(get("/api/v1/node/inexistant")).await;
        assert_eq!(response.status, 404);
    }

    #[tokio::test]
    async fn uptime_is_measured_not_decorative() {
        let gateway = gateway();
        let status = body_json(&gateway.route(get("/api/v1/network/status")).await);
        let uptime = status["uptime_seconds"].as_u64().unwrap();
        assert!(uptime < 60, "uptime inattendu : {uptime}");
    }
}
