use futures::prelude::*;
use libp2p::core::{ProtocolName, UpgradeInfo};
use libp2p::swarm::NegotiatedSubstream;
use libp2p::{InboundUpgrade, OutboundUpgrade};
use std::fmt::{Display, Formatter};
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

#[derive(Clone)]
pub(crate) struct ProtocolId {
    protocol: Protocol,
    schema_version: SchemaVersion,
    encoding: Encoding,
    // see https://github.com/ethereum/consensus-specs/blob/dev/specs/phase0/p2p-interface.md#protocol-identification
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

// ref: https://github.com/sigp/lighthouse/blob/e8c0d1f19b2736efb83c67a247e0022da5eaa7bb/beacon_node/eth2_libp2p/src/rpc/protocol.rs#L159
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

impl OutboundUpgrade<NegotiatedSubstream> for RpcProtocol {
    type Output = NegotiatedSubstream;
    type Error = Void;
    type Future = future::Ready<Result<Self::Output, Self::Error>>;

    fn upgrade_outbound(self, socket: NegotiatedSubstream, _info: Self::Info) -> Self::Future {
        future::ok(socket)
    }
}
