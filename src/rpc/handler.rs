use crate::rpc::behaviour::InstructionToHandler;
use crate::rpc::error::RPCError;
use crate::rpc::protocol::{
    InboundFramed, OutboundFramed, OutboundRequest, RpcProtocol, RpcRequestProtocol,
};
use futures::{FutureExt, SinkExt, StreamExt};
use libp2p::swarm::handler::{InboundUpgradeSend, OutboundUpgradeSend};
use libp2p::swarm::{
    ConnectionHandler, ConnectionHandlerEvent, ConnectionHandlerUpgrErr, KeepAlive,
    NegotiatedSubstream, SubstreamProtocol,
};
use libp2p::PeerId;
use lighthouse_network::rpc::methods::RPCCodedResponse;
use smallvec::SmallVec;
use std::collections::hash_map::Entry;
use std::collections::{HashMap, VecDeque};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;
use tokio::time::{sleep_until, Instant, Sleep};
use tracing::log::trace;
use tracing::{error, info, warn};
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
/// Maximum time given to the handler to perform shutdown operations.
const SHUTDOWN_TIMEOUT_SECS: u64 = 15;

enum HandlerState {
    /// The handler is active. All messages are sent and received.
    Active,
    /// The handler is shutting_down.
    ///
    /// While in this state the handler rejects new requests but tries to finish existing ones.
    /// Once the timer expires, all messages are killed.
    ShuttingDown(Pin<Box<Sleep>>),
    /// The handler is deactivated. A goodbye has been sent and no more messages are sent or
    /// received.
    Deactivated,
}

pub(crate) struct Handler {
    /// State of the handler.
    state: HandlerState,
    // Queue of outbound substreams to open.
    dial_queue: SmallVec<[OutboundRequest; 4]>,
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
    // The PeerId this handler communicate to. Note this is just for debugging.
    peer_id: Option<PeerId>,
}

impl Handler {
    pub(crate) fn new(fork_context: Arc<ForkContext>) -> Self {
        // SEE: https://github.com/sigp/lighthouse/blob/fff4dd6311695c1d772a9d6991463915edf223d5/beacon_node/lighthouse_network/src/rpc/protocol.rs#L114
        let max_rpc_size = 10 * 1_048_576; // 10M
        Handler {
            state: HandlerState::Active,
            dial_queue: SmallVec::new(),
            fork_context,
            max_rpc_size,
            out_events: SmallVec::new(),
            inbound_substreams: HashMap::new(),
            inbound_substream_id: SubstreamIdGenerator::new(),
            outbound_substreams: HashMap::new(),
            outbound_substream_id: SubstreamIdGenerator::new(),
            peer_id: None,
        }
    }

    fn peer_id(&self) -> String {
        self.peer_id
            .map(|p| p.to_string())
            .unwrap_or("no_peer_id".to_string())
    }

    // Status
    // https://github.com/ethereum/consensus-specs/blob/dev/specs/phase0/p2p-interface.md#status
    fn send_status(
        &mut self,
        peer_id: PeerId,
        status_message: lighthouse_network::rpc::StatusMessage,
    ) {
        self.dial_queue.push(OutboundRequest {
            peer_id,
            request: lighthouse_network::rpc::outbound::OutboundRequest::Status(status_message),
        });
    }

    // Goodbye
    // https://github.com/ethereum/consensus-specs/blob/dev/specs/phase0/p2p-interface.md#goodbye
    fn send_goodbye_and_shutdown(
        &mut self,
        peer_id: PeerId,
        reason: lighthouse_network::rpc::GoodbyeReason,
    ) {
        // Queue our goodbye message.
        // Ref: https://github.com/sigp/lighthouse/blob/3dd50bda11cefb3c17d851cbb8811610385c20aa/beacon_node/lighthouse_network/src/rpc/handler.rs#L239
        self.dial_queue.push(OutboundRequest {
            peer_id,
            request: lighthouse_network::rpc::outbound::OutboundRequest::Goodbye(reason),
        });

        // Update the state to start shutdown process.
        info!("[{peer_id}] send_goodbye_and_shutdown: Updated the handler state to ShuttingDown");
        self.state = HandlerState::ShuttingDown(Box::pin(sleep_until(
            Instant::now() + Duration::from_secs(SHUTDOWN_TIMEOUT_SECS),
        )));
    }

    fn send_request(
        &mut self,
        peer_id: PeerId,
        // TODO: request_id
        // request_id: lighthouse_network::service::api_types::RequestId<AppReqId>,
        request: lighthouse_network::rpc::outbound::OutboundRequest<MainnetEthSpec>,
    ) {
        match self.state {
            HandlerState::Active => {
                self.dial_queue.push(OutboundRequest { peer_id, request });
            }
            _ => {
                // TODO: handle this case
                // https://github.com/sigp/lighthouse/blob/4e5e7ee1fcbb60b9a36260da150ad8215c0d37ba/beacon_node/lighthouse_network/src/rpc/handler.rs#L270
            }
        }
    }

