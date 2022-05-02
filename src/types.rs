use types::Hash256;

// `compute_fork_digest(current_fork_version, genesis_validators_root)`
pub(crate) type ForkDigest = [u8; 4];

pub(crate) type Root = Hash256;

// `finalized_root` defaults to Root(b'\x00' * 32) for the genesis finalized checkpoint
pub(crate) fn default_finalized_root() -> Root {
    Root::from_low_u64_le(0)
}