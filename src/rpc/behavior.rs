use libp2p::PeerId;
use libp2p::core::connection::ConnectionId;
use libp2p::swarm::{IntoProtocolsHandler, NetworkBehaviour, NetworkBehaviourAction, PollParameters, ProtocolsHandler};
use std::task::{Context, Poll};
use crate::rpc::handler::Handler;

pub(crate) struct Behaviour;

impl NetworkBehaviour for Behaviour {
    type ProtocolsHandler = Handler;
    type OutEvent = ();

    fn new_handler(&mut self) -> Self::ProtocolsHandler {
        Handler
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
        Poll::Pending
    }
}
