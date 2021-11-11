use crate::discovery::boot_enrs;
use discv5::{Discv5, Discv5ConfigBuilder};
use enr::{CombinedKey, Enr, NodeId};
use futures::stream::FuturesUnordered;
use futures::{Future, FutureExt, StreamExt};
use libp2p::core::connection::ConnectionId;
use libp2p::swarm::{
    IntoProtocolsHandler, NetworkBehaviour, NetworkBehaviourAction, PollParameters,
    ProtocolsHandler,
};
use libp2p::PeerId;
use std::net::SocketAddr;
use std::task::{Context, Poll};
use tracing::{error, info, warn};

pub(crate) struct Behaviour {
    discv5: Discv5,
    /// Active discovery queries.
    active_queries: FuturesUnordered<std::pin::Pin<Box<dyn Future<Output = QueryResult> + Send>>>,
}

impl Behaviour {
    pub(crate) async fn new(local_enr: Enr<CombinedKey>, local_enr_key: CombinedKey) -> Self {
        // default configuration
        let config = Discv5ConfigBuilder::new().build();
        // construct the discv5 server
        let mut discv5 = Discv5::new(local_enr, local_enr_key, config).unwrap();

        for boot_enr in boot_enrs() {
            info!("Boot ENR: {}", boot_enr);
            if let Err(e) = discv5.add_enr(boot_enr) {
                warn!("Failed to add Boot ENR: {:?}", e);
            }
        }

        // start the discv5 server
        let listen_addr = "0.0.0.0:19000".parse::<SocketAddr>().unwrap();
        // TODO: error handling
        // SEE https://github.com/sigp/lighthouse/blob/73ec29c267f057e70e89856403060c4c35b5c0c8/beacon_node/eth2_libp2p/src/discovery/mod.rs#L235-L238
        discv5.start(listen_addr).await.unwrap();

        // establish a session by running a query
        // info!("Executing bootstrap query.");
        // let found = discv5
        //     .find_node(NodeId::random())
        //     .map(|result| {
        //         QueryResult { result }
        //     });
        // info!("Found: {:?}", found);

        // let mut event_stream = match runtime.block_on(discv5.event_stream()) {
        //     Ok(event_stream) => event_stream,
        //     Err(e) => {
        //         error!("Failed to obtain event stream: {}", e);
        //         exit(1);
        //     }
        // };

        // let peers: Arc<RwLock<Vec<Enr<CombinedKey>>>> = Arc::new(RwLock::new(vec![]));
        // runtime.spawn(async move {
        //     loop {
        //         tokio::select! {
        //             Some(event) = event_stream.recv() => {
        //                 match event {
        //                     Discv5Event::Discovered(enr) => {
        //                         info!("Discv5Event::Discovered: {}", enr);
        //                         peers.write().unwrap().push(enr);
        //                     }
        //                     Discv5Event::EnrAdded { enr, replaced } => {
        //                         info!("Discv5Event::EnrAdded: {}, {:?}", enr, replaced);
        //                     }
        //                     Discv5Event::TalkRequest(_)  => {}     // Ignore
        //                     Discv5Event::NodeInserted { node_id, replaced } => {
        //                         info!("Discv5Event::NodeInserted: {}, {:?}", node_id, replaced);
        //                     }
        //                     Discv5Event::SocketUpdated(socket_addr) => {
        //                         info!("External socket address updated: {}", socket_addr);
        //                     }
        //                 }
        //             }
        //         }
        //     }
        // });

        Behaviour {
            discv5,
            active_queries: FuturesUnordered::new(),
        }
    }

    pub(crate) async fn discover_peers(&mut self) {
        let target_node = NodeId::random();
        let query_future = self
            .discv5
            .find_node(target_node.clone())
            .map(|result| QueryResult { result });

        info!("Active query for discovery: target_node -> {}", target_node);
        self.active_queries.push(Box::pin(query_future));
    }
}

// ************************************************
// *** Discovery is not a real NetworkBehaviour ***
// ************************************************
// SEE https://github.com/sigp/lighthouse/blob/73ec29c267f057e70e89856403060c4c35b5c0c8/beacon_node/eth2_libp2p/src/discovery/mod.rs#L911
// NetworkBehaviour defines "what" bytes to send on the network.
// SEE https://docs.rs/libp2p/0.39.1/libp2p/tutorial/index.html#network-behaviour
impl NetworkBehaviour for Behaviour {
    type ProtocolsHandler = libp2p::swarm::protocols_handler::DummyProtocolsHandler;
    type OutEvent = DiscoveryEvent;

    fn new_handler(&mut self) -> Self::ProtocolsHandler {
        libp2p::swarm::protocols_handler::DummyProtocolsHandler::default()
    }

    fn inject_event(
        &mut self,
        _peer_id: PeerId,
        _connection: ConnectionId,
        _event: <<Self::ProtocolsHandler as IntoProtocolsHandler>::Handler as ProtocolsHandler>::OutEvent,
    ) {
        info!("inject_event: nothing to do");
        // SEE https://github.com/sigp/lighthouse/blob/73ec29c267f057e70e89856403060c4c35b5c0c8/beacon_node/eth2_libp2p/src/discovery/mod.rs#L948-L954
    }

    fn poll(
        &mut self,
        cx: &mut Context<'_>,
        _params: &mut impl PollParameters,
    ) -> Poll<NetworkBehaviourAction<Self::OutEvent, Self::ProtocolsHandler>> {
        info!("poll");
        if let Poll::Ready(Some(query_result)) = self.active_queries.poll_next_unpin(cx) {
            return match query_result.result {
                Ok(enrs) if enrs.is_empty() => {
                    info!("Discovery query yielded no results.");
                    Poll::Pending
                }
                Ok(enrs) => {
                    info!("Discovery query completed. found peers: {:?}", enrs);
                    Poll::Ready(NetworkBehaviourAction::GenerateEvent(
                        DiscoveryEvent::QueryResult(enrs),
                    ))
                }
                Err(query_error) => {
                    error!("Discovery query failed: {}", query_error);
                    Poll::Pending
                }
            };
        }
        Poll::Pending
    }
}

/// The result of a query.
struct QueryResult {
    result: Result<Vec<Enr<CombinedKey>>, discv5::QueryError>,
}

/// The events emitted by polling discovery.
pub enum DiscoveryEvent {
    QueryResult(Vec<Enr<CombinedKey>>),
}
