//! Relais vers le daemon local (F1-API-01, domaines ressources / usage).
//!
//! La gateway ne réimplémente PAS le protocole : elle utilise `DaemonClient`
//! et les types `Reply` du crate `polygoned`. La réponse HTTP est la
//! sérialisation serde du MÊME `Reply` que celui du socket — champs omis
//! compris (`gpu_free_vram_mb`, `storage_capacity`…). La parité est garantie
//! par le compilateur ; le test de contrat la vérifie contre un vrai daemon.
//!
//! Toute erreur est typée et publiée franchement : daemon absent → 503,
//! protocole inattendu → 502, capacité `ledger` absente → 501. Jamais de
//! valeur inventée, jamais de zéro prétendu « mesuré ».
//!
//! Le client daemon est bloquant (socket Unix std, timeout court) : chaque
//! appel passe par `spawn_blocking` pour ne pas geler le runtime tokio.

use std::path::PathBuf;
use std::time::Duration;

use polygoned::client::{ClientError, DaemonClient, HelloInfo};
use polygoned::socket::Reply;
use serde_json::Value;

/// Timeout de sonde : une API locale ne doit pas pendre quand le daemon est
/// parti. Plus long que le timeout CLI (800 ms) pour laisser à un daemon
/// chargé le temps de répondre, assez court pour rester une API réactive.
pub const PROBE_TIMEOUT: Duration = Duration::from_millis(1500);

/// Erreurs du relais, mappées sur des statuts HTTP distincts.
#[derive(Debug)]
pub enum RelayError {
    /// Socket absent, connexion refusée ou timeout : le daemon ne tourne pas
    /// (ou pas ici). 503 — l'app doit dire « daemon absent », pas « 0 Gio ».
    Absent(String),
    /// Réponse illisible ou inattendue sur le fil. 502.
    Protocol(String),
    /// Le daemon ne connaît pas la demande (capacité absente). 501.
    Unsupported,
    /// Le daemon a répondu une erreur protocolaire explicite. 502.
    Refused { code: String, message: String },
}

fn map_client_error(error: ClientError) -> RelayError {
    match error {
        ClientError::Io(e) => RelayError::Absent(e.to_string()),
        ClientError::Protocol(message) => RelayError::Protocol(message),
        ClientError::Server { code, message: _ } if code == "unknown_type" => {
            RelayError::Unsupported
        }
        ClientError::Server { code, message } => RelayError::Refused { code, message },
    }
}

fn connect(socket: &std::path::Path) -> Result<DaemonClient, RelayError> {
    DaemonClient::connect_with_timeout(socket, PROBE_TIMEOUT).map_err(map_client_error)
}

fn reply_to_value(reply: &Reply) -> Result<Value, RelayError> {
    serde_json::to_value(reply).map_err(|e| RelayError::Protocol(e.to_string()))
}

/// `status_response` du daemon, relayé TEL QUEL (mêmes champs, mêmes omissions).
pub async fn status(socket: PathBuf) -> Result<Value, RelayError> {
    tokio::task::spawn_blocking(move || {
        let mut client = connect(&socket)?;
        let reply = client.status().map_err(map_client_error)?;
        match reply {
            Reply::Status { .. } => reply_to_value(&reply),
            other => Err(RelayError::Protocol(format!(
                "status_response attendu, reçu : {other:?}"
            ))),
        }
    })
    .await
    .map_err(|e| RelayError::Protocol(format!("tâche de relais : {e}")))?
}

/// Résumé du `hello` : version du daemon, plateforme, tier, protocole négocié,
/// capacités. Ce que l'écran Vue d'ensemble affiche sans le redemander.
pub async fn hello(socket: PathBuf) -> Result<HelloInfo, RelayError> {
    tokio::task::spawn_blocking(move || Ok(connect(&socket)?.hello_info()))
        .await
        .map_err(|e| RelayError::Protocol(format!("tâche de relais : {e}")))?
}

/// Instantané du ledger (réservations actives + agrégats + refus).
///
/// La capacité `ledger` est vérifiée AVANT la demande : un daemon antérieur
/// ne l'annonce pas, et l'absence se traite comme indisponible (501), pas
/// comme un instantané vide — un ledger vide et un ledger inconnu ne sont
/// pas la même information.
pub async fn usage(socket: PathBuf) -> Result<Value, RelayError> {
    tokio::task::spawn_blocking(move || {
        let mut client = connect(&socket)?;
        if !client.capabilities().iter().any(|c| c == "ledger") {
            return Err(RelayError::Unsupported);
        }
        let reply = client.ledger().map_err(map_client_error)?;
        match reply {
            Reply::Ledger { .. } => reply_to_value(&reply),
            other => Err(RelayError::Protocol(format!(
                "ledger_response attendu, reçu : {other:?}"
            ))),
        }
    })
    .await
    .map_err(|e| RelayError::Protocol(format!("tâche de relais : {e}")))?
}
