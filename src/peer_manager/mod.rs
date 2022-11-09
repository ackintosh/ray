use crate::peer_db::{ConnectionStatus, SyncStatus};
use crate::PeerDB;
use delay_map::HashSetDelay;
use libp2p::PeerId;
use parking_lot::RwLock;
use smallvec::{smallvec, SmallVec};
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Duration;
use tracing::{info, trace};

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
    /// Request to send a STATUS to a peer.
    SendStatus(PeerId),
    /// The peer should be disconnected.
    DisconnectPeer(PeerId, lighthouse_network::rpc::GoodbyeReason),
}

// ////////////////////////////////////////////////////////
// PeerManager
// ////////////////////////////////////////////////////////

pub(crate) struct PeerManager {
    peer_db: Arc<RwLock<PeerDB>>,
    events: SmallVec<[PeerManagerEvent; 10]>,
    /// Target number of peers to connect to.
    target_peers_count: usize,
    /// The heartbeat interval to perform routine maintenance.
    heartbeat: tokio::time::Interval,
    /// A collection of peers awaiting to be Status'd.
    status_peers: HashSetDelay<PeerId>,
    /// Peers queued to be dialed.
    peers_to_dial: VecDeque<PeerId>,
}

impl PeerManager {
    pub(crate) fn new(target_peers_count: usize, peer_db: Arc<RwLock<PeerDB>>) -> Self {
        // Set up the peer manager heartbeat interval
        let heartbeat = tokio::time::interval(tokio::time::Duration::from_secs(HEARTBEAT_INTERVAL));

        // NOTE: The time in seconds between re-status's peers. Hardcoding this for now.
        let status_interval = Duration::from_secs(300);

        Self {
            peer_db,
            events: smallvec![],
            target_peers_count,
            heartbeat,
            status_peers: HashSetDelay::new(status_interval),
            peers_to_dial: VecDeque::new(),
        }
    }

    pub(crate) fn need_more_peers(&self) -> bool {
        let count = self.peer_db.read().active_peer_count();
        info!("Current peers count: {}", count);
        count < self.target_peers_count
    }

    pub(crate) fn dial_peer(&mut self, peer_id: PeerId) {
        self.peers_to_dial.push_back(peer_id);
    }

    // A STATUS message has been received from a peer. This resets the status timer.
    pub(crate) fn statusd_peer(&mut self, peer_id: PeerId) {
        self.status_peers.insert(peer_id);
    }

    pub(crate) fn goodbye(
        &mut self,
        peer_id: &PeerId,
        reason: lighthouse_network::rpc::GoodbyeReason,
    ) {
        trace!("[{}] sending goodbye to the peer.", peer_id);

        let mut guard = self.peer_db.write();

        if matches!(
            reason,
            lighthouse_network::rpc::GoodbyeReason::IrrelevantNetwork
        ) {
            guard.update_sync_status(peer_id, SyncStatus::IrrelevantPeer);
        }

        guard.update_connection_status(peer_id, ConnectionStatus::Disconnecting);

        self.events
            .push(PeerManagerEvent::DisconnectPeer(*peer_id, reason));
    }
}
