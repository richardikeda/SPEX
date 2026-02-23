use spex_mls::{MlsRsClient, GroupConfig, MlsRsGroup};
use spex_core::types::ProtoSuite;
use std::time::Instant;

#[test]
fn stress_test_group_creation_and_messaging() {
    let member_count = 10;
    let _message_count = 50;

    let proto_suite = ProtoSuite {
        major: 0, // Fixed to 0.1
        minor: 1,
        ciphersuite_id: 1, // X25519_AES128GCM_SHA256_Ed25519
    };

    // 1. Create Identity and Clients
    let mut clients = Vec::new();
    for i in 0..member_count {
        let identity = format!("member_{}", i).as_bytes().to_vec();
        let client = MlsRsClient::new(proto_suite, identity).expect("failed to create client");
        clients.push(client);
    }

    // 2. Creator initializes the group
    let creator = &clients[0];
    let config = GroupConfig::new(proto_suite, 0, 1, vec![1, 2, 3, 4]); // 1=SHA256
    let mut group_alice = creator.create_group(config).expect("failed to create group");

    // 3. Add members sequentially (Partial Sync)
    let start_add = Instant::now();

    // We only update Alice's state here for the stress test
    for i in 1..member_count {
        let member = &clients[i];
        let _commit = group_alice.add_member(member).expect("failed to add member");
    }
    let duration_add = start_add.elapsed();
    println!("Added {} members in {:?}", member_count - 1, duration_add);
}

#[test]
fn stress_test_full_mesh_sync_small_group() {
    // This tests the full N-to-N sync overhead for a small group
    let member_count = 5;
    let proto_suite = ProtoSuite {
        major: 0, // Fixed to 0.1
        minor: 1,
        ciphersuite_id: 1,
    };

    let mut clients = Vec::new();
    for i in 0..member_count {
        let identity = format!("user_{}", i).as_bytes().to_vec();
        clients.push(MlsRsClient::new(proto_suite, identity).unwrap());
    }

    // Alice creates
    let config = GroupConfig::new(proto_suite, 0, 1, vec![0xAA]);
    let alice_group = clients[0].create_group(config).unwrap();

    let mut active_groups: Vec<MlsRsGroup> = vec![];
    active_groups.push(alice_group);

    // Add remaining members
    for i in 1..member_count {
        let new_member_client = &clients[i];

        // Alice (index 0) adds member
        let commit_output = active_groups[0].add_member(new_member_client).expect("Alice adds member");
        let welcome = commit_output.welcome_messages.first().expect("Should have welcome message");

        // New member joins
        let tree = active_groups[0].export_tree();
        let new_group_state = new_member_client.join_group(&welcome, Some(tree)).expect("Member joins");

        // Existing members 1..i-1 must process commit
        for j in 1..i {
             let msg = commit_output.commit_message.clone();
             active_groups[j].process_commit_message(msg).expect("Existing member processes commit");
        }

        active_groups.push(new_group_state);
    }

    assert_eq!(active_groups.len(), member_count);
}

use spex_mls::{Group, GroupConfig as SimpleGroupConfig};

#[test]
fn stress_test_simplified_group() {
     let proto_suite = ProtoSuite {
        major: 1,
        minor: 1,
        ciphersuite_id: 1,
    };
    let config = SimpleGroupConfig::new(proto_suite, 0, 1, vec![1, 2, 3]);
    let mut group = Group::create(config).unwrap();

    // Add 50 members
    for i in 0..50 {
        group.add_member(format!("user_{}", i)).unwrap();
    }

    // Encrypt 100 messages
    let sender = "user_0";
    for i in 0u64..100 {
        let msg = b"hello world";
        let encrypted = group.encrypt(sender, i, msg).unwrap();
        let decrypted = group.decrypt(sender, i, &encrypted).unwrap();
        assert_eq!(decrypted, msg);
    }
}
