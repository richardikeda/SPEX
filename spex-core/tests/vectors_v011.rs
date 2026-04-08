// SPDX-License-Identifier: MPL-2.0
use ed25519_dalek::Signature;
use spex_core::hash::HashId;
use spex_core::{aead_ad, hash, mls_ext, sign, test_vectors, types};

#[test]
fn tv1_cfg_hash_sha256_matches() {
    let config_bytes = hex::decode(test_vectors::TV1_CONFIG_CBOR_HEX).unwrap();
    let got = hash::hash_bytes(HashId::Sha256, &config_bytes);
    let got_hex = hex::encode(got);
    assert_eq!(got_hex, test_vectors::TV1_CFG_HASH_SHA256_HEX);
}

#[test]
fn tv2_ed25519_pubkey_matches_seed() {
    let seed = hex::decode(test_vectors::TV2_ED25519_SEED_HEX).unwrap();
    let sk = sign::ed25519_signing_key_from_seed(&seed).unwrap();
    let vk = sign::ed25519_verify_key(&sk);
    assert_eq!(
        hex::encode(vk.to_bytes()),
        test_vectors::TV2_ED25519_PUB_HEX
    );
}

#[test]
fn tv2_card_hash_sha256_matches() {
    let card_wo_sig = hex::decode(test_vectors::TV2_CARD_WO_SIG_CBOR_HEX).unwrap();
    let got = hash::hash_bytes(HashId::Sha256, &card_wo_sig);
    assert_eq!(hex::encode(got), test_vectors::TV2_CARD_HASH_SHA256_HEX);
}

#[test]
fn tv2_signature_matches() {
    let seed = hex::decode(test_vectors::TV2_ED25519_SEED_HEX).unwrap();
    let sk = sign::ed25519_signing_key_from_seed(&seed).unwrap();

    let card_hash = hex::decode(test_vectors::TV2_CARD_HASH_SHA256_HEX).unwrap();
    let sig = sign::ed25519_sign_hash(&sk, &card_hash);
    assert_eq!(hex::encode(sig.to_bytes()), test_vectors::TV2_CARD_SIG_HEX);

    // sanity verify
    let vk = sign::ed25519_verify_key(&sk);
    let sig2 = Signature::from_bytes(&sig.to_bytes());
    sign::ed25519_verify_hash(&vk, &card_hash, &sig2).unwrap();
}

#[test]
fn tv3_mls_ext_proto_suite_bytes_match() {
    let got = mls_ext::ext_proto_suite_bytes(0, 1, 1, 0);
    assert_eq!(hex::encode(got), test_vectors::TV3_EXT_PROTO_SUITE_HEX);
}

#[test]
fn tv3_mls_ext_cfg_hash_bytes_match() {
    let cfg_hash = hex::decode(test_vectors::TV1_CFG_HASH_SHA256_HEX).unwrap();
    let got = mls_ext::ext_cfg_hash_bytes(1, &cfg_hash);
    assert_eq!(hex::encode(got), test_vectors::TV3_EXT_CFG_HASH_HEX);
}

#[test]
fn tv4_aead_ad_matches() {
    let thread_id =
        types::to_fixed::<32>(&hex::decode(test_vectors::TV4_THREAD_ID_HEX).unwrap()).unwrap();
    let cfg_hash =
        types::to_fixed::<32>(&hex::decode(test_vectors::TV4_CFG_HASH_SHA256_HEX).unwrap())
            .unwrap();
    let sender_userid =
        types::to_fixed::<20>(&hex::decode(test_vectors::TV4_SENDER_USERID_HEX).unwrap()).unwrap();

    let suite = types::ProtoSuite {
        major: test_vectors::TV4_PROTO_SUITE_MAJOR,
        minor: test_vectors::TV4_PROTO_SUITE_MINOR,
        ciphersuite_id: test_vectors::TV4_PROTO_SUITE_CIPHERSUITE_ID,
    };
    let got = aead_ad::build_ad(
        &thread_id,
        test_vectors::TV4_EPOCH,
        &cfg_hash,
        suite,
        test_vectors::TV4_SEQ,
        &sender_userid,
    );
    assert_eq!(hex::encode(got), test_vectors::TV4_AEAD_AD_HEX);
}
