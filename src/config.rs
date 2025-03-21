use discv5::Enr;
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};
use tracing::info;
use types::Config;

// Ref: kiln-testnet config
// https://github.com/eth-clients/merge-testnets/tree/main/kiln
pub(crate) struct NetworkConfig {
    // pub(crate) config: Config,
    // pub(crate) genesis_state_bytes: Vec<u8>,
    pub(crate) boot_enr: Vec<Enr>,
}

impl NetworkConfig {
    pub(crate) fn new() -> Result<Self, String> {
        let network_config_dir = env!("CARGO_MANIFEST_DIR")
            .parse::<PathBuf>()
            .map_err(|e| format!("should parse manifest dir as path: {}", e))?
            .join("network_config");

        Ok(NetworkConfig {
            // config: load_config(&network_config_dir)?,
            // genesis_state_bytes: load_genesis_state(&network_config_dir)?,
            boot_enr: load_boot_enr(&network_config_dir)?,
        })
    }

    // pub(crate) fn genesis_beacon_state(&self) -> Result<BeaconState<MainnetEthSpec>, String> {
    //     let spec = self.chain_spec()?;
    //     BeaconState::from_ssz_bytes(&self.genesis_state_bytes, &spec)
    //         .map_err(|e| format!("Failed to decode genesis state bytes: {:?}", e))
    // }
    //
    // pub(crate) fn chain_spec(&self) -> Result<ChainSpec, String> {
    //     ChainSpec::from_config::<MainnetEthSpec>(&self.config).ok_or_else(|| {
    //         "YAML configuration incompatible with spec constants for MainnetEthSpec".to_string()
    //     })
    // }
}

// fn load_config(network_config_dir: &Path) -> Result<Config, String> {
//     let path = network_config_dir.join("config.yaml");
//     info!("Loading network config from {}", path.display());
// 
//     File::open(path.clone())
//         .map_err(|e| format!("Unable to open {}: {:?}", path.display(), e))
//         .and_then(|file| {
//             serde_yaml::from_reader(file)
//                 .map_err(|e| format!("Unable to parse config {}: {:?}", path.display(), e))
//         })
// }

// fn load_genesis_state(network_config_dir: &Path) -> Result<Vec<u8>, String> {
//     let path = network_config_dir.join("genesis.ssz");
//     info!("Loading genesis state from {}", path.display());
// 
//     let file = File::open(path).map_err(|e| format!("Failed to open genesis.ssz: {}", e))?;
//     let mut reader = BufReader::new(file);
//     let mut buf = vec![];
//     reader
//         .read_to_end(&mut buf)
//         .map_err(|e| format!("Failed to read genesis.ssz: {}", e))?;
//     Ok(buf)
// }

fn load_boot_enr(network_config_dir: &Path) -> Result<Vec<Enr>, String> {
    let path = network_config_dir.join("boot_enr.yaml");
    info!("Loading boot-enr from {}", path.display());

    File::open(path)
        .map_err(|e| format!("Failed to open boot_enr.yaml: {}", e))
        .and_then(|file| {
            serde_yaml::from_reader(file).map_err(|e| format!("Unable to parse boot enr: {}", e))
        })
}
