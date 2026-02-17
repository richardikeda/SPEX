use std::fmt;

use chacha20poly1305::{
    aead::{Aead, KeyInit},
    ChaCha20Poly1305, Key, Nonce,
};
use mls_rs::extension::ExtensionType;
use mls_rs::{
    client_builder::{BaseConfig, IntoConfigOutput, WithCryptoProvider, WithIdentityProvider},
    group::ReceivedMessage,
    identity::basic::{BasicCredential, BasicIdentityProvider},
    identity::SigningIdentity,
    CipherSuite, CipherSuiteProvider, Client, CryptoProvider, Extension, ExtensionList,
    Group as MlsRsGroupState, MlsMessage, ProtocolVersion,
};
use mls_rs_crypto_rustcrypto::RustCryptoProvider;
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
        Self {
            epoch,
            group_secret,
        }
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
            let next_secret =
                derive_group_secret(self.hash_id, self.secrets.group_secret(), commit.epoch);
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
        let key = derive_aead_key(
            self.hash_id,
            self.secrets.group_secret(),
            sender_id,
            message_id,
        )?;
        let nonce = derive_aead_nonce(
            self.hash_id,
            self.secrets.group_secret(),
            sender_id,
            message_id,
        )?;
        let cipher = ChaCha20Poly1305::new(&key);
        let ciphertext = cipher
            .encrypt(&nonce, plaintext)
            .map_err(|_| MlsError::Encryption)?;

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
        self.validate_message(message)
            .map_err(MlsError::Validation)?;
        let key = derive_aead_key(
            self.hash_id,
            self.secrets.group_secret(),
            sender_id,
            message_id,
        )?;
        let nonce = derive_aead_nonce(
            self.hash_id,
            self.secrets.group_secret(),
            sender_id,
            message_id,
        )?;
        let cipher = ChaCha20Poly1305::new(&key);
        let plaintext = cipher
            .decrypt(&nonce, message.body.as_slice())
            .map_err(|_| MlsError::Decryption)?;
        Ok(plaintext)
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

#[derive(Debug)]
pub enum MlsError {
    UnsupportedHashId(u16),
    Validation(ValidationError),
    UnknownMember(String),
    Encryption,
    Decryption,
    Spex(SpexError),
}

