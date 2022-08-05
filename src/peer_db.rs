use libp2p::{Multiaddr, PeerId};
use std::collections::HashMap;

pub(crate) struct PeerDB {
    peers: HashMap<PeerId, Multiaddr>,
}

impl PeerDB {
    pub(crate) fn new() -> Self {
        PeerDB {
            peers: HashMap::new(),
        }
    }

    pub(crate) fn add_peer(&mut self, peer_id: PeerId, address: Multiaddr) {
        self.peers.insert(peer_id, address);
    }

    pub(crate) fn peer_count(&self) -> usize {
        self.peers.len()
    }
}
