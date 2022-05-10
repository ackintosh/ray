use crate::rpc::behaviour::RpcEvent;
use crate::rpc::error::RPCError;
use crate::rpc::message::Status;
use crate::rpc::protocol::{RpcProtocol, RpcRequestProtocol};
use libp2p::swarm::handler::{InboundUpgradeSend, OutboundUpgradeSend};
use libp2p::swarm::{
    ConnectionHandler, ConnectionHandlerEvent, ConnectionHandlerUpgrErr, KeepAlive,
    SubstreamProtocol,
};
use smallvec::SmallVec;
use std::task::{Context, Poll};
use tracing::info;

pub(crate) struct Handler {
    /// Queue of outbound substreams to open.
    dial_queue: SmallVec<[Status; 4]>, // TODO: Generalize the type of request
}

impl Handler {
    pub(crate) fn new() -> Self {
        Handler {
            dial_queue: SmallVec::new(),
        }
    }

    // https://github.com/ethereum/consensus-specs/blob/dev/specs/phase0/p2p-interface.md#status
    fn send_status(&mut self, status_request: Status) {
        self.dial_queue.push(status_request);
    }
}

// SEE https://github.com/sigp/lighthouse/blob/4af6fcfafd2c29bca82474ee378cda9ac254783a/beacon_node/eth2_libp2p/src/rpc/handler.rs#L311
impl ConnectionHandler for Handler {
    type InEvent = RpcEvent;
    type OutEvent = ();
    type Error = RPCError;
    type InboundProtocol = RpcProtocol;
    type OutboundProtocol = RpcRequestProtocol;
    type InboundOpenInfo = ();
    type OutboundOpenInfo = ();

    fn listen_protocol(&self) -> SubstreamProtocol<Self::InboundProtocol, Self::InboundOpenInfo> {
        info!("Handler::listen_protocol");
        SubstreamProtocol::new(RpcProtocol, ())
    }

    fn inject_fully_negotiated_inbound(
        &mut self,
        _protocol: <Self::InboundProtocol as InboundUpgradeSend>::Output,
        _info: Self::InboundOpenInfo,
    ) {
        // NOTE: Nothing to do for now.

        // TODO: Handle `Goodbye` message
        // spec: https://github.com/ethereum/consensus-specs/blob/dev/specs/phase0/p2p-interface.md#goodbye
    }

    fn inject_fully_negotiated_outbound(
        &mut self,
        _protocol: <Self::OutboundProtocol as OutboundUpgradeSend>::Output,
        _info: Self::OutboundOpenInfo,
    ) {
        // NOTE: We should do something like this, though nothing to do for now.
        // https://github.com/sigp/lighthouse/blob/db0beb51788576565cef9534ad9490a4a498b544/beacon_node/lighthouse_network/src/rpc/handler.rs#L373
    }

    fn inject_event(&mut self, event: Self::InEvent) {
        info!("inject_event. event: {:?}", event);
        match event {
            RpcEvent::SendStatus(status_request) => self.send_status(status_request),
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
                protocol: SubstreamProtocol::new(RpcRequestProtocol { request }, ()),
            });
        }

        Poll::Pending
    }
}