    fn send_response(
        &mut self,
        _peer_id: PeerId,
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
    type InEvent = InstructionToHandler;
    type OutEvent = HandlerReceived;
    type Error = RPCError;
    type InboundProtocol = RpcProtocol;
    type OutboundProtocol = RpcRequestProtocol;
    type InboundOpenInfo = ();
    type OutboundOpenInfo = lighthouse_network::rpc::outbound::OutboundRequest<MainnetEthSpec>;

    fn listen_protocol(&self) -> SubstreamProtocol<Self::InboundProtocol, Self::InboundOpenInfo> {
        info!("[{}] [ConnectionHandler::listen_protocol]", self.peer_id());

        SubstreamProtocol::new(
            RpcProtocol::new(
                self.fork_context.clone(),
                self.max_rpc_size,
                self.peer_id.clone(),
            ),
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
        info!(
            "[{}] inject_fully_negotiated_inbound. request: {request:?}",
            self.peer_id()
        );

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

        if request.expected_responses() > 0
            && self
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

    fn inject_event(&mut self, event: Self::InEvent) {
        info!("inject_event. event: {:?}", event);

        match event {
            InstructionToHandler::Status(status_message, peer_id) => {
                self.peer_id = Some(peer_id.clone()); // This is just for debugging.
                self.send_status(peer_id, status_message);
            }
            InstructionToHandler::Goodbye(reason, peer_id) => {
                self.peer_id = Some(peer_id.clone()); // This is just for debugging.
                self.send_goodbye_and_shutdown(peer_id, reason);
            }
            InstructionToHandler::Request(request, peer_id) => {
                self.peer_id = Some(peer_id.clone()); // This is just for debugging.
                self.send_request(peer_id, request);
            }
            InstructionToHandler::Response(substream_id, response, peer_id) => {
                self.peer_id = Some(peer_id.clone()); // This is just for debugging.
                self.send_response(peer_id, substream_id, response)
            }
        }
    }

    fn inject_dial_upgrade_error(
        &mut self,
        info: Self::OutboundOpenInfo,
        error: ConnectionHandlerUpgrErr<<Self::OutboundProtocol as OutboundUpgradeSend>::Error>,
    ) {
        warn!(
            "[{}] inject_dial_upgrade_error. info: {}, error: {}",
            self.peer_id(),
            info,
            error,
        );

        // TODO
        // ref: https://github.com/sigp/lighthouse/blob/3dd50bda11cefb3c17d851cbb8811610385c20aa/beacon_node/lighthouse_network/src/rpc/handler.rs#L453
    }

    fn connection_keep_alive(&self) -> KeepAlive {
        if matches!(self.state, HandlerState::Deactivated) {
            // The timeout has expired. Force the disconnect.
            KeepAlive::No
        } else {
            KeepAlive::Yes
        }
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
        // trace!("poll");

        // /////////////////////////////////////////////////////////////////////////////////////////////////
        // Check if we are shutting down, and if the timer ran out
        // /////////////////////////////////////////////////////////////////////////////////////////////////
        if let HandlerState::ShuttingDown(delay) = &mut self.state {
            match delay.as_mut().poll(cx) {
                Poll::Ready(_) => {
                    self.state = HandlerState::Deactivated;
                    info!("poll: Updated the handler state to Deactivated");
                    return Poll::Ready(ConnectionHandlerEvent::Close(RPCError::Disconnected));
                }
                Poll::Pending => {}
            }
        }

        // /////////////////////////////////////////////////////////////////////////////////////////////////
        // Establish outbound substreams
        // /////////////////////////////////////////////////////////////////////////////////////////////////
        if !self.dial_queue.is_empty() {
            let request = self.dial_queue.remove(0);
            info!(
                "[{}] ConnectionHandlerEvent::OutboundSubstreamRequest. request: {:?}",
                request.peer_id, request.request,
            );
            return Poll::Ready(ConnectionHandlerEvent::OutboundSubstreamRequest {
                protocol: SubstreamProtocol::new(
                    RpcRequestProtocol {
                        request: request.clone(),
                        max_rpc_size: self.max_rpc_size,
                        fork_context: self.fork_context.clone(),
                    },
                    request.request,
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

                                match substream.send(rpc_coded_response).await {
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
                                }
                            }
                            .boxed();

                            inbound_substream_info.state =
                                InboundSubstreamState::Busy(Box::pin(boxed_future));
                        } else {
                            // There is nothing left to process. Set the stream to idle and
                            // move on to the next one.
                            inbound_substream_info.state = InboundSubstreamState::Idle(substream);
                            break;
                        }
                    }
                    InboundSubstreamState::Busy(mut future) => {
                        match future.poll_unpin(cx) {
                            // The pending messages have been sent successfully and the stream has
                            // terminated
                            Poll::Ready(Ok(_stream)) => {
                                trace!(
                                    "[{}] Sent a response successfully.",
                                    self.peer_id
                                        .map(|p| p.to_string())
                                        .unwrap_or("no_peer_id".to_string())
                                );
                                inbound_substreams_to_remove.push(*substream_id);
                                // There is nothing more to process on this substream as it has
                                // been closed. Move on to the next one.
                                break;
                            }
                            // An error occurred when trying to send a response.
                            Poll::Ready(Err(error_message)) => {
                                // TODO: Report the error that occurred during the send process
                                error!(
                                    "[{}] Failed to send a response. error: {}",
                                    self.peer_id
                                        .map(|p| p.to_string())
                                        .unwrap_or("no_peer_id".to_string()),
                                    error_message
                                );
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
                        info!("[{}] received a response: {response:?}", self.peer_id());
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
                Poll::Ready(Some(Err(e))) => {
                    error!(
                        "[{}] An error occurred while processing outbound stream. error: {:?}",
                        self.peer_id(),
                        e,
                    );
                }
                Poll::Ready(None) => {
                    // ////////////////
                    // stream closed
                    // ////////////////
                    info!(
                        "[{}] Stream closed by remote. outbound_substream_id: {:?}",
                        self.peer_id(),
                        outbound_substream_id
                    );
                    // drop the stream
                    entry.remove_entry();

                    // TODO: Return an error
                    // ref: https://github.com/sigp/lighthouse/blob/3dd50bda11cefb3c17d851cbb8811610385c20aa/beacon_node/lighthouse_network/src/rpc/handler.rs#L884-L898
                    return Poll::Ready(ConnectionHandlerEvent::Close(RPCError::Disconnected));
                }
                Poll::Pending => {}
            }
        }

        Poll::Pending
    }
}
