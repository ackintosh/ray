use crate::discovery::DiscoveryEvent;
use discv5::enr::{CombinedKey, NodeId};
use discv5::{Discv5, Discv5ConfigBuilder, Discv5Event, Enr, QueryError};
use futures::stream::FuturesUnordered;
use futures::{Future, FutureExt, StreamExt};
use libp2p::core::connection::ConnectionId;
use libp2p::swarm::handler::DummyConnectionHandler;
use libp2p::swarm::{
    ConnectionHandler, IntoConnectionHandler, NetworkBehaviour, NetworkBehaviourAction,
    PollParameters,
};
use libp2p::{Multiaddr, PeerId};
use lru::LruCache;
use std::net::SocketAddr;
use std::task::{Context, Poll};
use std::time::Duration;
use tokio::sync::mpsc::Receiver;
use tracing::{error, info, trace, warn};

// ////////////////////////////////////////////////////////
// Internal message of Discovery module
// ////////////////////////////////////////////////////////

// The result of a query.
struct QueryResult {
    result: Result<Vec<Enr>, discv5::QueryError>,
}

// ////////////////////////////////////////////////////////
// Behaviour
// ////////////////////////////////////////////////////////

pub(crate) struct Behaviour {
    discv5: Discv5,
    event_stream: Receiver<Discv5Event>,
    // Active discovery queries.
    active_queries: FuturesUnordered<std::pin::Pin<Box<dyn Future<Output = QueryResult> + Send>>>,
    // A collection of seen live ENRs for quick lookup and to map peer-id's to ENRs.
    cached_enrs: LruCache<PeerId, Enr>,
}

impl Behaviour {
    pub(crate) async fn new(
        local_enr: Enr,
        local_enr_key: CombinedKey,
        boot_enr: &Vec<Enr>,
    ) -> Self {
        let config = Discv5ConfigBuilder::new()
            // For ease to observe the `Discv5Event::SocketUpdated` event, set a short duration here.
            .ping_interval(Duration::from_secs(10))
            .build();
        // construct the discv5 server
        let mut discv5 = Discv5::new(local_enr, local_enr_key, config).unwrap();

        for enr in boot_enr {
            info!("Boot ENR: {}", enr);
            if let Err(e) = discv5.add_enr(enr.clone()) {
                warn!("Failed to add Boot ENR: {:?}", e);
            }
        }

        // start the discv5 server
        let listen_addr = "0.0.0.0:9000".parse::<SocketAddr>().unwrap();
        // TODO: error handling
        // SEE https://github.com/sigp/lighthouse/blob/73ec29c267f057e70e89856403060c4c35b5c0c8/beacon_node/eth2_libp2p/src/discovery/mod.rs#L235-L238
        discv5.start(listen_addr).await.unwrap();
        info!(
            "Started Discovery v5 server. local_enr: {}",
            discv5.local_enr()
        );

        // TODO: error handling
        let event_stream = discv5.event_stream().await.unwrap();

        Behaviour {
            discv5,
            event_stream,
            active_queries: FuturesUnordered::new(),
            cached_enrs: LruCache::new(50),
        }
    }

    pub(crate) fn has_active_queries(&self) -> bool {
        !self.active_queries.is_empty()
    }

