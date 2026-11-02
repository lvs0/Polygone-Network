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

/// Validate the relay endpoint without resolving DNS or opening a connection.
/// IPv6 literals must be bracketed (`[fe80::1]:7000`). Hostnames are accepted
/// for deployments using a DNS name, but malformed labels and port zero are not.
fn valid_relay_endpoint(value: &str) -> bool {
    let (host, port) = if let Some(rest) = value.strip_prefix('[') {
        let Some((host, suffix)) = rest.split_once(']') else {
            return false;
        };
        let Some(port) = suffix.strip_prefix(':') else {
            return false;
        };
        if host.parse::<std::net::Ipv6Addr>().is_err() {
            return false;
        }
        (host, port)
    } else {
        let Some((host, port)) = value.rsplit_once(':') else {
            return false;
        };
        if host.is_empty() || host.contains(':') {
            return false;
        }
        if host.parse::<std::net::Ipv4Addr>().is_err() {
            let labels = host.split('.');
            if !labels.into_iter().all(|label| {
                !label.is_empty()
                    && label.len() <= 63
                    && label.as_bytes()[0].is_ascii_alphanumeric()
                    && label.as_bytes()[label.len() - 1].is_ascii_alphanumeric()
                    && label
                        .bytes()
                        .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
            }) {
                return false;
            }
        }
        (host, port)
    };
    let _ = host;
    port.parse::<u16>().is_ok_and(|port| port != 0)
}

