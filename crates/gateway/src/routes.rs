//! Routage pur : entrée [`HttpRequest`], sortie [`HttpResponse`].
//!
//! Rien ici n'ouvre de socket, ce qui permet de tester chaque route en
//! mémoire. La couche réseau de `lib.rs` ne fait que lire puis écrire.

use polygone_core::task::{TaskId, TaskState};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::http::{HttpRequest, HttpResponse};
use crate::static_files;
use crate::tasks::{CreateTaskRequest, DEFAULT_TASK_LIST_LIMIT, MAX_TASK_LIST_LIMIT, StoreError};
use crate::{
    Gateway, MAX_PENDING_PAIRINGS, NodeInfo, PAIR_TOKEN_TTL_SECS, PairAcceptRequest, PairRequest,
    PairToken, PendingPing, now_secs, uuid_short,
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

/// Durée de vie d'un challenge de ping : au-delà, la mesure est périmée.
pub const PING_TTL_SECS: u64 = 30;
/// Plafond de RTT accepté : au-delà, la mesure n'est pas crédible pour un pair.
pub const MAX_RTT_MS: u64 = 60_000;

/// Message signé pour demander un challenge de mesure.
pub fn ping_message(node_id: &str, timestamp: u64) -> String {
    format!("polygone-ping-v1\n{node_id}\n{timestamp}")
}

/// Message signé qui rapporte un RTT mesuré. Le challenge et la valeur sont
/// dans la signature : un pong ne peut pas être rejoué pour une autre mesure.
pub fn pong_message(node_id: &str, timestamp: u64, challenge: &str, rtt_ms: u64) -> String {
    format!("polygone-pong-v1\n{node_id}\n{timestamp}\n{challenge}\n{rtt_ms}")
}

#[derive(Debug, Deserialize)]
pub struct PongRequest {
    pub node_id: String,
    pub timestamp: u64,
    pub challenge: String,
    pub rtt_ms: u64,
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
            ("POST", "/api/v1/nodes/ping") => self.ping(&request.body).await,
            ("POST", "/api/v1/nodes/pong") => self.pong(&request.body).await,
            ("POST", "/api/v1/tasks") => self.create_task(&request.body).await,
            ("GET", "/api/v1/tasks") => self.list_tasks(&request.query).await,
            ("POST", path) if path.starts_with("/api/v1/tasks/") && path.ends_with("/cancel") => {
                self.cancel_task(path).await
            }
            ("GET", path) if path.starts_with("/api/v1/tasks/") => self.get_task(path).await,
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

        // The node id is derived from the signing key, not the KEM key: it is
        // the key that signs this node's envelopes.
        let node_id = match polygone_core::sign::node_id_from_sign_pk_hex(&pair.sign_public_key_hex)
        {
            Some(node_id) => node_id,
            None => return HttpResponse::text(400, "clé publique ML-DSA-65 invalide"),
        };
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

    /// Vérifie une signature de nœud avec la clé **épinglée à l'appairage**.
    /// Le corps de la requête ne peut donc jamais choisir sa propre clé.
    async fn authenticate(
        &self,
        node_id: &str,
        message: &str,
        signature_hex: &str,
    ) -> Result<(), HttpResponse> {
        let signing_key_hex = {
            let nodes = self.nodes.read().await;
            match nodes.get(node_id) {
                Some(node) => match node.signing_key_hex.as_deref() {
                    Some(key) => key.to_string(),
                    None => {
                        return Err(HttpResponse::text(
                            403,
                            "nœud sans clé de signature épinglée",
                        ));
                    }
                },
                None => {
                    return Err(HttpResponse::json(
                        404,
                        &json!({ "error": "nœud inconnu — appairage requis" }),
                    ));
                }
            }
        };

        let signature_bytes = match hex::decode(signature_hex.trim()) {
            Ok(bytes) => bytes,
            Err(_) => return Err(HttpResponse::text(400, "signature_hex invalide")),
        };
        if signature_bytes.len() != polygone_core::sign::SIGNATURE_SIZE {
            return Err(HttpResponse::text(403, "signature ML-DSA-65 invalide"));
        }
        let public_key = match polygone_core::sign::PublicKey::from_hex(&signing_key_hex) {
            Ok(key) => key,
            Err(_) => {
                return Err(HttpResponse::text(
                    403,
                    "clé de signature épinglée illisible",
                ));
            }
        };
        let verifier = polygone_core::sign::Verifier::from_public(public_key);
        let signature = polygone_core::sign::Signature::from_bytes(&signature_bytes);
        if !verifier.verify(message.as_bytes(), &signature) {
            return Err(HttpResponse::text(403, "signature ML-DSA-65 invalide"));
        }
        Ok(())
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
        if let Err(response) = self
            .authenticate(
                &request.node_id,
                &heartbeat_message(&request.node_id, request.timestamp),
                &request.signature_hex,
            )
            .await
        {
            return response;
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

    /// Demande un challenge de mesure. Le nœud doit être appairé et signer sa
    /// demande : un inconnu ne peut pas ouvrir de mesure.
    async fn ping(&self, body: &[u8]) -> HttpResponse {
        let request: HeartbeatRequest = match serde_json::from_slice(body) {
            Ok(value) => value,
            Err(_) => return HttpResponse::text(400, "corps JSON invalide"),
        };
        let now = now_secs();
        if now.abs_diff(request.timestamp) > HEARTBEAT_WINDOW_SECS {
            return HttpResponse::text(400, "horodatage hors fenêtre (±300 s)");
        }
        if let Err(response) = self
            .authenticate(
                &request.node_id,
                &ping_message(&request.node_id, request.timestamp),
                &request.signature_hex,
            )
            .await
        {
            return response;
        }

        let challenge = uuid_short();
        self.pings.write().await.insert(
            request.node_id.clone(),
            PendingPing {
                challenge: challenge.clone(),
                issued_at: now,
            },
        );

        // Un ping est une preuve de vie, mais il ne crée aucune mesure.
        let snapshot = {
            let mut nodes = self.nodes.write().await;
            if let Some(node) = nodes.get_mut(&request.node_id) {
                node.last_seen = now;
            }
            nodes.clone()
        };
        self.persist_nodes(&snapshot).await;

        HttpResponse::json(
            200,
            &json!({ "ok": true, "challenge": challenge, "expires_in_seconds": PING_TTL_SECS }),
        )
    }

    /// Accepte un RTT **seulement** après un ping servi, une seule fois, dans
    /// les 30 s. La valeur est mesurée par le nœud (le seul à voir son chemin
    /// réseau) ; la gateway garantit qu'elle est fraîche et signée, pas qu'elle
    /// est honnête — c'est écrit dans `docs/EXECUTION_STATUS.md`.
    async fn pong(&self, body: &[u8]) -> HttpResponse {
        let request: PongRequest = match serde_json::from_slice(body) {
            Ok(value) => value,
            Err(_) => return HttpResponse::text(400, "corps JSON invalide"),
        };
        let now = now_secs();
        if now.abs_diff(request.timestamp) > HEARTBEAT_WINDOW_SECS {
            return HttpResponse::text(400, "horodatage hors fenêtre (±300 s)");
        }
        if request.rtt_ms > MAX_RTT_MS {
            return HttpResponse::text(400, "rtt_ms hors bornes");
        }
        if let Err(response) = self
            .authenticate(
                &request.node_id,
                &pong_message(
                    &request.node_id,
                    request.timestamp,
                    &request.challenge,
                    request.rtt_ms,
                ),
                &request.signature_hex,
            )
            .await
        {
            return response;
        }

        let pending = {
            let pings = self.pings.read().await;
            pings
                .get(&request.node_id)
                .filter(|pending| pending.challenge == request.challenge)
                .cloned()
        };
        let Some(pending) = pending else {
            return HttpResponse::json(
                409,
                &json!({ "error": "aucun ping en attente pour ce challenge" }),
            );
        };
        if now.saturating_sub(pending.issued_at) > PING_TTL_SECS {
            self.pings.write().await.remove(&request.node_id);
            return HttpResponse::json(409, &json!({ "error": "ping expiré" }));
        }
        // Usage unique : un pong rejoué ne remplace pas la mesure suivante.
        self.pings.write().await.remove(&request.node_id);

        let snapshot = {
            let mut nodes = self.nodes.write().await;
            let Some(node) = nodes.get_mut(&request.node_id) else {
                return HttpResponse::json(
                    404,
                    &json!({ "error": "nœud inconnu — appairage requis" }),
                );
            };
            node.latency_ms = Some(request.rtt_ms);
            node.last_seen = now;
            nodes.clone()
        };
        self.persist_nodes(&snapshot).await;

        HttpResponse::json(200, &json!({ "ok": true, "latency_ms": request.rtt_ms }))
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

    /// Crée une tâche validée et publiée. La réponse dit explicitement qu'aucun
    /// exécuteur n'est branché : une tâche publiée n'est pas un calcul en cours.
    async fn create_task(&self, body: &[u8]) -> HttpResponse {
        let request: CreateTaskRequest = match serde_json::from_slice(body) {
            Ok(value) => value,
            Err(error) => {
                return task_error(StoreError::Invalid(format!(
                    "corps de tâche invalide : {error}"
                )));
            }
        };
        match self.tasks.create(request, now_secs()).await {
            Ok(task) => {
                HttpResponse::json(201, &json!({ "task": task, "executor_attached": false }))
            }
            Err(error) => task_error(error),
        }
    }

    async fn list_tasks(&self, query: &str) -> HttpResponse {
        let (state, limit, offset) = match parse_task_query(query) {
            Ok(parsed) => parsed,
            Err(message) => return task_error(StoreError::Invalid(message)),
        };
        match self.tasks.list(state, limit, offset, now_secs()).await {
            Ok(page) => HttpResponse::json(200, &json!(page)),
            Err(error) => task_error(error),
        }
    }

    async fn get_task(&self, path: &str) -> HttpResponse {
        let id = path.trim_start_matches("/api/v1/tasks/");
        let Ok(id) = TaskId::parse(id) else {
            return task_error(StoreError::NotFound);
        };
        match self.tasks.get(id.as_str(), now_secs()).await {
            Ok(task) => {
                HttpResponse::json(200, &json!({ "task": task, "executor_attached": false }))
            }
            Err(error) => task_error(error),
        }
    }

    async fn cancel_task(&self, path: &str) -> HttpResponse {
        let rest = path.trim_start_matches("/api/v1/tasks/");
        let Some(id) = rest.strip_suffix("/cancel") else {
            return task_error(StoreError::NotFound);
        };
        if id.contains('/') {
            return task_error(StoreError::NotFound);
        }
        let Ok(id) = TaskId::parse(id) else {
            return task_error(StoreError::NotFound);
        };
        match self.tasks.cancel(id.as_str(), now_secs()).await {
            Ok(task) => {
                HttpResponse::json(200, &json!({ "task": task, "executor_attached": false }))
            }
            Err(error) => task_error(error),
        }
    }
}

/// Traduit une erreur du magasin en réponse stable pour l'interface.
fn task_error(error: StoreError) -> HttpResponse {
    let (status, code) = match &error {
        StoreError::Invalid(_) => (400, "invalid_task"),
        StoreError::NotFound => (404, "task_not_found"),
        StoreError::Conflict(_) => (409, "invalid_transition"),
        StoreError::Full => (429, "too_many_tasks"),
        StoreError::Persistence(_) => (500, "task_storage_failure"),
    };
    HttpResponse::json(
        status,
        &json!({ "error": code, "message": error.to_string() }),
    )
}

/// Analyse `state`, `limit` et `offset`. Toute clé inconnue, dupliquée ou
/// malformée est refusée : une pagination devinée vaut moins qu'une erreur.
fn parse_task_query(query: &str) -> Result<(Option<TaskState>, usize, usize), String> {
    let mut state = None;
    let mut limit = DEFAULT_TASK_LIST_LIMIT;
    let mut offset = 0usize;
    let mut seen = std::collections::HashSet::new();

    if !query.is_empty() {
        for pair in query.split('&') {
            if pair.is_empty() {
                return Err("paramètre vide".into());
            }
            let (key, value) = pair
                .split_once('=')
                .ok_or_else(|| format!("paramètre sans valeur : {pair}"))?;
            if !seen.insert(key) {
                return Err(format!("paramètre dupliqué : {key}"));
            }
            if value.is_empty() || value.contains('%') {
                return Err(format!("valeur invalide pour {key}"));
            }
            match key {
                "state" => {
                    state = Some(
                        TaskState::parse(value).ok_or_else(|| format!("état inconnu : {value}"))?,
                    );
                }
                "limit" => {
                    let parsed: usize = value
                        .parse()
                        .map_err(|_| format!("limit invalide : {value}"))?;
                    if parsed == 0 || parsed > MAX_TASK_LIST_LIMIT {
                        return Err("limit hors bornes".into());
                    }
                    limit = parsed;
                }
                "offset" => {
                    offset = value
                        .parse()
                        .map_err(|_| format!("offset invalide : {value}"))?;
                }
                _ => return Err(format!("paramètre inconnu : {key}")),
            }
        }
    }
    Ok((state, limit, offset))
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

    fn get(target: &str) -> HttpRequest {
        match target.split_once('?') {
            Some((path, query)) => HttpRequest::new("GET", path, Vec::new()).with_query(query),
            None => HttpRequest::new("GET", target, Vec::new()),
        }
    }

    fn post(path: &str, body: &str) -> HttpRequest {
        HttpRequest::new("POST", path, body.as_bytes().to_vec())
    }

    fn body_json(response: &HttpResponse) -> serde_json::Value {
        serde_json::from_slice(&response.body).unwrap()
    }

    #[tokio::test]
    async fn options_is_204_with_cors() {
        let response = gateway()
            .route(HttpRequest::new("OPTIONS", "/api/v1/services", Vec::new()))
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

    fn ping_body(keys: &polygone_core::sign::KeyPair, id: &str, timestamp: u64) -> String {
        let signature = keys.signer.sign(ping_message(id, timestamp).as_bytes());
        json!({
            "node_id": id,
            "timestamp": timestamp,
            "signature_hex": hex::encode(signature.as_bytes()),
        })
        .to_string()
    }

    fn pong_body(
        keys: &polygone_core::sign::KeyPair,
        id: &str,
        timestamp: u64,
        challenge: &str,
        rtt_ms: u64,
    ) -> String {
        let signature = keys
            .signer
            .sign(pong_message(id, timestamp, challenge, rtt_ms).as_bytes());
        json!({
            "node_id": id,
            "timestamp": timestamp,
            "challenge": challenge,
            "rtt_ms": rtt_ms,
            "signature_hex": hex::encode(signature.as_bytes()),
        })
        .to_string()
    }

    async fn issue_challenge(
        gateway: &Gateway,
        keys: &polygone_core::sign::KeyPair,
        id: &str,
    ) -> String {
        let response = gateway
            .route(post("/api/v1/nodes/ping", &ping_body(keys, id, now_secs())))
            .await;
        assert_eq!(response.status, 200, "ping accepté");
        let body: serde_json::Value = serde_json::from_slice(&response.body).unwrap();
        body["challenge"].as_str().unwrap().to_string()
    }

    #[tokio::test]
    async fn latency_populated_after_pong() {
        let (dir, path) = temp_nodes_path("latency");
        let keys = polygone_core::sign::generate_keypair().unwrap();
        let silent = polygone_core::sign::generate_keypair().unwrap();
        let node_id = "aaaabbbbccccdddd";

        let gateway = Gateway::with_nodes_file(PathBuf::from("."), Some(path.clone()));
        seed_node(&gateway, node_id, &keys.verifier.public_key().to_hex()).await;
        seed_node(
            &gateway,
            "silencieux000000",
            &silent.verifier.public_key().to_hex(),
        )
        .await;

        let stamp = now_secs();
        let challenge = issue_challenge(&gateway, &keys, node_id).await;
        let response = gateway
            .route(post(
                "/api/v1/nodes/pong",
                &pong_body(&keys, node_id, stamp, &challenge, 42),
            ))
            .await;
        assert_eq!(response.status, 200, "pong accepté après ping");

        let nodes = gateway.nodes.read().await;
        assert_eq!(nodes[node_id].latency_ms, Some(42));
        // `last_seen` est horodaté par la gateway : il peut être postérieur au
        // timestamp du client si la requête traverse une frontière de seconde.
        assert!(nodes[node_id].last_seen >= stamp);
        assert_eq!(nodes["silencieux000000"].latency_ms, None);
        drop(nodes);
        assert_eq!(
            gateway.calculate_latency_median().await,
            Some(42),
            "la médiane ne doit utiliser que les nœuds mesurés"
        );
        drop(gateway);

        let reloaded = Gateway::with_nodes_file(PathBuf::from("."), Some(path.clone()));
        assert_eq!(
            reloaded.nodes.read().await[node_id].latency_ms,
            Some(42),
            "la mesure survit au redémarrage"
        );
        std::fs::remove_dir_all(dir).ok();
    }

    #[tokio::test]
    async fn pong_without_ping_is_refused() {
        let (dir, path) = temp_nodes_path("pong-sans-ping");
        let keys = polygone_core::sign::generate_keypair().unwrap();
        let node_id = "1111222233334444";
        let gateway = Gateway::with_nodes_file(PathBuf::from("."), Some(path.clone()));
        seed_node(&gateway, node_id, &keys.verifier.public_key().to_hex()).await;

        let response = gateway
            .route(post(
                "/api/v1/nodes/pong",
                &pong_body(&keys, node_id, now_secs(), "deadbeef", 5),
            ))
            .await;
        assert_eq!(response.status, 409, "aucun ping n'a été servi");
        assert_eq!(gateway.nodes.read().await[node_id].latency_ms, None);
        std::fs::remove_dir_all(dir).ok();
    }

    #[tokio::test]
    async fn pong_replay_is_refused() {
        let (dir, path) = temp_nodes_path("pong-replay");
        let keys = polygone_core::sign::generate_keypair().unwrap();
        let node_id = "5555666677778888";
        let gateway = Gateway::with_nodes_file(PathBuf::from("."), Some(path.clone()));
        seed_node(&gateway, node_id, &keys.verifier.public_key().to_hex()).await;

        let stamp = now_secs();
        let challenge = issue_challenge(&gateway, &keys, node_id).await;
        let body = pong_body(&keys, node_id, stamp, &challenge, 17);
        let first = gateway.route(post("/api/v1/nodes/pong", &body)).await;
        assert_eq!(first.status, 200);
        let second = gateway.route(post("/api/v1/nodes/pong", &body)).await;
        assert_eq!(second.status, 409, "un challenge ne sert qu'une fois");
        assert_eq!(gateway.nodes.read().await[node_id].latency_ms, Some(17));
        std::fs::remove_dir_all(dir).ok();
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
            .route(HttpRequest::new("PUT", "/api/v1/services", Vec::new()))
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
        let expected_node_id = polygone_core::sign::node_id_from_sign_pk_hex(&sign_public_key_hex)
            .expect("valid signing key");
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
        let expected_node_id = polygone_core::sign::node_id_from_sign_pk_hex(&sign_public_key_hex)
            .expect("valid signing key");
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
        assert_eq!(node.id, expected_node_id);
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

    fn task_body() -> String {
        json!({
            "owner": "ui-user",
            "kind": "echo",
            "input_hex": "68656c6c6f",
            "deadline_in_seconds": 300
        })
        .to_string()
    }

    #[tokio::test]
    async fn task_api_publishes_without_claiming_execution() {
        let gateway = gateway();
        let response = gateway.route(post("/api/v1/tasks", &task_body())).await;
        assert_eq!(response.status, 201);

        let payload = body_json(&response);
        assert_eq!(payload["task"]["state"], "published");
        assert_eq!(payload["task"]["spec"]["kind"], "echo");
        assert_eq!(payload["task"]["spec"]["input"], "68656c6c6f");
        assert_eq!(payload["executor_attached"], false);
        assert!(payload["task"]["result"].is_null());
        let id = payload["task"]["spec"]["id"].as_str().unwrap().to_string();
        assert!(id.starts_with("task_"));

        let fetched = body_json(&gateway.route(get(&format!("/api/v1/tasks/{id}"))).await);
        assert_eq!(fetched["task"]["state"], "published");

        let page = body_json(&gateway.route(get("/api/v1/tasks?limit=10")).await);
        assert_eq!(page["total"], 1);
        assert_eq!(page["tasks"][0]["spec"]["id"], id);
        assert_eq!(page["executor_attached"], false);

        let cancelled = gateway
            .route(post(&format!("/api/v1/tasks/{id}/cancel"), "{}"))
            .await;
        assert_eq!(cancelled.status, 200);
        assert_eq!(body_json(&cancelled)["task"]["state"], "cancelled");

        let again = gateway
            .route(post(&format!("/api/v1/tasks/{id}/cancel"), "{}"))
            .await;
        assert_eq!(again.status, 409);
        assert_eq!(body_json(&again)["error"], "invalid_transition");
    }

    #[tokio::test]
    async fn task_api_refuses_smuggled_state_and_bad_queries() {
        let gateway = gateway();

        // Un client ne peut pas fabriquer un état ou un résultat par le corps.
        let smuggled = json!({
            "owner": "ui-user",
            "kind": "echo",
            "state": "succeeded",
            "result": {"output": "", "output_hash": "00"}
        })
        .to_string();
        let response = gateway.route(post("/api/v1/tasks", &smuggled)).await;
        assert_eq!(response.status, 400);
        assert_eq!(body_json(&response)["error"], "invalid_task");

        let network = json!({
            "owner": "ui-user",
            "kind": "echo",
            "allow_network": true
        })
        .to_string();
        assert_eq!(
            gateway.route(post("/api/v1/tasks", &network)).await.status,
            400
        );

        for query in [
            "/api/v1/tasks?limit=0",
            "/api/v1/tasks?limit=1&limit=2",
            "/api/v1/tasks?state=done",
            "/api/v1/tasks?inconnu=1",
        ] {
            assert_eq!(gateway.route(get(query)).await.status, 400, "{query}");
        }

        assert_eq!(
            gateway.route(get("/api/v1/tasks/inconnue")).await.status,
            404
        );
        assert_eq!(
            gateway
                .route(post("/api/v1/tasks/inconnue/cancel", "{}"))
                .await
                .status,
            404
        );
        assert!(gateway.tasks.is_empty().await, "aucune tâche fantôme");
    }
}
