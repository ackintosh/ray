use std::fs::File;
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};
use types::{BeaconState, ChainSpec, Config, MainnetEthSpec};

// Ref: kiln-testnet config
// https://github.com/eth-clients/merge-testnets/tree/main/kiln
pub(crate) struct NetworkConfig {
    pub(crate) config: Config,
    pub(crate) genesis_state_bytes: Vec<u8>,
}

impl NetworkConfig {
    pub(crate) fn new() -> Result<Self, String> {
        let network_config_dir = env!("CARGO_MANIFEST_DIR")
            .parse::<PathBuf>()
            .map_err(|e| format!("should parse manifest dir as path: {}", e))?
            .join("network_config");

        Ok(NetworkConfig {
            config: load_config(&network_config_dir)?,
            genesis_state_bytes: load_genesis_state(&network_config_dir)?,
        })
    }

    pub(crate) fn genesis_beacon_state(&self) -> Result<BeaconState<MainnetEthSpec>, String> {
        let spec = self.chain_spec()?;
        BeaconState::from_ssz_bytes(&self.genesis_state_bytes, &spec)
            .map_err(|e| format!("Failed to decode genesis state bytes: {:?}", e))
    }

    pub(crate) fn chain_spec(&self) -> Result<ChainSpec, String> {
        ChainSpec::from_config::<MainnetEthSpec>(&self.config).ok_or_else(|| {
            "YAML configuration incompatible with spec constants for MainnetEthSpec".to_string()
        })
    }
}

fn load_config(network_config_dir: &Path) -> Result<Config, String> {
    let path_to_config = network_config_dir.join("config.yaml");

    File::open(path_to_config.clone())
        .map_err(|e| format!("Unable to open {}: {:?}", path_to_config.display(), e))
        .and_then(|file| {
            serde_yaml::from_reader(file).map_err(|e| {
                format!(
                    "Unable to parse config {}: {:?}",
                    path_to_config.display(),
                    e
                )
            })
        })
}

fn load_genesis_state(network_config_dir: &Path) -> Result<Vec<u8>, String> {
    let file = File::open(network_config_dir.join("genesis.ssz"))
        .map_err(|e| format!("Failed to open genesis.ssz: {}", e))?;
    let mut reader = BufReader::new(file);
    let mut buf = vec![];
    reader
        .read_to_end(&mut buf)
        .map_err(|e| format!("Failed to read genesis.ssz: {}", e))?;
    Ok(buf)
}
