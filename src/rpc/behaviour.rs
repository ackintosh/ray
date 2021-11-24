use crate::rpc::handler::Handler;
use libp2p::core::connection::ConnectionId;
use libp2p::swarm::{
    DialError, IntoProtocolsHandler, NetworkBehaviour, NetworkBehaviourAction, PollParameters,
    ProtocolsHandler,
};
use libp2p::PeerId;
use std::task::{Context, Poll};
use tracing::{info, warn};

pub(crate) struct Behaviour;

// NetworkBehaviour defines "what" bytes to send on the network.
// SEE https://docs.rs/libp2p/0.39.1/libp2p/tutorial/index.html#network-behaviour
impl NetworkBehaviour for Behaviour {
    type ProtocolsHandler = Handler;
    type OutEvent = RpcEvent;

    fn new_handler(&mut self) -> Self::ProtocolsHandler {
        Handler
    }

    fn inject_connected(&mut self, peer_id: &PeerId) {
        info!("inject_connected: {}", peer_id);
    }

    fn inject_event(
        &mut self,
        peer_id: PeerId,
        _connection: ConnectionId,
        _event: <<Self::ProtocolsHandler as IntoProtocolsHandler>::Handler as ProtocolsHandler>::OutEvent,
    ) {
        info!("inject_event: {}", peer_id);
    }

    fn inject_dial_failure(
        &mut self,
        peer_id: Option<PeerId>,
        _handler: Self::ProtocolsHandler,
        _error: &DialError,
    ) {
        warn!("inject_dial_failure: {:?}", peer_id);
    }

    fn poll(
        &mut self,
        _cx: &mut Context<'_>,
        _params: &mut impl PollParameters,
    ) -> Poll<NetworkBehaviourAction<Self::OutEvent, Self::ProtocolsHandler>> {
        info!("poll");
        Poll::Pending
    }
}

pub enum RpcEvent {
    DummyEvent, // TODO
}
