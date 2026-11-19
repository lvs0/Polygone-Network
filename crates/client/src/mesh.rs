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
//! No internet, no server, no telemetry: the mesh is the LAN.
//!
//! # Only the interfaces we mean
//!
//! Broadcasting from `0.0.0.0` to `255.255.255.255` leaves the kernel free to
//! pick an interface — including the VPN, the container bridge or the sandbox
//! bridge. That leaks the node id and its relay address to the café network or
//! to Tailscale, while reaching no LAN peer at all (MESH.1). Announcements are
//! therefore sent from a socket **bound to the interface's own address** to that
//! interface's directed broadcast, and the loopback/VPN/container interfaces are
//! never selected. `POLYGONE_MESH_IFACE` narrows the set further; it can only
//! remove interfaces, never re-enable an excluded one.

use anyhow::Result;
use std::net::{Ipv4Addr, SocketAddr, UdpSocket};
use std::time::{Duration, Instant};

/// The mesh UDP port.
pub const MESH_PORT: u16 = 7642;

/// Comma-separated allowlist of interface names, e.g. `wlp3s0` or `eth0,wlp3s0`.
pub const ENV_MESH_IFACE: &str = "POLYGONE_MESH_IFACE";

/// Interface names never used for LAN discovery, whatever the allowlist says.
///
/// Loopback is not a LAN. VPN and container/bridge interfaces carry traffic to
/// a *different* network than the one the mesh is meant to reach. The rule is
/// deliberately fail-closed: an operator who genuinely wants to announce over a
/// VPN must first change this list in code, not flip an environment variable.
fn is_excluded_interface_name(name: &str) -> bool {
    if name == "lo" {
        return true;
    }
    const EXCLUDED_PREFIXES: [&str; 12] = [
        "tun",
        "tap",
        "wg",
        "tailscale",
        "docker",
        "br-",
        "veth",
        "virbr",
        "ve-",
        "zt",
        "utun",
        "ppp",
    ];
    EXCLUDED_PREFIXES
        .iter()
        .any(|prefix| name.starts_with(prefix))
}

/// One IPv4 interface, as reported by `ip -4 -o addr show`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Iface {
    pub name: String,
    pub addr: Ipv4Addr,
    pub prefix: u8,
    /// The `brd` reported by the tool, when it reports one.
    pub broadcast: Option<Ipv4Addr>,
}

impl Iface {
    /// Directed broadcast of this interface: `brd` when present, otherwise
    /// derived from the prefix. `/31` and `/32` have none — returning the
    /// address itself would send the announcement to this node only.
    pub fn broadcast_addr(&self) -> Option<Ipv4Addr> {
        if let Some(broadcast) = self.broadcast {
            return Some(broadcast);
        }
        if self.prefix >= 31 {
            return None;
        }
        let mask = u32::MAX << (32 - u32::from(self.prefix));
        let network = u32::from(self.addr) & mask;
        Some(Ipv4Addr::from(network | !mask))
    }
}

/// Parse `ip -4 -o addr show` output. Unknown or malformed lines are skipped:
/// a line the parser does not understand must never become an interface we
/// broadcast on.
pub fn parse_ip_addr_output(text: &str) -> Vec<Iface> {
    let mut interfaces = Vec::new();
    for line in text.lines() {
        let tokens: Vec<&str> = line.split_whitespace().collect();
        let Some(inet) = tokens.iter().position(|token| *token == "inet") else {
            continue;
        };
        if inet == 0 {
            continue;
        }
        let name = tokens[inet - 1].trim_end_matches(':');
        if name.is_empty() || name.bytes().all(|byte| byte.is_ascii_digit()) {
            continue;
        }
        let Some((addr, prefix)) = tokens.get(inet + 1).and_then(|value| value.split_once('/'))
        else {
            continue;
        };
        let (Ok(addr), Ok(prefix)) = (addr.parse::<Ipv4Addr>(), prefix.parse::<u8>()) else {
            continue;
        };
        if prefix > 32 {
            continue;
        }
        let broadcast = tokens
            .iter()
            .position(|token| *token == "brd")
            .and_then(|brd| tokens.get(brd + 1))
            .and_then(|value| value.parse::<Ipv4Addr>().ok());
        interfaces.push(Iface {
            name: name.to_string(),
            addr,
            prefix,
            broadcast,
        });
    }
    interfaces
}

