//! Unified P2P network layer for Polygone.
//!
//! This module provides the complete peer-to-peer networking infrastructure
//! for the Polygone ephemeral privacy network, built on libp2p.
//!
//! ## Features
//!
//! - **Kademlia DHT**: Peer discovery and routing
//! - **GossipSub**: Real-time topology announcements
//! - **Request-Response**: Direct peer communication for chunks/tensors
//! - **Identify**: Protocol version and capability exchange
//! - **NAT Traversal**: Relay and hole-punching support
//!
//! ## Architecture
//!
//! ```text
//! Application (Drive/Petals/Hide)
//!              │
//!              ▼
//!    ┌─────────────────┐
//!    │    P2pNode      │
//!    │  (orchestration) │
//!    └────────┬────────┘
//!             │
//!    ┌────────▼────────┐
//!    │  libp2p Swarm   │
//!    │  ┌───────────┐  │
//!    │  │ Kademlia  │  │
//!    │  │ GossipSub │  │
//!    │  │ Req-Resp  │  │
//!    │  │ Identify  │  │
//!    │  └───────────┘  │
//!    └─────────────────┘
//! ```

use libp2p::futures::StreamExt;
pub use libp2p::Multiaddr;
pub use libp2p::PeerId;
use libp2p::SwarmBuilder;
use libp2p::{
    autonat, dcutr,
    gossipsub::{self, IdentTopic},
    identify::{self, Behaviour as Identify, Config as IdentifyConfig},
    identity::Keypair,
    kad::{
        self, store::MemoryStore, Behaviour as Kademlia, Config as KademliaConfig, GetRecordOk,
        Mode, QueryResult,
    },
    mdns, noise, ping, relay,
    request_response::{
        self, cbor::Behaviour as RequestResponse, OutboundRequestId, ProtocolSupport, ResponseChannel,
    },
    swarm::{NetworkBehaviour, StreamProtocol, Swarm, SwarmEvent},
    tcp, yamux,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;
use tokio::sync::{mpsc, oneshot};
use tracing::{debug, error, info, warn};

use crate::{
    error::{PolyResult, PolygoneError},
    network::{NodeId, Topology},
    protocol::SessionId,
};

// ── Re-exports ──────────────────────────────────────────────────────────────

// ── Constants ─────────────────────────────────────────────────────────────────

/// Protocol version for Polygone network
pub const POLYGONE_PROTOCOL_VERSION: &str = "/polygone/1.0.0";

/// Kademlia protocol name
pub const KADEMLIA_PROTOCOL_NAME: &str = "/polygone/kad/1.0.0";

/// Request-response protocol for direct messaging
pub const REQUEST_RESPONSE_PROTOCOL: &str = "/polygone/rr/1.0.0";

/// Gossip topic for topology announcements
pub const TOPOLOGY_TOPIC: &str = "polygone-topology";

/// Default bootstrap nodes (placeholder - replace with actual addresses)
pub const DEFAULT_BOOTSTRAP_NODES: &[&str] = &[
    "/dns4/bootstrap1.polygone.network/tcp/4001",
    "/dns4/bootstrap2.polygone.network/tcp/4001",
];

// ── Configuration ───────────────────────────────────────────────────────────

/// Configuration for P2P node initialization
#[derive(Debug, Clone)]
pub struct P2pConfig {
    /// Path to persistent identity (None for ephemeral)
    pub identity_path: Option<std::path::PathBuf>,
    /// Bootstrap nodes to connect to
    pub bootstrap_nodes: Vec<Multiaddr>,
    /// Addresses to listen on
    pub listen_addrs: Vec<Multiaddr>,
    /// DHT record TTL
    pub record_ttl: Duration,
    /// Connection idle timeout
    pub connection_timeout: Duration,
    /// Maximum concurrent connections
    pub max_connections: u32,
    /// Enable mDNS for local discovery
    pub enable_mdns: bool,
    /// Enable relay server mode
    pub relay_server: bool,
    /// Enable AutoNAT for external address discovery
    pub enable_autonat: bool,
}

impl Default for P2pConfig {
    fn default() -> Self {
        Self {
            identity_path: None,
            bootstrap_nodes: DEFAULT_BOOTSTRAP_NODES
                .iter()
                .filter_map(|s| s.parse().ok())
                .collect(),
            listen_addrs: vec!["/ip4/0.0.0.0/tcp/0".parse().unwrap()],
            record_ttl: Duration::from_secs(30),
            connection_timeout: Duration::from_secs(60),
            max_connections: 100,
            enable_mdns: true,
            relay_server: false,
            enable_autonat: true,
        }
    }
}

// ── Network Messages ──────────────────────────────────────────────────────────

/// Request types for direct peer-to-peer communication
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PolygoneRequest {
    /// Request a file chunk fragment (Drive)
    DriveChunk {
        file_id: [u8; 32],
        chunk_index: u32,
        fragment_index: u8,
    },
    /// Store a file chunk fragment (Drive)
    DriveStore {
        file_id: [u8; 32],
        chunk_index: u32,
        fragment_index: u8,
        data: Vec<u8>,
    },
    /// Request inference on tensor segment (Petals)
    PetalsInfer {
        session_id: [u8; 16],
        layer_start: u32,
        layer_end: u32,
        tensor_data: Vec<u8>,
        dims: Vec<usize>,
    },
    /// Open tunnel session (Hide)
    HideTunnel {
        session_id: [u8; 16],
        target_addr: String,
    },
    /// Forward tunnel data (Hide)
    HideData { session_id: [u8; 16], data: Vec<u8> },
}

/// Response types for direct peer-to-peer communication
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PolygoneResponse {
    /// Chunk data response (Drive)
    DriveChunk { success: bool, data: Vec<u8> },
    /// Store acknowledgment (Drive)
    DriveStore { success: bool },
    /// Inference result (Petals)
    PetalsInfer {
        success: bool,
        tensor_data: Option<Vec<u8>>,
        dims: Option<Vec<usize>>,
    },
    /// Tunnel establishment response (Hide)
    HideTunnel {
        success: bool,
        error: Option<String>,
    },
    /// Tunnel data response (Hide)
    HideData { data: Vec<u8> },
}

/// Gossip message for network-wide announcements
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GossipMessage {
    /// Announce ephemeral topology for a session
    TopologyAnnounce {
        session_id: SessionId,
        nodes: Vec<NodeId>,
        ttl_seconds: u32,
    },
    /// Announce relay capabilities (Petals)
    CapabilitiesAnnounce {
        peer_id: Vec<u8>,
        capabilities: Vec<Capability>,
        ttl_seconds: u32,
    },
    /// Heartbeat to maintain presence
    Heartbeat { peer_id: Vec<u8>, timestamp: u64 },
}

/// Peer capabilities for service discovery
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Capability {
    /// Drive storage provider
    DriveStorage { max_gb: u32 },
    /// Petals compute provider with layer range
    PetalsCompute { layer_start: u32, layer_end: u32 },
    /// Hide exit node
    HideExit,
    /// Relay node for NAT traversal
    Relay,
}

// ── Network Behaviour ────────────────────────────────────────────────────────

/// Combined network behaviour for Polygone
#[derive(NetworkBehaviour)]
pub struct PolygoneBehaviour {
    /// Kademlia DHT for peer and content discovery
    pub kademlia: Kademlia<MemoryStore>,
    /// GossipSub for real-time announcements
    pub gossipsub: gossipsub::Behaviour,
    /// Identify for protocol version exchange
    pub identify: Identify,
    /// Request-response for direct messaging
    pub request_response: RequestResponse<PolygoneRequest, PolygoneResponse>,
    /// mDNS for local peer discovery
    pub mdns: mdns::tokio::Behaviour,
    /// Relay for NAT traversal
    pub relay: relay::Behaviour,
    /// AutoNAT for external address detection
    pub autonat: autonat::Behaviour,
    /// DCUtR for hole punching
    pub dcutr: dcutr::Behaviour,
    /// Ping for connectivity checks
    pub ping: ping::Behaviour,
}

impl PolygoneBehaviour {
    fn new(keypair: &Keypair, config: &P2pConfig) -> anyhow::Result<Self> {
        let local_peer_id = PeerId::from(keypair.public());

        // Kademlia DHT
        let store = MemoryStore::new(local_peer_id);
        let mut kad_config = KademliaConfig::default();
        kad_config.set_protocol_names(vec![StreamProtocol::new(KADEMLIA_PROTOCOL_NAME)]);
        kad_config.set_record_ttl(Some(config.record_ttl));
        kad_config.set_provider_record_ttl(Some(config.record_ttl));
        kad_config.set_query_timeout(Duration::from_secs(10));
        let mut kademlia = Kademlia::with_config(local_peer_id, store, kad_config);
        kademlia.set_mode(Some(Mode::Server));

        // GossipSub
        let gossipsub_config = gossipsub::ConfigBuilder::default()
            .heartbeat_interval(Duration::from_secs(5))
            .validation_mode(gossipsub::ValidationMode::Strict)
            .build()?;
        let gossipsub = gossipsub::Behaviour::new(
            gossipsub::MessageAuthenticity::Signed(keypair.clone()),
            gossipsub_config,
        )
        .map_err(|e| anyhow::anyhow!("Gossipsub error: {}", e))?;

        // Identify
        let identify_config =
            IdentifyConfig::new(POLYGONE_PROTOCOL_VERSION.to_string(), keypair.public())
                .with_push_listen_addr_updates(true);
        let identify = Identify::new(identify_config);

        // Request-Response
        let protocols = vec![(
            StreamProtocol::new(REQUEST_RESPONSE_PROTOCOL),
            ProtocolSupport::Full,
        )];
        let rr_config =
            request_response::Config::default().with_request_timeout(Duration::from_secs(30));
        let request_response = RequestResponse::new(protocols, rr_config);

        // mDNS
        let mdns = mdns::tokio::Behaviour::new(mdns::Config::default(), local_peer_id)?;

        // Relay
        let relay = relay::Behaviour::new(local_peer_id, Default::default());

        // AutoNAT
        let autonat = autonat::Behaviour::new(local_peer_id, Default::default());

        // DCUtR
        let dcutr = dcutr::Behaviour::new(local_peer_id);

        // Ping
        let ping = ping::Behaviour::new(ping::Config::new());

        Ok(Self {
            kademlia,
            gossipsub,
            identify,
            request_response,
            mdns,
            relay,
            autonat,
            dcutr,
            ping,
        })
    }
}

// ── P2P Node ──────────────────────────────────────────────────────────────────

/// Events emitted by the P2P node
#[derive(Debug)]
pub enum NetworkEvent {
    /// New listening address
    NewListenAddr { address: Multiaddr },
    /// New peer connected
    PeerConnected { peer_id: PeerId },
    /// Peer disconnected
    PeerDisconnected { peer_id: PeerId },
    /// Incoming request from peer
    IncomingRequest {
        peer_id: PeerId,
        request: PolygoneRequest,
        channel: ResponseChannel<PolygoneResponse>,
    },
    /// Gossip message received
    GossipReceived {
        topic: String,
        message: GossipMessage,
        source: Option<PeerId>,
    },
    /// DHT record found
    DhtRecordFound { key: Vec<u8>, value: Vec<u8> },
    /// External address discovered (AutoNAT)
    ExternalAddrDiscovered { address: Multiaddr },
}

/// The main P2P node for Polygone network participation
pub struct P2pNode {
    /// libp2p swarm
    swarm: Swarm<PolygoneBehaviour>,
    /// Configuration
    config: P2pConfig,
    /// Event channel for application layer
    event_tx: mpsc::Sender<NetworkEvent>,
    /// Pending DHT queries
    pending_queries:
        HashMap<kad::QueryId, oneshot::Sender<std::result::Result<Vec<u8>, PolygoneError>>>,
    /// Pending request-response channels, keyed by outbound request ID.
    pending_responses: HashMap<OutboundRequestId, oneshot::Sender<PolygoneResponse>>,
    /// Topology cache (session_id -> topology)
    topology_cache: HashMap<SessionId, CachedTopology>,
    /// Gossip topic subscriptions
    subscribed_topics: Vec<IdentTopic>,
}

#[derive(Debug, Clone)]
struct CachedTopology {
    topology: Topology,
    expires_at: std::time::Instant,
}

// (Removed: RequestId alias — responses are now keyed by OutboundRequestId)

impl P2pNode {
    /// Create a new P2P node with the given configuration
    pub async fn new(config: P2pConfig) -> PolyResult<(Self, mpsc::Receiver<NetworkEvent>)> {
        let (event_tx, event_rx) = mpsc::channel(100);

        // Load or generate identity
        let keypair = if let Some(ref path) = config.identity_path {
            load_or_generate_identity(path).map_err(|e| PolygoneError::Network(e.to_string()))?
        } else {
            Keypair::generate_ed25519()
        };

        let local_peer_id = PeerId::from(keypair.public());
        info!("P2P Node created with PeerID: {}", local_peer_id);

        // Build swarm
        let swarm = build_swarm(keypair, &config)
            .await
            .map_err(|e| PolygoneError::Network(e.to_string()))?;

        let node = Self {
            swarm,
            config,
            event_tx,
            pending_queries: HashMap::new(),
            pending_responses: HashMap::new(),
            topology_cache: HashMap::new(),
            subscribed_topics: Vec::new(),
        };

        Ok((node, event_rx))
    }

    /// Get the local PeerId
    pub fn peer_id(&self) -> PeerId {
        *self.swarm.local_peer_id()
    }

    /// Start listening on the configured addresses
    pub async fn start_listening(&mut self) -> PolyResult<Vec<Multiaddr>> {
        let mut bound_addrs = Vec::new();

        for addr in &self.config.listen_addrs {
            match self.swarm.listen_on(addr.clone()) {
                Ok(_) => {
                    info!("Listening on {}", addr);
                    bound_addrs.push(addr.clone());
                }
                Err(e) => {
                    warn!("Failed to listen on {}: {}", addr, e);
                }
            }
        }

        if bound_addrs.is_empty() {
            return Err(PolygoneError::Network(
                "Failed to bind to any address".to_string(),
            ));
        }

        Ok(bound_addrs)
    }

    /// Connect to bootstrap nodes
    pub async fn bootstrap(&mut self) -> PolyResult<()> {
        for addr in &self.config.bootstrap_nodes {
            info!("Dialing bootstrap node: {}", addr);
            if let Err(e) = self.swarm.dial(addr.clone()) {
                warn!("Failed to dial bootstrap {}: {}", addr, e);
            }
        }

        // Bootstrap Kademlia routing table
        self.swarm.behaviour_mut().kademlia.bootstrap().map_err(|e| PolygoneError::Network(e.to_string()))?;

        Ok(())
    }

    /// Subscribe to a gossip topic
    pub fn subscribe_topic(&mut self, topic_name: &str) -> PolyResult<()> {
        let topic = IdentTopic::new(topic_name);
        self.swarm.behaviour_mut().gossipsub.subscribe(&topic).map_err(|e| PolygoneError::Network(e.to_string()))?;
        self.subscribed_topics.push(topic);
        info!("Subscribed to topic: {}", topic_name);
        Ok(())
    }

    /// Publish a gossip message
    pub fn publish_gossip(&mut self, topic: &str, message: GossipMessage) -> PolyResult<()> {
        let topic = IdentTopic::new(topic);
        let data = bincode::serialize(&message)
            .map_err(|e| PolygoneError::Serialization(e.to_string()))?;

        self.swarm.behaviour_mut().gossipsub.publish(topic, data).map_err(|e| PolygoneError::Network(e.to_string()))?;
        Ok(())
    }

    /// Announce a session topology to the network
    pub fn announce_topology(
        &mut self,
        session_id: SessionId,
        topology: &Topology,
    ) -> PolyResult<()> {
        let message = GossipMessage::TopologyAnnounce {
            session_id,
            nodes: topology.nodes.clone(),
            ttl_seconds: 30,
        };

        self.publish_gossip(TOPOLOGY_TOPIC, message)?;

        // Also cache locally
        self.topology_cache.insert(
            session_id,
            CachedTopology {
                topology: topology.clone(),
                expires_at: std::time::Instant::now() + Duration::from_secs(30),
            },
        );

        Ok(())
    }

    /// Put a record in the DHT
    pub fn put_dht_record(&mut self, key: Vec<u8>, value: Vec<u8>) -> PolyResult<kad::QueryId> {
        let record = kad::Record {
            key: kad::RecordKey::new(&key),
            value,
            publisher: None,
            expires: Some(std::time::Instant::now() + self.config.record_ttl),
        };

        let query_id = self
            .swarm
            .behaviour_mut()
            .kademlia
            .put_record(record, kad::Quorum::One).map_err(|e| PolygoneError::Network(e.to_string()))?;
        Ok(query_id)
    }

    /// Get a record from the DHT
    pub fn get_dht_record(
        &mut self,
        key: Vec<u8>,
    ) -> oneshot::Receiver<std::result::Result<Vec<u8>, PolygoneError>> {
        let (tx, rx) = oneshot::channel();
        let query_id = self
            .swarm
            .behaviour_mut()
            .kademlia
            .get_record(kad::RecordKey::new(&key));
        self.pending_queries.insert(query_id, tx);
        rx
    }

    /// Send a direct request to a peer and return a receiver for the response.
    ///
    /// The returned `oneshot::Receiver` will resolve when the peer sends back
    /// a response (or the request fails). The caller should `.await` the receiver.
    pub fn send_request(
        &mut self,
        peer_id: PeerId,
        request: PolygoneRequest,
    ) -> oneshot::Receiver<PolygoneResponse> {
        let (tx, rx) = oneshot::channel();
        let request_id = self
            .swarm
            .behaviour_mut()
            .request_response
            .send_request(&peer_id, request);
        // Track the pending response channel so the event handler can forward replies.
        self.pending_responses.insert(request_id, tx);
        rx
    }

    /// Respond to an incoming request
    pub fn send_response(
        &mut self,
        channel: ResponseChannel<PolygoneResponse>,
        response: PolygoneResponse,
    ) -> PolyResult<()> {
        self.swarm
            .behaviour_mut()
            .request_response
            .send_response(channel, response)
            .map_err(|_| PolygoneError::Network("send_response failed".into()))
    }

    /// Run the event loop (this drives the swarm)
    pub async fn run(mut self) -> PolyResult<()> {
        info!("P2P event loop started");

        loop {
            tokio::select! {
                event = self.swarm.select_next_some() => {
                    self.handle_swarm_event(event).await?;
                }
            }
        }
    }

    /// Handle a swarm event
    async fn handle_swarm_event(
        &mut self,
        event: SwarmEvent<PolygoneBehaviourEvent>,
    ) -> PolyResult<()> {
        match event {
            SwarmEvent::NewListenAddr { address, .. } => {
                info!("New listen address: {}", address);
                let _ = self
                    .event_tx
                    .send(NetworkEvent::NewListenAddr { address })
                    .await;
            }
            SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                debug!("Connected to peer: {}", peer_id);
                let _ = self
                    .event_tx
                    .send(NetworkEvent::PeerConnected { peer_id })
                    .await;
            }
            SwarmEvent::ConnectionClosed { peer_id, .. } => {
                debug!("Disconnected from peer: {}", peer_id);
                let _ = self
                    .event_tx
                    .send(NetworkEvent::PeerDisconnected { peer_id })
                    .await;
            }
            SwarmEvent::Behaviour(behaviour_event) => {
                self.handle_behaviour_event(behaviour_event).await?;
            }
            _ => {}
        }
        Ok(())
    }

    /// Handle behaviour-specific events
    async fn handle_behaviour_event(&mut self, event: PolygoneBehaviourEvent) -> PolyResult<()> {
        match event {
            PolygoneBehaviourEvent::Kademlia(kad::Event::OutboundQueryProgressed {
                result,
                id,
                ..
            }) => match result {
                QueryResult::GetRecord(Ok(GetRecordOk::FoundRecord(record))) => {
                    let value = record.record.value.clone();
                    if let Some(tx) = self.pending_queries.remove(&id) {
                        let _ = tx.send(Ok(value.clone()));
                    }
                    let _ = self
                        .event_tx
                        .send(NetworkEvent::DhtRecordFound {
                            key: record.record.key.as_ref().to_vec(),
                            value,
                        })
                        .await;
                }
                QueryResult::GetRecord(Err(e)) => {
                    if let Some(tx) = self.pending_queries.remove(&id) {
                        let _ = tx.send(Err(PolygoneError::Network(e.to_string())));
                    }
                }
                _ => {}
            },
PolygoneBehaviourEvent::RequestResponse(request_response::Event::Message {
                peer,
                message,
                ..
            }) => {
                match message {
                    request_response::Message::Request {
                        request, channel, ..
                    } => {
                        let _local_peer_id = *self.swarm.local_peer_id();
                        let _ = self
                            .event_tx
                            .send(NetworkEvent::IncomingRequest {
                                peer_id: peer,
                                request,
                                channel,
                            })
                            .await;
                    }
                    request_response::Message::Response { response, request_id, .. } => {
                        if let Some(tx) = self.pending_responses.remove(&request_id) {
                            let _ = tx.send(response);
                        } else {
                            debug!("Received response for untracked request: {peer}");
                        }
                    }
                }
            }
            PolygoneBehaviourEvent::Gossipsub(gossipsub::Event::Message { message, .. }) => {
                if let Ok(gossip_msg) = bincode::deserialize::<GossipMessage>(&message.data) {
                    let topic =
                        String::from_utf8_lossy(message.topic.as_str().as_bytes()).to_string();
                    let _ = self
                        .event_tx
                        .send(NetworkEvent::GossipReceived {
                            topic,
                            message: gossip_msg,
                            source: message.source,
                        })
                        .await;
                }
            }
            PolygoneBehaviourEvent::Identify(identify::Event::Received {
                peer_id, info, ..
            }) => {
                debug!("Identified peer {}: {:?}", peer_id, info);
                // Add observed addresses to Kademlia
                for addr in info.listen_addrs {
                    self.swarm
                        .behaviour_mut()
                        .kademlia
                        .add_address(&peer_id, addr);
                }
            }
            PolygoneBehaviourEvent::Autonat(autonat::Event::StatusChanged { old, new }) => {
                info!("AutoNAT status changed: {:?} → {:?}", old, new);
            }
            _ => {}
        }
        Ok(())
    }

    /// Get cached topology for a session
    pub fn get_cached_topology(&self, session_id: &SessionId) -> Option<&Topology> {
        self.topology_cache.get(session_id).map(|c| &c.topology)
    }

    /// Clean expired topology cache entries
    pub fn clean_cache(&mut self) {
        let now = std::time::Instant::now();
        self.topology_cache.retain(|_, v| v.expires_at > now);
    }
}

