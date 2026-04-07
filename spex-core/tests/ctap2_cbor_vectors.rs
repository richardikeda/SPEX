use serde_cbor::Value;
use spex_core::cbor::ctap2_canonical_value_from_slice;
use spex_core::test_vectors;
use spex_core::types::{ContactCard, Ctap2Cbor, GrantToken, InviteToken, ThreadConfig};
use std::collections::BTreeMap;

// Builds the ThreadConfig instance that matches the TV1 test vector.
fn build_tv1_config() -> ThreadConfig {
    let thread_id =
        hex::decode("00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff").unwrap();

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

    ThreadConfig {
        proto_major: 1,
        proto_minor: 0,
        ciphersuite_id: 1,
        flags: 1,
        thread_id,
        grants: vec![grant1, grant2],
        extensions: BTreeMap::new(),
    }
}

// Builds the ContactCard instance that matches the TV2 test vector without a signature.
fn build_tv2_card() -> ContactCard {
    let mut extensions = BTreeMap::new();
    extensions.insert(6, Value::Bytes(hex::decode("deadbeefcafebabe").unwrap()));

    let invite = InviteToken {
        major: 1,
        minor: 2,
        requires_puzzle: true,
        extensions: BTreeMap::new(),
    };

    ContactCard {
        user_id: vec![0xa1; 20],
        verifying_key: hex::decode(test_vectors::TV2_ED25519_PUB_HEX).unwrap(),
        device_id: vec![0x0c; 16],
        device_nonce: hex::decode("01a676b3").unwrap(),
        issued_at: 0x676b3a80,
        invite: Some(invite),
        signature: None,
        extensions,
    }
}

// Extracts integer keys from a CBOR map to validate ordering in tests.
fn map_keys(value: &Value) -> Vec<i128> {
    match value {
        Value::Map(entries) => entries
            .keys()
            .map(|key| match key {
                Value::Integer(value) => *value,
                _ => panic!("expected integer key"),
            })
            .collect(),
        _ => panic!("expected CBOR map"),
    }
}

#[test]
// Ensures the thread configuration CBOR matches the TV1 canonical bytes.
fn tv1_thread_config_ctap2_bytes_match() {
    let config = build_tv1_config();
    let got = config.to_ctap2_canonical_bytes().unwrap();
    let expected = hex::decode(test_vectors::TV1_CONFIG_CBOR_HEX).unwrap();
    assert_eq!(got, expected);
}

#[test]
// Ensures the contact card CBOR matches the TV2 canonical bytes (without signature).
fn tv2_contact_card_ctap2_bytes_match() {
    let card = build_tv2_card();
    let got = card.to_ctap2_canonical_bytes().unwrap();
    let expected = hex::decode(test_vectors::TV2_CARD_WO_SIG_CBOR_HEX).unwrap();
    assert_eq!(got, expected);
}

#[test]
// Checks optional field omission and key ordering inside the TV1 config grants.
fn tv1_thread_config_optional_fields_and_key_ordering() {
    let config = build_tv1_config();
    let config_value = config.to_cbor_value();
    assert_eq!(map_keys(&config_value), vec![0, 1, 2, 3, 4, 5]);

    let grants = match config_value {
        Value::Map(entries) => entries
            .into_iter()
            .find(|(key, _)| matches!(key, Value::Integer(5)))
            .and_then(|(_, value)| match value {
                Value::Array(values) => Some(values),
                _ => None,
            })
            .expect("expected grants array"),
        _ => panic!("expected config map"),
    };

    let grant1 = &grants[0];
    let grant2 = &grants[1];
    assert_eq!(map_keys(grant1), vec![0, 1, 2]);
    assert_eq!(map_keys(grant2), vec![0, 1, 3]);
}

#[test]
// Verifies card optional fields and extension key ordering for the TV2 contact card.
fn tv2_contact_card_optional_fields_and_extension_ordering() {
    let card = build_tv2_card();
    let card_value = card.to_cbor_value();
    assert_eq!(map_keys(&card_value), vec![0, 1, 2, 3, 4, 5, 6]);

    let extension_value = match card_value {
        Value::Map(entries) => entries
            .into_iter()
            .find(|(key, _)| matches!(key, Value::Integer(6)))
            .map(|(_, value)| value)
            .expect("expected extension at key 6"),
        _ => panic!("expected card map"),
    };

    assert_eq!(
        extension_value,
        Value::Bytes(hex::decode("deadbeefcafebabe").unwrap())
    );
}

#[test]
// Ensures truncated thread config CBOR fails with explicit decode errors (no panic path).
fn rejects_truncated_thread_config_ctap2_decode() {
    let config = build_tv1_config();
    let encoded = config.to_ctap2_canonical_bytes().expect("encode");
    let truncated = &encoded[..encoded.len().saturating_sub(1)];
    let decoded = ctap2_canonical_value_from_slice(truncated);
    assert!(decoded.is_err());
}

#[test]
// Ensures malformed card payloads fail decode deterministically without panics.
fn rejects_malformed_contact_card_ctap2_decode() {
    let card = build_tv2_card();
    let mut encoded = card.to_ctap2_canonical_bytes().expect("encode");
    encoded.push(0xFF);
    let decoded = ContactCard::decode_ctap2(&encoded);
    assert!(decoded.is_err());
}