/// Interfaces the mesh may use: no loopback, no link-local, no VPN or container
/// interface, then narrowed by the allowlist when one is set. An allowlist can
/// only **remove** interfaces: it never re-enables one the default rule excludes.
pub fn select_interfaces(all: &[Iface], allowlist: Option<&[String]>) -> Vec<Iface> {
    all.iter()
        .filter(|iface| {
            !iface.addr.is_loopback()
                && !iface.addr.is_link_local()
                && !is_excluded_interface_name(&iface.name)
        })
        .filter(|iface| match allowlist {
            Some(names) if !names.is_empty() => names.iter().any(|name| name == &iface.name),
            _ => true,
        })
        .cloned()
        .collect()
}

/// Allowlist from `POLYGONE_MESH_IFACE`; `None` when unset or empty.
pub fn interface_allowlist() -> Option<Vec<String>> {
    let raw = std::env::var(ENV_MESH_IFACE).ok()?;
    let names: Vec<String> = raw
        .split(',')
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(str::to_string)
        .collect();
    (!names.is_empty()).then_some(names)
}

/// IPv4 interfaces of this machine. Linux reads `ip -4 -o addr show`; any other
/// platform fails closed instead of broadcasting on an unknown interface.
pub fn system_interfaces() -> Result<Vec<Iface>> {
    #[cfg(target_os = "linux")]
    {
        let output = std::process::Command::new("ip")
            .args(["-4", "-o", "addr", "show"])
            .output()
            .map_err(|error| {
                anyhow::anyhow!("interfaces réseau illisibles (`ip` introuvable) : {error}")
            })?;
        if !output.status.success() {
            anyhow::bail!(
                "`ip -4 -o addr show` a échoué (code {}) : aucune annonce n'a été émise",
                output.status
            );
        }
        Ok(parse_ip_addr_output(&String::from_utf8_lossy(
            &output.stdout,
        )))
    }
    #[cfg(not(target_os = "linux"))]
    {
        anyhow::bail!(
            "la sélection d'interfaces n'est implémentée que sous Linux : aucune annonce n'a été émise"
        )
    }
}

/// Where the mesh is allowed to send: (interface, directed broadcast).
pub fn announce_targets() -> Result<Vec<(Iface, Ipv4Addr)>> {
    let allowlist = interface_allowlist();
    let selected = select_interfaces(&system_interfaces()?, allowlist.as_deref());
    Ok(selected
        .into_iter()
        .filter_map(|iface| iface.broadcast_addr().map(|broadcast| (iface, broadcast)))
        .collect())
}

fn no_lan_interface_error() -> anyhow::Error {
    anyhow::anyhow!(
        "aucune interface LAN autorisée : aucune annonce n'a été émise. \
         Le loopback et les interfaces VPN/containers sont toujours exclus ; \
         {ENV_MESH_IFACE} peut restreindre la liste (ex. {ENV_MESH_IFACE}=wlp3s0)"
    )
}

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
        || parts[2].len() != 32
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
    let targets = announce_targets()?;
    if targets.is_empty() {
        return Err(no_lan_interface_error());
    }

    // One ephemeral socket per interface, bound to that interface's own address:
    // the kernel then sends the PING out that interface and nowhere else.
    let mut sockets: Vec<UdpSocket> = Vec::new();
    for (iface, broadcast) in targets {
        let socket = UdpSocket::bind((iface.addr, 0))?;
        socket.set_broadcast(true)?;
        socket.set_read_timeout(Some(Duration::from_millis(200)))?;
        let ping = format!("POLYGONE v1 PING {}\n", socket.local_addr()?.port());
        let _ = socket.send_to(ping.as_bytes(), (broadcast, MESH_PORT));
        sockets.push(socket);
    }

    let mut peers: Vec<Peer> = Vec::new();
    let mut buf = [0u8; 512];
    let deadline = Instant::now() + timeout;

    while Instant::now() < deadline {
        for socket in sockets.iter() {
            if let Ok((n, addr)) = socket.recv_from(&mut buf) {
                let line = String::from_utf8_lossy(&buf[..n]);
                if let Some(peer) = parse_announce(line.trim(), addr) {
                    if !peers.iter().any(|known| known.node_id == peer.node_id) {
                        peers.push(peer);
                    }
                }
            }
        }
    }
    Ok(peers)
}

