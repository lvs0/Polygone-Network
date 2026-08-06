//! mesh — LAN peer discovery (plane 2 of the SPEC, Phase 4).
//!
//! Zero-dependency UDP broadcast: nodes announce their identity + relay
//! address on the local network; peers discover them without any hardcoded
//! address.
//!
//! Protocol (UDP, port 7642):
//!   announce : "POLYGONE v1 <node_id> <relay_host:port>\n"
//!   discover : "POLYGONE v1 PING\n"  (a peer asks for announces)
//!
//! Broadcast uses 255.255.255.255 — works on the local subnet. No internet,
//! no server, no telemetry: the mesh is the LAN.

use anyhow::Result;
use std::net::{SocketAddr, UdpSocket};
use std::time::Duration;

/// The mesh UDP port.
pub const MESH_PORT: u16 = 7642;

/// Build the announce packet for a node + its relay address + free RAM (MB).
/// Format: "POLYGONE v1 <node_id> <relay> [<free_ram_mb>]"
pub fn announce_packet(node_id: &str, relay: &str, free_ram_mb: Option<u32>) -> String {
    match free_ram_mb {
        Some(ram) => format!("POLYGONE v1 {node_id} {relay} {ram}\n"),
        None => format!("POLYGONE v1 {node_id} {relay}\n"),
    }
}

/// Discovered peer on the LAN.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Peer {
    pub node_id: String,
    pub relay: String,
    pub addr: SocketAddr,
    /// Free RAM (MB) announced by the peer, if present.
    pub free_ram_mb: Option<u32>,
}

/// Scan the LAN for announcing nodes during `timeout`. Returns unique peers.
///
/// The scanner binds an ephemeral port and asks the LAN who is here:
/// `PING <port>` on the mesh port. Announcers answer directly to the
/// scanner's port — no port conflict, works on a shared LAN.
pub fn discover(timeout: Duration) -> Result<Vec<Peer>> {
    let socket = UdpSocket::bind(("0.0.0.0", 0))?; // ephemeral — no conflict
    socket.set_broadcast(true)?;
    socket.set_read_timeout(Some(Duration::from_millis(200)))?;
    let my_port = socket.local_addr()?.port();

    // Ask the LAN who is here, with our response port.
    let ping = format!("POLYGONE v1 PING {my_port}\n");
    let _ = socket.send_to(ping.as_bytes(), ("255.255.255.255", MESH_PORT));

    let mut peers: Vec<Peer> = Vec::new();
    let mut buf = [0u8; 512];
    let deadline = std::time::Instant::now() + timeout;

    while std::time::Instant::now() < deadline {
        match socket.recv_from(&mut buf) {
            Ok((n, addr)) => {
                let line = String::from_utf8_lossy(&buf[..n]).trim().to_string();
                let parts: Vec<&str> = line.split_whitespace().collect();
                // "POLYGONE v1 <node_id> <relay> [<free_ram_mb>]"
                if parts.len() >= 4 && parts[0] == "POLYGONE" && parts[1] == "v1" {
                    let free_ram_mb = parts.get(4).and_then(|r| r.parse::<u32>().ok());
                    let peer = Peer {
                        node_id: parts[2].to_string(),
                        relay: parts[3].to_string(),
                        addr,
                        free_ram_mb,
                    };
                    if !peers.iter().any(|p| p.node_id == peer.node_id) {
                        peers.push(peer);
                    }
                }
            }
            Err(_) => continue, // read timeout — keep scanning until deadline
        }
    }
    Ok(peers)
}

/// Announce this node on the LAN until interrupted. Sends an announce every
/// 5 s AND answers discovery PINGs immediately (so a scan finds us fast).
pub async fn announce(node_id: &str, relay: &str) -> Result<()> {
    let socket = UdpSocket::bind(("0.0.0.0", MESH_PORT))?;
    socket.set_broadcast(true)?;
    socket.set_read_timeout(Some(Duration::from_millis(500)))?;

    println!("⬡ mesh : annonce sur le LAN (port {MESH_PORT}) — node {node_id} · relay {relay}");
    println!("  (Ctrl-C pour arrêter)");

    let mut buf = [0u8; 128];
    loop {
        // Live free RAM — the RES compute signal (ghost nodes).
        let packet = announce_packet(node_id, relay, free_ram_mb());
        socket.send_to(packet.as_bytes(), ("255.255.255.255", MESH_PORT))?;

        // Answer PINGs for up to 500 ms, then announce again.
        loop {
            match socket.recv_from(&mut buf) {
                Ok((n, from)) => {
                    let line = String::from_utf8_lossy(&buf[..n]);
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    // "POLYGONE v1 PING <port>" — reply straight to the scanner.
                    if parts.len() >= 4
                        && parts[0] == "POLYGONE"
                        && parts[1] == "v1"
                        && parts[2] == "PING"
                    {
                        if let Ok(port) = parts[3].parse::<u16>() {
                            let _ = socket.send_to(packet.as_bytes(), (from.ip(), port));
                        }
                    }
                }
                Err(_) => break, // read timeout — next announce round
            }
        }
    }
}

/// Free RAM in MB, read from /proc/meminfo (Linux) or vm_stat (macOS).
pub fn free_ram_mb() -> Option<u32> {
    #[cfg(target_os = "linux")]
    {
        let meminfo = std::fs::read_to_string("/proc/meminfo").ok()?;
        let mut free = 0u64;
        let mut avail = 0u64;
        for line in meminfo.lines() {
            if let Some(rest) = line.strip_prefix("MemFree:") {
                free = rest.trim().split_whitespace().next()?.parse().ok()?;
            }
            if let Some(rest) = line.strip_prefix("MemAvailable:") {
                avail = rest.trim().split_whitespace().next()?.parse().ok()?;
            }
        }
        // Prefer MemAvailable (more honest about usable memory).
        let kb = if avail > 0 { avail } else { free };
        Some((kb / 1024) as u32)
    }
    #[cfg(target_os = "macos")]
    {
        use std::process::Command;
        let out = Command::new("vm_stat").output().ok()?;
        let text = String::from_utf8_lossy(&out.stdout);
        // "Pages free: 12345" → free pages × 4096 bytes → MB.
        let pages: u64 = text
            .lines()
            .find_map(|l| l.trim().strip_prefix("Pages free:"))?
            .trim()
            .trim_end_matches('.')
            .parse()
            .ok()?;
        Some(((pages * 4096) / (1024 * 1024)) as u32)
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        None
    }
}

/// Print discovered peers as a table.
pub fn print_peers(peers: &[Peer]) {
    if peers.is_empty() {
        println!(
            "  aucun nœud trouvé sur le LAN (lancez « polygone annoncer » sur un autre poste)"
        );
        return;
    }
    println!("  {} nœud(s) trouvé(s) sur le LAN :", peers.len());
    for p in peers {
        match p.free_ram_mb {
            Some(ram) => println!(
                "    · {}  →  relay {}  · {} Mo libres",
                p.node_id, p.relay, ram
            ),
            None => println!("    · {}  →  relay {}", p.node_id, p.relay),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn announce_packet_format() {
        let p = announce_packet("abc123", "192.168.1.5:7000", Some(4096));
        assert!(p.starts_with("POLYGONE v1 abc123 192.168.1.5:7000 4096"));
        let p2 = announce_packet("abc123", "192.168.1.5:7000", None);
        assert_eq!(p2.trim(), "POLYGONE v1 abc123 192.168.1.5:7000");
    }
}