impl fmt::Display for MlsError {
    /// Formats the MLS error for display.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedHashId(value) => write!(f, "unsupported hash id: {value}"),
            Self::Validation(err) => write!(f, "validation error: {err:?}"),
            Self::UnknownMember(member) => write!(f, "unknown member: {member}"),
            Self::Encryption => write!(f, "encryption error"),
            Self::Decryption => write!(f, "decryption error"),
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

/// Derives an AEAD key for encrypting or decrypting a message payload.
fn derive_aead_key(
    hash_id: HashId,
    group_secret: &[u8],
    sender_id: &str,
    message_id: u64,
) -> Result<Key<ChaCha20Poly1305>, MlsError> {
    let hash = derive_message_metadata_hash(hash_id, group_secret, sender_id, message_id, b"key");
    hash.get(..32)
        .map(|bytes| *Key::<ChaCha20Poly1305>::from_slice(bytes))
        .ok_or(MlsError::Encryption)
}

/// Derives an AEAD nonce for encrypting or decrypting a message payload.
fn derive_aead_nonce(
    hash_id: HashId,
    group_secret: &[u8],
    sender_id: &str,
    message_id: u64,
) -> Result<Nonce, MlsError> {
    let hash = derive_message_metadata_hash(hash_id, group_secret, sender_id, message_id, b"nonce");
    hash.get(..12)
        .map(|bytes| *Nonce::from_slice(bytes))
        .ok_or(MlsError::Encryption)
}

/// Helper to derive a hash from message metadata and a label.
fn derive_message_metadata_hash(
    hash_id: HashId,
    group_secret: &[u8],
    sender_id: &str,
    message_id: u64,
    label: &[u8],
) -> Vec<u8> {
    let mut data = Vec::new();
    data.extend_from_slice(group_secret);
    data.extend_from_slice(sender_id.as_bytes());
    data.extend_from_slice(&message_id.to_be_bytes());
    data.extend_from_slice(label);
    hash_bytes(hash_id, &data)
}

type MlsRsConfig = IntoConfigOutput<
    WithIdentityProvider<BasicIdentityProvider, WithCryptoProvider<RustCryptoProvider, BaseConfig>>,
>;

/// Defines the MLS extension type for the SPEX proto suite metadata.
const SPEX_PROTO_SUITE_EXTENSION: ExtensionType = ExtensionType::new(mls_ext::EXT_SPEX_PROTO_SUITE);
/// Defines the MLS extension type for the SPEX cfg_hash metadata.
const SPEX_CFG_HASH_EXTENSION: ExtensionType = ExtensionType::new(mls_ext::EXT_SPEX_CFG_HASH);

/// Represents failures when integrating with the MLS-rs implementation.
#[derive(Debug)]
pub enum MlsRsError {
    UnsupportedProtocolVersion { major: u16, minor: u16 },
    UnsupportedCipherSuite(u16),
    UnsupportedCfgHashId(u16),
    InvalidExtension(&'static str),
    UnknownMember,
    Validation(ValidationError),
    LibraryError(String),
}

impl fmt::Display for MlsRsError {
    /// Formats MLS-rs integration errors for display.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedProtocolVersion { major, minor } => {
                write!(f, "unsupported protocol version {major}.{minor}")
            }
            Self::UnsupportedCipherSuite(value) => write!(f, "unsupported cipher suite: {value}"),
            Self::UnsupportedCfgHashId(value) => write!(f, "unsupported cfg_hash id: {value}"),
            Self::InvalidExtension(reason) => write!(f, "invalid MLS extension: {reason}"),
            Self::UnknownMember => write!(f, "unknown MLS member"),
            Self::Validation(err) => write!(f, "validation error: {err:?}"),
            Self::LibraryError(err) => write!(f, "mls-rs error: {err}"),
        }
    }
}

impl std::error::Error for MlsRsError {}

impl From<mls_rs::error::MlsError> for MlsRsError {
    /// Wraps an MLS-rs error in a local integration error.
    fn from(err: mls_rs::error::MlsError) -> Self {
        Self::LibraryError(err.to_string())
    }
}

/// Stores a configured MLS-rs client for SPEX operations.
#[derive(Clone)]
pub struct MlsRsClient {
    client: Client<MlsRsConfig>,
    proto_suite: ProtoSuite,
}

impl MlsRsClient {
    /// Creates a new MLS-rs client using the provided SPEX protocol suite and identity.
    pub fn new(proto_suite: ProtoSuite, identity: Vec<u8>) -> Result<Self, MlsRsError> {
        let crypto = RustCryptoProvider::default();
        let cipher_suite = cipher_suite_from_proto_suite(proto_suite, &crypto)?;
        let protocol_version = protocol_version_from_proto_suite(proto_suite)?;
        let cipher_suite_provider = crypto.cipher_suite_provider(cipher_suite).ok_or(
            MlsRsError::UnsupportedCipherSuite(proto_suite.ciphersuite_id),
        )?;
        let (secret_key, public_key) = cipher_suite_provider
            .signature_key_generate()
            .map_err(|err| MlsRsError::LibraryError(format!("{err:?}")))?;
        let credential = BasicCredential::new(identity).into_credential();
        let signing_identity = SigningIdentity::new(credential, public_key);

        let client = Client::builder()
            .crypto_provider(crypto)
            .identity_provider(BasicIdentityProvider::new())
            .extension_types([SPEX_PROTO_SUITE_EXTENSION, SPEX_CFG_HASH_EXTENSION])
            .used_protocol_version(protocol_version)
            .signing_identity(signing_identity, secret_key, cipher_suite)
            .build();

        Ok(Self {
            client,
            proto_suite,
        })
    }

    /// Generates a key package message that can be used to add this client to a group.
    pub fn key_package_message(&self) -> Result<MlsMessage, MlsRsError> {
        Ok(self.client.generate_key_package_message(
            ExtensionList::new(),
            ExtensionList::new(),
            None,
        )?)
    }

