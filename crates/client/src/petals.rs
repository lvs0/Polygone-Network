//! petals — the local AI service (D4 pilot).
//!
//! `polygone petals` talks to a local Ollama (default http://127.0.0.1:11434,
//! override with `POLYGONE_OLLAMA_URL`). No cloud, no account, no telemetry:
//! the model stays on your machine — the same sovereignty-first rule as the
//! rest of Polygone.
//!
//! Commands:
//!   polygone petals status          → models + counts
//!   polygone petals models          → list model names
//!   polygone petals ask <question>  → generate (--model to pick one)

use anyhow::Result;
use std::io::{Read, Write};

/// Base URL of the local Ollama.
pub fn ollama_url() -> String {
    std::env::var("POLYGONE_OLLAMA_URL").unwrap_or_else(|_| "http://127.0.0.1:11434".into())
}

/// Minimal HTTP/1.1 client over std::net — no external dependency.
fn http_request(method: &str, path: &str, body: Option<&str>) -> Result<(u16, String)> {
    let url = ollama_url();
    let rest = url.trim_start_matches("http://");
    let (host, port) = match rest.split_once(':') {
        Some((h, p)) => (h.to_string(), p.parse::<u16>().unwrap_or(80)),
        None => (rest.to_string(), 80),
    };

    let mut stream = std::net::TcpStream::connect((host.as_str(), port))?;
    let body = body.unwrap_or("");
    let req = format!(
        "{method} {path} HTTP/1.1\r\nHost: {host}:{port}\r\n\
         Content-Type: application/json\r\nContent-Length: {}\r\n\
         Connection: close\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(req.as_bytes())?;
    stream.flush()?;

    let mut raw = Vec::new();
    stream.read_to_end(&mut raw)?;
    let text = String::from_utf8_lossy(&raw).to_string();

    // Split headers / body on the first empty line.
    let (head, mut body_text) = match text.split_once("\r\n\r\n") {
        Some((h, b)) => (h.to_string(), b.to_string()),
        None => (String::new(), text.clone()),
    };
    let status = head
        .lines()
        .next()
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);

    // Ollama uses Transfer-Encoding: chunked — decode the chunks.
    if head
        .to_ascii_lowercase()
        .contains("transfer-encoding: chunked")
    {
        body_text = decode_chunked(&body_text);
    }
    Ok((status, body_text))
}

/// Decode an HTTP chunked body: `HEX\r\ndata\r\n0\r\n\r\n`.
fn decode_chunked(body: &str) -> String {
    let mut out = String::new();
    let mut rest = body;
    while let Some((size_line, after_size)) = rest.split_once("\r\n") {
        let size = usize::from_str_radix(size_line.trim(), 16).unwrap_or(0);
        if size == 0 {
            break;
        }
        let (chunk, after_chunk) = match after_size.split_at_checked(size) {
            Some((c, a)) => (c, a),
            None => break,
        };
        out.push_str(chunk);
        // Skip the trailing CRLF after the chunk.
        rest = after_chunk.strip_prefix("\r\n").unwrap_or(after_chunk);
    }
    out
}

/// List installed model names.
pub fn models() -> Result<Vec<String>> {
    let (status, body) = http_request("GET", "/api/tags", None)?;
    if status != 200 {
        anyhow::bail!("Ollama HTTP {status}: {body}");
    }
    let val: serde_json::Value = serde_json::from_str(&body)?;
    let names: Vec<String> = val
        .get("models")
        .and_then(|m| m.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|m| {
                    m.get("name")
                        .and_then(|n| n.as_str())
                        .map(|s| s.to_string())
                })
                .collect()
        })
        .unwrap_or_default();
    if names.is_empty() {
        anyhow::bail!("aucun modèle trouvé — l'Ollama répond mais est vide");
    }
    Ok(names)
}

/// Ask the default (or given) model.
pub fn ask(prompt: &str, model: Option<&str>) -> Result<String> {
    let model = match model {
        Some(m) => m.to_string(),
        None => {
            let all = models()?;
            all.first()
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("aucun modèle"))?
        }
    };
    let payload = serde_json::json!({
        "model": model,
        "prompt": prompt,
        "stream": false,
    });
    let (status, body) = http_request("POST", "/api/generate", Some(&payload.to_string()))?;
    if status != 200 {
        anyhow::bail!("Ollama HTTP {status}: {body}");
    }
    let val: serde_json::Value = serde_json::from_str(&body)?;
    let response = val
        .get("response")
        .and_then(|r| r.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    if response.is_empty() {
        anyhow::bail!("réponse vide du modèle {model}");
    }
    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn url_defaults_to_local_ollama() {
        let url = ollama_url();
        assert!(url.contains("127.0.0.1") || url.contains("localhost"));
    }

    #[test]
    fn decodes_chunked_body() {
        let chunked = "5\r\nhello\r\n6\r\n world\r\n0\r\n\r\n";
        assert_eq!(decode_chunked(chunked), "hello world");
    }

    #[test]
    fn decodes_single_chunk() {
        let chunked = "b\r\n{\"models\":[]}\r\n0\r\n\r\n";
        let decoded = decode_chunked(chunked);
        assert!(decoded.contains("\"models\""));
    }
}
