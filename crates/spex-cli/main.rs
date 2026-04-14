// SPDX-License-Identifier: MPL-2.0
use clap::{Parser, Subcommand};
use libp2p::identity::Keypair;
use libp2p::Multiaddr;
use spex_bridge::{export_abuse_logs, AbuseLogFilter};
use spex_client::{
    accept_request_for_state, create_checkpoint_entry, create_contact_card_payload,
    create_identity_in_state, create_recovery_entry, create_request_for_state,
    create_revocation_entry, create_thread_for_state, fingerprint_hex, ingest_inbox_payloads,
    load_checkpoint_log, load_state, log_consistency, publish_via_bridge, receive_inbox_messages,
    receive_transport_messages, record_inbox_messages, redeem_contact_card_to_state,
    rotate_identity_in_state, save_checkpoint_log, save_state, send_thread_message_for_state,
    transport_inbox_has_items, ClientError, ClientFailureReason,
};
use spex_core::hash::{hash_bytes, HashId};
use spex_core::log::{CheckpointEntry, CheckpointLog, LogConsistency};
use spex_transport::{
    inbox::{BridgeClient, InboxScanResponse, InboxSource},
    P2pNodeConfig, P2pTransport, TransportConfig,
};
use std::fs;
use std::path::PathBuf;
use std::time::Duration;

#[derive(Parser)]
#[command(name = "spex", version, about = "SPEX CLI")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Identity {
        #[command(subcommand)]
        command: IdentityCommand,
    },
    Card {
        #[command(subcommand)]
        command: CardCommand,
    },
    Request {
        #[command(subcommand)]
        command: RequestCommand,
    },
    Grant {
        #[command(subcommand)]
        command: GrantCommand,
    },
    Thread {
        #[command(subcommand)]
        command: ThreadCommand,
    },
    Msg {
        #[command(subcommand)]
        command: MsgCommand,
    },
    Inbox {
        #[command(subcommand)]
        command: InboxCommand,
    },
    Log {
        #[command(subcommand)]
        command: LogCommand,
    },
}

#[derive(Subcommand)]
enum IdentityCommand {
    New,
    Rotate,
}

#[derive(Subcommand)]
enum CardCommand {
    Create,
    Redeem {
        #[arg(long)]
        card: String,
    },
}

#[derive(Subcommand)]
enum RequestCommand {
    Send {
        #[arg(long)]
        to: String,
        #[arg(long, default_value_t = 1)]
        role: u64,
    },
}

#[derive(Subcommand)]
enum GrantCommand {
    Accept {
        #[arg(long)]
        request: String,
    },
    Deny {
        #[arg(long)]
        request: String,
    },
}

#[derive(Subcommand)]
enum ThreadCommand {
    New {
        #[arg(long, value_delimiter = ',')]
        members: Vec<String>,
    },
}

#[derive(Subcommand)]
enum MsgCommand {
    Send {
        #[arg(long)]
        thread: String,
        #[arg(long)]
        text: String,
        #[arg(long)]
        bridge_url: Option<String>,
        #[arg(long)]
        ttl_seconds: Option<u64>,
        #[arg(long)]
        p2p: bool,
        #[arg(long, value_delimiter = ',')]
        peer: Vec<String>,
        #[arg(long, value_delimiter = ',')]
        bootstrap: Vec<String>,
        #[arg(long, value_delimiter = ',')]
        listen_addr: Vec<String>,
        #[arg(long, default_value_t = 2)]
        p2p_wait_secs: u64,
    },
}

#[derive(Subcommand)]
enum InboxCommand {
    Poll {
        #[arg(long)]
        bridge_url: Option<String>,
        #[arg(long)]
        inbox_key: Option<String>,
        #[arg(long)]
        p2p: bool,
        #[arg(long, value_delimiter = ',')]
        peer: Vec<String>,
        #[arg(long, value_delimiter = ',')]
        bootstrap: Vec<String>,
        #[arg(long, value_delimiter = ',')]
        listen_addr: Vec<String>,
        #[arg(long, default_value_t = 5)]
        p2p_wait_secs: u64,
    },
}

