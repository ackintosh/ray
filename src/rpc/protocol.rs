use ::types::fork_context::ForkContext;
use futures::future::BoxFuture;
use futures::prelude::*;
use libp2p::core::UpgradeInfo;
use libp2p::{InboundUpgrade, OutboundUpgrade, PeerId, Stream};
use std::fmt::{Display, Formatter};
use std::sync::Arc;
use std::time::Duration;
use tokio_io_timeout::TimeoutStream;
use tokio_util::codec::Framed;
use tokio_util::compat::{Compat, FuturesAsyncReadCompatExt};
use tracing::{error, info};
use types::MainnetEthSpec;

// spec:
// https://github.com/ethereum/consensus-specs/blob/dev/specs/phase0/p2p-interface.md#protocol-identification
// ProtocolPrefix
const PROTOCOL_PREFIX: &str = "/eth2/beacon_chain/req";

// The number of seconds to wait for the first bytes of a request once a protocol has been
// established before the stream is terminated.
const REQUEST_TIMEOUT: u64 = 15;

#[derive(Clone, Debug)]
enum Protocol {
    // https://github.com/ethereum/consensus-specs/blob/dev/specs/phase0/p2p-interface.md#status
    Status,
    Goodbye,
    BlocksByRange,
}

impl Protocol {
    fn to_lighthouse_supported_protocol(
        &self,
    ) -> lighthouse_network::rpc::protocol::SupportedProtocol {
        match self {
            Protocol::Status => lighthouse_network::rpc::protocol::SupportedProtocol::StatusV1,
            Protocol::Goodbye => lighthouse_network::rpc::protocol::SupportedProtocol::GoodbyeV1,
            Protocol::BlocksByRange => {
                lighthouse_network::rpc::protocol::SupportedProtocol::BlocksByRangeV2
            }
        }
    }
}

impl Display for Protocol {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let protocol_name = match self {
            Protocol::Status => "status",
            Protocol::Goodbye => "goodbye",
            Protocol::BlocksByRange => "beacon_blocks_by_range",
        };
        f.write_str(protocol_name)
    }
}

#[derive(Clone, Debug)]
enum SchemaVersion {
    V1,
    V2,
}

impl Display for SchemaVersion {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let version = match self {
            SchemaVersion::V1 => "1",
            SchemaVersion::V2 => "2",
        };
        f.write_str(version)
    }
}

#[derive(Clone, Debug)]
enum Encoding {
    // see https://github.com/ethereum/consensus-specs/blob/dev/specs/phase0/p2p-interface.md#encoding-strategies
    SSZSnappy,
}

impl Display for Encoding {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let encoding = match self {
            Encoding::SSZSnappy => "ssz_snappy",
        };
        f.write_str(encoding)
    }
}

// /////////////////////////////////////////////////////////////////////////////////////////////////
// Protocol identification
// https://github.com/ethereum/consensus-specs/blob/dev/specs/phase0/p2p-interface.md#protocol-identification
// /////////////////////////////////////////////////////////////////////////////////////////////////
#[derive(Clone, Debug)]
pub(crate) struct ProtocolId {
    #[allow(dead_code)]
    protocol: Protocol,
    #[allow(dead_code)]
    schema_version: SchemaVersion,
    #[allow(dead_code)]
    encoding: Encoding,
    // https://github.com/ethereum/consensus-specs/blob/dev/specs/phase0/p2p-interface.md#protocol-identification
    // > /ProtocolPrefix/MessageName/SchemaVersion/Encoding
    protocol_id: String,
}

impl ProtocolId {
    fn new(protocol: Protocol, schema_version: SchemaVersion, encoding: Encoding) -> Self {
        let protocol_id = format!(
            "{}/{}/{}/{}",
            PROTOCOL_PREFIX, &protocol, schema_version, encoding
        );

        Self {
            protocol,
            schema_version,
            encoding,
            protocol_id,
        }
    }

