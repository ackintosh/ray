use enr::{CombinedKey, CombinedPublicKey, Enr};
use libp2p::PeerId;

// SEE: https://github.com/sigp/lighthouse/blob/4af6fcfafd2c29bca82474ee378cda9ac254783a/beacon_node/eth2_libp2p/src/discovery/enr_ext.rs#L200
pub(crate) fn enr_to_peer_id(enr: &Enr<CombinedKey>) -> PeerId {
    match enr.public_key() {
        CombinedPublicKey::Secp256k1(pk) => {
            let pk_bytes = pk.to_bytes();
            let libp2p_pk = libp2p::core::PublicKey::Secp256k1(
                libp2p::core::identity::secp256k1::PublicKey::decode(&pk_bytes)
                    .expect("valid public key"),
            );
            PeerId::from(libp2p_pk)
        }
        CombinedPublicKey::Ed25519(_) => unreachable!(), // not implemented as the ENR key is generated with secp256k1
    }
}