/// Announce this node on the LAN until interrupted. Sends an announce every
/// 5 s AND answers discovery PINGs immediately (so a scan finds us fast).
pub async fn announce(node_id: &str, relay: &str) -> Result<()> {
    let targets = announce_targets()?;
    if targets.is_empty() {
        return Err(no_lan_interface_error());
    }

    // Bind to each allowed interface address: the announcement leaves through
    // that interface, and the excluded ones never even see a socket.
    let mut sockets: Vec<(UdpSocket, Ipv4Addr)> = Vec::new();
    for (iface, broadcast) in targets {
        let socket = UdpSocket::bind((iface.addr, MESH_PORT))?;
        socket.set_broadcast(true)?;
        socket.set_read_timeout(Some(Duration::from_millis(200)))?;
        println!("  · interface {} ({})", iface.name, iface.addr);
        sockets.push((socket, broadcast));
    }

    println!("⬡ mesh : annonce sur le LAN (port {MESH_PORT}) — node {node_id} · relay {relay}");
    println!("  (Ctrl-C pour arrêter)");

    let mut buf = [0u8; 128];
    loop {
        // Live free RAM — the RES compute signal (ghost nodes).
        let packet = announce_packet(node_id, relay, free_ram_mb());
        for (socket, broadcast) in sockets.iter() {
            socket.send_to(packet.as_bytes(), (*broadcast, MESH_PORT))?;
        }

        // Answer PINGs for up to five seconds, then announce again.
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            for (socket, _) in sockets.iter() {
                let Ok((n, from)) = socket.recv_from(&mut buf) else {
                    continue;
                };
                let line = String::from_utf8_lossy(&buf[..n]);
                let parts: Vec<&str> = line.split_whitespace().collect();
                // "POLYGONE v1 PING <port>" — reply straight to the scanner,
                // from the interface that received the PING.
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
        }
    }
}

