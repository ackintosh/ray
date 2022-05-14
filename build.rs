use std::fs::File;
use std::path::PathBuf;
use zip::ZipArchive;

// Extracts zipped genesis state on first run.
// Ref: https://github.com/sigp/lighthouse/blob/1031f79aca762e692ade20df52ddd038fedcb999/common/eth2_network_config/build.rs#L20
fn main() {
    if let Err(e) = uncompress_genesis_state() {
        panic!("Failed to uncompress genesis state: {}", e);
    }
}

// Uncompress the genesis state archive into `network_config` folder.
// The `genesis.ssz.zip` file is copied from:
// https://github.com/sigp/lighthouse/tree/15b88115804fd8de4ed81e773aadecfe9afdd674/common/eth2_network_config/built_in_network_configs/kiln
fn uncompress_genesis_state() -> Result<(), String> {
    let network_config_dir = env!("CARGO_MANIFEST_DIR")
        .parse::<PathBuf>()
        .map_err(|e| format!("Failed to parse manifest dir: {}", e))?
        .join("network_config");

    let path_to_genesis_ssz = network_config_dir.join("genesis.ssz");

    if path_to_genesis_ssz.exists() {
        return Ok(());
    }

    let mut archive = ZipArchive::new(
        File::open(network_config_dir.join("genesis.ssz.zip"))
            .map_err(|e| format!("should open genesis.ssz.zip: {}", e))?,
    )
    .map_err(|e| format!("Failed to read zip file: {}", e))?;

    let mut genesis_ssz_file = archive
        .by_name("genesis.ssz")
        .map_err(|e| format!("should retrieve genesis.ssz: {}", e))?;

    let mut dest =
        File::create(path_to_genesis_ssz).map_err(|e| format!("Failed to create genesis.ssz: {}", e))?;

    std::io::copy(&mut genesis_ssz_file, &mut dest)
        .map_err(|e| format!("Failed to copy genesis.ssz: {}", e))?;

    Ok(())
}
