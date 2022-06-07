use crate::rpc::behaviour::MessageToHandler;
use crate::rpc::error::RPCError;
use crate::rpc::protocol::{RpcProtocol, RpcRequestProtocol};
use libp2p::swarm::handler::{InboundUpgradeSend, OutboundUpgradeSend};
use libp2p::swarm::{
    ConnectionHandler, ConnectionHandlerEvent, ConnectionHandlerUpgrErr, KeepAlive,
    SubstreamProtocol,
};
use smallvec::SmallVec;
use std::sync::Arc;
use std::task::{Context, Poll};
use tracing::info;
use types::fork_context::ForkContext;
use types::MainnetEthSpec;

// ////////////////////////////////////////////////////////
// Internal events of RPC module sent by Handler
// ////////////////////////////////////////////////////////

// RPC internal message sent from handler to the behaviour
#[derive(Debug)]
pub(crate) enum HandlerReceived {
    // A request received from the outside.
    Request(lighthouse_network::rpc::protocol::InboundRequest<MainnetEthSpec>),
    // A response received from the outside.
    // TODO: Response
}

// ////////////////////////////////////////////////////////
// Handler
// ////////////////////////////////////////////////////////

pub(crate) struct Handler {
    // Queue of outbound substreams to open.
    dial_queue: SmallVec<[lighthouse_network::rpc::outbound::OutboundRequest<MainnetEthSpec>; 4]>,
    fork_context: Arc<ForkContext>,
    max_rpc_size: usize,
    // Queue of events to produce in `poll()`.
    out_events: SmallVec<[HandlerReceived; 4]>,
}

impl Handler {
    pub(crate) fn new(fork_context: Arc<ForkContext>) -> Self {
        // SEE: https://github.com/sigp/lighthouse/blob/fff4dd6311695c1d772a9d6991463915edf223d5/beacon_node/lighthouse_network/src/rpc/protocol.rs#L114
        let max_rpc_size = 10 * 1_048_576; // 10M
        Handler {
            dial_queue: SmallVec::new(),
            fork_context,
            max_rpc_size,
            out_events: SmallVec::new(),
        }
    }

    // https://github.com/ethereum/consensus-specs/blob/dev/specs/phase0/p2p-interface.md#status
    fn send_status(&mut self, status_request: lighthouse_network::rpc::StatusMessage) {
        self.dial_queue
            .push(lighthouse_network::rpc::outbound::OutboundRequest::Status(
                status_request,
            ));
    }
}

// SEE https://github.com/sigp/lighthouse/blob/4af6fcfafd2c29bca82474ee378cda9ac254783a/beacon_node/eth2_libp2p/src/rpc/handler.rs#L311
impl ConnectionHandler for Handler {
    type InEvent = MessageToHandler;
    type OutEvent = HandlerReceived;
    type Error = RPCError;
    type InboundProtocol = RpcProtocol;
    type OutboundProtocol = RpcRequestProtocol;
    type InboundOpenInfo = ();
    type OutboundOpenInfo = ();

    fn listen_protocol(&self) -> SubstreamProtocol<Self::InboundProtocol, Self::InboundOpenInfo> {
        info!("Handler::listen_protocol");
        SubstreamProtocol::new(
            RpcProtocol {
                fork_context: self.fork_context.clone(),
                max_rpc_size: self.max_rpc_size,
            },
            (),
        )
    }

    // Injects the output of a successful upgrade on a new inbound substream.
    fn inject_fully_negotiated_inbound(
        &mut self,
        inbound: <Self::InboundProtocol as InboundUpgradeSend>::Output,
        _info: Self::InboundOpenInfo,
    ) {
        info!("inject_fully_negotiated_inbound. request: {:?}", inbound.0);

        // TODO: Handle `Goodbye` message
        // spec: https://github.com/ethereum/consensus-specs/blob/dev/specs/phase0/p2p-interface.md#goodbye

        // Inform the received request to the behaviour
        self.out_events.push(HandlerReceived::Request(inbound.0));
    }

    // Injects the output of a successful upgrade on a new outbound substream
    // The second argument is the information that was previously passed to ConnectionHandlerEvent::OutboundSubstreamRequest.
    fn inject_fully_negotiated_outbound(
        &mut self,
        _protocol: <Self::OutboundProtocol as OutboundUpgradeSend>::Output,
        _info: Self::OutboundOpenInfo,
    ) {
        info!("inject_fully_negotiated_outbound");
        // NOTE: We should do something like this, though nothing to do for now.
        // https://github.com/sigp/lighthouse/blob/db0beb51788576565cef9534ad9490a4a498b544/beacon_node/lighthouse_network/src/rpc/handler.rs#L373
    }

    fn inject_event(&mut self, event: Self::InEvent) {
        info!("inject_event. event: {:?}", event);
        match event {
            MessageToHandler::SendStatus(status_request) => self.send_status(status_request),
        }
    }

    fn inject_dial_upgrade_error(
        &mut self,
        _info: Self::OutboundOpenInfo,
        _error: ConnectionHandlerUpgrErr<<Self::OutboundProtocol as OutboundUpgradeSend>::Error>,
    ) {
        todo!()
    }

    fn connection_keep_alive(&self) -> KeepAlive {
        KeepAlive::Yes
    }

    fn poll(
        &mut self,
        _cx: &mut Context<'_>,
    ) -> Poll<
        ConnectionHandlerEvent<
            Self::OutboundProtocol,
            Self::OutboundOpenInfo,
            Self::OutEvent,
            Self::Error,
        >,
    > {
        info!("poll");

        // Establish outbound substreams
        if !self.dial_queue.is_empty() {
            let request = self.dial_queue.remove(0);
            info!(
                "ConnectionHandlerEvent::OutboundSubstreamRequest. request: {:?}",
                request
            );
            return Poll::Ready(ConnectionHandlerEvent::OutboundSubstreamRequest {
                protocol: SubstreamProtocol::new(
                    RpcRequestProtocol {
                        request,
                        max_rpc_size: self.max_rpc_size,
                        fork_context: self.fork_context.clone(),
                    },
                    (),
                ),
            });
        }

        // Inform events to the behaviour. `inject_event` of the behaviour is called with the event.
        if !self.out_events.is_empty() {
            return Poll::Ready(ConnectionHandlerEvent::Custom(self.out_events.remove(0)));
        }

        Poll::Pending
    }
}
