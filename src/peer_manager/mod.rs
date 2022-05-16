use libp2p::{Multiaddr, PeerId};
use smallvec::{smallvec, SmallVec};
use std::collections::HashMap;

pub(crate) mod behaviour;

pub(crate) struct PeerManager {
    peers: HashMap<PeerId, Multiaddr>,
    events: SmallVec<[PeerManagerEvent; 10]>,
    // Target number of peers to connect to.
    target_peers_count: usize,
}

/// The events that the `PeerManager` emits to `BehaviourComposer`.
#[derive(Debug)]
pub(crate) enum PeerManagerEvent {
    /// A peer has dialed us.
    PeerConnectedIncoming(PeerId),
    /// A peer has been dialed.
    PeerConnectedOutgoing(PeerId),
    /// Request the behaviour to discover more peers.
    NeedToDiscoverMorePeers,
}

impl PeerManager {
    pub(crate) fn new(target_peers_count: usize) -> Self {
        Self {
            peers: HashMap::new(),
            events: smallvec![],
            target_peers_count,
        }
    }
}