/// Remaining memory under a finite cgroup v2 memory.max. `None` means no
/// finite limit or the cgroup measurement is unavailable.
#[cfg(any(target_os = "linux", test))]
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

    fn iface(name: &str, addr: &str, prefix: u8, brd: Option<&str>) -> Iface {
        Iface {
            name: name.to_string(),
            addr: addr.parse().expect("adresse IPv4 de test"),
            prefix,
            broadcast: brd.map(|value| value.parse().expect("broadcast IPv4 de test")),
        }
    }

    #[test]
    fn parses_ip_output_and_derives_the_directed_broadcast() {
        let text = "1: lo    inet 127.0.0.1/8 scope host lo\n\
                    3: wlp3s0    inet 192.168.1.104/24 brd 192.168.1.255 scope global wlp3s0\n\
                    4: tailscale0    inet 100.65.62.54/32 scope global tailscale0";
        let parsed = parse_ip_addr_output(text);
        assert_eq!(parsed.len(), 3);
        assert_eq!(parsed[1].name, "wlp3s0");
        assert_eq!(
            parsed[1].broadcast_addr(),
            Some("192.168.1.255".parse().unwrap())
        );
        assert_eq!(parsed[2].broadcast_addr(), None, "/32 has no broadcast");
        assert_eq!(
            iface("eth0", "10.0.0.5", 24, None).broadcast_addr(),
            Some("10.0.0.255".parse().unwrap()),
            "broadcast must be derived when `ip` omits brd"
        );
    }

    #[test]
    fn vpn_loopback_and_container_interfaces_are_never_announced() {
        let all = vec![
            iface("lo", "127.0.0.1", 8, None),
            iface("wlp3s0", "192.168.1.104", 24, Some("192.168.1.255")),
            iface("tailscale0", "100.65.62.54", 32, None),
            iface("docker0", "172.17.0.1", 16, Some("172.17.255.255")),
            iface("ve-sandbox", "10.77.0.1", 24, None),
            iface("eth0", "169.254.3.4", 16, None),
        ];
        let names: Vec<String> = select_interfaces(&all, None)
            .into_iter()
            .map(|selected| selected.name)
            .collect();
        assert_eq!(names, vec!["wlp3s0"]);
    }

    #[test]
    fn allowlist_can_only_narrow_and_never_resurrects_an_excluded_interface() {
        let all = vec![
            iface("wlp3s0", "192.168.1.104", 24, Some("192.168.1.255")),
            iface("eth0", "10.0.0.5", 24, None),
            iface("tailscale0", "100.65.62.54", 32, None),
        ];
        let only_eth0 = vec!["eth0".to_string()];
        let names: Vec<String> = select_interfaces(&all, Some(&only_eth0))
            .into_iter()
            .map(|selected| selected.name)
            .collect();
        assert_eq!(names, vec!["eth0"]);

        // Naming the VPN interface explicitly is not a way to get it back.
        let vpn = vec!["tailscale0".to_string()];
        assert!(select_interfaces(&all, Some(&vpn)).is_empty());
    }

    /// MESH.1 at packet level: the selection never yields an excluded
    /// interface, so the mesh has no socket to announce from there. The first
    /// half is the positive witness — the listener does see datagrams — so the
    /// property is not passing because the harness is deaf.
    #[test]
    fn no_announcement_can_reach_an_excluded_interface() {
        let listener = UdpSocket::bind(("127.0.0.1", 0)).expect("listener loopback");
        listener
            .set_read_timeout(Some(Duration::from_millis(300)))
            .expect("délai de lecture");
        let sender = UdpSocket::bind(("127.0.0.1", 0)).expect("sender loopback");
        sender
            .send_to(b"POLYGONE v1 PING 1\n", listener.local_addr().unwrap())
            .expect("envoi du témoin");
        let mut buf = [0u8; 64];
        assert!(
            listener.recv_from(&mut buf).is_ok(),
            "the listener must receive the witness datagram"
        );

        let all = vec![iface("lo", "127.0.0.1", 8, None)];
        assert!(select_interfaces(&all, None).is_empty());
        assert!(
            select_interfaces(&all, Some(&["lo".to_string()])).is_empty(),
            "an allowlist must not make loopback announceable"
        );
    }

    #[test]
    fn parses_only_well_formed_announcements_from_mesh_port() {
        let addr: SocketAddr = "192.168.1.20:7642".parse().unwrap();
        let peer = parse_announce(
            "POLYGONE v1 0123456789abcdef0123456789abcdef 192.168.1.20:7000 4096",
            addr,
        )
        .unwrap();
        assert_eq!(peer.node_id, "0123456789abcdef0123456789abcdef");
        assert_eq!(peer.relay, "192.168.1.20:7000");
        assert_eq!(peer.free_ram_mb, Some(4096));
    }

    #[test]
    fn rejects_malformed_or_untrusted_announcement_shapes() {
        let addr: SocketAddr = "192.168.1.20:7642".parse().unwrap();
        for line in [
            "POLYGONE v1 short 192.168.1.20:7000",
            "POLYGONE v1 0123456789abcdef0123456789abcdef not-a-relay",
            "POLYGONE v1 0123456789abcdef0123456789abcdef 192.168.1.20:0",
            "POLYGONE v1 0123456789abcdef0123456789abcdef 192.168.1.20:7000 not-a-number",
            "POLYGONE v1 0123456789abcdef0123456789abcdef 192.168.1.20:7000 4096 extra",
            "POLYGONE v2 0123456789abcdef0123456789abcdef 192.168.1.20:7000",
        ] {
            assert!(parse_announce(line, addr).is_none(), "accepted {line:?}");
        }
        let wrong_port: SocketAddr = "192.168.1.20:12345".parse().unwrap();
        assert!(parse_announce(
            "POLYGONE v1 0123456789abcdef0123456789abcdef 192.168.1.20:7000",
            wrong_port
        )
        .is_none());
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
