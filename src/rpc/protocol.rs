use crate::rpc::message::Status;
use futures::future::BoxFuture;
use futures::prelude::*;
use libp2p::core::{ProtocolName, UpgradeInfo};
use libp2p::swarm::NegotiatedSubstream;
use libp2p::{InboundUpgrade, OutboundUpgrade};
use ssz::Encode;
use std::fmt::{Display, Formatter};
use std::io::Error;
use tracing::{debug, info};
use void::Void;

// spec:
// https://github.com/ethereum/consensus-specs/blob/dev/specs/phase0/p2p-interface.md#protocol-identification
// ProtocolPrefix
const PROTOCOL_PREFIX: &str = "/eth2/beacon_chain/req";

#[derive(Clone)]
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

#[derive(Clone)]
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

#[derive(Clone)]
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
#[derive(Clone)]
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
pub(crate) struct RpcProtocol;

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

impl InboundUpgrade<NegotiatedSubstream> for RpcProtocol {
    type Output = NegotiatedSubstream;
    type Error = Void;
    type Future = future::Ready<Result<Self::Output, Self::Error>>;

    fn upgrade_inbound(self, socket: NegotiatedSubstream, _info: Self::Info) -> Self::Future {
        future::ok(socket)
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
