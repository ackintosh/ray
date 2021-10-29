use std::error::Error;
use libp2p::{Multiaddr, PeerId};
use libp2p::core::connection::{ConnectionId, ListenerId};
use libp2p::swarm::{DialError, IntoProtocolsHandler, NetworkBehaviour, NetworkBehaviourAction, PollParameters, ProtocolsHandler};
use std::task::{Context, Poll};
use libp2p::core::ConnectedPoint;
use tracing::{info, warn};
use crate::rpc::handler::Handler;

pub(crate) struct Behaviour;

impl NetworkBehaviour for Behaviour {
    type ProtocolsHandler = Handler;
    type OutEvent = ();

    fn new_handler(&mut self) -> Self::ProtocolsHandler {
        Handler
    }

    fn inject_connected(&mut self, peer_id: &PeerId) {
        info!("inject_connected: {}", peer_id);
    }

    fn inject_dial_failure(&mut self, peer_id: Option<PeerId>, _handler: Self::ProtocolsHandler, _error: &DialError) {
        warn!("inject_dial_failure: {:?}", peer_id);
    }

    fn inject_event(
        &mut self,
        peer_id: PeerId,
        connection: ConnectionId,
        event: <<Self::ProtocolsHandler as IntoProtocolsHandler>::Handler as ProtocolsHandler>::OutEvent
    ) {
        println!("inject_event: {}", peer_id);
    }

    fn poll(
        &mut self,
        cx: &mut Context<'_>,
        params: &mut impl PollParameters
    ) -> Poll<NetworkBehaviourAction<Self::OutEvent, Self::ProtocolsHandler>> {
        println!("poll");
        info!("poll");
        Poll::Pending
    }
}
