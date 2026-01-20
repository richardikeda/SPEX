use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine};
use clap::{Parser, Subcommand};
use spex_client::{
    accept_request_payload,
    append_rotation_checkpoint,
    create_checkpoint_entry,
    create_contact_card_payload,
    create_identity,
    create_recovery_entry,
    create_request_payload,
    create_revocation_entry,
    create_thread_state,
    fingerprint_hex,
    load_checkpoint_log,
    load_state,
    log_consistency,
    now_unix,
    receive_inbox_messages,
    receive_transport_messages,
    redeem_contact_card_payload,
    rotate_identity,
    save_checkpoint_log,
    save_state,
    publish_thread_message_transport,
    stage_transport_delivery,
    transport_inbox_has_items,
    ClientError,
    ContactState,
    GrantState,
    InboxItem,
    MessageState,
    RequestState,
};
use spex_core::log::{CheckpointEntry, CheckpointLog, LogConsistency};
use spex_transport::inbox::BridgeClient;
use std::fs;

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
    },
}

#[derive(Subcommand)]
enum InboxCommand {
    Poll {
        #[arg(long)]
        bridge_url: Option<String>,
        #[arg(long)]
        inbox_key: Option<String>,
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

/// Runs the CLI entry point and dispatches commands.
#[tokio::main]
async fn main() -> Result<(), ClientError> {
    let cli = Cli::parse();
    let mut state = load_state()?;

    match cli.command {
        Commands::Identity { command } => match command {
            IdentityCommand::New => {
                let identity = create_identity();
                let fingerprint = fingerprint_hex(&hex::decode(&identity.verifying_key_hex)?);
                println!("user_id: {}", identity.user_id_hex);
                println!("fingerprint: {}", fingerprint);
                state.identity = Some(identity);
            }
            IdentityCommand::Rotate => {
                let identity = state.identity.as_mut().ok_or(ClientError::MissingIdentity)?;
                let old_verifying_key_hex = identity.verifying_key_hex.clone();
                let rotated = rotate_identity(identity);
                println!("new user_id: {}", rotated.user_id_hex);
                println!(
                    "new fingerprint: {}",
                    fingerprint_hex(&hex::decode(&rotated.verifying_key_hex)?)
                );
                let rotation = spex_client::KeyRotationState {
                    rotated_at: now_unix(),
                    old_verifying_key_hex,
                    new_verifying_key_hex: rotated.verifying_key_hex.clone(),
                };
                state.key_rotations.push(rotation);
                append_rotation_checkpoint(&mut state, rotated, &old_verifying_key_hex)?;
            }
        },
        Commands::Card { command } => match command {
            CardCommand::Create => {
                let identity = state.identity.as_ref().ok_or(ClientError::MissingIdentity)?;
                let payload = create_contact_card_payload(identity)?;
                println!("card: {}", payload);
            }
            CardCommand::Redeem { card } => {
                let card = redeem_contact_card_payload(&card)?;
                let user_id_hex = hex::encode(&card.user_id);
                let verifying_hex = hex::encode(&card.verifying_key);
                let fingerprint = fingerprint_hex(&card.verifying_key);
                if let Some(existing) = state.contacts.get(&user_id_hex) {
                    if existing.verifying_key_hex != verifying_hex {
                        println!(
                            "ALERT: key change detected for {} (old {}, new {})",
                            user_id_hex, existing.fingerprint, fingerprint
                        );
                    }
                }
                state.contacts.insert(
                    user_id_hex.clone(),
                    ContactState {
                        user_id_hex: user_id_hex.clone(),
                        verifying_key_hex: verifying_hex,
                        fingerprint: fingerprint.clone(),
                        device_id_hex: hex::encode(&card.device_id),
                        last_seen_at: now_unix(),
                    },
                );
                println!("contact saved: {}", user_id_hex);
                println!("fingerprint: {}", fingerprint);
            }
        },
        Commands::Request { command } => match command {
            RequestCommand::Send { to, role } => {
                let identity = state.identity.as_ref().ok_or(ClientError::MissingIdentity)?;
                let (request, token) = create_request_payload(identity, &to, role)?;
                state.requests.push(RequestState {
                    token_base64: token.clone(),
                    to_user_id: to,
                    role,
                    created_at: request.created_at,
                    puzzle: request.puzzle,
                });
                println!("request: {}", token);
            }
        },
        Commands::Grant { command } => match command {
            GrantCommand::Accept { request } => {
                let identity = state.identity.as_ref().ok_or(ClientError::MissingIdentity)?;
                let (request_token, signed_grant) = accept_request_payload(identity, &request)?;
                let grant_token = BASE64_STANDARD.encode(serde_json::to_vec(&signed_grant)?);
                state.grants.push(GrantState {
                    token_base64: grant_token.clone(),
                    from_user_id: request_token.from_user_id,
                    role: request_token.role,
                    created_at: now_unix(),
                    signature_base64: Some(signed_grant.signature.clone()),
                });
                println!("grant: {}", grant_token);
            }
            GrantCommand::Deny { request } => {
                let request_token = spex_client::parse_request_token(&request)?;
                println!(
                    "request denied: {} (role {})",
                    request_token.from_user_id, request_token.role
                );
            }
        },
        Commands::Thread { command } => match command {
            ThreadCommand::New { members } => {
                let identity = state.identity.as_ref().ok_or(ClientError::MissingIdentity)?;
                let thread = create_thread_state(identity, members)?;
                let thread_id = thread.thread_id_hex.clone();
                state.threads.insert(thread_id.clone(), thread);
                println!("thread: {}", thread_id);
            }
        },
        Commands::Msg { command } => match command {
            MsgCommand::Send { thread, text } => {
                let identity = state.identity.as_ref().ok_or(ClientError::MissingIdentity)?;
                let thread_state = state
                    .threads
                    .get_mut(&thread)
                    .ok_or(ClientError::ThreadNotFound)?;
                let (_envelope, manifest, chunks, outbox_item) =
                    publish_thread_message_transport(identity, thread_state, text.as_bytes())?;
                let chunk_count = chunks.len();
                let message = MessageState {
                    sender_user_id: identity.user_id_hex.clone(),
                    text: text.clone(),
                    sent_at: now_unix(),
                };
                thread_state.messages.push(message);
                state.transport_outbox.push(outbox_item);
                stage_transport_delivery(
                    &mut state,
                    thread_state,
                    &identity.user_id_hex,
                    &manifest,
                    &chunks,
                )?;
                println!(
                    "message published via transport ({} chunks)",
                    chunk_count
                );
            }
        },
        Commands::Inbox { command } => match command {
            InboxCommand::Poll {
                bridge_url,
                inbox_key,
            } => {
                if let Some(inbox_key) = inbox_key {
                    let inbox_key_bytes = hex::decode(inbox_key)?;
                    let response = if transport_inbox_has_items(&state, &inbox_key_bytes) {
                        receive_transport_messages(&mut state, &inbox_key_bytes)?
                    } else {
                        let bridge = bridge_url.map(BridgeClient::new);
                        receive_inbox_messages(&mut state, &inbox_key_bytes, bridge.as_ref()).await?
                    };
                    for item in response.items {
                        let message_text = String::from_utf8(item.plaintext)
                            .map_err(|_| ClientError::InvalidMessageEncoding)?;
                        let thread_state = state
                            .threads
                            .get_mut(&item.thread_id_hex)
                            .ok_or(ClientError::ThreadNotFound)?;
                        thread_state.messages.push(MessageState {
                            sender_user_id: item.sender_user_id_hex.clone(),
                            text: message_text.clone(),
                            sent_at: now_unix(),
                        });
                        println!(
                            "message: {} -> {}",
                            item.sender_user_id_hex, message_text
                        );
                    }
                    println!(
                        "inbox: {} items (source: {:?})",
                        response.items.len(),
                        response.source
                    );
                } else {
                    println!("local inbox: {} items", state.inbox.len());
                    for item in &state.inbox {
                        println!("- {}", item.payload_base64);
                    }
                    state.inbox.clear();
                }
            }
        },
        Commands::Log { command } => match command {
            LogCommand::AppendCheckpoint => {
                let identity = state.identity.as_ref().ok_or(ClientError::MissingIdentity)?;
                let entry = create_checkpoint_entry(identity)?;
                let mut log = load_checkpoint_log(&state)?;
                log.append(CheckpointEntry::Key(entry))
                    .map_err(|err| ClientError::Log(err.to_string()))?;
                save_checkpoint_log(&mut state, &log)?;
                println!("checkpoint appended (len: {})", log.entries.len());
            }
            LogCommand::CreateRecoveryKey => {
                let identity = state.identity.as_ref().ok_or(ClientError::MissingIdentity)?;
                let entry = create_recovery_entry(identity)?;
                let mut log = load_checkpoint_log(&state)?;
                log.append(CheckpointEntry::Recovery(entry.clone()))
                    .map_err(|err| ClientError::Log(err.to_string()))?;
                save_checkpoint_log(&mut state, &log)?;
                println!("recovery key: {}", hex::encode(entry.recovery_key));
                println!("checkpoint appended (len: {})", log.entries.len());
            }
            LogCommand::RevokeKey {
                key_hex,
                recovery_hex,
                reason,
            } => {
                let identity = state.identity.as_ref().ok_or(ClientError::MissingIdentity)?;
                let entry = create_revocation_entry(identity, &key_hex, recovery_hex, reason)?;
                let mut log = load_checkpoint_log(&state)?;
                log.append(CheckpointEntry::Revocation(entry))
                    .map_err(|err| ClientError::Log(err.to_string()))?;
                save_checkpoint_log(&mut state, &log)?;
                println!("revocation appended (len: {})", log.entries.len());
            }
            LogCommand::Export { path } => {
                let log = load_checkpoint_log(&state)?;
                let encoded = log
                    .export_base64()
                    .map_err(|err| ClientError::Log(err.to_string()))?;
                fs::write(path, encoded)?;
                println!("log exported");
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
