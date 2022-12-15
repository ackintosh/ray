use crate::peer_db::ConnectionStatus;
use crate::peer_manager::{PeerManager, PeerManagerEvent};
use futures::StreamExt;
use libp2p::core::connection::ConnectionId;
use libp2p::core::ConnectedPoint;
use libp2p::swarm::dial_opts::{DialOpts, PeerCondition};
use libp2p::swarm::dummy::ConnectionHandler as DummyConnectionHandler;
use libp2p::swarm::{
    ConnectionHandler, IntoConnectionHandler, NetworkBehaviour, NetworkBehaviourAction,
    PollParameters,
};
use libp2p::{Multiaddr, PeerId};
use std::task::{Context, Poll};
use std::time::Instant;
use tracing::info;
use tracing::log::{error, trace};

// SEE https://github.com/sigp/lighthouse/blob/eee0260a68696db58e92385ebd11a9a08e4c4665/beacon_node/lighthouse_network/src/peer_manager/network_behaviour.rs#L21
impl NetworkBehaviour for PeerManager {
    type ConnectionHandler = DummyConnectionHandler;
    type OutEvent = PeerManagerEvent;

    fn new_handler(&mut self) -> Self::ConnectionHandler {
        DummyConnectionHandler {}
    }

    fn inject_connection_established(
        &mut self,
        peer_id: &PeerId,
        _connection_id: &ConnectionId,
        endpoint: &ConnectedPoint,
        _failed_addresses: Option<&Vec<Multiaddr>>,
        _other_established: usize,
    ) {
        trace!(
            "[{}] Connection established. endpoint: {:?}",
            peer_id,
            endpoint
        );
        // TODO: Check the connection limits
        // https://github.com/sigp/lighthouse/blob/81c667b58e78243df38dc2d7311cb285f7c1d4f4/beacon_node/lighthouse_network/src/peer_manager/network_behaviour.rs#L142

        let address = match endpoint {
            // We dialed the node
            ConnectedPoint::Dialer {
                address,
                role_override: _,
            } => {
                self.peer_db.write().add_peer(*peer_id, address.clone());
                self.events
                    .push(PeerManagerEvent::PeerConnectedOutgoing(*peer_id));
                address
            }
            // We received the node
            ConnectedPoint::Listener {
                local_addr: _,
                send_back_addr,
            } => {
                self.peer_db
                    .write()
                    .add_peer(*peer_id, send_back_addr.clone());
                self.events
                    .push(PeerManagerEvent::PeerConnectedIncoming(*peer_id));
                send_back_addr
            }
        };
        info!(
            "inject_connection_established -> Registered a peer. peer_id: {} address: {}",
            peer_id, address,
        );
    }

    fn inject_connection_closed(
        &mut self,
        peer_id: &PeerId,
        _: &ConnectionId,
        endpoint: &ConnectedPoint,
        _: <Self::ConnectionHandler as IntoConnectionHandler>::Handler,
        remaining_established: usize,
    ) {
        if remaining_established > 0 {
            return;
        }

        trace!("[{}] Connection closed. endpoint: {:?}", peer_id, endpoint);

        self.status_peers.remove(peer_id);
        self.peer_db.write().update_connection_status(
            peer_id,
            ConnectionStatus::Disconnected {
                since: Instant::now(),
            },
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
        cx: &mut Context<'_>,
        _params: &mut impl PollParameters,
    ) -> Poll<NetworkBehaviourAction<Self::OutEvent, Self::ConnectionHandler>> {
        trace!("poll");

        while self.heartbeat.poll_tick(cx).is_ready() {
            if self.need_more_peers() {
                return Poll::Ready(NetworkBehaviourAction::GenerateEvent(
                    PeerManagerEvent::NeedMorePeers,
                ));
            }
        }

        if !self.events.is_empty() {
            // Emit peer manager event
            return Poll::Ready(NetworkBehaviourAction::GenerateEvent(self.events.remove(0)));
        }

        // Clients need to send Status request again to learn if the peer has a higher head.
        // https://github.com/ethereum/consensus-specs/blob/dev/specs/phase0/p2p-interface.md#status
        loop {
            match self.status_peers.poll_next_unpin(cx) {
                Poll::Ready(Some(Ok(peer_id))) => {
                    self.status_peers.insert(peer_id);
                    self.events.push(PeerManagerEvent::SendStatus(peer_id));
                }
                Poll::Ready(Some(Err(e))) => {
                    error!("Failed to check for peers to status. error: {}", e);
                }
                Poll::Ready(None) | Poll::Pending => {
                    break;
                }
            }
        }

        if let Some(peer_id) = self.peers_to_dial.pop_front() {
            trace!("[{}] Dialing to the peer.", peer_id);

            let handler = self.new_handler();
            return Poll::Ready(NetworkBehaviourAction::Dial {
                opts: DialOpts::peer_id(peer_id)
                    .condition(PeerCondition::Disconnected)
                    .build(),
                handler,
            });
        }

        Poll::Pending
    }
}
