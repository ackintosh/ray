use crate::peer_manager::{PeerManager, PeerManagerEvent};
use libp2p::core::connection::ConnectionId;
use libp2p::core::ConnectedPoint;
use libp2p::swarm::{ConnectionHandler, IntoConnectionHandler, NetworkBehaviour, NetworkBehaviourAction, PollParameters};
use libp2p::{Multiaddr, PeerId};
use std::task::{Context, Poll};
use libp2p::swarm::handler::DummyConnectionHandler;
use tracing::info;
use crate::peer_manager::PeerManagerEvent::{PeerConnectedIncoming, PeerConnectedOutgoing};

// SEE https://github.com/sigp/lighthouse/blob/eee0260a68696db58e92385ebd11a9a08e4c4665/beacon_node/lighthouse_network/src/peer_manager/network_behaviour.rs#L21
impl NetworkBehaviour for PeerManager {
    type ConnectionHandler = DummyConnectionHandler;
    type OutEvent = PeerManagerEvent;

    fn new_handler(&mut self) -> Self::ConnectionHandler {
        DummyConnectionHandler::default()
    }

    fn inject_connection_established(
        &mut self,
        peer_id: &PeerId,
        _connection_id: &ConnectionId,
        endpoint: &ConnectedPoint,
        _failed_addresses: Option<&Vec<Multiaddr>>,
        _other_established: usize,
    ) {
        info!("inject_connection_established: peer_id: {}", peer_id);
        // TODO: Check the connection limits
        // https://github.com/sigp/lighthouse/blob/81c667b58e78243df38dc2d7311cb285f7c1d4f4/beacon_node/lighthouse_network/src/peer_manager/network_behaviour.rs#L142

        let address = match endpoint {
            // We dialed the node
            ConnectedPoint::Dialer { address, role_override: _ } => {
                self.peers.insert(*peer_id, address.clone());
                self.events.push(PeerConnectedOutgoing(peer_id.clone()));
                address
            }
            // We received the node
            ConnectedPoint::Listener {
                local_addr: _,
                send_back_addr,
            } => {
                self.peers.insert(*peer_id, send_back_addr.clone());
                self.events.push(PeerConnectedIncoming(peer_id.clone()));
                send_back_addr
            }
        };
        info!(
            "inject_connection_established -> Registered a peer. peer_id: {} address: {}",
            peer_id, address,
        );
    }

    fn inject_event(
        &mut self,
        _peer_id: PeerId,
        _connection: ConnectionId,
        _event: <<Self::ConnectionHandler as IntoConnectionHandler>::Handler as ConnectionHandler>::OutEvent,
    ) {
        unreachable!("PeerManager does not emit events")
    }

    fn poll(
        &mut self,
        _cx: &mut Context<'_>,
        _params: &mut impl PollParameters,
    ) -> Poll<NetworkBehaviourAction<Self::OutEvent, Self::ConnectionHandler>> {
        info!("poll");
        if !self.events.is_empty() {
            return Poll::Ready(NetworkBehaviourAction::GenerateEvent(self.events.remove(0)));
        }

        Poll::Pending
    }
}