    pub(crate) fn discover_peers(&mut self) {
        let target_node = NodeId::random();
        let query_future = self
            .discv5
            .find_node(target_node)
            .map(|result: Result<Vec<Enr>, QueryError>| QueryResult { result });

        info!(
            "Active query for discovery: target_node(random) -> {}",
            target_node
        );
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
    type ConnectionHandler = DummyConnectionHandler;
    type OutEvent = DiscoveryEvent;

    fn new_handler(&mut self) -> Self::ConnectionHandler {
        DummyConnectionHandler::default()
    }

    fn addresses_of_peer(&mut self, peer_id: &PeerId) -> Vec<Multiaddr> {
        trace!("addresses_of_peer: peer_id: {}", peer_id);

        // First search the local cache.
        if let Some(enr) = self.cached_enrs.get(peer_id) {
            let multiaddr = crate::identity::enr_to_multiaddrs(enr);
            trace!(
                "addresses_of_peer: Found from the cached_enrs. peer_id: {}, multiaddr: {:?}",
                peer_id,
                multiaddr,
            );
            return multiaddr;
        }

        // Not in the local cache, look in the routing table.
        match crate::identity::peer_id_to_node_id(peer_id) {
            Ok(node_id) => match self.discv5.find_enr(&node_id) {
                Some(enr) => {
                    let multiaddr = crate::identity::enr_to_multiaddrs(&enr);
                    trace!(
                        "addresses_of_peer: Found from the DHT. peer_id: {}, node_id: {}, multiaddr: {:?}",
                        peer_id,
                        node_id,
                        multiaddr,
                    );
                    multiaddr
                }
                None => {
                    warn!(
                        "addresses_of_peer: No addresses found. peer_id: {}, node_id: {}",
                        peer_id, node_id,
                    );
                    vec![]
                }
            },
            Err(e) => {
                warn!(
                    "addresses_of_peer: Failed to derive node_id from peer_id. error: {:?}, peer_id: {}",
                    e,
                    peer_id,
                );
                vec![]
            }
        }
    }

    fn inject_event(
        &mut self,
        _peer_id: PeerId,
        _connection: ConnectionId,
        _event: <<Self::ConnectionHandler as IntoConnectionHandler>::Handler as ConnectionHandler>::OutEvent,
    ) {
        trace!("inject_event -> nothing to do");
        // SEE https://github.com/sigp/lighthouse/blob/73ec29c267f057e70e89856403060c4c35b5c0c8/beacon_node/eth2_libp2p/src/discovery/mod.rs#L948-L954
    }

    #[allow(clippy::single_match)]
    fn poll(
        &mut self,
        cx: &mut Context<'_>,
        _params: &mut impl PollParameters,
    ) -> Poll<NetworkBehaviourAction<Self::OutEvent, Self::ConnectionHandler>> {
        trace!("poll");

        if let Poll::Ready(Some(query_result)) = self.active_queries.poll_next_unpin(cx) {
            trace!("poll -> self.active_queries");
            return match query_result.result {
                Ok(enrs) if enrs.is_empty() => {
                    info!("Discovery query yielded no results.");
                    Poll::Pending
                }
                Ok(enrs) => {
                    info!("Discovery query completed. found peers: {:?}", enrs);
                    // NOTE: Ideally we need to filter out peers from the result.
                    // https://github.com/sigp/lighthouse/blob/9c5a8ab7f2098d1ffc567af27f385c55f471cb9c/beacon_node/eth2_libp2p/src/peer_manager/mod.rs#L256
                    let peers = enrs
                        .iter()
                        .map(crate::identity::enr_to_peer_id)
                        .collect::<Vec<_>>();

                    // Cache the found ENRs
                    for (p, e) in peers.iter().zip(enrs.iter()) {
                        self.cached_enrs.put(*p, e.clone());
                    }

                    Poll::Ready(NetworkBehaviourAction::GenerateEvent(
                        DiscoveryEvent::FoundPeers(peers),
                    ))
                }
                Err(query_error) => {
                    error!("Discovery query failed: {}", query_error);
                    Poll::Pending
                }
            };
        }

        while let Poll::Ready(Some(event)) = self.event_stream.poll_recv(cx) {
            match event {
                Discv5Event::SocketUpdated(socket_addr) => {
                    info!("Discv5Event::SocketUpdated. {:?}", socket_addr);
                }
                _ => {} // Discv5Event::Discovered(_) => {}
                        // Discv5Event::NodeInserted { .. } => {}
                        // Discv5Event::EnrAdded { .. } => {}
                        // Discv5Event::TalkRequest(_) => {}
            }
        }

        Poll::Pending
    }
}
