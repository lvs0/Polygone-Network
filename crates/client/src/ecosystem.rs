//! État réel de l'écosystème local — PWA, Hide, Petals, Compute.
//!
//! Chaque sonde mesure ce qui est joignable depuis cette machine, jamais ce qui
//! devrait l'être. Une absence est rapportée comme telle ; aucune valeur par
//! défaut ne transforme un service arrêté en service « prêt ».

use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::path::Path;
use std::time::Duration;

use serde_json::Value;

/// Résultat d'une sonde.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    /// Le service répond réellement.
    Ready,
    /// Le service est absent ou ne répond pas.
    Absent,
    /// La mesure n'a pas pu être faite (plateforme, permission).
    Unknown,
}

/// Une ligne du rapport d'écosystème.
#[derive(Debug, Clone)]
pub struct Probe {
    pub name: &'static str,
    pub state: State,
    pub detail: String,
}

const CONNECT_TIMEOUT: Duration = Duration::from_millis(700);
const READ_TIMEOUT: Duration = Duration::from_millis(900);

/// Sonde la coque PWA réellement installée : page console, service worker,
/// manifeste et feuille de style doivent tous être présents.
pub fn probe_pwa(site_dir: &Path) -> Probe {
    let required = [
        "index.html",
        "node.html",
        "sw.js",
        "manifest.webmanifest",
        "dashboard.js",
        "dashboard.css",
    ];
    let missing: Vec<&str> = required
        .iter()
        .copied()
        .filter(|file| !site_dir.join(file).is_file())
        .collect();
    if missing.is_empty() {
        Probe {
            name: "PWA",
            state: State::Ready,
            detail: format!("console + coque hors-ligne dans {}", site_dir.display()),
        }
    } else {
        Probe {
            name: "PWA",
            state: State::Absent,
            detail: format!(
                "fichiers manquants dans {} : {}",
                site_dir.display(),
                missing.join(", ")
            ),
        }
    }
}

/// Sonde un service TCP (relais Hide, par exemple) par une connexion réelle.
pub fn probe_tcp(name: &'static str, address: &str) -> Probe {
    match connect(address) {
        Ok(_) => Probe {
            name,
            state: State::Ready,
            detail: format!("{address} accepte une connexion TCP"),
        },
        Err(error) => Probe {
            name,
            state: State::Absent,
            detail: format!("{address} injoignable : {error}"),
        },
    }
}

/// Interroge Ollama et compte les modèles réellement listés.
pub fn probe_ollama(address: &str) -> Probe {
    let request = format!("GET /api/tags HTTP/1.1\r\nHost: {address}\r\nConnection: close\r\n\r\n");
    let response = match http_get(address, &request) {
        Ok(response) => response,
        Err(error) => {
            return Probe {
                name: "Petals",
                state: State::Absent,
                detail: format!("Ollama {address} injoignable : {error}"),
            }
        }
    };

    let Some((head, body)) = response.split_once("\r\n\r\n") else {
        return Probe {
            name: "Petals",
            state: State::Unknown,
            detail: "réponse HTTP sans corps".into(),
        };
    };
    if !head.starts_with("HTTP/1.1 200") && !head.starts_with("HTTP/1.0 200") {
        return Probe {
            name: "Petals",
            state: State::Absent,
            detail: format!("Ollama a répondu : {}", head.lines().next().unwrap_or("?")),
        };
    }
    let count = serde_json::from_str::<Value>(body)
        .ok()
        .and_then(|value| value.get("models").and_then(Value::as_array).map(Vec::len));
    match count {
        Some(count) => Probe {
            name: "Petals",
            state: State::Ready,
            detail: format!("{count} modèle(s) local(aux) via {address}"),
        },
        None => Probe {
            name: "Petals",
            state: State::Unknown,
            detail: "réponse Ollama sans liste `models`".into(),
        },
    }
}

/// Sonde le daemon par son socket Unix. Le socket qui accepte une connexion
/// indique un daemon vivant ; il ne prouve pas qu'une allocation serait
/// accordée, et le détail le dit.
#[cfg(unix)]
pub fn probe_daemon(socket: &Path) -> Probe {
    match std::os::unix::net::UnixStream::connect(socket) {
        Ok(_) => Probe {
            name: "Compute",
            state: State::Ready,
            detail: format!(
                "daemon joignable sur {} (bail non demandé)",
                socket.display()
            ),
        },
        Err(error) => Probe {
            name: "Compute",
            state: State::Absent,
            detail: format!("daemon injoignable sur {} : {error}", socket.display()),
        },
    }
}

