use crate::rpc::handler::{Handler, HandlerReceived, SubstreamId};
use crate::rpc::{ReceivedRequest, ReceivedResponse, RpcEvent};
use crate::types::{ForkDigest, Root};
use libp2p::core::connection::ConnectionId;
use libp2p::swarm::{
    ConnectionHandler, DialError, IntoConnectionHandler, NetworkBehaviour, NetworkBehaviourAction,
    NotifyHandler, PollParameters,
};
use libp2p::{Multiaddr, PeerId};
use std::sync::Arc;
use std::task::{Context, Poll};
use tracing::{info, trace, warn};
use types::{Epoch, ForkContext, MainnetEthSpec, Slot};

// ////////////////////////////////////////////////////////
// Internal message of RPC module sent by Behaviour
// ////////////////////////////////////////////////////////

// RPC internal message sent from behaviour to handlers
#[derive(Debug)]
pub(crate) enum MessageToHandler {
    SendStatus(lighthouse_network::rpc::StatusMessage),
    SendResponse(SubstreamId, lighthouse_network::Response<MainnetEthSpec>),
}

// ////////////////////////////////////////////////////////
// Behaviour
// ////////////////////////////////////////////////////////

pub(crate) struct Behaviour {
    events: Vec<NetworkBehaviourAction<RpcEvent, Handler>>,
    fork_context: Arc<ForkContext>,
}

impl Behaviour {
    pub(crate) fn new(fork_context: Arc<ForkContext>) -> Self {
        Behaviour {
            events: vec![],
            fork_context,
        }
    }

    // https://github.com/ethereum/consensus-specs/blob/dev/specs/phase0/p2p-interface.md#status
    pub(crate) fn send_status(
        &mut self,
        peer_id: PeerId,
        fork_digest: ForkDigest,
        finalized_root: Root,
        finalized_epoch: Epoch,
        head_root: Root,
        head_slot: Slot,
    ) {
        // Notify ConnectionHandler, then the handler's `inject_event` is invoked with the event.
        self.events.push(NetworkBehaviourAction::NotifyHandler {
            peer_id,
            handler: NotifyHandler::Any,
            event: MessageToHandler::SendStatus(lighthouse_network::rpc::StatusMessage {
                fork_digest,
                finalized_root,
                finalized_epoch,
                head_root,
                head_slot,
            }),
        })
    }

    pub(crate) fn send_response(
        &mut self,
        peer_id: PeerId,
        connection_id: ConnectionId,
        substream_id: SubstreamId,
        response: lighthouse_network::Response<MainnetEthSpec>,
    ) {
        self.events.push(NetworkBehaviourAction::NotifyHandler {
            peer_id,
            handler: NotifyHandler::One(connection_id),
            event: MessageToHandler::SendResponse(substream_id, response),
        })
    }
}

// NetworkBehaviour defines "what" bytes to send on the network.
// SEE https://docs.rs/libp2p/0.39.1/libp2p/tutorial/index.html#network-behaviour
impl NetworkBehaviour for Behaviour {
    type ConnectionHandler = Handler;
    type OutEvent = RpcEvent;

    fn new_handler(&mut self) -> Self::ConnectionHandler {
        Handler::new(self.fork_context.clone())
    }

    fn addresses_of_peer(&mut self, _peer_id: &PeerId) -> Vec<Multiaddr> {
        trace!("addresses_of_peer -> nothing to do because this event is handled by discovery.");
        vec![]
    }

    // Informs the behaviour about an event generated by the handler dedicated to the peer identified by peer_id. for the behaviour.
    fn inject_event(
        &mut self,
        peer_id: PeerId,
        connection_id: ConnectionId,
        event: <<Self::ConnectionHandler as IntoConnectionHandler>::Handler as ConnectionHandler>::OutEvent,
    ) {
        info!("inject_event. peer_id: {}, event: {:?}", peer_id, event);
        match event {
            HandlerReceived::Request(inbound_request) => {
                self.events.push(NetworkBehaviourAction::GenerateEvent(
                    RpcEvent::ReceivedRequest(ReceivedRequest {
                        peer_id,
                        connection_id,
                        substream_id: inbound_request.substream_id,
                        request: inbound_request.request,
                    }),
                ));
            }
            HandlerReceived::Response(response) => {
                self.events.push(NetworkBehaviourAction::GenerateEvent(
                    RpcEvent::ReceivedResponse(ReceivedResponse { peer_id, response }),
                ));
            }
        };
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
        trace!("poll");

        if !self.events.is_empty() {
            return Poll::Ready(self.events.remove(0));
        }

        Poll::Pending
    }
}
