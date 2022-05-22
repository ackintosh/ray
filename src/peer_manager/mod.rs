use libp2p::{Multiaddr, PeerId};
use smallvec::{smallvec, SmallVec};
use std::collections::HashMap;
use tracing::info;

pub(crate) mod behaviour;

// The heartbeat performs regular updates such as updating reputations and performing discovery
// requests. This defines the interval in seconds.
const HEARTBEAT_INTERVAL: u64 = 30;

pub(crate) struct PeerManager {
    peers: HashMap<PeerId, Multiaddr>,
    events: SmallVec<[PeerManagerEvent; 10]>,
    // Target number of peers to connect to.
    target_peers_count: usize,
    // The heartbeat interval to perform routine maintenance.
    heartbeat: tokio::time::Interval,
}

/// The events that the `PeerManager` emits to `BehaviourComposer`.
#[derive(Debug)]
pub(crate) enum PeerManagerEvent {
    /// A peer has dialed us.
    PeerConnectedIncoming(PeerId),
    /// A peer has been dialed.
    PeerConnectedOutgoing(PeerId),
    /// Request the behaviour to discover more peers.
    NeedMorePeers,
}

impl PeerManager {
    pub(crate) fn new(target_peers_count: usize) -> Self {
        // Set up the peer manager heartbeat interval
        let heartbeat = tokio::time::interval(tokio::time::Duration::from_secs(HEARTBEAT_INTERVAL));

        Self {
            peers: HashMap::new(),
            events: smallvec![],
            target_peers_count,
            heartbeat,
        }
    }

    pub(crate) fn need_more_peers(&self) -> bool {
        info!("Current peers count: {}", self.peers.len());
        self.peers.len() < self.target_peers_count
    }
}
