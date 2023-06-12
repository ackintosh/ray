use crate::discovery::DiscoveryEvent;
use crate::network::ReqId;
use crate::peer_manager::PeerManagerEvent;
use crate::rpc::RpcEvent;
use libp2p::swarm::NetworkBehaviour;

// Composite trait for a request id.
// ref: https://github.com/sigp/lighthouse/blob/8102a010857979e13d658f83594df12bd281f3a2/beacon_node/lighthouse_network/src/rpc/mod.rs#L43-L45
// pub trait ReqId: Send + 'static + std::fmt::Debug + Copy + Clone {}
// impl<T> ReqId for T where T: Send + 'static + std::fmt::Debug + Copy + Clone {}

/// Identifier of a request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequestId<AppReqId> {
    Application(AppReqId),
    Internal,
}

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
pub(crate) struct BehaviourComposer<AppReqId: ReqId> {
    /* Sub-Behaviours */
    pub(crate) discovery: crate::discovery::behaviour::Behaviour,
    pub(crate) peer_manager: crate::peer_manager::PeerManager,
    pub(crate) rpc: crate::rpc::behaviour::Behaviour<RequestId<AppReqId>>,
}

impl<AppReqId: ReqId> BehaviourComposer<AppReqId> {
    pub(crate) fn new(
        discovery: crate::discovery::behaviour::Behaviour,
        peer_manager: crate::peer_manager::PeerManager,
        rpc: crate::rpc::behaviour::Behaviour<RequestId<AppReqId>>,
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