    /// Creates a new MLS group with SPEX extensions populated from the provided configuration.
    pub fn create_group(&self, config: GroupConfig) -> Result<MlsRsGroup, MlsRsError> {
        validate_cfg_hash_id(config.cfg_hash_id)?;
        if config.proto_suite != self.proto_suite {
            return Err(MlsRsError::Validation(ValidationError::ProtoSuiteMismatch));
        }
        let group_context_extensions = build_group_context_extensions(&config)?;
        let group =
            self.client
                .create_group(group_context_extensions, ExtensionList::new(), None)?;

        Ok(MlsRsGroup { group, config })
    }

    /// Joins an MLS group from a welcome message and optional ratchet tree.
    pub fn join_group(
        &self,
        welcome: &MlsMessage,
        ratchet_tree: Option<mls_rs::group::ExportedTree<'_>>,
    ) -> Result<MlsRsGroup, MlsRsError> {
        let (group, _info) = self.client.join_group(ratchet_tree, welcome, None)?;
        let config = group_config_from_context(group.context())?;
        Ok(MlsRsGroup { group, config })
    }

    /// Performs an external commit to resynchronize or add a member using group info.
    pub fn resync_from_group_info(
        &self,
        group_info: MlsMessage,
        ratchet_tree: Option<mls_rs::group::ExportedTree<'_>>,
    ) -> Result<(MlsRsGroup, MlsMessage), MlsRsError> {
        let mut builder = self.client.external_commit_builder()?;
        if let Some(tree) = ratchet_tree {
            builder = builder.with_tree_data(tree.into_owned());
        }
        let (group, commit_message) = builder.build(group_info)?;
        let config = group_config_from_context(group.context())?;
        Ok((MlsRsGroup { group, config }, commit_message))
    }
}

/// Wraps an MLS-rs group and exposes SPEX-specific helpers.
pub struct MlsRsGroup {
    group: MlsRsGroupState<MlsRsConfig>,
    config: GroupConfig,
}

impl MlsRsGroup {
    /// Adds a member to the MLS group using the provided client key package.
    pub fn add_member(
        &mut self,
        member: &MlsRsClient,
    ) -> Result<mls_rs::group::CommitOutput, MlsRsError> {
        let key_package = member.key_package_message()?;
        let commit = self
            .group
            .commit_builder()
            .add_member(key_package)?
            .build()?;
        self.group.apply_pending_commit()?;
        Ok(commit)
    }

    /// Removes a member identified by their basic credential identifier.
    pub fn remove_member_by_identity(
        &mut self,
        identity: &[u8],
    ) -> Result<mls_rs::group::CommitOutput, MlsRsError> {
        let member_index = self
            .member_index_by_identity(identity)
            .ok_or(MlsRsError::UnknownMember)?;
        let commit = self
            .group
            .commit_builder()
            .remove_member(member_index)?
            .build()?;
        self.group.apply_pending_commit()?;
        Ok(commit)
    }

    /// Issues a self-update commit to rotate keys using an update path.
    pub fn self_update(&mut self) -> Result<mls_rs::group::CommitOutput, MlsRsError> {
        let commit = self.group.commit_builder().build()?;
        self.group.apply_pending_commit()?;
        Ok(commit)
    }

    /// Processes an incoming MLS commit message and applies the resulting epoch change.
    pub fn process_commit_message(
        &mut self,
        message: MlsMessage,
    ) -> Result<mls_rs::group::CommitMessageDescription, MlsRsError> {
        match self.group.process_incoming_message(message)? {
            ReceivedMessage::Commit(description) => Ok(description),
            _ => Err(MlsRsError::LibraryError(
                "unexpected MLS message type".to_string(),
            )),
        }
    }

    /// Exports a group info message that allows external commits.
    pub fn group_info_message_allowing_external_commit(
        &self,
        include_tree: bool,
    ) -> Result<MlsMessage, MlsRsError> {
        Ok(self
            .group
            .group_info_message_allowing_ext_commit(include_tree)?)
    }

