use crate::discovery::DiscoveryEvent;
use crate::peer_manager::PeerManagerEvent;
use crate::rpc::RpcEvent;
use libp2p::NetworkBehaviour;

/// Events `BehaviourComposer` emits.
#[derive(Debug)]
pub(crate) enum BehaviourComposerEvent {
    Discovery(DiscoveryEvent),
    PeerManager(PeerManagerEvent),
    Rpc(RpcEvent),
}

/// The core behaviour that combines the sub-behaviours.
#[derive(NetworkBehaviour)]
#[behaviour(out_event = "BehaviourComposerEvent")]
pub(crate) struct BehaviourComposer {
    /* Sub-Behaviours */
    pub(crate) discovery: crate::discovery::behaviour::Behaviour,
    pub(crate) peer_manager: crate::peer_manager::PeerManager,
    pub(crate) rpc: crate::rpc::behaviour::Behaviour,
}

impl BehaviourComposer {
    pub(crate) fn new(
        discovery: crate::discovery::behaviour::Behaviour,
        peer_manager: crate::peer_manager::PeerManager,
        rpc: crate::rpc::behaviour::Behaviour,
    ) -> Self {
        Self {
            discovery,
            peer_manager,
            rpc,
        }
    }
}

impl From<DiscoveryEvent> for BehaviourComposerEvent {
    fn from(event: DiscoveryEvent) -> Self {
        BehaviourComposerEvent::Discovery(event)
    }
}

impl From<PeerManagerEvent> for BehaviourComposerEvent {
    fn from(event: PeerManagerEvent) -> Self {
        BehaviourComposerEvent::PeerManager(event)
    }
}

impl From<RpcEvent> for BehaviourComposerEvent {
    fn from(event: RpcEvent) -> Self {
        BehaviourComposerEvent::Rpc(event)
    }
}
