#[test]
#[ignore]
fn test_peer_persistence_across_restarts() {
    // TODO: Implement this when the transport layer supports persistent peer storage.
    // 1. Start a node with a persistent path.
    // 2. Connect to a bootstrap peer.
    // 3. Stop the node.
    // 4. Restart the node.
    // 5. Verify it reconnects to the bootstrap peer automatically from disk state.
    panic!("Peer persistence not yet implemented");
}

#[test]
#[ignore]
fn test_anti_eclipse_peer_scoring() {
    // TODO: Implement this when peer scoring is added.
    // 1. Connect malicious peers that serve bad data.
    // 2. Verify their score drops.
    // 3. Verify they are disconnected/banned.
    panic!("Peer scoring not yet implemented");
}
