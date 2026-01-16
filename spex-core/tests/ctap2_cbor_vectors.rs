use serde_cbor::Value;
use spex_core::test_vectors;
use spex_core::types::{ContactCard, Ctap2Cbor, GrantToken, InviteToken, ThreadConfig};
use std::collections::BTreeMap;

#[test]
fn tv1_thread_config_ctap2_bytes_match() {
    let thread_id = hex::decode("00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff")
        .unwrap();

    let grant1 = GrantToken {
        user_id: vec![0xa1; 20],
        role: 1,
        flags: Some(7),
        expires_at: None,
        extensions: BTreeMap::new(),
    };
    let grant2 = GrantToken {
        user_id: vec![0xb2; 20],
        role: 2,
        flags: None,
        expires_at: Some(1),
        extensions: BTreeMap::new(),
    };

    let config = ThreadConfig {
        proto_major: 1,
        proto_minor: 0,
        ciphersuite_id: 1,
        flags: 1,
        thread_id,
        grants: vec![grant1, grant2],
        extensions: BTreeMap::new(),
    };

    let got = config.to_ctap2_canonical_bytes().unwrap();
    assert_eq!(hex::encode(got), test_vectors::TV1_CONFIG_CBOR_HEX);
}

#[test]
fn tv2_contact_card_ctap2_bytes_match() {
    let mut extensions = BTreeMap::new();
    extensions.insert(
        6,
        Value::Bytes(hex::decode("deadbeefcafebabe").unwrap()),
    );

    let invite = InviteToken {
        major: 1,
        minor: 2,
        requires_puzzle: true,
        extensions: BTreeMap::new(),
    };

    let card = ContactCard {
        user_id: vec![0xa1; 20],
        verifying_key: hex::decode(test_vectors::TV2_ED25519_PUB_HEX).unwrap(),
        device_id: vec![0x0c; 16],
        device_nonce: hex::decode("01a676b3").unwrap(),
        issued_at: 0x676b3a80,
        invite: Some(invite),
        signature: None,
        extensions,
    };

    let got = card.to_ctap2_canonical_bytes().unwrap();
    assert_eq!(hex::encode(got), test_vectors::TV2_CARD_WO_SIG_CBOR_HEX);
}
