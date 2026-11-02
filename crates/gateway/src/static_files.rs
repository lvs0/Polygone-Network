//! Service du site statique, avec refus explicite des traversées de chemin.
//!
//! Le site est public : une requête ne doit jamais pouvoir sortir de
//! `site_dir`. Trois barrières, dans cet ordre : décodage pourcentage refusé
//! s'il cache un séparateur, composants de chemin validés, puis vérification
//! du préfixe canonique après résolution.

use std::path::Path;

use tokio::fs;

use crate::http::HttpResponse;

/// Type MIME d'un fichier d'après son extension.
pub fn mime_for(path: &str) -> &'static str {
    let extension = path.rsplit('.').next().unwrap_or("").to_ascii_lowercase();

    match extension.as_str() {
        "html" | "htm" => "text/html; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "webmanifest" => "application/manifest+json",
        "json" => "application/json",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "ico" => "image/x-icon",
        "woff2" => "font/woff2",
        "txt" | "md" => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

/// Sert `raw_path` sous `site_dir`, ou renvoie 403/404 sans rien lire.
pub async fn serve(site_dir: &Path, raw_path: &str) -> HttpResponse {
    let lowered = raw_path.to_ascii_lowercase();
    // Un séparateur encodé n'a aucune raison légitime ici : on refuse avant
    // même de décoder, plutôt que de deviner l'intention.
    if lowered.contains("%2f") || lowered.contains("%5c") {
        return forbidden();
    }

    let Some(decoded) = percent_decode(raw_path) else {
        return forbidden();
    };
    if decoded.contains('\0') || decoded.contains('\\') || decoded.contains("..") {
        return forbidden();
    }

    let relative = decoded.trim_start_matches('/');
    let relative = if relative.is_empty() {
        "index.html"
    } else {
        relative
    };
    if relative
        .split('/')
        .any(|part| part.is_empty() || part == ".")
    {
        return forbidden();
    }

    let Ok(site) = fs::canonicalize(site_dir).await else {
        return not_found();
    };
    let Ok(full) = fs::canonicalize(site.join(relative)).await else {
        return not_found();
    };
    if !full.starts_with(&site) {
        return forbidden();
    }

    match fs::read(&full).await {
        Ok(body) => HttpResponse::new(200, mime_for(relative), body).no_cache(),
        Err(_) => not_found(),
    }
}

/// Décodage pourcentage minimal. `None` si la séquence est invalide ou si le
/// résultat n'est pas de l'UTF-8 : dans les deux cas, la requête est refusée.
fn percent_decode(input: &str) -> Option<String> {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;

    while index < bytes.len() {
        if bytes[index] == b'%' {
            let hex = input.get(index + 1..index + 3)?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            index += 3;
        } else {
            out.push(bytes[index]);
            index += 1;
        }
    }

    String::from_utf8(out).ok()
}

fn forbidden() -> HttpResponse {
    HttpResponse::text(403, "chemin refusé")
}

fn not_found() -> HttpResponse {
    HttpResponse::text(404, "fichier introuvable")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static COUNTER: AtomicUsize = AtomicUsize::new(0);

    /// Site jetable dans le dossier temporaire du système — pas de dépendance
    /// de test supplémentaire, et aucun fichier du dépôt n'est touché.
    fn temp_site() -> PathBuf {
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir =
            std::env::temp_dir().join(format!("polygone-gateway-test-{}-{n}", std::process::id()));
        std::fs::create_dir_all(dir.join("assets")).unwrap();
        std::fs::write(dir.join("index.html"), b"<h1>polygone</h1>").unwrap();
        std::fs::write(dir.join("style.css"), b"body{}").unwrap();
        std::fs::write(dir.join("manifest.webmanifest"), b"{}").unwrap();
        std::fs::write(dir.join("assets/icon.svg"), b"<svg/>").unwrap();
        dir
    }

    #[tokio::test]
    async fn root_serves_index_with_html_mime() {
        let site = temp_site();
        let response = serve(&site, "/").await;

        assert_eq!(response.status, 200);
        assert_eq!(response.content_type, "text/html; charset=utf-8");
        assert_eq!(response.body, b"<h1>polygone</h1>");
        assert!(
            response
                .extra_headers
                .iter()
                .any(|(name, value)| name == "Cache-Control" && value == "no-cache")
        );
    }

    #[tokio::test]
    async fn nested_file_uses_its_mime() {
        let site = temp_site();
        assert_eq!(
            serve(&site, "/style.css").await.content_type,
            "text/css; charset=utf-8"
        );
        assert_eq!(
            serve(&site, "/assets/icon.svg").await.content_type,
            "image/svg+xml"
        );
        assert_eq!(
            serve(&site, "/manifest.webmanifest").await.content_type,
            "application/manifest+json"
        );
    }

    #[tokio::test]
    async fn plain_traversal_is_forbidden() {
        let site = temp_site();
        for path in ["/../etc/passwd", "/assets/../../etc/passwd", "/..\\windows"] {
            assert_eq!(serve(&site, path).await.status, 403, "chemin {path}");
        }
    }

    #[tokio::test]
    async fn encoded_traversal_is_forbidden() {
        let site = temp_site();
        for path in [
            "/%2e%2e/etc/passwd",
            "/assets%2f..%2f..%2fetc/passwd",
            "/a%5cb",
        ] {
            assert_eq!(serve(&site, path).await.status, 403, "chemin {path}");
        }
    }

    #[tokio::test]
    async fn unknown_file_is_404() {
        let site = temp_site();
        assert_eq!(serve(&site, "/absent.html").await.status, 404);
    }

    #[tokio::test]
    async fn missing_site_is_404_not_panic() {
        let response = serve(Path::new("/chemin/qui/n/existe/pas"), "/").await;
        assert_eq!(response.status, 404);
    }
}
