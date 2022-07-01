use crate::beacon_chain::BeaconChain;
use crate::discovery::DiscoveryEvent;
use crate::peer_manager::PeerManagerEvent;
use crate::rpc::RpcEvent;
use libp2p::swarm::dial_opts::{DialOpts, PeerCondition};
use libp2p::swarm::handler::DummyConnectionHandler;
use libp2p::swarm::NetworkBehaviour;
use libp2p::swarm::{NetworkBehaviourAction, NetworkBehaviourEventProcess, PollParameters};
use libp2p::{NetworkBehaviour, PeerId};
use lighthouse_network::rpc::protocol::InboundRequest;
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
    internal_events: VecDeque<InternalComposerEvent>,
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
        if let Some(event) = self.internal_events.pop_front() {
            return match event {
                InternalComposerEvent::DialPeer(peer_id) => {
                    let handler = self.new_handler();
                    Poll::Ready(NetworkBehaviourAction::Dial {
                        opts: DialOpts::peer_id(peer_id)
                            .condition(PeerCondition::Disconnected)
                            .build(),
                        handler,
                    })
                }
            };
        }

        Poll::Pending
    }

    fn create_status_message(&self) -> lighthouse_network::rpc::StatusMessage {
        let enr_fork_id = self.beacon_chain.enr_fork_id();
        let head = self.beacon_chain.head();
        let finalized_checkpoint = head.beacon_state.finalized_checkpoint();

        lighthouse_network::rpc::StatusMessage {
            fork_digest: enr_fork_id.fork_digest,
            finalized_root: finalized_checkpoint.root,
            finalized_epoch: finalized_checkpoint.epoch,
            head_root: head.beacon_block.canonical_root(),
            head_slot: head.beacon_block.slot(),
        }
    }
}

enum InternalComposerEvent {
    DialPeer(PeerId),
}

impl NetworkBehaviourEventProcess<DiscoveryEvent> for BehaviourComposer {
    fn inject_event(&mut self, event: DiscoveryEvent) {
        info!(
            "NetworkBehaviourEventProcess<DiscoveryEvent> event: {:?}",
            event
        );

        match event {
            DiscoveryEvent::FoundPeers(peer_ids) => {
                if self.peer_manager.need_more_peers() {
                    info!("Requesting more peers to be discovered.");
                    self.discovery.discover_peers();
                }

                for peer in peer_ids {
                    self.internal_events
                        .push_back(InternalComposerEvent::DialPeer(peer));
                }
            }
        };
    }
}

impl NetworkBehaviourEventProcess<PeerManagerEvent> for BehaviourComposer {
    fn inject_event(&mut self, event: PeerManagerEvent) {
        info!(
            "NetworkBehaviourEventProcess<PeerManagerEvent> event: {:?}",
            event
        );

        match event {
            PeerManagerEvent::PeerConnectedIncoming(peer_id) => {
                warn!("PeerManagerEvent::PeerConnectedIncoming, but no implementation for the event for now. peer_id: {}", peer_id);
            }
            PeerManagerEvent::PeerConnectedOutgoing(peer_id) => {
                // Spec: The dialing client MUST send a Status request upon connection.
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
            PeerManagerEvent::NeedMorePeers => {
                if !self.discovery.has_active_queries() {
                    self.discovery.discover_peers();
                }
            }
        }
    }
}

impl NetworkBehaviourEventProcess<RpcEvent> for BehaviourComposer {
    fn inject_event(&mut self, event: RpcEvent) {
        info!("NetworkBehaviourEventProcess<RpcEvent> event: {:?}", event);

        match event {
            RpcEvent::ReceivedRequest(request) => match request.request {
                InboundRequest::Status(_message) => {
                    let status_response = self.create_status_message();
                    self.rpc.send_response(
                        request.peer_id,
                        lighthouse_network::Response::Status(status_response),
                    );
                }
                InboundRequest::Goodbye(_) => {
                    todo!()
                }
                InboundRequest::BlocksByRange(_) => {
                    todo!()
                }
                InboundRequest::BlocksByRoot(_) => {
                    todo!()
                }
                InboundRequest::Ping(_) => {
                    todo!()
                }
                InboundRequest::MetaData(_) => {
                    todo!()
                }
            },
            RpcEvent::ReceivedResponse(_) => {
                todo!()
            }
        }
    }
}
