use crate::rpc::message::Status;
use ::types::fork_context::ForkContext;
use futures::future::BoxFuture;
use futures::prelude::*;
use libp2p::core::{ProtocolName, UpgradeInfo};
use libp2p::swarm::NegotiatedSubstream;
use libp2p::{InboundUpgrade, OutboundUpgrade};
use ssz::Encode;
use std::fmt::{Display, Formatter};
use std::io::Error;
use std::sync::Arc;
use std::time::Duration;
use tokio_io_timeout::TimeoutStream;
use tokio_util::codec::Framed;
use tokio_util::compat::{Compat, FuturesAsyncReadCompatExt};
use tracing::{debug, info};
use types::MainnetEthSpec;
use void::Void;

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
}

impl Display for Protocol {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let protocol_name = match self {
            Protocol::Status => "status",
        };
        f.write_str(protocol_name)
    }
}

#[derive(Clone, Debug)]
enum SchemaVersion {
    V1,
}

impl Display for SchemaVersion {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let version = match self {
            SchemaVersion::V1 => "1",
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

    fn into_lighthouse_protocol_id(&self) -> lighthouse_network::rpc::protocol::ProtocolId {
        // TODO:
        lighthouse_network::rpc::protocol::ProtocolId::new(
            lighthouse_network::rpc::protocol::Protocol::Status,
            lighthouse_network::rpc::protocol::Version::V1,
            lighthouse_network::rpc::protocol::Encoding::SSZSnappy,
        )
    }
}

impl ProtocolName for ProtocolId {
    fn protocol_name(&self) -> &[u8] {
        self.protocol_id.as_bytes()
    }
}

// /////////////////////////////////////////////////////////////////////////////////////////////////
// Request
// * implements `UpgradeInfo` and `OutboundUpgrade`
// * ref: https://github.com/libp2p/rust-libp2p/blob/master/protocols/request-response/src/handler/protocol.rs -> `RequestProtocol`
// /////////////////////////////////////////////////////////////////////////////////////////////////
pub(crate) struct RpcRequestProtocol {
    pub(crate) request: Status, // TODO: Generalize the type of request
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

impl OutboundUpgrade<NegotiatedSubstream> for RpcRequestProtocol {
    type Output = NegotiatedSubstream;
    type Error = RpcError;
    type Future = BoxFuture<'static, Result<Self::Output, Self::Error>>;

    fn upgrade_outbound(
        self,
        mut socket: NegotiatedSubstream,
        protocol_id: Self::Info,
    ) -> Self::Future {
        info!("upgrade_outbound: request: {:?}", self.request);

        let encoded_message: Vec<u8> = match protocol_id.encoding {
            // https://github.com/sigp/lighthouse/blob/fff4dd6311695c1d772a9d6991463915edf223d5/beacon_node/lighthouse_network/src/rpc/codec/ssz_snappy.rs#L214
            Encoding::SSZSnappy => self.request.as_ssz_bytes(),
        };
        debug!("Encoded request message: {:?}", encoded_message);

        async move {
            let number_of_bytes_written = socket.write(&encoded_message).await?;
            info!("Sent a request message. {}bytes", number_of_bytes_written);
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
}

impl UpgradeInfo for RpcProtocol {
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

pub type InboundOutput<TSocket> = (
    lighthouse_network::rpc::protocol::InboundRequest<MainnetEthSpec>,
    InboundFramed<TSocket>,
);
pub type InboundFramed<TSocket> = Framed<
    std::pin::Pin<Box<TimeoutStream<Compat<TSocket>>>>,
    lighthouse_network::rpc::codec::InboundCodec<MainnetEthSpec>,
>;

impl<TSocket> InboundUpgrade<TSocket> for RpcProtocol
where
    TSocket: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    type Output = InboundOutput<TSocket>;
    type Error = Void;
    // type Future = future::Ready<Result<Self::Output, Self::Error>>;
    type Future = BoxFuture<'static, Result<Self::Output, Self::Error>>;

    fn upgrade_inbound(self, socket: TSocket, protocol_id: Self::Info) -> Self::Future {
        info!("upgrade_inbound: protocol_id: {:?}", protocol_id);

        async move {
            let codec: lighthouse_network::rpc::codec::InboundCodec<MainnetEthSpec> =
                match protocol_id.encoding {
                    Encoding::SSZSnappy => {
                        let ssz_snappy_codec =
                        lighthouse_network::rpc::codec::base::BaseInboundCodec::new(
                            lighthouse_network::rpc::codec::ssz_snappy::SSZSnappyInboundCodec::new(
                                protocol_id.into_lighthouse_protocol_id(),
                                self.max_rpc_size,
                                self.fork_context.clone(),
                            ),
                        );
                        lighthouse_network::rpc::codec::InboundCodec::SSZSnappy(ssz_snappy_codec)
                    }
                };

            // convert the socket to tokio compatible socket
            let socket = socket.compat();
            let mut timed_socket = TimeoutStream::new(socket);
            timed_socket.set_read_timeout(Some(Duration::from_secs(5)));
            let socket = Framed::new(Box::pin(timed_socket), codec);

            match tokio::time::timeout(Duration::from_secs(REQUEST_TIMEOUT), socket.into_future())
                .await
            {
                Err(_e) => todo!(),
                Ok((Some(Ok(request)), stream)) => Ok((request, stream)),
                Ok((Some(Err(_)), _)) => todo!(),
                Ok((None, _)) => todo!(),
            }
        }
        .boxed()
    }
}

// /////////////////////////////////////////////////////////////////////////////////////////////////
// Error
// /////////////////////////////////////////////////////////////////////////////////////////////////
pub(crate) enum RpcError {
    IoError(String),
}

impl From<std::io::Error> for RpcError {
    fn from(error: Error) -> Self {
        RpcError::IoError(error.to_string())
    }
}
