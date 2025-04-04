use crate::behaviour::RequestId;
use crate::discovery::DiscoveryEvent;
use crate::peer_manager::PeerManagerEvent;
use crate::rpc::status::status_message;
use crate::rpc::RpcEvent;
use crate::sync::{SyncOperation, SyncRequestId};
use crate::{
    build_network_behaviour, build_network_transport, BehaviourComposer, BehaviourComposerEvent,
    NetworkConfig, PeerDB,
};
use beacon_chain::BeaconChainTypes;
use discv5::enr::CombinedKey;
use discv5::Enr;
use futures::StreamExt;
use libp2p::identity::Keypair;
use libp2p::swarm::SwarmEvent;
use libp2p::{PeerId, Swarm, SwarmBuilder};
use parking_lot::RwLock;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Weak};
use tokio::runtime::Runtime;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tracing::{debug, error, info, trace, warn};
use types::MainnetEthSpec;

/// The executor for libp2p
struct Executor(Weak<Runtime>);

impl libp2p::swarm::Executor for Executor {
    fn exec(&self, f: Pin<Box<dyn Future<Output = ()> + Send>>) {
        if let Some(runtime) = self.0.upgrade() {
            trace!("Executor: Spawning a task");
            runtime.spawn(f);
        } else {
            warn!("Executor: Couldn't spawn task. Runtime shutting down");
        }
    }
}

pub trait ReqId: Send + 'static + std::fmt::Debug + Copy + Clone {}
impl<T> ReqId for T where T: Send + 'static + std::fmt::Debug + Copy + Clone {}

pub(crate) struct Network<T: BeaconChainTypes> {
    swarm: Swarm<BehaviourComposer<ApplicationRequestId>>,
    network_receiver: UnboundedReceiver<NetworkMessage>,
    lh_beacon_chain: Arc<beacon_chain::BeaconChain<T>>,
    sync_sender: UnboundedSender<SyncOperation>,
}

