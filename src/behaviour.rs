use crate::beacon_chain::BeaconChain;
use crate::discovery::behaviour::DiscoveryEvent;
use crate::peer_manager::PeerManagerEvent;
use crate::rpc::behaviour::RpcEvent;
use libp2p::swarm::handler::DummyConnectionHandler;
use libp2p::swarm::{NetworkBehaviourAction, NetworkBehaviourEventProcess, PollParameters};
use libp2p::NetworkBehaviour;
use std::collections::VecDeque;
use std::task::{Context, Poll};
use tracing::{info, warn};

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
    #[allow(dead_code)]
    internal_events: VecDeque<InternalComposerMessage>, // NOTE: unused for now
    #[behaviour(ignore)]
    beacon_chain: BeaconChain,
}

impl BehaviourComposer {
    pub(crate) fn new(
        discovery: crate::discovery::behaviour::Behaviour,
        peer_manager: crate::peer_manager::PeerManager,
        rpc: crate::rpc::behaviour::Behaviour,
        beacon_chain: BeaconChain,
    ) -> Self {
        Self {
            discovery,
            peer_manager,
            rpc,
            internal_events: VecDeque::new(),
            beacon_chain,
        }
    }

    // TODO: Consider factoring parts into `type` definitions
    // https://rust-lang.github.io/rust-clippy/master/index.html#type_complexity
    #[allow(clippy::type_complexity)]
    fn poll(
        &mut self,
        _cx: &mut Context<'_>,
        _params: &mut impl PollParameters,
    ) -> Poll<
        NetworkBehaviourAction<
            (),
            libp2p::swarm::IntoConnectionHandlerSelect<
                libp2p::swarm::IntoConnectionHandlerSelect<
                    DummyConnectionHandler,
                    DummyConnectionHandler,
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
        info!(
            "NetworkBehaviourEventProcess<PeerManagerEvent>::inject_event. event: {:?}",
            event
        );

        match event {
            PeerManagerEvent::PeerConnectedIncoming(peer_id) => {
                warn!("PeerManagerEvent::PeerConnectedIncoming, but no implementation for the event for now. peer_id: {}", peer_id);
            }
            PeerManagerEvent::PeerConnectedOutgoing(peer_id) => {
                // The dialing client MUST send a Status request upon connection.
                // https://github.com/ethereum/consensus-specs/blob/dev/specs/phase0/p2p-interface.md#status

                // Ref: Building a `StatusMessage`
                // https://github.com/sigp/lighthouse/blob/4bf1af4e8520f235de8fe5f94afedf953df5e6a4/beacon_node/network/src/router/processor.rs#L374

                let enr_fork_id = self.beacon_chain.enr_fork_id();
                let head = self.beacon_chain.head();
                let finalized_checkpoint = head.beacon_state.finalized_checkpoint();
                self.rpc.send_status(
                    peer_id,
                    enr_fork_id.fork_digest,
                    finalized_checkpoint.root,
                    finalized_checkpoint.epoch,
                    head.beacon_block.canonical_root(),
                    head.beacon_block.slot(),
                );
            }
            PeerManagerEvent::NeedToDiscoverMorePeers => {
                self.discovery.discover_peers();
            }
        }
    }
}

impl NetworkBehaviourEventProcess<RpcEvent> for BehaviourComposer {
    fn inject_event(&mut self, _event: RpcEvent) {
        info!("NetworkBehaviourEventProcess<RpcEvent>::inject_event");
    }
}