// ── Helper Functions ──────────────────────────────────────────────────────────

/// Load or generate a persistent libp2p identity
pub fn load_or_generate_identity(path: &Path) -> anyhow::Result<Keypair> {
    use std::fs;

    if path.exists() {
        let bytes = fs::read(path)?;
        let keypair = Keypair::from_protobuf_encoding(&bytes)
            .map_err(|e| anyhow::anyhow!("Failed to decode identity: {}", e))?;
        info!("Loaded persistent identity from {}", path.display());
        Ok(keypair)
    } else {
        let keypair = Keypair::generate_ed25519();
        let bytes = keypair.to_protobuf_encoding()?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, bytes)?;
        info!("Generated and saved new identity to {}", path.display());
        Ok(keypair)
    }
}

/// Build the libp2p swarm with all behaviours
pub async fn build_swarm(
    keypair: Keypair,
    config: &P2pConfig,
) -> anyhow::Result<Swarm<PolygoneBehaviour>> {
    let behaviour = PolygoneBehaviour::new(&keypair, config)?;

    let swarm = SwarmBuilder::with_existing_identity(keypair)
        .with_tokio()
        .with_tcp(
            tcp::Config::default(),
            noise::Config::new,
            yamux::Config::default,
        )?
        .with_dns()?
        .with_behaviour(|_| behaviour)?
        .with_swarm_config(|c| c.with_idle_connection_timeout(config.connection_timeout))
        .build();

    Ok(swarm)
}

