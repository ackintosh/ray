use std::fmt::{Debug, Display, Formatter};

#[derive(Debug)]
pub enum RPCError {}

impl Display for RPCError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str("RPC Error")
    }
}

impl std::error::Error for RPCError {
}
