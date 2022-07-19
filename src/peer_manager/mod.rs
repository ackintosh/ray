use hashset_delay::HashSetDelay;
use libp2p::{Multiaddr, PeerId};
use smallvec::{smallvec, SmallVec};
use std::collections::HashMap;
use std::time::Duration;
use tracing::info;

pub(crate) mod behaviour;

// The heartbeat performs regular updates such as updating reputations and performing discovery
// requests. This defines the interval in seconds.
const HEARTBEAT_INTERVAL: u64 = 30;

// ////////////////////////////////////////////////////////
// Public events sent by PeerManager module
// ////////////////////////////////////////////////////////

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

// ////////////////////////////////////////////////////////
// PeerManager
// ////////////////////////////////////////////////////////

pub(crate) struct PeerManager {
    peers: HashMap<PeerId, Multiaddr>,
    events: SmallVec<[PeerManagerEvent; 10]>,
    // Target number of peers to connect to.
    target_peers_count: usize,
    // The heartbeat interval to perform routine maintenance.
    heartbeat: tokio::time::Interval,
    // A collection of peers awaiting to be Status'd.
    status_peers: HashSetDelay<PeerId>,
}

impl PeerManager {
    pub(crate) fn new(target_peers_count: usize) -> Self {
        // Set up the peer manager heartbeat interval
        let heartbeat = tokio::time::interval(tokio::time::Duration::from_secs(HEARTBEAT_INTERVAL));

        // NOTE: The time in seconds between re-status's peers. Hardcoding this for now.
        let status_interval = Duration::from_secs(300);

        Self {
            peers: HashMap::new(),
            events: smallvec![],
            target_peers_count,
            heartbeat,
            status_peers: HashSetDelay::new(status_interval),
        }
    }

    pub(crate) fn need_more_peers(&self) -> bool {
        info!("Current peers count: {}", self.peers.len());
        self.peers.len() < self.target_peers_count
    }

    // A STATUS message has been received from a peer. This resets the status timer.
    pub(crate) fn statusd_peer(&mut self, peer_id: PeerId) {
        self.status_peers.insert(peer_id);
    }
}