    fn lighthouse_protocol_id(&self) -> lighthouse_network::rpc::protocol::ProtocolId {
        lighthouse_network::rpc::protocol::ProtocolId::new(
            self.protocol.to_lighthouse_supported_protocol(),
            lighthouse_network::rpc::protocol::Encoding::SSZSnappy,
        )
    }
}

impl AsRef<str> for ProtocolId {
    fn as_ref(&self) -> &str {
        self.protocol_id.as_ref()
    }
}

// /////////////////////////////////////////////////////////////////////////////////////////////////
// Request
// * implements `UpgradeInfo` and `OutboundUpgrade`
// * ref: https://github.com/libp2p/rust-libp2p/blob/master/protocols/request-response/src/handler/protocol.rs -> `RequestProtocol`
// /////////////////////////////////////////////////////////////////////////////////////////////////
#[derive(Clone)]
pub(super) struct OutboundRequest {
    pub(super) peer_id: PeerId, // Only for debugging
    pub(super) request: lighthouse_network::rpc::RequestType<MainnetEthSpec>,
}

pub(crate) struct RpcRequestProtocol {
    // pub(super) request: lighthouse_network::rpc::outbound::OutboundRequest<MainnetEthSpec>,
    pub(super) request: OutboundRequest,
    pub(super) max_rpc_size: usize,
    pub(super) fork_context: Arc<ForkContext>,
}

impl UpgradeInfo for RpcRequestProtocol {
    type Info = ProtocolId;
    type InfoIter = Vec<Self::Info>;

    // The list of supported RPC protocols
    fn protocol_info(&self) -> Self::InfoIter {
        vec![ProtocolId::new(
            Protocol::Status,
            SchemaVersion::V1,
            Encoding::SSZSnappy,
        )]
    }
}

pub(crate) type OutboundFramed =
    Framed<Compat<Stream>, lighthouse_network::rpc::codec::SSZSnappyOutboundCodec<MainnetEthSpec>>;

impl OutboundUpgrade<Stream> for RpcRequestProtocol {
    type Output = OutboundFramed;
    type Error = lighthouse_network::rpc::RPCError;
    type Future = BoxFuture<'static, Result<Self::Output, Self::Error>>;

    fn upgrade_outbound(self, socket: Stream, protocol_id: Self::Info) -> Self::Future {
        info!(
            "[{}] RpcRequestProtocol::upgrade_outbound: request: {:?}",
            self.request.peer_id, self.request.request
        );
        // convert to a tokio compatible socket
        let socket = socket.compat();
        let codec = match protocol_id.encoding {
            Encoding::SSZSnappy => lighthouse_network::rpc::codec::SSZSnappyOutboundCodec::new(
                protocol_id.lighthouse_protocol_id(),
                self.max_rpc_size,
                self.fork_context.clone(),
            ),
        };

        let mut socket = Framed::new(socket, codec);

        let request = self.request.request;
        let peer_id = self.request.peer_id;
        async move {
            match socket.send(request.clone()).await {
                Ok(_) => {
                    info!("[{}] [RpcRequestProtocol::upgrade_outbound] sent outbound rpc: {:?}", peer_id, request);
                }
                Err(rpc_error) => {
                    error!("[{}] [RpcRequestProtocol::upgrade_outbound] RPCError: {rpc_error}, request: {:?}", peer_id, request);
                    return Err(rpc_error);
                }
            }
            socket.close().await?;
            Ok(socket)
        }
        .boxed()
    }
}

