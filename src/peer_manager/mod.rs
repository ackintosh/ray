use libp2p::PeerId;
use std::collections::HashMap;

pub(crate) mod behaviour;

pub(crate) struct PeerManager {
    peers: HashMap<PeerId, ()>,
}

impl PeerManager {
    pub(crate) fn new() -> Self {
        Self {
            peers: HashMap::new(),
        }
    }
}
