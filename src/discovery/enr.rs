use crate::types::Enr;
use libp2p::bytes::Bytes;
use ssz::Decode;
use types::EnrForkId;

const ETH2_ENR_KEY: &str = "eth2";

pub(crate) trait Eth2Enr {
    fn eth2(&self) -> Result<EnrForkId, String>;
}

impl Eth2Enr for Enr {
    fn eth2(&self) -> Result<EnrForkId, String> {
        let eth2_bytes: Bytes = self
            .get_decodable(ETH2_ENR_KEY)
            .ok_or("ENR has no eth2 field")?
            .map_err(|e| format!("Failed to decode eth2 field: {}", e.to_string()))?;

        EnrForkId::from_ssz_bytes(&eth2_bytes)
            .map_err(|e| format!("Could not decode EnrForkId: {e:?}"))
    }
}
