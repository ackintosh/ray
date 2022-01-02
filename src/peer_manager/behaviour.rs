use std::error::Error;
use libp2p::core::connection::{ConnectionId, ListenerId};
use libp2p::swarm::protocols_handler::DummyProtocolsHandler;
use libp2p::swarm::{DialError, IntoProtocolsHandler, NetworkBehaviour, NetworkBehaviourAction, PollParameters, ProtocolsHandler};
use libp2p::{Multiaddr, PeerId};
use std::task::{Context, Poll};
use libp2p::core::ConnectedPoint;
use tracing::info;

pub(crate) struct Behaviour;

// SEE https://github.com/sigp/lighthouse/blob/eee0260a68696db58e92385ebd11a9a08e4c4665/beacon_node/lighthouse_network/src/peer_manager/network_behaviour.rs#L21
impl NetworkBehaviour for Behaviour {
    type ProtocolsHandler = DummyProtocolsHandler;
    type OutEvent = PeerManagerEvent;

    fn new_handler(&mut self) -> Self::ProtocolsHandler {
        libp2p::swarm::protocols_handler::DummyProtocolsHandler::default()
    }

    fn inject_connected(&mut self, peer_id: &PeerId) {
        info!("inject_connected: peer_id: {}", peer_id);
    }

    fn inject_connection_established(&mut self, peer_id: &PeerId, _connection_id: &ConnectionId, _endpoint: &ConnectedPoint, _failed_addresses: Option<&Vec<Multiaddr>>) {
        info!("inject_connection_established: peer_id: {}", peer_id);
        // TODO: Check the connection limits
        // https://github.com/sigp/lighthouse/blob/81c667b58e78243df38dc2d7311cb285f7c1d4f4/beacon_node/lighthouse_network/src/peer_manager/network_behaviour.rs#L142
    }

    fn inject_event(
        &mut self,
        _peer_id: PeerId,
        _connection: ConnectionId,
        _event: <<Self::ProtocolsHandler as IntoProtocolsHandler>::Handler as ProtocolsHandler>::OutEvent,
    ) {
        unreachable!("PeerManager does not emit events")
    }

    fn poll(
        &mut self,
        cx: &mut Context<'_>,
        params: &mut impl PollParameters,
    ) -> Poll<NetworkBehaviourAction<Self::OutEvent, Self::ProtocolsHandler>> {
        info!("poll");
        Poll::Pending
    }
}

/// The events emitted from PeerManager to BehaviourComposer.
// NOTE: unused for now
pub(crate) enum PeerManagerEvent {}