/// Parse only well-formed LAN announcements received from the mesh port.
/// UDP discovery is unauthenticated: callers must still treat peers and their
/// advertised RAM as hints, never as trusted identity or resource guarantees.
fn parse_announce(line: &str, addr: SocketAddr) -> Option<Peer> {
    if addr.port() != MESH_PORT {
        return None;
    }
    let parts: Vec<&str> = line.split_whitespace().collect();
    if !(parts.len() == 4 || parts.len() == 5)
        || parts[0] != "POLYGONE"
        || parts[1] != "v1"
        || parts[2].len() != 16
        || !parts[2].bytes().all(|byte| byte.is_ascii_hexdigit())
        || !valid_relay_endpoint(parts[3])
    {
        return None;
    }
    let free_ram_mb = match parts.get(4) {
        Some(value) => Some(value.parse::<u32>().ok()?),
        None => None,
    };
    Some(Peer {
        node_id: parts[2].to_ascii_lowercase(),
        relay: parts[3].to_string(),
        addr,
        free_ram_mb,
    })
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
                let line = String::from_utf8_lossy(&buf[..n]);
                if let Some(peer) = parse_announce(line.trim(), addr) {
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
    socket.set_read_timeout(Some(Duration::from_secs(5)))?;

    println!("⬡ mesh : annonce sur le LAN (port {MESH_PORT}) — node {node_id} · relay {relay}");
    println!("  (Ctrl-C pour arrêter)");

    let mut buf = [0u8; 128];
    loop {
        // Live free RAM — the RES compute signal (ghost nodes).
        let packet = announce_packet(node_id, relay, free_ram_mb());
        socket.send_to(packet.as_bytes(), ("255.255.255.255", MESH_PORT))?;

        // Answer PINGs for up to five seconds, then announce again.
        while let Ok((n, from)) = socket.recv_from(&mut buf) {
            let line = String::from_utf8_lossy(&buf[..n]);
            let parts: Vec<&str> = line.split_whitespace().collect();
            // "POLYGONE v1 PING <port>" — reply straight to the scanner.
            if parts.len() >= 4 && parts[0] == "POLYGONE" && parts[1] == "v1" && parts[2] == "PING"
            {
                if let Ok(port) = parts[3].parse::<u16>() {
                    let _ = socket.send_to(packet.as_bytes(), (from.ip(), port));
                }
            }
        }
    }
}

/// Remaining memory under a finite cgroup v2 memory.max. `None` means no
/// finite limit or the cgroup measurement is unavailable.
fn cgroup_memory_headroom(limit: &str, current: &str) -> Option<u64> {
    let limit = limit.trim().parse::<u64>().ok()?;
    let current = current.trim().parse::<u64>().ok()?;
    Some(limit.saturating_sub(current))
}

#[cfg(target_os = "linux")]
fn linux_cgroup_memory_headroom() -> Option<u64> {
    let membership = std::fs::read_to_string("/proc/self/cgroup").ok()?;
    let relative = membership
        .lines()
        .find_map(|line| line.strip_prefix("0::"))?
        .trim_start_matches('/');
    let relative_path = std::path::Path::new(relative);
    if relative_path
        .components()
        .any(|component| !matches!(component, std::path::Component::Normal(_)))
    {
        return None;
    }
    let root = std::path::Path::new("/sys/fs/cgroup");
    let mut paths = vec![root.to_path_buf()];
    let mut current_path = root.to_path_buf();
    for component in relative_path.components() {
        current_path.push(component.as_os_str());
        paths.push(current_path.clone());
    }

    // A parent slice can be constrained even when the leaf says `max`. The
    // tightest remaining headroom across the whole hierarchy is authoritative.
    let mut headroom: Option<u64> = None;
    for path in paths {
        let limit = std::fs::read_to_string(path.join("memory.max"));
        let current = std::fs::read_to_string(path.join("memory.current"));
        if let (Ok(limit), Ok(current)) = (limit, current) {
            if let Some(available) = cgroup_memory_headroom(&limit, &current) {
                headroom = Some(headroom.map_or(available, |old| old.min(available)));
            }
        }
    }
    headroom
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
                free = rest.split_whitespace().next()?.parse().ok()?;
            }
            if let Some(rest) = line.strip_prefix("MemAvailable:") {
                avail = rest.split_whitespace().next()?.parse().ok()?;
            }
        }
        // Prefer MemAvailable (more honest about usable memory), then apply
        // the stricter finite cgroup v2 headroom when this process is limited.
        let kb = if avail > 0 { avail } else { free };
        let host_available = kb.saturating_mul(1024);
        let effective_available = linux_cgroup_memory_headroom()
            .map(|cgroup_available| host_available.min(cgroup_available))
            .unwrap_or(host_available);
        Some((effective_available / (1024 * 1024)).min(u32::MAX as u64) as u32)
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
    fn parses_only_well_formed_announcements_from_mesh_port() {
        let addr: SocketAddr = "192.168.1.20:7642".parse().unwrap();
        let peer =
            parse_announce("POLYGONE v1 0123456789abcdef 192.168.1.20:7000 4096", addr).unwrap();
        assert_eq!(peer.node_id, "0123456789abcdef");
        assert_eq!(peer.relay, "192.168.1.20:7000");
        assert_eq!(peer.free_ram_mb, Some(4096));
    }

    #[test]
    fn rejects_malformed_or_untrusted_announcement_shapes() {
        let addr: SocketAddr = "192.168.1.20:7642".parse().unwrap();
        for line in [
            "POLYGONE v1 short 192.168.1.20:7000",
            "POLYGONE v1 0123456789abcdef not-a-relay",
            "POLYGONE v1 0123456789abcdef 192.168.1.20:0",
            "POLYGONE v1 0123456789abcdef 192.168.1.20:7000 not-a-number",
            "POLYGONE v1 0123456789abcdef 192.168.1.20:7000 4096 extra",
            "POLYGONE v2 0123456789abcdef 192.168.1.20:7000",
        ] {
            assert!(parse_announce(line, addr).is_none(), "accepted {line:?}");
        }
        let wrong_port: SocketAddr = "192.168.1.20:12345".parse().unwrap();
        assert!(
            parse_announce("POLYGONE v1 0123456789abcdef 192.168.1.20:7000", wrong_port).is_none()
        );
    }

    #[test]
    fn accepts_bracketed_ipv6_and_dns_relay_endpoints() {
        assert!(valid_relay_endpoint("[2001:db8::1]:7000"));
        assert!(valid_relay_endpoint("relay.example.org:7000"));
        assert!(!valid_relay_endpoint("[2001:db8::1]7000"));
        assert!(!valid_relay_endpoint("bad..example:7000"));
        assert!(!valid_relay_endpoint("-bad.example:7000"));
    }

    #[test]
    fn finite_cgroup_memory_reports_remaining_headroom() {
        assert_eq!(
            cgroup_memory_headroom("268435456", "134217728"),
            Some(134217728)
        );
        assert_eq!(cgroup_memory_headroom("268435456", "300000000"), Some(0));
    }

    #[test]
    fn unlimited_or_invalid_cgroup_memory_falls_back_to_host_measurement() {
        assert_eq!(cgroup_memory_headroom("max", "134217728"), None);
        assert_eq!(cgroup_memory_headroom("not-a-number", "1"), None);
    }

    #[test]
    fn announce_packet_format() {
        let p = announce_packet("abc123", "192.168.1.5:7000", Some(4096));
        assert!(p.starts_with("POLYGONE v1 abc123 192.168.1.5:7000 4096"));
        let p2 = announce_packet("abc123", "192.168.1.5:7000", None);
        assert_eq!(p2.trim(), "POLYGONE v1 abc123 192.168.1.5:7000");
    }
}
