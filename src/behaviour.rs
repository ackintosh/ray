use crate::discovery::behaviour::DiscoveryEvent;
use crate::rpc::behaviour::RpcEvent;
use enr::{CombinedKey, Enr};
use libp2p::swarm::{NetworkBehaviourAction, NetworkBehaviourEventProcess, PollParameters};
use libp2p::NetworkBehaviour;
use std::collections::VecDeque;
use std::task::{Context, Poll};
use tracing::info;

// The core behaviour that combines the sub-behaviours.
#[derive(NetworkBehaviour)]
#[behaviour(event_process = true, poll_method = "poll")]
pub(crate) struct BehaviourComposer {
    /* Sub-Behaviours */
    discovery: crate::discovery::behaviour::Behaviour,
    rpc: crate::rpc::behaviour::Behaviour,

    /* Auxiliary Fields */
    #[behaviour(ignore)]
    internal_events: VecDeque<InternalComposerMessage>,
}

impl BehaviourComposer {
    pub(crate) fn new(
        discovery: crate::discovery::behaviour::Behaviour,
        rpc: crate::rpc::behaviour::Behaviour,
    ) -> Self {
        Self {
            discovery,
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
                libp2p::swarm::protocols_handler::DummyProtocolsHandler,
                crate::rpc::handler::Handler,
            >,
        >,
    > {
        info!("poll");
        Poll::Pending
    }
}

/// Internal type to pass messages from sub-behaviours to the poll of the BehaviourComposer.
enum InternalComposerMessage {
    /// Dial a Peer.
    DialPeer(Enr<CombinedKey>),
}

impl NetworkBehaviourEventProcess<DiscoveryEvent> for BehaviourComposer {
    fn inject_event(&mut self, event: DiscoveryEvent) {
        info!("NetworkBehaviourEventProcess<DiscoveryEvent>::inject_event");
        match event {
            DiscoveryEvent::QueryResult(enrs) => {
                for enr in enrs {
                    self.internal_events
                        .push_back(InternalComposerMessage::DialPeer(enr));
                }
            }
        }
    }
}

impl NetworkBehaviourEventProcess<RpcEvent> for BehaviourComposer {
    fn inject_event(&mut self, _event: RpcEvent) {
        info!("NetworkBehaviourEventProcess<RpcEvent>::inject_event");
    }
}
