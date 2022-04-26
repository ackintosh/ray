use crate::rpc::handler::Handler;
use libp2p::core::connection::ConnectionId;
use libp2p::swarm::{
    ConnectionHandler, DialError, IntoConnectionHandler, NetworkBehaviour, NetworkBehaviourAction,
    PollParameters,
};
use libp2p::{Multiaddr, PeerId};
use std::task::{Context, Poll};
use tracing::{info, warn};

pub(crate) struct Behaviour;

// NetworkBehaviour defines "what" bytes to send on the network.
// SEE https://docs.rs/libp2p/0.39.1/libp2p/tutorial/index.html#network-behaviour
impl NetworkBehaviour for Behaviour {
    type ConnectionHandler = Handler;
    type OutEvent = RpcEvent;

    fn new_handler(&mut self) -> Self::ConnectionHandler {
        Handler
    }

    fn addresses_of_peer(&mut self, _peer_id: &PeerId) -> Vec<Multiaddr> {
        info!("addresses_of_peer -> nothing to do because this event is handled by discovery.");
        vec![]
    }

    fn inject_event(
        &mut self,
        peer_id: PeerId,
        _connection: ConnectionId,
        _event: <<Self::ConnectionHandler as IntoConnectionHandler>::Handler as ConnectionHandler>::OutEvent,
    ) {
        info!("inject_event -> {}", peer_id);
    }

    fn inject_dial_failure(
        &mut self,
        peer_id: Option<PeerId>,
        _handler: Self::ConnectionHandler,
        error: &DialError,
    ) {
        warn!(
            "inject_dial_failure: peer_id: {:?}, error: {}",
            peer_id, error
        );
    }

    fn poll(
        &mut self,
        _cx: &mut Context<'_>,
        _params: &mut impl PollParameters,
    ) -> Poll<NetworkBehaviourAction<Self::OutEvent, Self::ConnectionHandler>> {
        info!("poll");
        Poll::Pending
    }
}

pub enum RpcEvent {
    DummyEvent, // TODO
}
