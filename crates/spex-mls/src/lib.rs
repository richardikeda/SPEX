use std::fmt;

use spex_core::{
    error::SpexError,
    hash::{hash_bytes, hash_ctap2_cbor_value, HashId},
    mls_ext,
    types::{ProtoSuite, ThreadConfig},
};

pub use mls_rs;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GroupContext {
    proto_suite: ProtoSuite,
    flags: u8,
    cfg_hash_id: u16,
    cfg_hash: Vec<u8>,
    extensions: Vec<Vec<u8>>,
}

impl GroupContext {
    /// Returns the protocol suite for the group context.
    pub fn proto_suite(&self) -> ProtoSuite {
        self.proto_suite
    }

    /// Returns the current MLS flags for the group context.
    pub fn flags(&self) -> u8 {
        self.flags
    }

    /// Returns the configured hash identifier for cfg_hash.
    pub fn cfg_hash_id(&self) -> u16 {
        self.cfg_hash_id
    }

    /// Returns the configuration hash for the group context.
    pub fn cfg_hash(&self) -> &[u8] {
        &self.cfg_hash
    }

    /// Returns the MLS extensions stored in the group context.
    pub fn extensions(&self) -> &[Vec<u8>] {
        &self.extensions
    }

    /// Rebuilds the SPEX MLS extensions from the current context fields.
    fn rebuild_extensions(&mut self) {
        self.extensions = mls_extensions(
            self.proto_suite,
            self.flags,
            self.cfg_hash_id,
            &self.cfg_hash,
        );
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GroupConfig {
    pub proto_suite: ProtoSuite,
    pub flags: u8,
    pub cfg_hash_id: u16,
    pub cfg_hash: Vec<u8>,
    pub initial_secret: Option<Vec<u8>>,
}

impl GroupConfig {
    /// Creates a new group configuration for initializing a group context.
    pub fn new(proto_suite: ProtoSuite, flags: u8, cfg_hash_id: u16, cfg_hash: Vec<u8>) -> Self {
        Self {
            proto_suite,
            flags,
            cfg_hash_id,
            cfg_hash,
            initial_secret: None,
        }
    }

    /// Adds an explicit initial secret to the group configuration.
    pub fn with_initial_secret(mut self, initial_secret: Vec<u8>) -> Self {
        self.initial_secret = Some(initial_secret);
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GroupSecrets {
    epoch: u64,
    group_secret: Vec<u8>,
}

impl GroupSecrets {
    /// Builds a new group secret container for the provided epoch.
    pub fn new(epoch: u64, group_secret: Vec<u8>) -> Self {
        Self { epoch, group_secret }
    }

    /// Returns the epoch associated with the group secret.
    pub fn epoch(&self) -> u64 {
        self.epoch
    }

    /// Returns the current group secret bytes.
    pub fn group_secret(&self) -> &[u8] {
        &self.group_secret
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MemberSecret {
    member_id: String,
    secret: Vec<u8>,
}

impl MemberSecret {
    /// Returns the member identifier tied to this secret.
    pub fn member_id(&self) -> &str {
        &self.member_id
    }

    /// Returns the derived secret bytes for this member.
    pub fn secret(&self) -> &[u8] {
        &self.secret
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Group {
    epoch: u64,
    context: GroupContext,
    members: Vec<String>,
    secrets: GroupSecrets,
    hash_id: HashId,
}

impl Group {
    /// Creates a new MLS group from the provided configuration.
    pub fn create(config: GroupConfig) -> Result<Self, MlsError> {
        let hash_id = hash_id_from_u16(config.cfg_hash_id)?;
        let mut context = GroupContext {
            proto_suite: config.proto_suite,
            flags: config.flags,
            cfg_hash_id: config.cfg_hash_id,
            cfg_hash: config.cfg_hash,
            extensions: Vec::new(),
        };
        context.rebuild_extensions();

        let seed = config
            .initial_secret
            .unwrap_or_else(|| derive_initial_secret_seed(hash_id, &context));
        let group_secret = derive_group_secret(hash_id, &seed, 0);
        let secrets = GroupSecrets::new(0, group_secret);

        Ok(Self {
            epoch: 0,
            context,
            members: Vec::new(),
            secrets,
            hash_id,
        })
    }

    /// Applies a commit to the group, updating the context, secrets, and epoch.
    pub fn apply_commit(&mut self, commit: Commit) -> Result<&GroupContext, MlsError> {
        if let Some(proto_suite) = commit.proto_suite {
            self.context.proto_suite = proto_suite;
        }
        if let Some(flags) = commit.flags {
            self.context.flags = flags;
        }
        if let Some(cfg_hash_id) = commit.cfg_hash_id {
            self.hash_id = hash_id_from_u16(cfg_hash_id)?;
            self.context.cfg_hash_id = cfg_hash_id;
        }
        if let Some(cfg_hash) = commit.cfg_hash {
            self.context.cfg_hash = cfg_hash;
        }
        for member_id in commit.added_members {
            if !self.members.contains(&member_id) {
                self.members.push(member_id);
            }
        }
        for member_id in commit.removed_members {
            self.members.retain(|member| member != &member_id);
        }
        if let Some(group_secret) = commit.group_secret {
            self.secrets = GroupSecrets::new(commit.epoch, group_secret);
        } else if commit.epoch != self.secrets.epoch {
            let next_secret = derive_group_secret(self.hash_id, self.secrets.group_secret(), commit.epoch);
            self.secrets = GroupSecrets::new(commit.epoch, next_secret);
        }
        self.context.rebuild_extensions();
        self.epoch = commit.epoch;
        Ok(&self.context)
    }

    /// Adds a member to the group and returns the generated commit.
    pub fn add_member(&mut self, member_id: impl Into<String>) -> Result<Commit, MlsError> {
        let mut commit = Commit::new(self.epoch + 1);
        commit.added_members.push(member_id.into());
        let committed = commit.clone();
        self.apply_commit(commit)?;
        Ok(committed)
    }

    /// Removes a member from the group and returns the generated commit.
    pub fn remove_member(&mut self, member_id: impl Into<String>) -> Result<Commit, MlsError> {
        let mut commit = Commit::new(self.epoch + 1);
        commit.removed_members.push(member_id.into());
        let committed = commit.clone();
        self.apply_commit(commit)?;
        Ok(committed)
    }

    /// Returns a reference to the current group context.
    pub fn context(&self) -> &GroupContext {
        &self.context
    }

    /// Returns the group configuration hash.
    pub fn cfg_hash(&self) -> &[u8] {
        self.context.cfg_hash()
    }

    /// Returns the MLS extensions for the current group context.
    pub fn extensions(&self) -> &[Vec<u8>] {
        self.context.extensions()
    }

    /// Returns the list of current member identifiers.
    pub fn members(&self) -> &[String] {
        &self.members
    }

    /// Returns the current epoch number for the group.
    pub fn epoch(&self) -> u64 {
        self.epoch
    }

    /// Returns the current group secrets snapshot.
    pub fn secrets(&self) -> &GroupSecrets {
        &self.secrets
    }

    /// Returns the current group secret bytes.
    pub fn group_secret(&self) -> &[u8] {
        self.secrets.group_secret()
    }

    /// Derives per-member secrets for the current group.
    pub fn distribute_secrets(&self) -> Vec<MemberSecret> {
        self.members
            .iter()
            .map(|member| MemberSecret {
                member_id: member.clone(),
                secret: derive_member_secret(self.hash_id, self.secrets.group_secret(), member),
            })
            .collect()
    }

    /// Encrypts a plaintext payload for the group using the current secrets.
    pub fn encrypt(
        &self,
        sender_id: &str,
        message_id: u64,
        plaintext: &[u8],
    ) -> Result<GroupMessage, MlsError> {
        self.ensure_member(sender_id)?;
        let ciphertext = xor_with_keystream(
            plaintext,
            &derive_message_keystream(self.hash_id, self.secrets.group_secret(), sender_id, message_id, plaintext.len()),
        );
        Ok(GroupMessage::new(
            self.epoch,
            self.context.cfg_hash.clone(),
            self.context.proto_suite,
            ciphertext,
        ))
    }

    /// Decrypts a ciphertext payload for the group using the current secrets.
    pub fn decrypt(
        &self,
        sender_id: &str,
        message_id: u64,
        message: &GroupMessage,
    ) -> Result<Vec<u8>, MlsError> {
        self.ensure_member(sender_id)?;
        self.validate_message(message).map_err(MlsError::Validation)?;
        let keystream = derive_message_keystream(
            self.hash_id,
            self.secrets.group_secret(),
            sender_id,
            message_id,
            message.body.len(),
        );
        Ok(xor_with_keystream(&message.body, &keystream))
    }

    /// Validates that a message matches the current group epoch and configuration.
    pub fn validate_message(&self, message: &GroupMessage) -> Result<(), ValidationError> {
        if message.epoch != self.epoch {
            return Err(ValidationError::EpochMismatch {
                expected: self.epoch,
                found: message.epoch,
            });
        }
        if message.cfg_hash != self.context.cfg_hash {
            return Err(ValidationError::CfgHashMismatch);
        }
        if message.proto_suite != self.context.proto_suite {
            return Err(ValidationError::ProtoSuiteMismatch);
        }
        Ok(())
    }

    /// Ensures the provided member identifier belongs to the group.
    fn ensure_member(&self, member_id: &str) -> Result<(), MlsError> {
        if self.members.iter().any(|member| member == member_id) {
            Ok(())
        } else {
            Err(MlsError::UnknownMember(member_id.to_string()))
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Commit {
    pub epoch: u64,
    pub proto_suite: Option<ProtoSuite>,
    pub flags: Option<u8>,
    pub cfg_hash_id: Option<u16>,
    pub cfg_hash: Option<Vec<u8>>,
    pub group_secret: Option<Vec<u8>>,
    pub added_members: Vec<String>,
    pub removed_members: Vec<String>,
}

impl Commit {
    /// Creates a new commit with the specified epoch.
    pub fn new(epoch: u64) -> Self {
        Self {
            epoch,
            ..Self::default()
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GroupMessage {
    pub epoch: u64,
    pub cfg_hash: Vec<u8>,
    pub proto_suite: ProtoSuite,
    pub body: Vec<u8>,
}

impl GroupMessage {
    /// Creates a new group message carrying MLS metadata and payload bytes.
    pub fn new(epoch: u64, cfg_hash: Vec<u8>, proto_suite: ProtoSuite, body: Vec<u8>) -> Self {
        Self {
            epoch,
            cfg_hash,
            proto_suite,
            body,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ValidationError {
    EpochMismatch { expected: u64, found: u64 },
    CfgHashMismatch,
    ProtoSuiteMismatch,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MlsError {
    UnsupportedHashId(u16),
    Validation(ValidationError),
    UnknownMember(String),
    Spex(SpexError),
}

impl fmt::Display for MlsError {
    /// Formats the MLS error for display.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedHashId(value) => write!(f, "unsupported hash id: {value}"),
            Self::Validation(err) => write!(f, "validation error: {err:?}"),
            Self::UnknownMember(member) => write!(f, "unknown member: {member}"),
            Self::Spex(err) => write!(f, "spex error: {err}"),
        }
    }
}

impl std::error::Error for MlsError {}

impl From<SpexError> for MlsError {
    /// Wraps a SpexError in an MLS error.
    fn from(err: SpexError) -> Self {
        Self::Spex(err)
    }
}

/// Computes a configuration hash for a thread config payload.
pub fn cfg_hash_for_thread_config(
    hash_id: HashId,
    thread_config: &ThreadConfig,
) -> Result<Vec<u8>, MlsError> {
    Ok(hash_ctap2_cbor_value(hash_id, thread_config)?)
}

/// Builds the SPEX MLS extensions for the provided configuration.
pub fn mls_extensions(
    proto_suite: ProtoSuite,
    flags: u8,
    cfg_hash_id: u16,
    cfg_hash: &[u8],
) -> Vec<Vec<u8>> {
    vec![
        mls_ext::ext_proto_suite_bytes(
            proto_suite.major,
            proto_suite.minor,
            proto_suite.ciphersuite_id,
            flags,
        ),
        mls_ext::ext_cfg_hash_bytes(cfg_hash_id, cfg_hash),
    ]
}

/// Resolves a SPEX hash identifier into a supported HashId enum.
fn hash_id_from_u16(hash_id: u16) -> Result<HashId, MlsError> {
    match hash_id {
        1 => Ok(HashId::Sha256),
        _ => Err(MlsError::UnsupportedHashId(hash_id)),
    }
}

/// Derives an initial secret seed based on the current group context.
fn derive_initial_secret_seed(hash_id: HashId, context: &GroupContext) -> Vec<u8> {
    let mut seed = Vec::new();
    seed.extend_from_slice(&context.cfg_hash);
    seed.extend_from_slice(&context.proto_suite.major.to_be_bytes());
    seed.extend_from_slice(&context.proto_suite.minor.to_be_bytes());
    seed.extend_from_slice(&context.proto_suite.ciphersuite_id.to_be_bytes());
    seed.push(context.flags);
    hash_bytes(hash_id, &seed)
}

/// Derives the group secret for the provided epoch from a seed.
fn derive_group_secret(hash_id: HashId, seed: &[u8], epoch: u64) -> Vec<u8> {
    let mut data = Vec::new();
    data.extend_from_slice(seed);
    data.extend_from_slice(&epoch.to_be_bytes());
    hash_bytes(hash_id, &data)
}

/// Derives a per-member secret from the group secret.
fn derive_member_secret(hash_id: HashId, group_secret: &[u8], member_id: &str) -> Vec<u8> {
    let mut data = Vec::new();
    data.extend_from_slice(group_secret);
    data.extend_from_slice(member_id.as_bytes());
    hash_bytes(hash_id, &data)
}

/// Builds a keystream for encrypting or decrypting a message payload.
fn derive_message_keystream(
    hash_id: HashId,
    group_secret: &[u8],
    sender_id: &str,
    message_id: u64,
    length: usize,
) -> Vec<u8> {
    let mut out = Vec::with_capacity(length);
    let mut counter = 0u32;
    while out.len() < length {
        let mut data = Vec::new();
        data.extend_from_slice(group_secret);
        data.extend_from_slice(sender_id.as_bytes());
        data.extend_from_slice(&message_id.to_be_bytes());
        data.extend_from_slice(&counter.to_be_bytes());
        out.extend_from_slice(&hash_bytes(hash_id, &data));
        counter = counter.saturating_add(1);
    }
    out.truncate(length);
    out
}

/// XORs the payload with the provided keystream.
fn xor_with_keystream(payload: &[u8], keystream: &[u8]) -> Vec<u8> {
    payload
        .iter()
        .zip(keystream.iter())
        .map(|(byte, key)| byte ^ key)
        .collect()
}
