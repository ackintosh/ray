use libp2p::{Multiaddr, PeerId};
use std::collections::HashMap;
use tracing::{error, info, warn};

pub(crate) struct PeerDB {
    peers: HashMap<PeerId, PeerInfo>,
}

struct PeerInfo {
    #[allow(dead_code)]
    listening_address: Multiaddr,
    sync_status: SyncStatus,
    connection_status: ConnectionStatus,
}

#[derive(Debug)]
pub(crate) enum SyncStatus {
    // At the current state as our node or ahead of us.
    Synced,
    // The peer has greater knowledge about the canonical chain than we do.
    Advanced,
    // Is behind our current head and not useful for block downloads.
    Behind,
    // This peer is in an incompatible network.
    IrrelevantPeer,
    // Not currently known as a STATUS handshake has not occurred.
    Unknown,
}

#[derive(Debug)]
pub enum ConnectionStatus {
    // The peer is connected.
    Connected,
    // The peer is being disconnected.
    Disconnecting,
}

impl PeerInfo {
    fn new(listening_address: Multiaddr) -> Self {
        PeerInfo {
            listening_address,
            sync_status: SyncStatus::Unknown,
            connection_status: ConnectionStatus::Connected,
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
                error!("update_sync_status: Peer not found. peer_id: {}", peer_id);
            }
            Some(peer_info) => {
                info!(
                    "Updated sync_status: before: {:?}, after: {:?}, peer: {}",
                    peer_info.sync_status, sync_status, peer_id
                );
                peer_info.sync_status = sync_status;
            }
        }
    }

    pub(crate) fn update_connection_status(
        &mut self,
        peer_id: &PeerId,
        connection_status: ConnectionStatus,
    ) {
        match self.peers.get_mut(peer_id) {
            None => error!(
                "update_connection_status: Peer not found. peer_id: {}",
                peer_id
            ),
            Some(peer_info) => {
                info!(
                    "Updated connection_status: before: {:?}, after: {:?}, peer: {}",
                    peer_info.connection_status, connection_status, peer_id
                );
                peer_info.connection_status = connection_status;
            }
        }
    }

    pub(crate) fn peer_count(&self) -> usize {
        self.peers.len()
    }
}
