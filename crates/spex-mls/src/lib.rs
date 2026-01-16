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
        Self { epoch: 0, context }
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
        self.context.rebuild_extensions();
        self.epoch = commit.epoch;
        &self.context
    }

    /// Returns a reference to the current group context.
    pub fn context(&self) -> &GroupContext {
        &self.context
    }

    /// Returns the current epoch number for the group.
    pub fn epoch(&self) -> u64 {
        self.epoch
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Commit {
    pub epoch: u64,
    pub proto_suite: Option<ProtoSuite>,
    pub flags: Option<u8>,
    pub cfg_hash_id: Option<u16>,
    pub cfg_hash: Option<Vec<u8>>,
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
