use std::fs::File;
use std::path::PathBuf;
use types::Config;

pub(crate) struct NetworkConfig {
    pub(crate) config: Config,
}

impl NetworkConfig {
    pub(crate) fn new() -> Result<Self, String> {
        Ok(NetworkConfig {
            config: load_config()?,
        })
    }
}

fn load_config() -> Result<Config, String> {
    let path = env!("CARGO_MANIFEST_DIR")
        .parse::<PathBuf>()
        .expect("should parse manifest dir as path")
        .join("network_config")
        .join("config.yaml");

    File::open(path.clone())
        .map_err(|e| format!("Unable to open {}: {:?}", path.display(), e))
        .and_then(|file| {
            serde_yaml::from_reader(file)
                .map_err(|e| format!("Unable to parse config {}: {:?}", path.display(), e))
        })
}