// ── Bootstrap Node ─────────────────────────────────────────────────────────────

/// Run a bootstrap node (dedicated DHT bootstrap).
///
/// This node does not participate in any sessions — it only:
/// 1. Listens for incoming connections
/// 2. Subscribes to topology gossip
/// 3. Logs peer connections and DHT activity
///
/// # Errors
/// Returns `Err` if the node fails to start listening or encounters
/// a fatal swarm error. Individual peer/dial failures are logged but
/// do not terminate the node.
pub async fn run_bootstrap_node(config: P2pConfig) -> PolyResult<()> {
    let (mut node, mut event_rx) = P2pNode::new(config).await?;

    info!("⬡ POLYGONE BOOTSTRAP NODE");
    info!("  PeerID: {}", node.peer_id());

    // Start listening
    let addrs = node.start_listening().await?;
    for addr in &addrs {
        info!("  Listening on: {}", addr);
    }

    // Subscribe to topology announcements
    node.subscribe_topic(TOPOLOGY_TOPIC)?;
    info!("  Subscribed to topic: {}", TOPOLOGY_TOPIC);

    // Spawn event handler
    let event_handler = tokio::spawn(async move {
        let mut peer_count: u64 = 0;
        while let Some(event) = event_rx.recv().await {
            match event {
                NetworkEvent::PeerConnected { peer_id } => {
                    peer_count += 1;
                    info!("Bootstrap: Peer connected: {} (total: {})", peer_id, peer_count);
                }
                NetworkEvent::PeerDisconnected { peer_id } => {
                    debug!("Bootstrap: Peer disconnected: {}", peer_id);
                }
                NetworkEvent::GossipReceived {
                    topic,
                    message,
                    source,
                } => {
                    debug!(
                        "Bootstrap: Gossip on {} from {:?}: {:?}",
                        topic, source, message
                    );
                }
                NetworkEvent::IncomingRequest {
                    peer_id,
                    request: _,
                    channel: _,
                } => {
                    debug!("Bootstrap: Incoming request from {}", peer_id);
                }
                NetworkEvent::DhtRecordFound { key, value } => {
                    debug!(
                        "Bootstrap: DHT record found key={:?} value_len={}",
                        key,
                        value.len()
                    );
                }
                NetworkEvent::NewListenAddr { address } => {
                    debug!("Bootstrap: New listen address: {}", address);
                }
                NetworkEvent::ExternalAddrDiscovered { address } => {
                    info!("Bootstrap: External address discovered: {}", address);
                }
            }
        }
        info!("Bootstrap event handler shutting down");
    });

    // Run the node
    let node_future = node.run();

    tokio::select! {
        _ = event_handler => {
            warn!("Bootstrap event handler terminated unexpectedly");
        }
        result = node_future => {
            if let Err(e) = result {
                error!("Bootstrap node error: {}", e);
                return Err(e);
            }
        }
    }

    Ok(())
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_p2p_config_default() {
        let config = P2pConfig::default();
        assert!(!config.bootstrap_nodes.is_empty());
        assert!(!config.listen_addrs.is_empty());
    }

    #[tokio::test]
    async fn test_gossip_message_serialization() {
        let msg = GossipMessage::Heartbeat {
            peer_id: vec![1, 2, 3],
            timestamp: 1234567890,
        };
        let bytes = bincode::serialize(&msg).unwrap();
        let decoded: GossipMessage = bincode::deserialize(&bytes).unwrap();

        match decoded {
            GossipMessage::Heartbeat { peer_id, timestamp } => {
                assert_eq!(peer_id, vec![1, 2, 3]);
                assert_eq!(timestamp, 1234567890);
            }
            _ => panic!("Wrong message type"),
        }
    }
}