// /////////////////////////////////////////////////////////////////////////////////////////////////
// Response
// * implements `UpgradeInfo` and `InboundUpgrade`
// * ref: https://github.com/libp2p/rust-libp2p/blob/master/protocols/request-response/src/handler/protocol.rs -> `ResponseProtocol`
// * ref: https://github.com/sigp/lighthouse/blob/e8c0d1f19b2736efb83c67a247e0022da5eaa7bb/beacon_node/eth2_libp2p/src/rpc/protocol.rs#L159
// /////////////////////////////////////////////////////////////////////////////////////////////////
pub(crate) struct RpcProtocol {
    pub(crate) fork_context: Arc<ForkContext>,
    pub(crate) max_rpc_size: usize,
    // The PeerId this communicate to. Note this is just for debugging.
    peer_id: PeerId,
}

impl RpcProtocol {
    pub(crate) fn new(
        fork_context: Arc<ForkContext>,
        max_rpc_size: usize,
        peer_id: PeerId,
    ) -> RpcProtocol {
        RpcProtocol {
            fork_context,
            max_rpc_size,
            peer_id,
        }
    }
}

impl UpgradeInfo for RpcProtocol {
    type Info = ProtocolId;
    type InfoIter = Vec<Self::Info>;

    // The list of supported RPC protocols
    fn protocol_info(&self) -> Self::InfoIter {
        vec![
            ProtocolId::new(Protocol::Status, SchemaVersion::V1, Encoding::SSZSnappy),
            ProtocolId::new(Protocol::Goodbye, SchemaVersion::V1, Encoding::SSZSnappy),
            ProtocolId::new(
                Protocol::BlocksByRange,
                SchemaVersion::V2,
                Encoding::SSZSnappy,
            ),
            ProtocolId::new(
                Protocol::BlocksByRange,
                SchemaVersion::V1,
                Encoding::SSZSnappy,
            ),
        ]
    }
}

pub type InboundOutput<TSocket> = (
    lighthouse_network::rpc::protocol::RequestType<MainnetEthSpec>,
    InboundFramed<TSocket>,
);
pub type InboundFramed<TSocket> = Framed<
    std::pin::Pin<Box<TimeoutStream<Compat<TSocket>>>>,
    lighthouse_network::rpc::codec::SSZSnappyInboundCodec<MainnetEthSpec>,
>;

impl<TSocket> InboundUpgrade<TSocket> for RpcProtocol
where
    TSocket: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    type Output = InboundOutput<TSocket>;
    type Error = lighthouse_network::rpc::RPCError;
    type Future = BoxFuture<'static, Result<Self::Output, Self::Error>>;

    fn upgrade_inbound(self, socket: TSocket, protocol_id: Self::Info) -> Self::Future {
        info!(
            "[{}] [RpcProtocol::upgrade_inbound] protocol_id: {:?}",
            self.peer_id, protocol_id,
        );

        async move {
            // convert the socket to tokio compatible socket
            let socket = socket.compat();
            let codec = match protocol_id.encoding {
                Encoding::SSZSnappy => {
                    lighthouse_network::rpc::codec::SSZSnappyInboundCodec::new(
                        protocol_id.lighthouse_protocol_id(),
                        self.max_rpc_size,
                        self.fork_context.clone(),
                    )
                }
            };
            let mut timed_socket = TimeoutStream::new(socket);
            timed_socket.set_read_timeout(Some(Duration::from_secs(5)));
            let socket = Framed::new(Box::pin(timed_socket), codec);

            match tokio::time::timeout(Duration::from_secs(REQUEST_TIMEOUT), socket.into_future())
                .await
            {
                Err(_e) => todo!(),
                Ok((Some(Ok(request)), stream)) => {
                    info!("[{}] [RpcProtocol::upgrade_inbound] received inbound message: {:?}", self.peer_id, request);
                    Ok((request, stream))
                },
                Ok((Some(Err(rpc_error)), _)) => {
                    error!(
                        "[{}] [RpcProtocol::upgrade_inbound] protocol_id: {protocol_id:?}, rpc_error: {rpc_error:?}",
                        self.peer_id
                    );
                    Err(rpc_error)
                }
                Ok((None, _)) => todo!(),
            }
        }
        .boxed()
    }
}
