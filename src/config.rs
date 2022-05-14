use std::fs::File;
use std::io::{BufReader, Read};
use std::path::PathBuf;
use types::Config;

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
}

fn load_config(network_config_dir: &PathBuf) -> Result<Config, String> {
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

fn load_genesis_state(network_config_dir: &PathBuf) -> Result<Vec<u8>, String> {
    let file = File::open(network_config_dir.join("genesis.ssz"))
        .map_err(|e| format!("Failed to open genesis.ssz: {}", e))?;
    let mut reader = BufReader::new(file);
    let mut buf = vec![];
    reader
        .read_to_end(&mut buf)
        .map_err(|e| format!("Failed to read genesis.ssz: {}", e))?;
    Ok(buf)
}
