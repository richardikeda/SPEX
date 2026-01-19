use spex_core::{mls_ext, types::ProtoSuite};

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
        self.extensions = vec![
            mls_ext::ext_proto_suite_bytes(
                self.proto_suite.major,
                self.proto_suite.minor,
                self.proto_suite.ciphersuite_id,
                self.flags,
            ),
            mls_ext::ext_cfg_hash_bytes(self.cfg_hash_id, &self.cfg_hash),
        ];
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GroupConfig {
    pub proto_suite: ProtoSuite,
    pub flags: u8,
    pub cfg_hash_id: u16,
    pub cfg_hash: Vec<u8>,
}

impl GroupConfig {
    /// Creates a new group configuration for initializing a group context.
    pub fn new(proto_suite: ProtoSuite, flags: u8, cfg_hash_id: u16, cfg_hash: Vec<u8>) -> Self {
        Self {
            proto_suite,
            flags,
            cfg_hash_id,
            cfg_hash,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Group {
    epoch: u64,
    context: GroupContext,
    members: Vec<String>,
}

impl Group {
    /// Creates a new group from a configuration and builds its initial context.
    pub fn create(config: GroupConfig) -> Self {
        let mut context = GroupContext {
            proto_suite: config.proto_suite,
            flags: config.flags,
            cfg_hash_id: config.cfg_hash_id,
            cfg_hash: config.cfg_hash,
            extensions: Vec::new(),
        };
        context.rebuild_extensions();
        Self {
            epoch: 0,
            context,
            members: Vec::new(),
        }
    }

    /// Applies a commit to the group, updating the context and epoch.
    pub fn apply_commit(&mut self, commit: Commit) -> &GroupContext {
        if let Some(proto_suite) = commit.proto_suite {
            self.context.proto_suite = proto_suite;
        }
        if let Some(flags) = commit.flags {
            self.context.flags = flags;
        }
        if let Some(cfg_hash_id) = commit.cfg_hash_id {
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
        self.context.rebuild_extensions();
        self.epoch = commit.epoch;
        &self.context
    }

    /// Adds a member to the group and returns the generated commit.
    pub fn add_member(&mut self, member_id: impl Into<String>) -> Commit {
        let mut commit = Commit::new(self.epoch + 1);
        commit.added_members.push(member_id.into());
        let committed = commit.clone();
        self.apply_commit(commit);
        committed
    }

    /// Removes a member from the group and returns the generated commit.
    pub fn remove_member(&mut self, member_id: impl Into<String>) -> Commit {
        let mut commit = Commit::new(self.epoch + 1);
        commit.removed_members.push(member_id.into());
        let committed = commit.clone();
        self.apply_commit(commit);
        committed
    }

    /// Returns a reference to the current group context.
    pub fn context(&self) -> &GroupContext {
        &self.context
    }

    /// Returns the list of current member identifiers.
    pub fn members(&self) -> &[String] {
        &self.members
    }

    /// Returns the current epoch number for the group.
    pub fn epoch(&self) -> u64 {
        self.epoch
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
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Commit {
    pub epoch: u64,
    pub proto_suite: Option<ProtoSuite>,
    pub flags: Option<u8>,
    pub cfg_hash_id: Option<u16>,
    pub cfg_hash: Option<Vec<u8>>,
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
