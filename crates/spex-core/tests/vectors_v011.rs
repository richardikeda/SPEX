use spex_core::{crypto, mls_ext, vectors};
use spex_core::crypto::{HashId};
use ed25519_dalek::{Signature};

#[test]
fn tv1_cfg_hash_sha256_matches() {
    let config_bytes = hex::decode(vectors::TV1_CONFIG_CBOR_HEX).unwrap();
    let got = crypto::hash_bytes(HashId::Sha256, &config_bytes);
    let got_hex = hex::encode(got);
    assert_eq!(got_hex, vectors::TV1_CFG_HASH_SHA256_HEX);
}

#[test]
fn tv2_ed25519_pubkey_matches_seed() {
    let seed = hex::decode(vectors::TV2_ED25519_SEED_HEX).unwrap();
    let sk = crypto::ed25519_signing_key_from_seed(&seed).unwrap();
    let vk = crypto::ed25519_verify_key(&sk);
    assert_eq!(hex::encode(vk.to_bytes()), vectors::TV2_ED25519_PUB_HEX);
}

#[test]
fn tv2_card_hash_sha256_matches() {
    let card_wo_sig = hex::decode(vectors::TV2_CARD_WO_SIG_CBOR_HEX).unwrap();
    let got = crypto::hash_bytes(HashId::Sha256, &card_wo_sig);
    assert_eq!(hex::encode(got), vectors::TV2_CARD_HASH_SHA256_HEX);
}

#[test]
fn tv2_signature_matches() {
    let seed = hex::decode(vectors::TV2_ED25519_SEED_HEX).unwrap();
    let sk = crypto::ed25519_signing_key_from_seed(&seed).unwrap();

    let card_hash = hex::decode(vectors::TV2_CARD_HASH_SHA256_HEX).unwrap();
    let sig = crypto::ed25519_sign_hash(&sk, &card_hash);
    assert_eq!(hex::encode(sig.to_bytes()), vectors::TV2_CARD_SIG_HEX);

    // sanity verify
    let vk = crypto::ed25519_verify_key(&sk);
    let sig2 = Signature::from_bytes(&sig.to_bytes());
    crypto::ed25519_verify_hash(&vk, &card_hash, &sig2).unwrap();
}

#[test]
fn tv3_mls_ext_proto_suite_bytes_match() {
    let got = mls_ext::ext_proto_suite_bytes(0, 1, 1, 0);
    assert_eq!(hex::encode(got), vectors::TV3_EXT_PROTO_SUITE_HEX);
}

#[test]
fn tv3_mls_ext_cfg_hash_bytes_match() {
    let cfg_hash = hex::decode(vectors::TV1_CFG_HASH_SHA256_HEX).unwrap();
    let got = mls_ext::ext_cfg_hash_bytes(1, &cfg_hash);
    assert_eq!(hex::encode(got), vectors::TV3_EXT_CFG_HASH_HEX);
}
