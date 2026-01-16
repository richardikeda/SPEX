use spex_core::{test_vectors, types::ProtoSuite};
use spex_mls::{Group, GroupConfig};

#[test]
fn tv3_group_context_has_spex_extensions() {
    let proto_suite = ProtoSuite {
        major: 0,
        minor: 1,
        ciphersuite_id: 1,
    };
    let cfg_hash = hex::decode(test_vectors::TV1_CFG_HASH_SHA256_HEX).unwrap();
    let config = GroupConfig::new(proto_suite, 0, 1, cfg_hash);

    let group = Group::create(config);
    let extensions = group.context().extensions();

    assert_eq!(extensions.len(), 2);
    assert_eq!(
        hex::encode(&extensions[0]),
        test_vectors::TV3_EXT_PROTO_SUITE_HEX
    );
    assert_eq!(
        hex::encode(&extensions[1]),
        test_vectors::TV3_EXT_CFG_HASH_HEX
    );
}
