use crate::rpc::behaviour::MessageToHandler;
use crate::rpc::error::RPCError;
use crate::rpc::protocol::{InboundFramed, OutboundFramed, RpcProtocol, RpcRequestProtocol};
use futures::{FutureExt, SinkExt, StreamExt};
use libp2p::swarm::handler::{InboundUpgradeSend, OutboundUpgradeSend};
use libp2p::swarm::{
    ConnectionHandler, ConnectionHandlerEvent, ConnectionHandlerUpgrErr, KeepAlive,
    NegotiatedSubstream, SubstreamProtocol,
};
use lighthouse_network::rpc::methods::RPCCodedResponse;
use smallvec::SmallVec;
use std::collections::hash_map::Entry;
use std::collections::{HashMap, VecDeque};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use tracing::{error, info};
use types::fork_context::ForkContext;
use types::MainnetEthSpec;

struct SubstreamIdGenerator {
    current_id: usize,
}

impl SubstreamIdGenerator {
    fn new() -> Self {
        SubstreamIdGenerator { current_id: 0 }
    }

    // Returns a sequential ID for substreams.
    fn next(&mut self) -> SubstreamId {
        let id = SubstreamId(self.current_id);
        self.current_id += 1;
        id
    }
}

// Identifier of inbound and outbound substreams from the handler's perspective.
#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq)]
pub struct SubstreamId(usize);

enum InboundSubstreamState {
    // The underlying substream is not being used.
    Idle(InboundFramed<NegotiatedSubstream>),
    // The underlying substream is processing responses.
    Busy(Pin<Box<dyn Future<Output = Result<InboundFramed<NegotiatedSubstream>, String>> + Send>>),
    // Temporary state during processing
    Poisoned,
}

struct InboundSubstreamInfo {
    // State of the substream.
    state: InboundSubstreamState,
    // Responses queued for sending.
    responses_to_send: VecDeque<lighthouse_network::Response<MainnetEthSpec>>,
}

// ////////////////////////////////////////////////////////
// Internal events of RPC module sent by Handler
// ////////////////////////////////////////////////////////

// RPC internal message sent from handler to the behaviour
#[derive(Debug)]
pub(crate) enum HandlerReceived {
    // A request received from the outside.
    Request(InboundRequest),
    // A response received from the outside.
    Response(lighthouse_network::rpc::methods::RPCResponse<MainnetEthSpec>),
}

