use crate::beacon_chain::BeaconChain;
use crate::discovery::DiscoveryEvent;
use crate::peer_manager::PeerManagerEvent;
use crate::rpc::RpcEvent;
use crate::sync::SyncOperation;
use libp2p::swarm::dial_opts::{DialOpts, PeerCondition};
use libp2p::swarm::NetworkBehaviour;
use libp2p::swarm::{NetworkBehaviourAction, NetworkBehaviourEventProcess, PollParameters};
use libp2p::{NetworkBehaviour, PeerId};
use lighthouse_network::rpc::methods::RPCResponse;
use lighthouse_network::rpc::protocol::InboundRequest;
use parking_lot::RwLock;
use std::collections::VecDeque;
use std::sync::Arc;
use std::task::{Context, Poll};
use tokio::sync::mpsc::UnboundedSender;
use tracing::log::error;
use tracing::{info, trace, warn};

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
    beacon_chain: Arc<RwLock<BeaconChain>>,
    #[behaviour(ignore)]
    sync_sender: UnboundedSender<SyncOperation>,
}

impl BehaviourComposer {
    pub(crate) fn new(
        discovery: crate::discovery::behaviour::Behaviour,
        peer_manager: crate::peer_manager::PeerManager,
        rpc: crate::rpc::behaviour::Behaviour,
        beacon_chain: Arc<RwLock<BeaconChain>>,
        sync_sender: UnboundedSender<SyncOperation>,
    ) -> Self {
        Self {
            discovery,
            peer_manager,
            rpc,
            internal_events: VecDeque::new(),
            beacon_chain,
            sync_sender,
        }
    }

    fn handle_status(&mut self, peer_id: PeerId, message: lighthouse_network::rpc::StatusMessage) {
        if self.beacon_chain.read().is_relevant(&message) {
            self.sync_sender
                .send(SyncOperation::AddPeer(peer_id, message.into()))
                .unwrap_or_else(|e| {
                    error!("Failed to send message to the sync manager: {}", e);
                });
        } else {
            // TODO: say goodbye
            // https://github.com/sigp/lighthouse/blob/7af57420810772b2a1b0d7d75a0d045c0333093b/beacon_node/network/src/beacon_processor/worker/rpc_methods.rs#L109
        }
    }

    fn poll(
        &mut self,
        _cx: &mut Context<'_>,
        _params: &mut impl PollParameters,
    ) -> Poll<NetworkBehaviourAction<(), <BehaviourComposer as NetworkBehaviour>::ConnectionHandler>>
    {
        trace!("poll");

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
                self.rpc
                    .send_status(peer_id, self.beacon_chain.read().create_status_message());
            }
            PeerManagerEvent::NeedMorePeers => {
                if !self.discovery.has_active_queries() {
                    self.discovery.discover_peers();
                }
            }
            PeerManagerEvent::SendStatus(peer_id) => {
                self.rpc
                    .send_status(peer_id, self.beacon_chain.read().create_status_message());
            }
        }
    }
}

impl NetworkBehaviourEventProcess<RpcEvent> for BehaviourComposer {
    fn inject_event(&mut self, event: RpcEvent) {
        info!("NetworkBehaviourEventProcess<RpcEvent> event: {:?}", event);

        match event {
            RpcEvent::ReceivedRequest(request) => {
                match request.request {
                    InboundRequest::Status(message) => {
                        info!("RpcEvent::ReceivedRequest InboundRequest::Status. request_message: {:?}", message);

                        // Inform the peer manager that we have received a `Status` from a peer.
                        self.peer_manager.statusd_peer(request.peer_id);

                        self.handle_status(request.peer_id, message);

                        self.rpc.send_response(
                            request.peer_id,
                            request.connection_id,
                            request.substream_id,
                            lighthouse_network::Response::Status(
                                self.beacon_chain.read().create_status_message(),
                            ),
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
                }
            }
            RpcEvent::ReceivedResponse(response) => {
                match response.response {
                    RPCResponse::Status(message) => {
                        info!(
                            "RpcEvent::ReceivedResponse RPCResponse::Status message: {:?}",
                            message
                        );

                        // Inform the peer manager that we have received a `Status` from a peer.
                        self.peer_manager.statusd_peer(response.peer_id);

                        self.handle_status(response.peer_id, message);
                    }
                    RPCResponse::BlocksByRange(_) => {
                        todo!()
                    }
                    RPCResponse::BlocksByRoot(_) => {
                        todo!()
                    }
                    RPCResponse::Pong(_) => {
                        todo!()
                    }
                    RPCResponse::MetaData(_) => {
                        todo!()
                    }
                }
            }
        }
    }
}
