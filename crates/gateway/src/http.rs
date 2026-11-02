//! Requête et réponse HTTP minimales, sans dépendance externe.
//!
//! Le serveur n'implémente que ce dont il a besoin : une ligne de requête, un
//! corps, des en-têtes. Tout ce qui n'est pas reconnu est refusé franchement
//! plutôt que deviné.

/// Origine autorisée pour l'API (le site est public et sans compte).
pub const CORS_ORIGIN: &str = "*";

/// Requête déjà décodée par la couche socket.
#[derive(Debug, Clone)]
pub struct HttpRequest {
    pub method: String,
    /// Chemin sans la chaîne de requête (`?…` retiré).
    pub path: String,
    pub body: Vec<u8>,
}

/// Réponse prête à écrire sur le socket.
#[derive(Debug, Clone)]
pub struct HttpResponse {
    pub status: u16,
    pub content_type: String,
    pub body: Vec<u8>,
    pub extra_headers: Vec<(String, String)>,
}

impl HttpResponse {
    pub fn new(status: u16, content_type: &str, body: Vec<u8>) -> Self {
        Self {
            status,
            content_type: content_type.to_string(),
            body,
            extra_headers: Vec::new(),
        }
    }

    /// Corps texte UTF-8.
    pub fn text(status: u16, body: &str) -> Self {
        Self::new(
            status,
            "text/plain; charset=utf-8",
            body.as_bytes().to_vec(),
        )
    }

    /// Corps JSON sérialisé. Une valeur non sérialisable donne un JSON d'erreur
    /// plutôt qu'une réponse vide : l'appelant ne doit jamais mentir sur le
    /// contenu qu'il renvoie.
    pub fn json(status: u16, value: &serde_json::Value) -> Self {
        let body =
            serde_json::to_vec(value).unwrap_or_else(|_| b"{\"error\":\"serialize\"}".to_vec());
        Self::new(status, "application/json", body)
    }

    /// Réponse sans corps (204, 304…).
    pub fn empty(status: u16) -> Self {
        Self::new(status, "text/plain; charset=utf-8", Vec::new())
    }

    /// Ajoute les en-têtes CORS communs à toutes les réponses de l'API.
    pub fn cors(mut self) -> Self {
        self.push_header("Access-Control-Allow-Origin", CORS_ORIGIN);
        self.push_header("Access-Control-Allow-Methods", "GET, POST, OPTIONS");
        self.push_header("Access-Control-Allow-Headers", "content-type");
        self
    }

    /// Empêche un cache intermédiaire de servir une version périmée du site.
    pub fn no_cache(mut self) -> Self {
        self.push_header("Cache-Control", "no-cache");
        self
    }

    pub fn push_header(&mut self, name: &str, value: &str) {
        self.extra_headers
            .push((name.to_string(), value.to_string()));
    }

    /// Sérialise la réponse complète, en-têtes compris.
    pub fn encode(&self) -> Vec<u8> {
        let mut head = format!(
            "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n",
            self.status,
            status_text(self.status),
            self.content_type,
            self.body.len()
        );
        for (name, value) in &self.extra_headers {
            head.push_str(name);
            head.push_str(": ");
            head.push_str(value);
            head.push_str("\r\n");
        }
        head.push_str("\r\n");

        let mut out = Vec::with_capacity(head.len() + self.body.len());
        out.extend_from_slice(head.as_bytes());
        out.extend_from_slice(&self.body);
        out
    }
}

/// Texte normalisé du code de statut.
pub fn status_text(code: u16) -> &'static str {
    match code {
        200 => "OK",
        201 => "Created",
        204 => "No Content",
        400 => "Bad Request",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        408 => "Request Timeout",
        413 => "Payload Too Large",
        431 => "Request Header Fields Too Large",
        429 => "Too Many Requests",
        500 => "Internal Server Error",
        _ => "Unknown",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_carries_status_length_and_headers() {
        let response = HttpResponse::text(404, "nope").cors();
        let raw = String::from_utf8(response.encode()).unwrap();

        assert!(raw.starts_with("HTTP/1.1 404 Not Found\r\n"));
        assert!(raw.contains("Content-Length: 4\r\n"));
        assert!(raw.contains("Access-Control-Allow-Origin: *\r\n"));
        assert!(raw.ends_with("\r\n\r\nnope"));
    }

    #[test]
    fn empty_body_has_zero_length() {
        let raw = String::from_utf8(HttpResponse::empty(204).encode()).unwrap();
        assert!(raw.contains("Content-Length: 0\r\n"));
        assert!(raw.starts_with("HTTP/1.1 204 No Content\r\n"));
    }
}