// A request received from the outside.
#[derive(Debug)]
pub struct InboundRequest {
    pub(crate) substream_id: SubstreamId,
    pub(crate) request: lighthouse_network::rpc::protocol::InboundRequest<MainnetEthSpec>,
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
    // Current inbound substreams awaiting processing.
    inbound_substreams: HashMap<SubstreamId, InboundSubstreamInfo>,
    // Sequential ID generator for inbound substreams.
    inbound_substream_id: SubstreamIdGenerator,
    // Map of outbound substreams that need to be driven to completion.
    outbound_substreams: HashMap<SubstreamId, OutboundFramed>,
    // Sequential ID generator for outbound substreams.
    outbound_substream_id: SubstreamIdGenerator,
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
            inbound_substreams: HashMap::new(),
            inbound_substream_id: SubstreamIdGenerator::new(),
            outbound_substreams: HashMap::new(),
            outbound_substream_id: SubstreamIdGenerator::new(),
        }
    }

    // https://github.com/ethereum/consensus-specs/blob/dev/specs/phase0/p2p-interface.md#status
    fn send_status(&mut self, status_request: lighthouse_network::rpc::StatusMessage) {
        self.dial_queue
            .push(lighthouse_network::rpc::outbound::OutboundRequest::Status(
                status_request,
            ));
    }

    fn send_response(
        &mut self,
        substream_id: SubstreamId,
        response: lighthouse_network::Response<MainnetEthSpec>,
    ) {
        match self.inbound_substreams.get_mut(&substream_id) {
            None => {
                error!(
                    "InboundSubstream not found. substream_id: {}",
                    substream_id.0
                )
            }
            Some(inbound_substream_info) => {
                inbound_substream_info.responses_to_send.push_back(response);
            }
        }
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
    type OutboundOpenInfo = lighthouse_network::rpc::outbound::OutboundRequest<MainnetEthSpec>;

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
        let (request, substream) = inbound;
        info!("inject_fully_negotiated_inbound. request: {:?}", request);

        let inbound_substream_id = self.inbound_substream_id.next();

        // Store the inbound substream
        if let Some(_old_substream) = self.inbound_substreams.insert(
            inbound_substream_id,
            InboundSubstreamInfo {
                state: InboundSubstreamState::Idle(substream),
                responses_to_send: VecDeque::new(),
            },
        ) {
            error!(
                "inbound_substream_id is duplicated. substream_id: {}",
                inbound_substream_id.0
            );
        }

        // TODO: Handle `Goodbye` message
        // spec: https://github.com/ethereum/consensus-specs/blob/dev/specs/phase0/p2p-interface.md#goodbye

        // Inform the received request to the behaviour
        self.out_events
            .push(HandlerReceived::Request(InboundRequest {
                substream_id: inbound_substream_id,
                request,
            }));
    }

    // Injects the output of a successful upgrade on a new outbound substream
    // The second argument is the information that was previously passed to ConnectionHandlerEvent::OutboundSubstreamRequest.
    fn inject_fully_negotiated_outbound(
        &mut self,
        stream: <Self::OutboundProtocol as OutboundUpgradeSend>::Output,
        info: Self::OutboundOpenInfo,
    ) {
        info!("inject_fully_negotiated_outbound. info: {:?}", info);
        let request = info;
        let outbound_substream_id = self.outbound_substream_id.next();

        if request.expected_responses() > 0 {
            if self
                .outbound_substreams
                .insert(outbound_substream_id, stream)
                .is_some()
            {
                error!(
                    "Duplicate outbound substream id: {:?}",
                    outbound_substream_id
                );
            }
        }
    }

    fn inject_event(&mut self, event: Self::InEvent) {
        info!("inject_event. event: {:?}", event);
        match event {
            MessageToHandler::SendStatus(status_request) => self.send_status(status_request),
            MessageToHandler::SendResponse(substream_id, response) => {
                self.send_response(substream_id, response)
            }
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
        cx: &mut Context<'_>,
    ) -> Poll<
        ConnectionHandlerEvent<
            Self::OutboundProtocol,
            Self::OutboundOpenInfo,
            Self::OutEvent,
            Self::Error,
        >,
    > {
        info!("poll");

        // /////////////////////////////////////////////////////////////////////////////////////////////////
        // Establish outbound substreams
        // /////////////////////////////////////////////////////////////////////////////////////////////////
        if !self.dial_queue.is_empty() {
            let request = self.dial_queue.remove(0);
            info!(
                "ConnectionHandlerEvent::OutboundSubstreamRequest. request: {:?}",
                request
            );
            return Poll::Ready(ConnectionHandlerEvent::OutboundSubstreamRequest {
                protocol: SubstreamProtocol::new(
                    RpcRequestProtocol {
                        request: request.clone(),
                        max_rpc_size: self.max_rpc_size,
                        fork_context: self.fork_context.clone(),
                    },
                    request,
                ),
            });
        }

        // /////////////////////////////////////////////////////////////////////////////////////////////////
        // Inform events to the behaviour.
        // `crate::rpc::Behaviour::inject_event()` is called with the event returned here.
        // /////////////////////////////////////////////////////////////////////////////////////////////////
        if !self.out_events.is_empty() {
            return Poll::Ready(ConnectionHandlerEvent::Custom(self.out_events.remove(0)));
        }

        // /////////////////////////////////////////////////////////////////////////////////////////////////
        // Drive inbound streams that need to be processed
        // /////////////////////////////////////////////////////////////////////////////////////////////////
        let mut inbound_substreams_to_remove = vec![];
        for (substream_id, inbound_substream_info) in self.inbound_substreams.iter_mut() {
            loop {
                match std::mem::replace(
                    &mut inbound_substream_info.state,
                    InboundSubstreamState::Poisoned,
                ) {
                    InboundSubstreamState::Idle(mut substream) => {
                        if let Some(response_to_send) =
                            inbound_substream_info.responses_to_send.pop_front()
                        {
                            let boxed_future = async move {
                                let rpc_coded_response: RPCCodedResponse<MainnetEthSpec> =
                                    response_to_send.into();

                                return match substream.send(rpc_coded_response).await {
                                    Ok(_) => match substream.close().await {
                                        Ok(_) => Ok(substream),
                                        Err(rpc_error) => Err(format!(
                                            "Failed to close substream. error: {}",
                                            rpc_error
                                        )),
                                    },
                                    Err(rpc_error) => {
                                        let mut error_message = format!(
                                            "Failed to send response. rpc_error: {}",
                                            rpc_error
                                        );
                                        if let Err(e) = substream.close().await {
                                            error_message = format!(
                                                "Failed to close substream. error: {}, {}",
                                                e, error_message
                                            );
                                        }

                                        Err(error_message)
                                    }
                                };
                            }
                            .boxed();

                            inbound_substream_info.state =
                                InboundSubstreamState::Busy(Box::pin(boxed_future));
                        }
                    }
                    InboundSubstreamState::Busy(mut future) => {
                        match future.poll_unpin(cx) {
                            // The pending messages have been sent successfully and the stream has
                            // terminated
                            Poll::Ready(Ok(_stream)) => {
                                info!("Sent a response successfully.");
                                inbound_substreams_to_remove.push(*substream_id);
                                // There is nothing more to process on this substream as it has
                                // been closed. Move on to the next one.
                                break;
                            }
                            // An error occurred when trying to send a response.
                            Poll::Ready(Err(error_message)) => {
                                // TODO: Report the error that occurred during the send process
                                error!("Failed to send a response: {}", error_message);
                                inbound_substreams_to_remove.push(*substream_id);
                                break;
                            }
                            // The sending future has not completed. Leave the state as busy and
                            // try to progress later.
                            Poll::Pending => {
                                inbound_substream_info.state = InboundSubstreamState::Busy(future);
                                break;
                            }
                        }
                    }
                    InboundSubstreamState::Poisoned => unreachable!(),
                }
            }
        }
        // Remove closed substreams
        for id in inbound_substreams_to_remove {
            self.inbound_substreams.remove(&id);
        }

        // /////////////////////////////////////////////////////////////////////////////////////////////////
        // Drive outbound streams that need to be processed
        // /////////////////////////////////////////////////////////////////////////////////////////////////
        for outbound_substream_id in self.outbound_substreams.keys().copied().collect::<Vec<_>>() {
            let mut entry = match self.outbound_substreams.entry(outbound_substream_id) {
                Entry::Occupied(entry) => entry,
                Entry::Vacant(_) => unreachable!(),
            };

            match entry.get_mut().poll_next_unpin(cx) {
                Poll::Ready(Some(Ok(rpc_coded_response))) => match rpc_coded_response {
                    RPCCodedResponse::Success(response) => {
                        return Poll::Ready(ConnectionHandlerEvent::Custom(
                            HandlerReceived::Response(response),
                        ));
                    }
                    RPCCodedResponse::Error(_, _) => {
                        todo!()
                    }
                    RPCCodedResponse::StreamTermination(_) => {
                        todo!()
                    }
                },
                Poll::Pending => {}
                _ => {
                    todo!()
                }
            }
        }

        Poll::Pending
    }
}
