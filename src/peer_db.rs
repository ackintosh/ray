use libp2p::{Multiaddr, PeerId};
use std::collections::HashMap;
use tracing::error;

pub(crate) struct PeerDB {
    peers: HashMap<PeerId, PeerInfo>,
}

struct PeerInfo {
    listening_address: Multiaddr,
    sync_status: SyncStatus,
}

pub(crate) enum SyncStatus {
    // At the current state as our node or ahead of us.
    Synced,
    // The peer has greater knowledge about the canonical chain than we do.
    Advanced,
    // Is behind our current head and not useful for block downloads.
    Behind,
    // Not currently known as a STATUS handshake has not occurred.
    Unknown,
}

impl PeerInfo {
    fn new(listening_address: Multiaddr) -> Self {
        PeerInfo {
            listening_address,
            sync_status: SyncStatus::Unknown,
        }
    }
}

impl PeerDB {
    pub(crate) fn new() -> Self {
        PeerDB {
            peers: HashMap::new(),
        }
    }

    pub(crate) fn add_peer(&mut self, peer_id: PeerId, address: Multiaddr) {
        self.peers.insert(peer_id, PeerInfo::new(address));
    }

    pub(crate) fn update_sync_status(&mut self, peer_id: &PeerId, sync_status: SyncStatus) {
        match self.peers.get_mut(peer_id) {
            None => {
                error!("Peer not found: {}", peer_id);
            }
            Some(peer_info) => {
                peer_info.sync_status = sync_status;
            }
        }
    }

    pub(crate) fn peer_count(&self) -> usize {
        self.peers.len()
    }
}