    /// Exports the current ratchet tree for resynchronization workflows.
    pub fn export_tree(&self) -> mls_rs::group::ExportedTree<'_> {
        self.group.export_tree()
    }

    /// Returns the current MLS epoch.
    pub fn epoch(&self) -> u64 {
        self.group.current_epoch()
    }

    /// Returns the roster of member identifiers in the group.
    pub fn member_identities(&self) -> Vec<Vec<u8>> {
        self.group
            .roster()
            .members_iter()
            .filter_map(|member| {
                member
                    .signing_identity
                    .credential
                    .as_basic()
                    .map(|basic| basic.identifier.clone())
            })
            .collect()
    }

    /// Validates that the MLS group context extensions match the configured cfg_hash and proto suite.
    pub fn validate_cfg_hash_proto_suite(&self) -> Result<(), MlsRsError> {
        validate_group_context_extensions(self.group.context(), &self.config)
    }

    /// Looks up the member index for a given identifier.
    fn member_index_by_identity(&self, identity: &[u8]) -> Option<u32> {
        self.group.roster().members_iter().find_map(|member| {
            member
                .signing_identity
                .credential
                .as_basic()
                .and_then(|basic| (basic.identifier == identity).then_some(member.index))
        })
    }
}

/// Builds MLS group context extensions for SPEX metadata.
fn build_group_context_extensions(config: &GroupConfig) -> Result<ExtensionList, MlsRsError> {
    if config.cfg_hash.len() > 255 {
        return Err(MlsRsError::InvalidExtension(
            "cfg_hash length exceeds 255 bytes",
        ));
    }
    let proto_extension = mls_ext::ext_proto_suite_bytes(
        config.proto_suite.major,
        config.proto_suite.minor,
        config.proto_suite.ciphersuite_id,
        config.flags,
    );
    let cfg_extension = mls_ext::ext_cfg_hash_bytes(config.cfg_hash_id, &config.cfg_hash);
    let proto_data = strip_extension_data(&proto_extension, mls_ext::EXT_SPEX_PROTO_SUITE)?;
    let cfg_data = strip_extension_data(&cfg_extension, mls_ext::EXT_SPEX_CFG_HASH)?;

    let mut list = ExtensionList::new();
    list.set(Extension::new(SPEX_PROTO_SUITE_EXTENSION, proto_data));
    list.set(Extension::new(SPEX_CFG_HASH_EXTENSION, cfg_data));
    Ok(list)
}

/// Builds a group configuration by parsing the MLS group context extensions.
fn group_config_from_context(
    context: &mls_rs::group::GroupContext,
) -> Result<GroupConfig, MlsRsError> {
    let extensions = context.extensions();
    let proto_ext =
        extensions
            .get(SPEX_PROTO_SUITE_EXTENSION)
            .ok_or(MlsRsError::InvalidExtension(
                "missing proto suite extension",
            ))?;
    let cfg_ext = extensions
        .get(SPEX_CFG_HASH_EXTENSION)
        .ok_or(MlsRsError::InvalidExtension("missing cfg_hash extension"))?;

    let (proto_suite, flags) = parse_proto_suite_extension(&proto_ext.extension_data)?;
    let (cfg_hash_id, cfg_hash) = parse_cfg_hash_extension(&cfg_ext.extension_data)?;

    Ok(GroupConfig::new(proto_suite, flags, cfg_hash_id, cfg_hash))
}

/// Extracts extension payload data from a serialized SPEX extension.
fn strip_extension_data(raw: &[u8], expected_type: u16) -> Result<Vec<u8>, MlsRsError> {
    if raw.len() < 4 {
        return Err(MlsRsError::InvalidExtension(
            "extension payload is too short",
        ));
    }
    let ext_type = u16::from_be_bytes([raw[0], raw[1]]);
    if ext_type != expected_type {
        return Err(MlsRsError::InvalidExtension("extension type mismatch"));
    }
    let length = u16::from_be_bytes([raw[2], raw[3]]) as usize;
    if raw.len() != 4 + length {
        return Err(MlsRsError::InvalidExtension("extension length mismatch"));
    }
    Ok(raw[4..].to_vec())
}