#[cfg(not(unix))]
pub fn probe_daemon(_socket: &Path) -> Probe {
    Probe {
        name: "Compute",
        state: State::Unknown,
        detail: "socket Unix non disponible sur cette plateforme".into(),
    }
}

/// Toutes les sondes, dans l'ordre d'affichage.
pub fn probes(site_dir: &Path, relay: &str, ollama: &str, socket: &Path) -> Vec<Probe> {
    vec![
        probe_pwa(site_dir),
        probe_tcp("Hide", relay),
        probe_ollama(ollama),
        probe_daemon(socket),
    ]
}

/// Affiche le rapport. Retourne `true` si tous les services sont prêts.
pub fn print(probes: &[Probe]) -> bool {
    println!("⬡ Écosystème Polygone");
    for probe in probes {
        let (symbol, label) = match probe.state {
            State::Ready => ("✓", "prêt"),
            State::Absent => ("✗", "absent"),
            State::Unknown => ("?", "inconnu"),
        };
        println!("  {symbol} {:<8} {:<8} {}", probe.name, label, probe.detail);
    }
    probes.iter().all(|probe| probe.state == State::Ready)
}

fn connect(address: &str) -> std::io::Result<TcpStream> {
    let target = address
        .to_socket_addrs()?
        .next()
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidInput, "adresse vide"))?;
    let stream = TcpStream::connect_timeout(&target, CONNECT_TIMEOUT)?;
    stream.set_read_timeout(Some(READ_TIMEOUT))?;
    stream.set_write_timeout(Some(READ_TIMEOUT))?;
    Ok(stream)
}

fn http_get(address: &str, request: &str) -> std::io::Result<String> {
    let mut stream = connect(address)?;
    stream.write_all(request.as_bytes())?;
    let mut response = Vec::new();
    let mut chunk = [0u8; 4096];
    loop {
        match stream.read(&mut chunk) {
            Ok(0) => break,
            Ok(read) => {
                response.extend_from_slice(&chunk[..read]);
                if response.len() > 1_048_576 {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "réponse HTTP trop volumineuse",
                    ));
                }
            }
            Err(error)
                if error.kind() == std::io::ErrorKind::WouldBlock
                    || error.kind() == std::io::ErrorKind::TimedOut =>
            {
                break;
            }
            Err(error) => return Err(error),
        }
    }
    String::from_utf8(response)
        .map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidData, "réponse non UTF-8"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;
    use std::path::PathBuf;

    fn unique_dir(label: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "polygone-ecosystem-{label}-{}-{nanos}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn pwa_requires_every_shell_file() {
        let dir = unique_dir("pwa");
        for file in [
            "index.html",
            "node.html",
            "sw.js",
            "manifest.webmanifest",
            "dashboard.js",
            "dashboard.css",
        ] {
            std::fs::write(dir.join(file), b"ok").unwrap();
        }
        let ready = probe_pwa(&dir);
        assert_eq!(ready.state, State::Ready);

        std::fs::remove_file(dir.join("sw.js")).unwrap();
        let absent = probe_pwa(&dir);
        assert_eq!(absent.state, State::Absent);
        assert!(absent.detail.contains("sw.js"));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn ollama_probe_reads_models() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap().to_string();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0u8; 512];
            let _ = stream.read(&mut request);
            let body = r#"{"models":[{"name":"a"},{"name":"b"}]}"#;
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            stream.write_all(response.as_bytes()).unwrap();
        });

        let probe = probe_ollama(&address);
        server.join().unwrap();
        assert_eq!(probe.state, State::Ready, "{}", probe.detail);
        assert!(probe.detail.contains("2 modèle"));
    }

    #[test]
    fn tcp_probe_on_closed_port_is_absent() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap().to_string();
        drop(listener);
        let probe = probe_tcp("Hide", &address);
        assert_eq!(probe.state, State::Absent);
    }

    #[test]
    fn daemon_socket_absent_is_reported() {
        let dir = unique_dir("daemon");
        let probe = probe_daemon(&dir.join("absent.sock"));
        assert_eq!(probe.state, State::Absent);
        std::fs::remove_dir_all(&dir).ok();
    }
}
