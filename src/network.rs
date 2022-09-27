use crate::discovery::DiscoveryEvent;
use crate::peer_manager::PeerManagerEvent;
use crate::rpc::RpcEvent;
use crate::sync::SyncOperation;
use crate::{
    build_network_behaviour, build_network_transport, BeaconChain, BehaviourComposer,
    BehaviourComposerEvent, NetworkConfig, PeerDB,
};
use discv5::enr::CombinedKey;
use discv5::Enr;
use futures::StreamExt;
use libp2p::identity::Keypair;
use libp2p::swarm::{SwarmBuilder, SwarmEvent};
use libp2p::{PeerId, Swarm};
use lighthouse_network::rpc::methods::RPCResponse;
use lighthouse_network::rpc::protocol::InboundRequest;
use lighthouse_network::rpc::StatusMessage;
use parking_lot::RwLock;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Weak};
use tokio::runtime::Runtime;
use tokio::sync::mpsc::UnboundedSender;
use tracing::{debug, error, info, warn};

/// The executor for libp2p
struct Executor(Weak<Runtime>);

impl libp2p::core::Executor for Executor {
    fn exec(&self, f: Pin<Box<dyn Future<Output = ()> + Send>>) {
        if let Some(runtime) = self.0.upgrade() {
            info!("Executor: Spawning a task");
            runtime.spawn(f);
        } else {
            warn!("Executor: Couldn't spawn task. Runtime shutting down");
        }
    }
}

pub(crate) struct Network {
    swarm: Swarm<BehaviourComposer>,
    beacon_chain: Arc<RwLock<BeaconChain>>,
    sync_sender: UnboundedSender<SyncOperation>,
}

impl Network {
    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn new(
        beacon_chain: Arc<RwLock<BeaconChain>>,
        sync_sender: UnboundedSender<SyncOperation>,
        key_pair: Keypair,
        enr: Enr,
        enr_key: CombinedKey,
        network_config: NetworkConfig,
        peer_db: Arc<RwLock<PeerDB>>,
        runtime: Arc<Runtime>,
    ) -> Self {
        let transport = build_network_transport(key_pair).await;

        let local_peer_id = crate::identity::enr_to_peer_id(&enr);

        let behaviour =
            build_network_behaviour(enr, enr_key, network_config, peer_db, beacon_chain.clone())
                .await;

        let swarm = SwarmBuilder::new(transport, behaviour, local_peer_id)
            .executor(Box::new(Executor(Arc::downgrade(&runtime))))
            .build();

        Network {
            swarm,
            beacon_chain,
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

        match self.swarm.next().await.unwrap() {
            SwarmEvent::NewListenAddr { .. } => {}
            e => panic!("Unexpected event {:?}", e),
        };
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
                self.swarm
                    .behaviour_mut()
                    .rpc
                    .send_status(peer_id, self.beacon_chain.read().create_status_message());
            }
            PeerManagerEvent::NeedMorePeers => {
                let behaviour = self.swarm.behaviour_mut();
                if !behaviour.discovery.has_active_queries() {
                    behaviour.discovery.discover_peers();
                }
            }
            PeerManagerEvent::SendStatus(peer_id) => {
                self.swarm
                    .behaviour_mut()
                    .rpc
                    .send_status(peer_id, self.beacon_chain.read().create_status_message());
            }
            PeerManagerEvent::DisconnectPeer(peer_id, goodbye_reason) => {
                self.swarm
                    .behaviour_mut()
                    .rpc
                    .send_goodbye(peer_id, goodbye_reason);
            }
        }
    }

    // /////////////////////////////////////////////////////////////////////////////////////////////
    // RPC
    // /////////////////////////////////////////////////////////////////////////////////////////////
    fn handle_rpc_event(&mut self, event: RpcEvent) {
        match event {
            RpcEvent::ReceivedRequest(request) => match &request.request {
                InboundRequest::Status(message) => {
                    if self.validate_status_message(&request.peer_id, message) {
                        let behaviour = self.swarm.behaviour_mut();
                        behaviour.peer_manager.statusd_peer(request.peer_id);
                        behaviour.rpc.send_response(
                            request.peer_id,
                            request.connection_id,
                            request.substream_id,
                            lighthouse_network::Response::Status(
                                self.beacon_chain.read().create_status_message(),
                            ),
                        );
                    }
                }
                InboundRequest::Goodbye(_) => {}
                InboundRequest::BlocksByRange(_) => {}
                InboundRequest::BlocksByRoot(_) => {}
                InboundRequest::Ping(_) => {}
                InboundRequest::MetaData(_) => {}
            },
            RpcEvent::ReceivedResponse(response) => match &response.response {
                RPCResponse::Status(message) => {
                    if self.validate_status_message(&response.peer_id, message) {
                        self.swarm
                            .behaviour_mut()
                            .peer_manager
                            .statusd_peer(response.peer_id);
                    }
                }
                RPCResponse::BlocksByRange(_) => {}
                RPCResponse::BlocksByRoot(_) => {}
                RPCResponse::Pong(_) => {}
                RPCResponse::MetaData(_) => {}
            },
        }
    }

    fn validate_status_message(&mut self, peer_id: &PeerId, message: &StatusMessage) -> bool {
        if self.beacon_chain.read().is_relevant(message) {
            self.sync_sender
                .send(SyncOperation::AddPeer(*peer_id, message.clone().into()))
                .unwrap_or_else(|e| {
                    error!("Failed to send message to the sync manager: {}", e);
                });
            true
        } else {
            self.swarm.behaviour_mut().peer_manager.goodbye(
                peer_id,
                lighthouse_network::rpc::GoodbyeReason::IrrelevantNetwork,
            );
            false
        }
    }
}