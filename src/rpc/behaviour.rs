use crate::network::ReqId;
use crate::rpc::handler::{Handler, SubstreamId, ToBehaviour};
use crate::rpc::{ReceivedRequest, ReceivedResponse, RpcEvent};
use libp2p::core::Endpoint;
use libp2p::swarm::{
    CloseConnection, ConnectionDenied, ConnectionId, FromSwarm, NetworkBehaviour, NotifyHandler,
    PollParameters, THandler, THandlerInEvent, THandlerOutEvent, ToSwarm,
};
use libp2p::{Multiaddr, PeerId};
use std::sync::Arc;
use std::task::{Context, Poll};
use tracing::{info, trace};
use types::{ForkContext, MainnetEthSpec};

// ////////////////////////////////////////////////////////
// Internal message of RPC module sent by Behaviour
// ////////////////////////////////////////////////////////

// RPC internal message sent from behaviour to handlers
#[derive(Debug)]
pub(crate) enum InstructionToHandler<Id> {
    Status(Id, lighthouse_network::rpc::StatusMessage, PeerId),
    Goodbye(Id, lighthouse_network::rpc::GoodbyeReason, PeerId),
    Request(
        Id,
        lighthouse_network::rpc::outbound::OutboundRequest<MainnetEthSpec>,
        PeerId,
    ),
    Response(
        SubstreamId,
        lighthouse_network::Response<MainnetEthSpec>,
        PeerId,
    ),
}

// ////////////////////////////////////////////////////////
// Behaviour
// ////////////////////////////////////////////////////////

pub(crate) struct Behaviour<Id: ReqId> {
    events: Vec<ToSwarm<RpcEvent, InstructionToHandler<Id>>>,
    fork_context: Arc<ForkContext>,
}

impl<Id: ReqId> Behaviour<Id> {
    pub(crate) fn new(fork_context: Arc<ForkContext>) -> Self {
        Behaviour {
            events: vec![],
            fork_context,
        }
    }

    // Status
    // https://github.com/ethereum/consensus-specs/blob/dev/specs/phase0/p2p-interface.md#status
    pub(crate) fn send_status(
        &mut self,
        request_id: Id,
        peer_id: PeerId,
        message: lighthouse_network::rpc::StatusMessage,
    ) {
        trace!("[{}] Sending Status to the peer.", peer_id);
        // Notify ConnectionHandler, then the handler's `inject_event` is invoked with the event.
        self.events.push(ToSwarm::NotifyHandler {
            peer_id,
            handler: NotifyHandler::Any,
            event: InstructionToHandler::Status(request_id, message, peer_id),
        })
    }

    // Goodbye
    // https://github.com/ethereum/consensus-specs/blob/dev/specs/phase0/p2p-interface.md#goodbye
    pub(crate) fn send_goodbye(
        &mut self,
        request_id: Id,
        peer_id: PeerId,
        reason: lighthouse_network::rpc::GoodbyeReason,
    ) {
        self.events.push(ToSwarm::NotifyHandler {
            peer_id,
            handler: NotifyHandler::Any,
            event: InstructionToHandler::Goodbye(request_id, reason, peer_id),
        })
    }

    pub(crate) fn send_request(
        &mut self,
        peer_id: PeerId,
        request: lighthouse_network::service::api_types::Request,
        request_id: Id,
    ) {
        self.events.push(ToSwarm::NotifyHandler {
            peer_id,
            handler: NotifyHandler::Any,
            event: InstructionToHandler::Request(request_id, request.into(), peer_id),
        })
    }

    pub(crate) fn send_response(
        &mut self,
        peer_id: PeerId,
        connection_id: ConnectionId,
        substream_id: SubstreamId,
        response: lighthouse_network::Response<MainnetEthSpec>,
    ) {
        self.events.push(ToSwarm::NotifyHandler {
            peer_id,
            handler: NotifyHandler::One(connection_id),
            event: InstructionToHandler::Response(substream_id, response, peer_id),
        })
    }
}

// NetworkBehaviour defines "what" bytes to send on the network.
// SEE https://docs.rs/libp2p/0.39.1/libp2p/tutorial/index.html#network-behaviour
impl<Id: ReqId> NetworkBehaviour for Behaviour<Id> {
    type ConnectionHandler = Handler<Id>;
    type ToSwarm = RpcEvent;

    fn handle_established_inbound_connection(
        &mut self,
        _connection_id: ConnectionId,
        peer_id: PeerId,
        _local_addr: &Multiaddr,
        _remote_addr: &Multiaddr,
    ) -> Result<THandler<Self>, ConnectionDenied> {
        Ok(Handler::new(peer_id, self.fork_context.clone()))
    }

    fn handle_established_outbound_connection(
        &mut self,
        _connection_id: ConnectionId,
        peer_id: PeerId,
        _addr: &Multiaddr,
        _role_override: Endpoint,
    ) -> Result<THandler<Self>, ConnectionDenied> {
        Ok(Handler::new(peer_id, self.fork_context.clone()))
    }

    fn on_swarm_event(&mut self, event: FromSwarm<Self::ConnectionHandler>) {
        match event {
            FromSwarm::ConnectionClosed(_)
            | FromSwarm::ConnectionEstablished(_)
            | FromSwarm::AddressChange(_)
            | FromSwarm::DialFailure(_)
            | FromSwarm::ListenFailure(_)
            | FromSwarm::NewListener(_)
            | FromSwarm::NewListenAddr(_)
            | FromSwarm::ExpiredListenAddr(_)
            | FromSwarm::ListenerError(_)
            | FromSwarm::ListenerClosed(_)
            | FromSwarm::NewExternalAddrCandidate(_)
            | FromSwarm::ExternalAddrExpired(_)
            | FromSwarm::ExternalAddrConfirmed(_) => {
                // Rpc Behaviour does not act on these swarm events. We use a comprehensive match
                // statement to ensure future events are dealt with appropriately.
            }
        };
    }

    fn on_connection_handler_event(
        &mut self,
        peer_id: PeerId,
        connection_id: ConnectionId,
        event: THandlerOutEvent<Self>,
    ) {
        match event {
            ToBehaviour::RequestReceived(inbound_request) => {
                info!(
                    "[{}] [on_connection_handler_event] Received request: {:?}",
                    peer_id, inbound_request
                );
                self.events
                    .push(ToSwarm::GenerateEvent(RpcEvent::ReceivedRequest(
                        ReceivedRequest {
                            peer_id,
                            connection_id,
                            substream_id: inbound_request.substream_id,
                            request: inbound_request.request,
                        },
                    )));
            }
            ToBehaviour::ResponseReceived(response) => {
                info!(
                    "[{}] [on_connection_handler_event] Received response: {:?}",
                    peer_id, response
                );
                self.events
                    .push(ToSwarm::GenerateEvent(RpcEvent::ReceivedResponse(
                        ReceivedResponse { peer_id, response },
                    )));
            }
            ToBehaviour::CloseConnection(rpc_error) => {
                info!(
                    "[{}] [on_connection_handler_event] Close connection: {:?}",
                    peer_id, rpc_error
                );
                self.events.push(ToSwarm::CloseConnection {
                    peer_id,
                    connection: CloseConnection::All,
                });
            }
        }
    }

    fn poll(
        &mut self,
        _cx: &mut Context<'_>,
        _params: &mut impl PollParameters,
    ) -> Poll<ToSwarm<Self::ToSwarm, THandlerInEvent<Self>>> {
        if !self.events.is_empty() {
            return Poll::Ready(self.events.remove(0));
        }

        Poll::Pending
    }
}