#[derive(Subcommand)]
enum LogCommand {
    AppendCheckpoint,
    CreateRecoveryKey,
    RevokeKey {
        #[arg(long)]
        key_hex: String,
        #[arg(long)]
        recovery_hex: Option<String>,
        #[arg(long)]
        reason: Option<String>,
    },
    Export {
        #[arg(long)]
        path: String,
    },
    ExportAbuse {
        #[arg(long)]
        db_path: String,
        #[arg(long)]
        path: String,
        #[arg(long)]
        identity: Option<String>,
        #[arg(long)]
        request_kind: Option<String>,
        #[arg(long)]
        outcome: Option<String>,
        #[arg(long)]
        since: Option<u64>,
        #[arg(long)]
        until: Option<u64>,
        #[arg(long)]
        limit: Option<usize>,
    },
    Import {
        #[arg(long)]
        path: String,
    },
    GossipVerify {
        #[arg(long)]
        path: String,
    },
    Info,
}

/// Parses a list of multiaddr strings into libp2p multiaddr values.
fn parse_multiaddrs(values: &[String]) -> Result<Vec<Multiaddr>, ClientError> {
    values
        .iter()
        .map(|value| {
            value
                .parse()
                .map_err(|err| ClientError::Transport(format!("invalid multiaddr: {err}")))
        })
        .collect()
}

/// Builds a P2P node configuration from CLI flags and default values.
fn build_p2p_config(
    listen_addrs: Vec<Multiaddr>,
    peers: Vec<Multiaddr>,
    bootstrap_nodes: Vec<Multiaddr>,
    publish_wait: Duration,
    manifest_wait: Duration,
    query_timeout: Duration,
) -> P2pNodeConfig {
    let mut config = P2pNodeConfig::default();
    if !listen_addrs.is_empty() {
        config.listen_addrs = listen_addrs;
    }
    config.peers = peers;
    config.bootstrap_nodes = bootstrap_nodes;
    config.publish_wait = publish_wait;
    config.manifest_wait = manifest_wait;
    config.query_timeout = query_timeout;
    config
}

/// Determines whether P2P networking should be enabled for a CLI command.
fn should_use_p2p(
    enabled: bool,
    peers: &[String],
    bootstrap: &[String],
    listen: &[String],
) -> bool {
    enabled || !peers.is_empty() || !bootstrap.is_empty() || !listen.is_empty()
}

/// Builds a P2P transport node from CLI flags and shared timeout settings.
async fn build_p2p_transport(
    listen_addr: &[String],
    peer: &[String],
    bootstrap: &[String],
    wait_secs: u64,
) -> Result<P2pTransport, ClientError> {
    let wait_duration = Duration::from_secs(wait_secs);
    let listen_addrs = parse_multiaddrs(listen_addr)?;
    let peers = parse_multiaddrs(peer)?;
    let bootstrap_nodes = parse_multiaddrs(bootstrap)?;
    let node_config = build_p2p_config(
        listen_addrs,
        peers,
        bootstrap_nodes,
        wait_duration,
        wait_duration,
        wait_duration,
    );
    let transport = P2pTransport::new(
        Keypair::generate_ed25519(),
        TransportConfig::default(),
        node_config,
    )
    .await
    .map_err(|err| ClientError::Transport(err.to_string()))?;
    Ok(transport)
}

/// Recovers inbox payloads via P2P manifests, falling back to the HTTP bridge if needed.
async fn recover_payloads_with_fallback(
    transport: &mut P2pTransport,
    inbox_key: &[u8],
    wait: Duration,
    bridge: Option<&BridgeClient>,
) -> Result<InboxScanResponse, ClientError> {
    let payloads = match transport.recover_payloads_for_inbox(inbox_key, wait).await {
        Ok(payloads) => payloads,
        Err(err) => {
            if let Some(bridge_client) = bridge {
                let response = bridge_client
                    .scan_inbox(inbox_key)
                    .await
                    .map_err(|err| ClientError::Transport(err.to_string()))?;
                return Ok(response);
            }
            return Err(ClientError::Transport(err.to_string()));
        }
    };
    if !payloads.is_empty() {
        return Ok(InboxScanResponse {
            items: payloads,
            source: InboxSource::Kademlia,
        });
    }
    if let Some(bridge_client) = bridge {
        let response = bridge_client
            .scan_inbox(inbox_key)
            .await
            .map_err(|err| ClientError::Transport(err.to_string()))?;
        return Ok(response);
    }
    Ok(InboxScanResponse {
        items: Vec::new(),
        source: InboxSource::Kademlia,
    })
}

