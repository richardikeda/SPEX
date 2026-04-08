// SPDX-License-Identifier: MPL-2.0
use std::{
    fs,
    net::SocketAddr,
    path::Path,
    process::Command,
    sync::{Arc, OnceLock},
};

use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine};
use reqwest::{Client, StatusCode};
use serde_json::json;
use spex_bridge::{app, init_state_with_clock, Clock};
use spex_client::{
    create_identity, create_identity_in_state, create_thread_for_state, load_state, now_unix,
    save_state, sign_grant, LocalState,
};
use spex_core::{
    hash::HashId,
    pow::{self, PowNonceParams, PowParams},
    types::{Ctap2Cbor, Envelope, GrantToken},
};
use spex_transport::inbox::derive_inbox_scan_key;
use tempfile::TempDir;

/// Returns a deterministic clock to make grant-expiry tests stable.
struct FixedClock {
    now: u64,
}

impl Clock for FixedClock {
    /// Returns the fixed UNIX timestamp for bridge validation.
    fn now(&self) -> u64 {
        self.now
    }
}

/// Keeps local test paths and passphrase settings for one CLI state file.
struct CliStateEnv {
    state_path: String,
    passphrase_path: String,
}

/// Builds a temporary CLI state environment with passphrase-backed encryption enabled.
fn make_cli_state_env(prefix: &str) -> (TempDir, CliStateEnv) {
    let dir = tempfile::tempdir().expect("create temporary directory");
    let state_path = dir.path().join(format!("{prefix}_state.json"));
    let passphrase_path = dir.path().join(format!("{prefix}_passphrase.txt"));
    fs::write(&passphrase_path, "test-passphrase").expect("write passphrase file");
    (
        dir,
        CliStateEnv {
            state_path: state_path.display().to_string(),
            passphrase_path: passphrase_path.display().to_string(),
        },
    )
}

/// Builds the CLI binary once when the dynamic test binary path variable is unavailable.
fn ensure_cli_binary_path() -> String {
    static CLI_PATH: OnceLock<String> = OnceLock::new();
    CLI_PATH
        .get_or_init(|| {
            if let Ok(path) = std::env::var("CARGO_BIN_EXE_spex") {
                return path;
            }
            let status = Command::new("cargo")
                .args(["build", "-p", "spex-cli", "--bin", "spex"])
                .status()
                .expect("build spex binary");
            assert!(status.success(), "cargo build for spex binary failed");
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../target/debug/spex")
                .display()
                .to_string()
        })
        .clone()
}

/// Runs the SPEX CLI binary with explicit state/passphrase environment variables.
fn run_spex(env: &CliStateEnv, args: &[&str]) -> std::process::Output {
    let cli_path = ensure_cli_binary_path();
    Command::new(cli_path)
        .args(args)
        .env("SPEX_STATE_PATH", &env.state_path)
        .env("SPEX_STATE_PASSPHRASE_FILE", &env.passphrase_path)
        .output()
        .expect("execute spex command")
}