impl<T> Network<T>
where
    T: BeaconChainTypes,
{
    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn new(
        network_receiver: UnboundedReceiver<NetworkMessage>,
        lh_beacon_chain: Arc<beacon_chain::BeaconChain<T>>,
        sync_sender: UnboundedSender<SyncOperation>,
        key_pair: Keypair,
        enr: Enr,
        enr_key: CombinedKey,
        network_config: NetworkConfig,
        peer_db: Arc<RwLock<PeerDB>>,
        runtime: Arc<Runtime>,
    ) -> Self {
        let transport = build_network_transport(key_pair.clone()).await;
        let behaviour = build_network_behaviour(
            enr,
            enr_key,
            network_config,
            peer_db,
            lh_beacon_chain.clone(),
        )
        .await;
        let swarm = SwarmBuilder::with_existing_identity(key_pair)
            .with_tokio()
            .with_other_transport(|_| transport)
            .expect("infallible")
            .with_behaviour(|_| behaviour)
            .expect("infallible")
            .with_swarm_config(|_| {
                libp2p::swarm::Config::with_executor(Executor(Arc::downgrade(&runtime)))
            })
            .build();

        Network {
            swarm,
            network_receiver,
            lh_beacon_chain,
            sync_sender,
        }
    }

    async fn start(&mut self) {
        let listen_multiaddr = {
            let mut multiaddr =
                libp2p::core::multiaddr::Multiaddr::from(std::net::Ipv4Addr::new(0, 0, 0, 0));
            multiaddr.push(libp2p::core::multiaddr::Protocol::Tcp(9000));
            multiaddr
        };

        self.swarm
            .listen_on(listen_multiaddr)
            .expect("Swarm starts listening");

        loop {
            match self.swarm.next().await.unwrap() {
                SwarmEvent::NewListenAddr { .. } => break,
                e => warn!("Unexpected event {:?}", e),
            };
        }
    }

    pub(crate) async fn spawn(mut self, runtime: Arc<Runtime>) {
        self.start().await;

        let fut = async move {
            loop {
                tokio::select! {
                    // SEE:
                    // https://github.com/sigp/lighthouse/blob/9667dc2f0379272fe0f36a2ec015c5a560bca652/beacon_node/network/src/service.rs#L309
                    // https://github.com/sigp/lighthouse/blob/0aee7ec873bcc7206b9acf2741f46c209b510c57/beacon_node/eth2_libp2p/src/service.rs#L305
                    event = self.swarm.select_next_some() => {
                        match event {
                            SwarmEvent::Behaviour(behaviour_event) => self.handle_behaviour_event(behaviour_event),
                            SwarmEvent::ConnectionEstablished { peer_id, .. } => info!("SwarmEvent::ConnectionEstablished. peer_id: {}", peer_id),
                            ev => {
                                debug!("SwarmEvent: {:?}", ev);
                            }
                        }
                    }
                    Some(message) = self.network_receiver.recv() => self.on_network_message(message),
                }
            }
        };

        runtime.spawn(fut);
    }

    fn handle_behaviour_event(&mut self, event: BehaviourComposerEvent) {
        match event {
            BehaviourComposerEvent::Discovery(discovery_event) => {
                self.handle_discovery_event(discovery_event)
            }
            BehaviourComposerEvent::PeerManager(peer_manager_event) => {
                self.handle_peer_manager_event(peer_manager_event)
            }
            BehaviourComposerEvent::Rpc(rpc_event) => self.handle_rpc_event(rpc_event),
        }
    }

    // /////////////////////////////////////////////////////////////////////////////////////////////
    // Discovery
    // /////////////////////////////////////////////////////////////////////////////////////////////
    fn handle_discovery_event(&mut self, event: DiscoveryEvent) {
        match event {
            DiscoveryEvent::FoundPeers(peer_ids) => {
                let behaviour = self.swarm.behaviour_mut();

                if behaviour.peer_manager.need_more_peers() {
                    info!("Requesting more peers to be discovered.");
                    behaviour.discovery.discover_peers();
                }

                for peer in peer_ids {
                    self.swarm.behaviour_mut().peer_manager.dial_peer(peer);
                }
            }
        };
    }

    // /////////////////////////////////////////////////////////////////////////////////////////////
    // PeerManager
    // /////////////////////////////////////////////////////////////////////////////////////////////
    fn handle_peer_manager_event(&mut self, event: PeerManagerEvent) {
        match event {
            PeerManagerEvent::PeerConnectedIncoming(peer_id) => {
                warn!("PeerManagerEvent::PeerConnectedIncoming, but no implementation for the event for now. peer_id: {}", peer_id);
            }
            PeerManagerEvent::PeerConnectedOutgoing(peer_id) => {
                // Spec: The dialing client MUST send a Status request upon connection.
                // https://github.com/ethereum/consensus-specs/blob/dev/specs/phase0/p2p-interface.md#status
                self.swarm.behaviour_mut().rpc.send_status(
                    RequestId::Internal,
                    peer_id,
                    status_message(&self.lh_beacon_chain),
                );
            }
            PeerManagerEvent::NeedMorePeers => {
                let behaviour = self.swarm.behaviour_mut();
                if !behaviour.discovery.has_active_queries() {
                    behaviour.discovery.discover_peers();
                }
            }
            PeerManagerEvent::SendStatus(peer_id) => {
                self.swarm.behaviour_mut().rpc.send_status(
                    RequestId::Internal,
                    peer_id,
                    status_message(&self.lh_beacon_chain),
                );
            }
            PeerManagerEvent::DisconnectPeer(peer_id, goodbye_reason) => {
                self.swarm.behaviour_mut().rpc.send_goodbye(
                    RequestId::Internal,
                    peer_id,
                    goodbye_reason,
                );
            }
        }
    }

    // /////////////////////////////////////////////////////////////////////////////////////////////
    // RPC
    // /////////////////////////////////////////////////////////////////////////////////////////////
    fn handle_rpc_event(&mut self, event: RpcEvent) {
        match event {
            RpcEvent::ReceivedRequest(request) => match &request.request {
                lighthouse_network::rpc::protocol::RequestType::Status(message) => {
                    if self.validate_status_message(&request.peer_id, message) {
                        let behaviour = self.swarm.behaviour_mut();
                        behaviour.peer_manager.statusd_peer(request.peer_id);
                        behaviour.rpc.send_response(
                            request.peer_id,
                            request.connection_id,
                            request.substream_id,
                            lighthouse_network::Response::Status(
                                status_message(&self.lh_beacon_chain),
                            ),
                        );
                    }
                }
                lighthouse_network::rpc::protocol::RequestType::Goodbye(reason) => {
                    // NOTE: We currently do not inform the application that we are
                    // disconnecting here. The RPC handler will automatically
                    // disconnect for us.
                    // The actual disconnection event will be relayed from `PeerManager` to the application.
                    debug!("[{}] Peer sent goodbye. reason: {}", request.peer_id, reason);
                },
                lighthouse_network::rpc::protocol::RequestType::BlocksByRange(blocks_by_range_request) => warn!("[{}] Received `InboundRequest::BlocksByRange` (request: {:?}) but it was not handled.", request.peer_id, blocks_by_range_request),
                lighthouse_network::rpc::protocol::RequestType::BlocksByRoot(blocks_by_root_request) => warn!("[{}] Received `InboundRequest::BlocksByRoot` (request: {:?}) but it was not handled.", request.peer_id, blocks_by_root_request),
                lighthouse_network::rpc::protocol::RequestType::BlobsByRange(_) => todo!(),
                lighthouse_network::rpc::protocol::RequestType::BlobsByRoot(_) => todo!(),
                lighthouse_network::rpc::protocol::RequestType::Ping(ping) => warn!("[{}] Received `InboundRequest::Ping` (ping: {:?}) but it was not handled.", request.peer_id, ping),
                lighthouse_network::rpc::protocol::RequestType::MetaData(_) => warn!("[{}] Received `InboundRequest::MetaData` but it was not handled.", request.peer_id),
                lighthouse_network::rpc::protocol::RequestType::LightClientBootstrap(_) => todo!(),
                lighthouse_network::rpc::protocol::RequestType::DataColumnsByRoot(_) => {}
                lighthouse_network::rpc::protocol::RequestType::DataColumnsByRange(_) => {}
                lighthouse_network::rpc::protocol::RequestType::LightClientOptimisticUpdate => {}
                lighthouse_network::rpc::protocol::RequestType::LightClientFinalityUpdate => {}
                lighthouse_network::rpc::protocol::RequestType::LightClientUpdatesByRange(_) => {}
            },
            RpcEvent::ReceivedResponse(response) => match &response.response {
                lighthouse_network::rpc::methods::RpcResponse::Success(success_response) => match success_response {
                    lighthouse_network::rpc::methods::RpcSuccessResponse::Status(message) => {
                        if self.validate_status_message(&response.peer_id, message) {
                            self.swarm
                                .behaviour_mut()
                                .peer_manager
                                .statusd_peer(response.peer_id);
                        }
                    }
                    lighthouse_network::rpc::methods::RpcSuccessResponse::BlocksByRange(_) => {}
                    lighthouse_network::rpc::methods::RpcSuccessResponse::BlocksByRoot(_) => {}
                    lighthouse_network::rpc::methods::RpcSuccessResponse::BlobsByRange(_) => {}
                    lighthouse_network::rpc::methods::RpcSuccessResponse::LightClientBootstrap(_) => {}
                    lighthouse_network::rpc::methods::RpcSuccessResponse::LightClientOptimisticUpdate(_) => {}
                    lighthouse_network::rpc::methods::RpcSuccessResponse::LightClientFinalityUpdate(_) => {}
                    lighthouse_network::rpc::methods::RpcSuccessResponse::LightClientUpdatesByRange(_) => {}
                    lighthouse_network::rpc::methods::RpcSuccessResponse::BlobsByRoot(_) => {}
                    lighthouse_network::rpc::methods::RpcSuccessResponse::DataColumnsByRoot(_) => {}
                    lighthouse_network::rpc::methods::RpcSuccessResponse::DataColumnsByRange(_) => {}
                    lighthouse_network::rpc::methods::RpcSuccessResponse::Pong(_) => {}
                    lighthouse_network::rpc::methods::RpcSuccessResponse::MetaData(_) => {}
                }
                _ => {}
            },
        }
    }

    fn validate_status_message(
        &mut self,
        peer_id: &PeerId,
        message: &lighthouse_network::rpc::StatusMessage,
    ) -> bool {
        trace!("[{}] validating status message.", peer_id);

        if self.check_peer_relevance(peer_id, message) {
            info!("[{}] the peer is relevant to our beacon chain.", peer_id);

            self.sync_sender
                .send(SyncOperation::AddPeer(*peer_id, message.clone().into()))
                .unwrap_or_else(|e| {
                    error!("Failed to send message to the sync manager: {}", e);
                });
            true
        } else {
            info!("[{}] the remote chain is not relevant to ours.", peer_id);
            self.swarm.behaviour_mut().peer_manager.goodbye(
                peer_id,
                lighthouse_network::rpc::GoodbyeReason::IrrelevantNetwork,
            );
            false
        }
    }

    // Determine if the node is relevant to us.
    // ref: https://github.com/sigp/lighthouse/blob/7af57420810772b2a1b0d7d75a0d045c0333093b/beacon_node/network/src/beacon_processor/worker/rpc_methods.rs#L61
    fn check_peer_relevance(
        &self,
        peer_id: &PeerId,
        remote_status: &lighthouse_network::rpc::StatusMessage,
    ) -> bool {
        let local_status = status_message(&self.lh_beacon_chain);

        if local_status.fork_digest != remote_status.fork_digest {
            info!(
                "[{}] The node is not relevant to us: Incompatible forks. Ours:{} Theirs:{}",
                peer_id,
                hex::encode(local_status.fork_digest),
                hex::encode(remote_status.fork_digest)
            );
            return false;
        }

        if remote_status.head_slot > self.lh_beacon_chain.slot().expect("slot") {
            info!(
                "[{}] The node is not relevant to us: Different system clocks or genesis time",
                peer_id
            );
            return false;
        }

        // NOTE: We can implement more checks to be production-ready.
        // https://github.com/sigp/lighthouse/blob/7af57420810772b2a1b0d7d75a0d045c0333093b/beacon_node/network/src/beacon_processor/worker/rpc_methods.rs#L86-L97

        true
    }

    /// Handle a message sent to the network service.
    fn on_network_message(&mut self, message: NetworkMessage) {
        match message {
            NetworkMessage::SendRequest {
                peer_id,
                request,
                request_id,
            } => self.send_request(peer_id, request, request_id),
        }
    }

    fn send_request(
        &mut self,
        peer_id: PeerId,
        request: lighthouse_network::rpc::protocol::RequestType<MainnetEthSpec>,
        request_id: ApplicationRequestId,
    ) {
        self.swarm.behaviour_mut().rpc.send_request(
            peer_id,
            request,
            RequestId::Application(request_id),
        );
    }
}

/// Application level requests sent to the network.
// ref:
#[derive(Debug, Clone, Copy)]
pub(crate) enum ApplicationRequestId {
    Sync(SyncRequestId),
    Router,
}

/// Types of messages that the network service can receive.
pub(crate) enum NetworkMessage {
    SendRequest {
        peer_id: PeerId,
        request: lighthouse_network::rpc::protocol::RequestType<MainnetEthSpec>,
        request_id: ApplicationRequestId,
    },
}
