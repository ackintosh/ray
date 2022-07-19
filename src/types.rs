use types::Hash256;

pub(crate) type Root = Hash256;

// `finalized_root` defaults to Root(b'\x00' * 32) for the genesis finalized checkpoint
#[allow(dead_code)]
pub(crate) fn default_finalized_root() -> Root {
    Root::from_low_u64_le(0)
}
