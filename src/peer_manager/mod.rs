use libp2p::{Multiaddr, PeerId};
use std::collections::HashMap;

pub(crate) mod behaviour;

pub(crate) struct PeerManager {
    peers: HashMap<PeerId, Multiaddr>,
}

impl PeerManager {
    pub(crate) fn new() -> Self {
        Self {
            peers: HashMap::new(),
        }
    }
}