/// Parses a SPEX proto suite extension payload into a ProtoSuite value and flags.
fn parse_proto_suite_extension(data: &[u8]) -> Result<(ProtoSuite, u8), MlsRsError> {
    if data.len() != 7 {
        return Err(MlsRsError::InvalidExtension(
            "proto suite extension length mismatch",
        ));
    }
    let major = u16::from_be_bytes([data[0], data[1]]);
    let minor = u16::from_be_bytes([data[2], data[3]]);
    let ciphersuite_id = u16::from_be_bytes([data[4], data[5]]);
    let flags = data[6];
    Ok((
        ProtoSuite {
            major,
            minor,
            ciphersuite_id,
        },
        flags,
    ))
}

/// Parses a SPEX cfg_hash extension payload into a hash id and cfg hash bytes.
fn parse_cfg_hash_extension(data: &[u8]) -> Result<(u16, Vec<u8>), MlsRsError> {
    if data.len() < 3 {
        return Err(MlsRsError::InvalidExtension(
            "cfg_hash extension is too short",
        ));
    }
    let hash_id = u16::from_be_bytes([data[0], data[1]]);
    let length = data[2] as usize;
    if data.len() != 3 + length {
        return Err(MlsRsError::InvalidExtension(
            "cfg_hash extension length mismatch",
        ));
    }
    Ok((hash_id, data[3..].to_vec()))
}

/// Validates cfg_hash/proto_suite in the MLS group context extensions.
fn validate_group_context_extensions(
    context: &mls_rs::group::GroupContext,
    config: &GroupConfig,
) -> Result<(), MlsRsError> {
    let extensions = context.extensions();
    let proto_ext =
        extensions
            .get(SPEX_PROTO_SUITE_EXTENSION)
            .ok_or(MlsRsError::InvalidExtension(
                "missing proto suite extension",
            ))?;
    let cfg_ext = extensions
        .get(SPEX_CFG_HASH_EXTENSION)
        .ok_or(MlsRsError::InvalidExtension("missing cfg_hash extension"))?;

    let (proto_suite, flags) = parse_proto_suite_extension(&proto_ext.extension_data)?;
    if proto_suite != config.proto_suite || flags != config.flags {
        return Err(MlsRsError::Validation(ValidationError::ProtoSuiteMismatch));
    }

    let (hash_id, cfg_hash) = parse_cfg_hash_extension(&cfg_ext.extension_data)?;
    if hash_id != config.cfg_hash_id || cfg_hash != config.cfg_hash {
        return Err(MlsRsError::Validation(ValidationError::CfgHashMismatch));
    }

    Ok(())
}

/// Validates that the configured hash identifier is supported.
fn validate_cfg_hash_id(cfg_hash_id: u16) -> Result<(), MlsRsError> {
    match hash_id_from_u16(cfg_hash_id) {
        Ok(_) => Ok(()),
        Err(_) => Err(MlsRsError::UnsupportedCfgHashId(cfg_hash_id)),
    }
}

/// Converts a SPEX protocol suite into an MLS protocol version.
fn protocol_version_from_proto_suite(
    proto_suite: ProtoSuite,
) -> Result<ProtocolVersion, MlsRsError> {
    match (proto_suite.major, proto_suite.minor) {
        (0, 1) => Ok(ProtocolVersion::MLS_10),
        _ => Err(MlsRsError::UnsupportedProtocolVersion {
            major: proto_suite.major,
            minor: proto_suite.minor,
        }),
    }
}

/// Converts a SPEX protocol suite into a supported MLS cipher suite.
fn cipher_suite_from_proto_suite(
    proto_suite: ProtoSuite,
    crypto: &RustCryptoProvider,
) -> Result<CipherSuite, MlsRsError> {
    let cipher_suite = CipherSuite::new(proto_suite.ciphersuite_id);
    if crypto.supported_cipher_suites().contains(&cipher_suite) {
        Ok(cipher_suite)
    } else {
        Err(MlsRsError::UnsupportedCipherSuite(
            proto_suite.ciphersuite_id,
        ))
    }
}
