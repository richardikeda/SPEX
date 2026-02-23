use std::{env, fs};

use spex_client::{load_state, random_hex, save_state, IdentityState, LocalState};

/// Ensures encrypted state persists and restores using a passphrase.
#[test]
fn saves_and_loads_encrypted_state_with_passphrase() {
    let dir = tempfile::tempdir().expect("temp dir");
    let state_path = dir.path().join("state.json");
    let passphrase_path = dir.path().join("passphrase.txt");
    std::fs::write(&passphrase_path, "test-passphrase").expect("write passphrase");

    env::set_var("SPEX_STATE_PASSPHRASE_FILE", &passphrase_path);
    env::set_var("SPEX_STATE_PATH", &state_path);

    let state = LocalState {
        identity: Some(IdentityState {
            user_id_hex: random_hex(32),
            signing_key_hex: random_hex(32),
            verifying_key_hex: random_hex(32),
            device_id_hex: random_hex(16),
            device_nonce_hex: random_hex(16),
        }),
        ..Default::default()
    };
    let expected_user_id = state
        .identity
        .as_ref()
        .expect("identity")
        .user_id_hex
        .clone();

    save_state(&state).expect("save");
    let contents = fs::read_to_string(&state_path).expect("read state file");
    assert!(contents.contains("spex_encrypted_state"));
    assert!(!contents.contains("user_id_hex"));

    let loaded = load_state().expect("load");
    let loaded_identity = loaded.identity.expect("loaded identity");
    assert_eq!(loaded_identity.user_id_hex, expected_user_id);

    env::remove_var("SPEX_STATE_PASSPHRASE_FILE");
    env::remove_var("SPEX_STATE_PATH");
}
