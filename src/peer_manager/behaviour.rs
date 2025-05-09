use crate::peer_db::ConnectionStatus;
use crate::peer_manager::{PeerManager, PeerManagerEvent};
use futures::StreamExt;
use libp2p::core::transport::PortUse;
use libp2p::core::{ConnectedPoint, Endpoint};
use libp2p::swarm::dial_opts::{DialOpts, PeerCondition};
use libp2p::swarm::dummy::ConnectionHandler as DummyConnectionHandler;
use libp2p::swarm::{
    ConnectionDenied, ConnectionId, FromSwarm, NetworkBehaviour, THandler, THandlerInEvent,
    THandlerOutEvent, ToSwarm,
};
use libp2p::{Multiaddr, PeerId};
use std::task::{Context, Poll};
use std::time::Instant;
use tracing::info;
use tracing::log::{error, trace};

// SEE https://github.com/sigp/lighthouse/blob/eee0260a68696db58e92385ebd11a9a08e4c4665/beacon_node/lighthouse_network/src/peer_manager/network_behaviour.rs#L21
impl NetworkBehaviour for PeerManager {
    type ConnectionHandler = DummyConnectionHandler;
    type ToSwarm = PeerManagerEvent;

    fn handle_established_inbound_connection(
        &mut self,
        _connection_id: ConnectionId,
        _peer: PeerId,
        _local_addr: &Multiaddr,
        _remote_addr: &Multiaddr,
    ) -> Result<THandler<Self>, ConnectionDenied> {
        Ok(DummyConnectionHandler)
    }

    fn handle_established_outbound_connection(
        &mut self,
        _connection_id: ConnectionId,
        _peer: PeerId,
        _addr: &Multiaddr,
        _role_override: Endpoint,
        _port_use: PortUse,
    ) -> Result<THandler<Self>, ConnectionDenied> {
        Ok(DummyConnectionHandler)
    }

    fn on_swarm_event(&mut self, event: FromSwarm) {
        match event {
            FromSwarm::ConnectionEstablished(connection_established) => {
                trace!(
                    "[{}] Connection established. endpoint: {:?}",
                    connection_established.peer_id,
                    connection_established.endpoint
                );
                // TODO: Check the connection limits
                // https://github.com/sigp/lighthouse/blob/81c667b58e78243df38dc2d7311cb285f7c1d4f4/beacon_node/lighthouse_network/src/peer_manager/network_behaviour.rs#L142

                let address = match connection_established.endpoint {
                    // We dialed the node
                    ConnectedPoint::Dialer {
                        address,
                        role_override: _,
                        port_use: _,
                    } => {
                        self.peer_db
                            .write()
                            .add_peer(connection_established.peer_id, address.clone());
                        self.events.push(PeerManagerEvent::PeerConnectedOutgoing(
                            connection_established.peer_id,
                        ));
                        address
                    }
                    // We received the node
                    ConnectedPoint::Listener {
                        local_addr: _,
                        send_back_addr,
                    } => {
                        self.peer_db
                            .write()
                            .add_peer(connection_established.peer_id, send_back_addr.clone());
                        self.events.push(PeerManagerEvent::PeerConnectedIncoming(
                            connection_established.peer_id,
                        ));
                        send_back_addr
                    }
                };
                info!("[{}] on_swarm_event ConnectionEstablished -> Registered a peer. address: {address}", connection_established.peer_id);
            }
            FromSwarm::ConnectionClosed(connection_closed) => {
                if connection_closed.remaining_established > 0 {
                    return;
                }

                self.status_peers.remove(&connection_closed.peer_id);
                self.peer_db.write().update_connection_status(
                    &connection_closed.peer_id,
                    ConnectionStatus::Disconnected {
                        since: Instant::now(),
                    },
                );
                info!(
                    "[{}] on_swarm_event ConnectionClosed. endpoint: {:?}",
                    connection_closed.peer_id, connection_closed.endpoint
                );
            }
            FromSwarm::DialFailure(_) => {
                // TODO: https://github.com/sigp/lighthouse/blob/ff9b09d9646b712b2fd9fe26feeed5758daa0aa6/beacon_node/lighthouse_network/src/peer_manager/network_behaviour.rs#L130
            }
            FromSwarm::ExternalAddrConfirmed(_) => {
                // TODO:https://github.com/sigp/lighthouse/blob/ff9b09d9646b712b2fd9fe26feeed5758daa0aa6/beacon_node/lighthouse_network/src/peer_manager/network_behaviour.rs#L138
            }
            FromSwarm::AddressChange(_)
            | FromSwarm::ListenFailure(_)
            | FromSwarm::NewListener(_)
            | FromSwarm::NewListenAddr(_)
            | FromSwarm::ExpiredListenAddr(_)
            | FromSwarm::ListenerError(_)
            | FromSwarm::ListenerClosed(_)
            | FromSwarm::NewExternalAddrCandidate(_)
            | FromSwarm::ExternalAddrExpired(_) => {
                // The rest of the events we ignore since they are handled in their associated
                // `SwarmEvent`
            }
            _ => todo!(),
        }
    }

    fn on_connection_handler_event(
        &mut self,
        _peer_id: PeerId,
        _connection_id: ConnectionId,
        _event: THandlerOutEvent<Self>,
    ) {
        unreachable!("PeerManager does not use ConnectionHandler.")
    }

    fn poll(
        &mut self,
        cx: &mut Context<'_>,
    ) -> Poll<ToSwarm<Self::ToSwarm, THandlerInEvent<Self>>> {
        trace!("poll");

        while self.heartbeat.poll_tick(cx).is_ready() {
            if self.need_more_peers() {
                return Poll::Ready(ToSwarm::GenerateEvent(PeerManagerEvent::NeedMorePeers));
            }
        }

        if !self.events.is_empty() {
            // Emit peer manager event
            return Poll::Ready(ToSwarm::GenerateEvent(self.events.remove(0)));
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

            return Poll::Ready(ToSwarm::Dial {
                opts: DialOpts::peer_id(peer_id)
                    .condition(PeerCondition::Disconnected)
                    .build(),
            });
        }

        Poll::Pending
    }
}