/// Exports abuse audit records from a bridge SQLite database as JSON lines.
fn export_abuse_log_file(
    path: &str,
    records: &[spex_bridge::AbuseLogRecord],
) -> Result<(), ClientError> {
    let mut lines = Vec::with_capacity(records.len());
    for record in records {
        let encoded = serde_json::to_string(record)?;
        lines.push(encoded);
    }
    fs::write(path, lines.join("\n"))?;
    Ok(())
}

/// Returns a short SHA-256 digest prefix for sensitive output summaries.
fn digest_prefix(value: &str) -> String {
    let digest = hash_bytes(HashId::Sha256, value.as_bytes());
    let hex_digest = hex::encode(digest);
    hex_digest[..16].to_string()
}

/// Runs the CLI entry point and dispatches commands.
#[tokio::main]
async fn main() {
    if let Err(err) = run_cli().await {
        let reason = ClientFailureReason::from(&err);
        eprintln!("Error: {}", reason);
        eprintln!("Details: {}", err);
        std::process::exit(1);
    }
}

async fn run_cli() -> Result<(), ClientError> {
    let cli = Cli::parse();
    let mut state = load_state()?;

    match cli.command {
        Commands::Identity { command } => match command {
            IdentityCommand::New => {
                let identity = create_identity_in_state(&mut state);
                let fingerprint = fingerprint_hex(&hex::decode(&identity.verifying_key_hex)?);
                println!("identity created");
                println!("user_id digest: {}", digest_prefix(&identity.user_id_hex));
                println!("fingerprint: {}", fingerprint);
            }
            IdentityCommand::Rotate => {
                let rotation = rotate_identity_in_state(&mut state)?;
                println!("identity rotated");
                println!(
                    "new user_id digest: {}",
                    digest_prefix(&rotation.new_identity.user_id_hex)
                );
                println!(
                    "new fingerprint: {}",
                    fingerprint_hex(&hex::decode(&rotation.new_identity.verifying_key_hex)?)
                );
            }
        },
        Commands::Card { command } => match command {
            CardCommand::Create => {
                let identity = state
                    .identity
                    .as_ref()
                    .ok_or(ClientError::MissingIdentity)?;
                let payload = create_contact_card_payload(identity)?;
                println!(
                    "card generated (base64_len: {}, sha256: {})",
                    payload.len(),
                    digest_prefix(&payload)
                );
            }
            CardCommand::Redeem { card } => {
                let outcome = redeem_contact_card_to_state(&mut state, &card)?;
                if let Some(reason) = outcome.failure_reason() {
                    if let Some(previous) = outcome.previous_fingerprint.as_ref() {
                        println!(
                            "ALERT: {} (old {}, new {})",
                            reason, previous, outcome.contact.fingerprint
                        );
                    }
                }
                println!(
                    "contact saved digest: {}",
                    digest_prefix(&outcome.contact.user_id_hex)
                );
                println!("fingerprint: {}", outcome.contact.fingerprint);
            }
        },
        Commands::Request { command } => match command {
            RequestCommand::Send { to, role } => {
                let outcome = create_request_for_state(&mut state, &to, role)?;
                println!(
                    "request generated (base64_len: {}, sha256: {})",
                    outcome.token_base64.len(),
                    digest_prefix(&outcome.token_base64)
                );
            }
        },
        Commands::Grant { command } => match command {
            GrantCommand::Accept { request } => {
                let outcome = accept_request_for_state(&mut state, &request)?;
                println!(
                    "grant generated (base64_len: {}, sha256: {})",
                    outcome.grant_token_base64.len(),
                    digest_prefix(&outcome.grant_token_base64)
                );
            }
            GrantCommand::Deny { request } => {
                let request_token = spex_client::parse_request_token(&request)?;
                println!("request denied (role {})", request_token.role);
            }
        },
        Commands::Thread { command } => match command {
            ThreadCommand::New { members } => {
                let thread_id = create_thread_for_state(&mut state, members)?;
                println!("thread: {}", thread_id);
            }
        },
        Commands::Msg { command } => match command {
            MsgCommand::Send {
                thread,
                text,
                bridge_url,
                ttl_seconds,
                p2p,
                peer,
                bootstrap,
                listen_addr,
                p2p_wait_secs,
            } => {
                let sender_identity = state.identity.clone().ok_or(ClientError::MissingIdentity)?;
                let dispatch = send_thread_message_for_state(&mut state, &thread, &text)?;
                if let Some(url) = bridge_url {
                    let bridge = BridgeClient::new(url);
                    for recipient_inbox_key in &dispatch.recipient_inbox_keys {
                        publish_via_bridge(
                            &sender_identity,
                            recipient_inbox_key,
                            &dispatch.envelope,
                            &bridge,
                            ttl_seconds,
                        )
                        .await?;
                    }
                    println!(
                        "bridge: published envelope to {} inboxes",
                        dispatch.recipient_inbox_keys.len()
                    );
                }
                if should_use_p2p(p2p, &peer, &bootstrap, &listen_addr) {
                    let mut transport =
                        build_p2p_transport(&listen_addr, &peer, &bootstrap, p2p_wait_secs).await?;
                    transport
                        .publish_to_inboxes(
                            &dispatch.recipient_inbox_keys,
                            &dispatch.manifest,
                            &dispatch.chunks,
                        )
                        .await
                        .map_err(|err| ClientError::Transport(err.to_string()))?;
                    println!(
                        "p2p: published manifest to {} inboxes",
                        dispatch.recipient_inbox_keys.len()
                    );
                }
                println!(
                    "message published via transport ({} chunks)",
                    dispatch.chunk_count
                );
            }
        },
        Commands::Inbox { command } => match command {
            InboxCommand::Poll {
                bridge_url,
                inbox_key,
                p2p,
                peer,
                bootstrap,
                listen_addr,
                p2p_wait_secs,
            } => {
                if let Some(inbox_key) = inbox_key {
                    let inbox_key_bytes = hex::decode(inbox_key)?;
                    if should_use_p2p(p2p, &peer, &bootstrap, &listen_addr) {
                        let mut transport =
                            build_p2p_transport(&listen_addr, &peer, &bootstrap, p2p_wait_secs)
                                .await?;
                        let bridge = bridge_url.as_ref().map(BridgeClient::new);
                        let response = recover_payloads_with_fallback(
                            &mut transport,
                            &inbox_key_bytes,
                            Duration::from_secs(p2p_wait_secs),
                            bridge.as_ref(),
                        )
                        .await?;
                        let receipts = ingest_inbox_payloads(&mut state, response.items)?;
                        for receipt in &receipts {
                            println!(
                                "message: sender_digest={} chars={}",
                                digest_prefix(&receipt.sender_user_id_hex),
                                receipt.text.chars().count()
                            );
                        }
                        println!(
                            "inbox: {} items (source: {:?})",
                            receipts.len(),
                            response.source
                        );
                    } else {
                        let response = if transport_inbox_has_items(&state, &inbox_key_bytes) {
                            receive_transport_messages(&mut state, &inbox_key_bytes)?
                        } else {
                            let bridge = bridge_url.map(BridgeClient::new);
                            receive_inbox_messages(&mut state, &inbox_key_bytes, bridge.as_ref())
                                .await?
                        };
                        let receipts = record_inbox_messages(&mut state, response.items)?;
                        for receipt in &receipts {
                            println!(
                                "message: sender_digest={} chars={}",
                                digest_prefix(&receipt.sender_user_id_hex),
                                receipt.text.chars().count()
                            );
                        }
                        println!(
                            "inbox: {} items (source: {:?})",
                            receipts.len(),
                            response.source
                        );
                    }
                } else {
                    println!("local inbox: {} items", state.inbox.len());
                    state.inbox.clear();
                }
            }
        },
        Commands::Log { command } => match command {
            LogCommand::AppendCheckpoint => {
                let identity = state
                    .identity
                    .as_ref()
                    .ok_or(ClientError::MissingIdentity)?;
                let entry = create_checkpoint_entry(identity)?;
                let mut log = load_checkpoint_log(&state)?;
                log.append(CheckpointEntry::Key(entry))
                    .map_err(|err| ClientError::Log(err.to_string()))?;
                save_checkpoint_log(&mut state, &log)?;
                println!("checkpoint appended (len: {})", log.entries.len());
            }
            LogCommand::CreateRecoveryKey => {
                let identity = state
                    .identity
                    .as_ref()
                    .ok_or(ClientError::MissingIdentity)?;
                let entry = create_recovery_entry(identity)?;
                let mut log = load_checkpoint_log(&state)?;
                log.append(CheckpointEntry::Recovery(entry.clone()))
                    .map_err(|err| ClientError::Log(err.to_string()))?;
                save_checkpoint_log(&mut state, &log)?;
                let recovery_hex = hex::encode(entry.recovery_key);
                println!(
                    "recovery key generated (hex_len: {}, sha256: {})",
                    recovery_hex.len(),
                    digest_prefix(&recovery_hex)
                );
                println!("checkpoint appended (len: {})", log.entries.len());
            }
            LogCommand::RevokeKey {
                key_hex,
                recovery_hex,
                reason,
            } => {
                let identity = state
                    .identity
                    .as_ref()
                    .ok_or(ClientError::MissingIdentity)?;
                let mut log = load_checkpoint_log(&state)?;
                let entry = create_revocation_entry(identity, &key_hex, recovery_hex, reason)?;
                let changed = spex_client::append_revocation_entry_checked(
                    &mut log,
                    identity,
                    entry,
                    60 * 60 * 24 * 365,
                )?;
                if changed {
                    save_checkpoint_log(&mut state, &log)?;
                    println!("revocation appended (len: {})", log.entries.len());
                } else {
                    println!("revocation already present (len: {})", log.entries.len());
                }
            }
            LogCommand::Export { path } => {
                let log = load_checkpoint_log(&state)?;
                let encoded = log
                    .export_base64()
                    .map_err(|err| ClientError::Log(err.to_string()))?;
                fs::write(path, encoded)?;
                println!("log exported");
            }
            LogCommand::ExportAbuse {
                db_path,
                path,
                identity,
                request_kind,
                outcome,
                since,
                until,
                limit,
            } => {
                let filter = AbuseLogFilter {
                    identity,
                    request_kind,
                    outcome,
                    since_timestamp: since,
                    until_timestamp: until,
                    limit,
                };
                let records = export_abuse_logs(&PathBuf::from(db_path), &filter)
                    .map_err(|err| ClientError::Log(err.to_string()))?;
                export_abuse_log_file(&path, &records)?;
                println!("abuse log exported (records: {})", records.len());
            }
            LogCommand::Import { path } => {
                let encoded = fs::read_to_string(path)?;
                let log = CheckpointLog::import_base64(encoded.trim())
                    .map_err(|err| ClientError::Log(err.to_string()))?;
                save_checkpoint_log(&mut state, &log)?;
                println!("log imported (len: {})", log.entries.len());
            }
            LogCommand::GossipVerify { path } => {
                let encoded = fs::read_to_string(path)?;
                let remote = CheckpointLog::import_base64(encoded.trim())
                    .map_err(|err| ClientError::Log(err.to_string()))?;
                let local = load_checkpoint_log(&state)?;
                let consistency = log_consistency(&local, &remote);
                let local_root = local
                    .merkle_root()
                    .map_err(|err| ClientError::Log(err.to_string()))?;
                let remote_root = remote
                    .merkle_root()
                    .map_err(|err| ClientError::Log(err.to_string()))?;
                println!("local entries: {}", local.entries.len());
                println!("remote entries: {}", remote.entries.len());
                println!("local root: {}", hex::encode(local_root));
                println!("remote root: {}", hex::encode(remote_root));
                println!(
                    "consistency: {}",
                    match consistency {
                        LogConsistency::Identical => "identical",
                        LogConsistency::LocalBehind => "local-behind",
                        LogConsistency::LocalAhead => "local-ahead",
                        LogConsistency::Diverged => "diverged",
                    }
                );
            }
            LogCommand::Info => {
                let log = load_checkpoint_log(&state)?;
                let root = log
                    .merkle_root()
                    .map_err(|err| ClientError::Log(err.to_string()))?;
                println!("entries: {}", log.entries.len());
                println!("root: {}", hex::encode(root));
            }
        },
    }

    save_state(&state)?;
    Ok(())
}