/// Spawns a local bridge server on an ephemeral port and returns its base URL.
async fn spawn_bridge(base_dir: &Path, now: u64) -> String {
    let db_path = base_dir.join("bridge.db");
    let clock = Arc::new(FixedClock { now });
    let state = init_state_with_clock(db_path, clock).expect("initialize bridge state");
    let router = app(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind bridge listener");
    let addr: SocketAddr = listener.local_addr().expect("resolve bridge addr");
    tokio::spawn(async move {
        axum::serve(listener, router)
            .await
            .expect("serve bridge router");
    });
    format!("http://{addr}")
}

/// Builds signed grant and PoW values compatible with bridge inbox PUT requests.
fn build_grant_and_puzzle(
    identity: &spex_client::IdentityState,
    recipient_seed: &[u8],
    expires_at: u64,
) -> serde_json::Value {
    let grant = GrantToken {
        user_id: hex::decode(&identity.user_id_hex).expect("decode user id"),
        role: 1,
        flags: None,
        expires_at: Some(expires_at),
        extensions: Default::default(),
    };
    let signed = sign_grant(identity, &grant).expect("sign grant token");
    let params = PowParams::default();
    let nonce = pow::generate_pow_nonce(PowNonceParams::default());
    let puzzle_input = pow::build_puzzle_input(&nonce, recipient_seed);
    let puzzle_output = pow::generate_puzzle_output(recipient_seed, &puzzle_input, params)
        .expect("generate valid pow output");
    json!({
        "grant": {
            "user_id": signed.user_id,
            "role": signed.role,
            "flags": signed.flags,
            "expires_at": signed.expires_at,
            "verifying_key": signed.verifying_key,
            "signature": signed.signature
        },
        "puzzle": {
            "recipient_key": BASE64_STANDARD.encode(recipient_seed),
            "puzzle_input": BASE64_STANDARD.encode(&puzzle_input),
            "puzzle_output": BASE64_STANDARD.encode(&puzzle_output),
            "params": {
                "memory_kib": params.memory_kib,
                "iterations": params.iterations,
                "parallelism": params.parallelism,
                "output_len": params.output_len
            }
        }
    })
}

#[test]
fn test_cli_identity_create() {
    // Creates an identity and validates state file creation, expected keys, and decryptable format.
    let (_dir, env) = make_cli_state_env("identity");
    let output = run_spex(&env, &["identity", "new"]);
    assert!(
        output.status.success(),
        "identity new failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("user_id:"));
    assert!(stdout.contains("fingerprint:"));

    let contents = fs::read_to_string(&env.state_path).expect("read encrypted state file");
    let encrypted_json: serde_json::Value =
        serde_json::from_str(&contents).expect("parse encrypted state wrapper");
    assert_eq!(encrypted_json["format"], "spex_encrypted_state");
    assert_eq!(encrypted_json["version"], 1);

    std::env::set_var("SPEX_STATE_PATH", &env.state_path);
    std::env::set_var("SPEX_STATE_PASSPHRASE_FILE", &env.passphrase_path);
    let loaded = load_state().expect("load encrypted state");
    std::env::remove_var("SPEX_STATE_PATH");
    std::env::remove_var("SPEX_STATE_PASSPHRASE_FILE");

    let identity = loaded
        .identity
        .expect("identity must exist in persisted state");
    assert!(!identity.user_id_hex.is_empty());
    assert!(!identity.signing_key_hex.is_empty());
    assert!(!identity.verifying_key_hex.is_empty());
    assert!(!identity.device_id_hex.is_empty());
    assert!(!identity.device_nonce_hex.is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_cli_message_send() {
    // Sends message via PUT /inbox/:key and confirms subsequent inbox polling observes delivery.
    let (dir, env) = make_cli_state_env("message");
    let now = now_unix();
    let bridge_url = spawn_bridge(dir.path(), now).await;

    let mut state = LocalState::default();
    let sender = create_identity_in_state(&mut state);
    let recipient = create_identity();
    let thread_id = create_thread_for_state(
        &mut state,
        vec![sender.user_id_hex.clone(), recipient.user_id_hex.clone()],
    )
    .expect("create test thread");
    std::env::set_var("SPEX_STATE_PATH", &env.state_path);
    std::env::set_var("SPEX_STATE_PASSPHRASE_FILE", &env.passphrase_path);
    save_state(&state).expect("persist initial sender state");
    std::env::remove_var("SPEX_STATE_PATH");
    std::env::remove_var("SPEX_STATE_PASSPHRASE_FILE");

    let send = run_spex(
        &env,
        &[
            "msg",
            "send",
            "--thread",
            &thread_id,
            "--text",
            "hello-bridge",
            "--bridge-url",
            &bridge_url,
            "--ttl-seconds",
            "120",
        ],
    );
    assert!(
        send.status.success(),
        "message send failed: {}",
        String::from_utf8_lossy(&send.stderr)
    );

    let inbox_key = derive_inbox_scan_key(
        HashId::Sha256,
        &hex::decode(recipient.user_id_hex).expect("decode recipient id"),
    );
    let inbox_key_hex = hex::encode(&inbox_key.hashed_key);
    let poll = run_spex(
        &env,
        &[
            "inbox",
            "poll",
            "--bridge-url",
            &bridge_url,
            "--inbox-key",
            &inbox_key_hex,
        ],
    );
    assert!(
        poll.status.success(),
        "inbox poll failed: {}",
        String::from_utf8_lossy(&poll.stderr)
    );
    assert!(String::from_utf8_lossy(&poll.stdout).contains("inbox: 1 items"));

    let persisted = fs::read_to_string(&env.state_path).expect("read persisted sender state");
    let persisted_json: serde_json::Value =
        serde_json::from_str(&persisted).expect("parse persisted encrypted state");
    assert_eq!(persisted_json["format"], "spex_encrypted_state");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_cli_message_send_negative_cases() {
    // Exercises negative bridge validation paths and malformed inbox key handling.
    let (dir, _) = make_cli_state_env("negative");
    let now = now_unix();
    let bridge_url = spawn_bridge(dir.path(), now).await;
    let client = Client::new();

    let identity = create_identity();
    let recipient_seed = hex::decode(&identity.user_id_hex).expect("decode identity user id");
    let valid_parts = build_grant_and_puzzle(&identity, &recipient_seed, now + 120);
    let valid_envelope = Envelope::default()
        .to_ctap2_canonical_bytes()
        .expect("serialize canonical envelope");
    let inbox_scan_key = derive_inbox_scan_key(HashId::Sha256, &recipient_seed);
    let endpoint = format!(
        "{bridge_url}/inbox/{}",
        hex::encode(inbox_scan_key.hashed_key)
    );

    let expired_parts = build_grant_and_puzzle(&identity, &recipient_seed, now.saturating_sub(1));
    let expired_req = json!({
        "data": BASE64_STANDARD.encode(&valid_envelope),
        "grant": expired_parts["grant"],
        "puzzle": expired_parts["puzzle"],
        "ttl_seconds": 30
    });
    let expired_res = client
        .put(&endpoint)
        .json(&expired_req)
        .send()
        .await
        .expect("send expired grant request");
    assert!(
        expired_res.status() == StatusCode::UNAUTHORIZED
            || expired_res.status() == StatusCode::FORBIDDEN
    );

    let mut invalid_pow_req = json!({
        "data": BASE64_STANDARD.encode(&valid_envelope),
        "grant": valid_parts["grant"],
        "puzzle": valid_parts["puzzle"],
        "ttl_seconds": 30
    });
    invalid_pow_req["puzzle"]["puzzle_output"] = json!(BASE64_STANDARD.encode([0u8; 32]));
    let invalid_pow_res = client
        .put(&endpoint)
        .json(&invalid_pow_req)
        .send()
        .await
        .expect("send invalid pow request");
    assert!(
        invalid_pow_res.status().is_client_error(),
        "invalid PoW must be rejected with a 4xx status, got {}",
        invalid_pow_res.status()
    );

    let malformed_req = json!({
        "data": "not-base64",
        "grant": valid_parts["grant"],
        "puzzle": valid_parts["puzzle"]
    });
    let malformed_res = client
        .put(&endpoint)
        .json(&malformed_req)
        .send()
        .await
        .expect("send malformed payload request");
    assert!(
        malformed_res.status() == StatusCode::BAD_REQUEST
            || malformed_res.status() == StatusCode::FORBIDDEN
    );

    let (_key_dir, key_env) = make_cli_state_env("wrong-key");
    let wrong_key_poll = run_spex(&key_env, &["inbox", "poll", "--inbox-key", "not-hex"]);
    assert!(!wrong_key_poll.status.success());
}

#[test]
fn test_cli_revocation_and_recovery_multi_environment() {
    // Verifies revocation/recovery flow export-import across two state environments.
    let (_dir_a, env_a) = make_cli_state_env("rev-a");
    let (_dir_b, env_b) = make_cli_state_env("rev-b");

    let create_a = run_spex(&env_a, &["identity", "new"]);
    assert!(create_a.status.success(), "identity a failed");

    let checkpoint = run_spex(&env_a, &["log", "append-checkpoint"]);
    assert!(checkpoint.status.success(), "append checkpoint failed");

    let recovery = run_spex(&env_a, &["log", "create-recovery-key"]);
    assert!(recovery.status.success(), "create recovery failed");
    let recovery_stdout = String::from_utf8_lossy(&recovery.stdout);
    let recovery_hex = recovery_stdout
        .lines()
        .find_map(|line| line.strip_prefix("recovery key: "))
        .expect("extract recovery key")
        .to_string();

    std::env::set_var("SPEX_STATE_PATH", &env_a.state_path);
    std::env::set_var("SPEX_STATE_PASSPHRASE_FILE", &env_a.passphrase_path);
    let state_a = load_state().expect("load env a state");
    std::env::remove_var("SPEX_STATE_PATH");
    std::env::remove_var("SPEX_STATE_PASSPHRASE_FILE");
    let verify_key = state_a.identity.expect("identity").verifying_key_hex;

    let revoke = run_spex(
        &env_a,
        &[
            "log",
            "revoke-key",
            "--key-hex",
            &verify_key,
            "--recovery-hex",
            &recovery_hex,
            "--reason",
            "operator-test",
        ],
    );
    assert!(revoke.status.success(), "revoke with recovery failed");

    let export_path = tempfile::NamedTempFile::new().expect("temp export file");
    let export = run_spex(
        &env_a,
        &[
            "log",
            "export",
            "--path",
            export_path.path().to_str().expect("export path"),
        ],
    );
    assert!(export.status.success(), "export failed");

    let create_b = run_spex(&env_b, &["identity", "new"]);
    assert!(create_b.status.success(), "identity b failed");
    let import = run_spex(
        &env_b,
        &[
            "log",
            "import",
            "--path",
            export_path.path().to_str().expect("import path"),
        ],
    );
    assert!(import.status.success(), "import failed");
}

#[test]
fn test_cli_revocation_negative_cases() {
    // Covers no-permission/missing identity, nonexistent key, and invalid recovery key handling.
    let (_dir, env) = make_cli_state_env("rev-neg");

    let without_identity = run_spex(
        &env,
        &["log", "revoke-key", "--key-hex", &hex::encode([1u8; 32])],
    );
    assert!(!without_identity.status.success());

    let create = run_spex(&env, &["identity", "new"]);
    assert!(create.status.success(), "identity create failed");

    let checkpoint = run_spex(&env, &["log", "append-checkpoint"]);
    assert!(checkpoint.status.success(), "append checkpoint failed");

    let missing_key = run_spex(
        &env,
        &["log", "revoke-key", "--key-hex", &hex::encode([7u8; 32])],
    );
    assert!(!missing_key.status.success());

    std::env::set_var("SPEX_STATE_PATH", &env.state_path);
    std::env::set_var("SPEX_STATE_PASSPHRASE_FILE", &env.passphrase_path);
    let state = load_state().expect("load state");
    std::env::remove_var("SPEX_STATE_PATH");
    std::env::remove_var("SPEX_STATE_PASSPHRASE_FILE");
    let verifying_key = state.identity.expect("identity").verifying_key_hex;

    let invalid_recovery = run_spex(
        &env,
        &[
            "log",
            "revoke-key",
            "--key-hex",
            &verifying_key,
            "--recovery-hex",
            &hex::encode([9u8; 32]),
        ],
    );
    assert!(!invalid_recovery.status.success());
}
