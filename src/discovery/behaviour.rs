use crate::discovery::DiscoveryEvent;
use discv5::enr::{CombinedKey, NodeId};
use discv5::{Discv5, Discv5ConfigBuilder, Discv5Event, Enr, QueryError};
use futures::stream::FuturesUnordered;
use futures::{Future, FutureExt, StreamExt};
use libp2p::core::Endpoint;
use libp2p::swarm::dummy::{ConnectionHandler as DummyConnectionHandler, ConnectionHandler};
use libp2p::swarm::{
    ConnectionDenied, ConnectionId, DialError, DialFailure, FromSwarm, NetworkBehaviour,
    PollParameters, THandler, THandlerInEvent, THandlerOutEvent, ToSwarm,
};
use libp2p::{Multiaddr, PeerId};
use lru::LruCache;
use std::net::SocketAddr;
use std::num::NonZeroUsize;
use std::task::{Context, Poll};
use std::time::Duration;
use tokio::sync::mpsc::Receiver;
use tracing::{debug, error, info, trace, warn};

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
            cached_enrs: LruCache::new(NonZeroUsize::new(50).expect("non zero usize")),
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

    fn on_dial_failure(&self, peer_id: Option<PeerId>, dial_error: &DialError) {
        if let Some(peer_id) = peer_id {
            match dial_error {
                DialError::LocalPeerId { .. }
                | DialError::NoAddresses
                | DialError::WrongPeerId { .. }
                | DialError::Denied { .. }
                | DialError::Transport(_) => {
                    debug!("[{peer_id}] Marking peer disconnected in DHT. error: {dial_error}");
                    match crate::identity::peer_id_to_node_id(&peer_id) {
                        Ok(node_id) => {
                            let _ = self.discv5.disconnect_node(&node_id);
                        }
                        Err(e) => {
                            warn!(
                                "[{peer_id}] Failed to convert from PeerId to NodeId. error: {e}"
                            );
                        }
                    }
                }
                DialError::Aborted | DialError::DialPeerConditionFalse(_) => {}
            }
        }
    }
}

// ************************************************
// *** Discovery is not a real NetworkBehaviour ***
// ************************************************
// A NetworkBehaviour represent a protocol's state across all peers and connections.
// SEE https://github.com/libp2p/rust-libp2p/releases/tag/libp2p-v0.52.0
// NetworkBehaviour defines "what" bytes to send on the network.
// SEE https://docs.rs/libp2p/0.39.1/libp2p/tutorial/index.html#network-behaviour
impl NetworkBehaviour for Behaviour {
    type ConnectionHandler = DummyConnectionHandler;
    type ToSwarm = DiscoveryEvent;

    fn handle_established_inbound_connection(
        &mut self,
        _connection_id: ConnectionId,
        _peer: PeerId,
        _local_addr: &Multiaddr,
        _remote_addr: &Multiaddr,
    ) -> Result<THandler<Self>, ConnectionDenied> {
        Ok(DummyConnectionHandler)
    }

    fn handle_pending_outbound_connection(
        &mut self,
        _connection_id: ConnectionId,
        maybe_peer: Option<PeerId>,
        _addresses: &[Multiaddr],
        _effective_role: Endpoint,
    ) -> Result<Vec<Multiaddr>, ConnectionDenied> {
        if let Some(peer_id) = maybe_peer {
            trace!("[{peer_id}] handle_pending_outbound_connection");
            // First search the local cache.
            if let Some(enr) = self.cached_enrs.get(&peer_id) {
                let multiaddr = crate::identity::enr_to_multiaddrs(enr);
                trace!("[{peer_id}] handle_pending_outbound_connection: Found from the cached_enrs. multiaddr: {multiaddr:?}");
                return Ok(multiaddr);
            }

            // Not in the local cache, look in the routing table.
            match crate::identity::peer_id_to_node_id(&peer_id) {
                Ok(node_id) => match self.discv5.find_enr(&node_id) {
                    Some(enr) => {
                        let multiaddr = crate::identity::enr_to_multiaddrs(&enr);
                        trace!("[{peer_id}] handle_pending_outbound_connection: Found from the DHT. node_id: {node_id}, multiaddr: {multiaddr:?}");
                        Ok(multiaddr)
                    }
                    None => {
                        warn!("[{peer_id}] handle_pending_outbound_connection: No addresses found. node_id: {node_id}");
                        Ok(vec![])
                    }
                },
                Err(e) => {
                    warn!("[{peer_id}] handle_pending_outbound_connection: Failed to derive node_id from peer_id. error: {:?}", e);
                    Ok(vec![])
                }
            }
        } else {
            Ok(vec![])
        }
    }

    fn handle_established_outbound_connection(
        &mut self,
        _connection_id: ConnectionId,
        _peer: PeerId,
        _addr: &Multiaddr,
        _role_override: Endpoint,
    ) -> Result<THandler<Self>, ConnectionDenied> {
        Ok(DummyConnectionHandler)
    }

    fn on_swarm_event(&mut self, event: FromSwarm<Self::ConnectionHandler>) {
        match event {
            FromSwarm::DialFailure(DialFailure {
                peer_id,
                error,
                connection_id: _,
            }) => {
                self.on_dial_failure(peer_id, error);
            }
            FromSwarm::ConnectionEstablished(_)
            | FromSwarm::ConnectionClosed(_)
            | FromSwarm::AddressChange(_)
            | FromSwarm::ListenFailure(_)
            | FromSwarm::NewListener(_)
            | FromSwarm::NewListenAddr(_)
            | FromSwarm::ExpiredListenAddr(_)
            | FromSwarm::ListenerError(_)
            | FromSwarm::ListenerClosed(_)
            | FromSwarm::NewExternalAddrCandidate(_)
            | FromSwarm::ExternalAddrExpired(_)
            | FromSwarm::ExternalAddrConfirmed(_) => {
                // Ignore events not relevant to discovery
            }
        }
    }

    fn on_connection_handler_event(
        &mut self,
        _peer_id: PeerId,
        _connection_id: ConnectionId,
        _event: THandlerOutEvent<Self>,
    ) {
        // Nothing to do.
    }

    #[allow(clippy::single_match)]
    fn poll(
        &mut self,
        cx: &mut Context<'_>,
        _params: &mut impl PollParameters,
    ) -> Poll<ToSwarm<Self::ToSwarm, THandlerInEvent<Self>>> {
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

                    Poll::Ready(ToSwarm::GenerateEvent(DiscoveryEvent::FoundPeers(peers)))
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
