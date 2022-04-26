use crate::discovery::behaviour::DiscoveryEvent;
use crate::rpc::behaviour::RpcEvent;
use libp2p::swarm::{NetworkBehaviourAction, NetworkBehaviourEventProcess, PollParameters};
use libp2p::NetworkBehaviour;
use std::collections::VecDeque;
use std::task::{Context, Poll};
use tracing::info;
use crate::peer_manager::PeerManagerEvent;

// The core behaviour that combines the sub-behaviours.
#[derive(NetworkBehaviour)]
#[behaviour(event_process = true, poll_method = "poll")] // By default `event_process` is false since libp2p-swarm-derive v0.25.0 SEE https://github.com/libp2p/rust-libp2p/blob/v0.40.0/swarm-derive/CHANGELOG.md#0250-2021-11-01
pub(crate) struct BehaviourComposer {
    /* Sub-Behaviours */
    discovery: crate::discovery::behaviour::Behaviour,
    peer_manager: crate::peer_manager::PeerManager,
    rpc: crate::rpc::behaviour::Behaviour,

    /* Auxiliary Fields */
    #[behaviour(ignore)]
    internal_events: VecDeque<InternalComposerMessage>, // NOTE: unused for now
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
            internal_events: VecDeque::new(),
        }
    }

    fn poll(
        &mut self,
        _cx: &mut Context<'_>,
        _params: &mut impl PollParameters,
    ) -> Poll<
        NetworkBehaviourAction<
            (),
            libp2p::swarm::IntoProtocolsHandlerSelect<
                libp2p::swarm::IntoProtocolsHandlerSelect<
                    libp2p::swarm::protocols_handler::DummyProtocolsHandler,
                    libp2p::swarm::protocols_handler::DummyProtocolsHandler,
                >,
                crate::rpc::handler::Handler,
            >,
        >,
    > {
        info!("poll");

        // Handle internal events
        // see https://github.com/sigp/lighthouse/blob/0aee7ec873bcc7206b9acf2741f46c209b510c57/beacon_node/eth2_libp2p/src/behaviour/mod.rs#L1047
        // if let Some(event) = self.internal_events.pop_front() {
        //     return match event {
        //     }
        // }

        Poll::Pending
    }
}

// NOTE: unused for now
/// Internal type to pass messages from sub-behaviours to the poll of the BehaviourComposer.
enum InternalComposerMessage {}

impl NetworkBehaviourEventProcess<DiscoveryEvent> for BehaviourComposer {
    fn inject_event(&mut self, _event: DiscoveryEvent) {
        info!("NetworkBehaviourEventProcess<DiscoveryEvent>::inject_event");
    }
}

impl NetworkBehaviourEventProcess<PeerManagerEvent> for BehaviourComposer {
    fn inject_event(&mut self, event: PeerManagerEvent) {
        info!("NetworkBehaviourEventProcess<PeerManagerEvent>::inject_event. event: {:?}", event);
    }
}

impl NetworkBehaviourEventProcess<RpcEvent> for BehaviourComposer {
    fn inject_event(&mut self, _event: RpcEvent) {
        info!("NetworkBehaviourEventProcess<RpcEvent>::inject_event");
    }
}
