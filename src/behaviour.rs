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
#[behaviour(event_process = true, poll_method = "poll")] // By default `event_process` is false since libp2p-swarm-derive v0.25.0 SEE https://github.com/libp2p/rust-libp2p/blob/v0.40.0/swarm-derive/CHANGELOG.md#0250-2021-11-01
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

        // Handle internal events
        // see https://github.com/sigp/lighthouse/blob/0aee7ec873bcc7206b9acf2741f46c209b510c57/beacon_node/eth2_libp2p/src/behaviour/mod.rs#L1047
        // if let Some(event) = self.internal_events.pop_front() {
        //     match event {
        //         InternalComposerMessage::DialPeer(enr) => {
        //             return Poll::Ready(
        //                 NetworkBehaviourAction::DialPeer {
        //                     peer_id: enr.pee
        //                 }
        //             )
        //         }
        //     }
        // }

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
