use spex_core::{test_vectors, types::ProtoSuite};
use spex_mls::{Commit, Group, GroupConfig};

/// Ensures TV3 extensions are present in a newly created group context.
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

/// Ensures commits rebuild group context extensions using TV3 vectors.
#[test]
fn tv3_group_context_commit_rebuilds_extensions() {
    let initial_suite = ProtoSuite {
        major: 1,
        minor: 0,
        ciphersuite_id: 2,
    };
    let initial_cfg_hash = vec![0xAA; 32];
    let config = GroupConfig::new(initial_suite, 1, 2, initial_cfg_hash);
    let mut group = Group::create(config);

    let proto_suite = ProtoSuite {
        major: 0,
        minor: 1,
        ciphersuite_id: 1,
    };
    let cfg_hash = hex::decode(test_vectors::TV1_CFG_HASH_SHA256_HEX).unwrap();
    let mut commit = Commit::new(1);
    commit.proto_suite = Some(proto_suite);
    commit.flags = Some(0);
    commit.cfg_hash_id = Some(1);
    commit.cfg_hash = Some(cfg_hash);

    let context = group.apply_commit(commit);
    let extensions = context.extensions();

    assert_eq!(group.epoch(), 1);
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
