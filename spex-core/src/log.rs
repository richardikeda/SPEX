use crate::{
    cbor,
    error::SpexError,
    hash::{hash_bytes, HashId},
    types::Ctap2Cbor,
};
use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine};
use serde::{Deserialize, Serialize};
use serde_cbor::Value;
use std::collections::BTreeMap;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct KeyCheckpoint {
    pub user_id: Vec<u8>,
    pub verifying_key: Vec<u8>,
    pub device_id: Vec<u8>,
    pub issued_at: u64,
}

impl Ctap2Cbor for KeyCheckpoint {
    /// Encodes the key checkpoint as a CTAP2 canonical CBOR value.
    fn to_cbor_value(&self) -> Value {
        let mut map = BTreeMap::new();
        map.insert(Value::Integer(0), Value::Bytes(self.user_id.clone()));
        map.insert(
            Value::Integer(1),
            Value::Bytes(self.verifying_key.clone()),
        );
        map.insert(Value::Integer(2), Value::Bytes(self.device_id.clone()));
        map.insert(Value::Integer(3), Value::Integer(self.issued_at as i128));
        Value::Map(map.into_iter().collect())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct RecoveryKey {
    pub user_id: Vec<u8>,
    pub recovery_key: Vec<u8>,
    pub issued_at: u64,
}

impl Ctap2Cbor for RecoveryKey {
    /// Encodes the recovery key as a CTAP2 canonical CBOR value.
    fn to_cbor_value(&self) -> Value {
        let mut map = BTreeMap::new();
        map.insert(Value::Integer(0), Value::Bytes(self.user_id.clone()));
        map.insert(Value::Integer(1), Value::Bytes(self.recovery_key.clone()));
        map.insert(Value::Integer(2), Value::Integer(self.issued_at as i128));
        Value::Map(map.into_iter().collect())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct RevocationDeclaration {
    pub user_id: Vec<u8>,
    pub revoked_key_hash: Vec<u8>,
    pub revoked_at: u64,
    pub recovery_key_hash: Option<Vec<u8>>,
    pub reason: Option<String>,
}

impl Ctap2Cbor for RevocationDeclaration {
    /// Encodes the revocation declaration as a CTAP2 canonical CBOR value.
    fn to_cbor_value(&self) -> Value {
        let mut map = BTreeMap::new();
        map.insert(Value::Integer(0), Value::Bytes(self.user_id.clone()));
        map.insert(
            Value::Integer(1),
            Value::Bytes(self.revoked_key_hash.clone()),
        );
        map.insert(Value::Integer(2), Value::Integer(self.revoked_at as i128));
        if let Some(hash) = &self.recovery_key_hash {
            map.insert(Value::Integer(3), Value::Bytes(hash.clone()));
        }
        if let Some(reason) = &self.reason {
            map.insert(Value::Integer(4), Value::Text(reason.clone()));
        }
        Value::Map(map.into_iter().collect())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum CheckpointEntry {
    Key(KeyCheckpoint),
    Recovery(RecoveryKey),
    Revocation(RevocationDeclaration),
}

impl Ctap2Cbor for CheckpointEntry {
    /// Encodes a checkpoint log entry as a typed CTAP2 canonical CBOR value.
    fn to_cbor_value(&self) -> Value {
        let mut map = BTreeMap::new();
        match self {
            CheckpointEntry::Key(entry) => {
                map.insert(Value::Integer(0), Value::Integer(0));
                map.insert(Value::Integer(1), entry.to_cbor_value());
            }
            CheckpointEntry::Recovery(entry) => {
                map.insert(Value::Integer(0), Value::Integer(1));
                map.insert(Value::Integer(1), entry.to_cbor_value());
            }
            CheckpointEntry::Revocation(entry) => {
                map.insert(Value::Integer(0), Value::Integer(2));
                map.insert(Value::Integer(1), entry.to_cbor_value());
            }
        }
        Value::Map(map.into_iter().collect())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LogConsistency {
    Identical,
    LocalBehind,
    LocalAhead,
    Diverged,
}

#[derive(Clone, Debug, Default)]
struct MerkleState {
    frontier: Vec<Option<Vec<u8>>>,
    leaf_count: usize,
}

impl MerkleState {
    /// Creates a new empty Merkle state for append-only updates.
    fn new() -> Self {
        Self {
            frontier: Vec::new(),
            leaf_count: 0,
        }
    }

    /// Builds a Merkle state from a list of checkpoint entries.
    fn from_entries(entries: &[CheckpointEntry]) -> Result<Self, SpexError> {
        let mut state = Self::new();
        for entry in entries {
            let leaf_hash = CheckpointLog::leaf_hash(entry)?;
            state.append_leaf(leaf_hash);
        }
        Ok(state)
    }

    /// Appends a leaf hash into the Merkle frontier and returns the new root.
    fn append_leaf(&mut self, leaf_hash: Vec<u8>) -> Vec<u8> {
        let mut carry = leaf_hash;
        let mut level = 0;
        let mut index = self.leaf_count;
        while index & 1 == 1 {
            if let Some(left) = self.frontier.get_mut(level).and_then(Option::take) {
                carry = hash_node(&left, &carry);
            }
            index >>= 1;
            level += 1;
        }
        if self.frontier.len() <= level {
            self.frontier.resize_with(level + 1, || None);
        }
        self.frontier[level] = Some(carry);
        self.leaf_count += 1;
        self.root()
    }

    /// Computes the Merkle root from the current frontier.
    fn root(&self) -> Vec<u8> {
        let mut root: Option<Vec<u8>> = None;
        for node in self.frontier.iter().rev() {
            if let Some(hash) = node {
                root = Some(match root {
                    None => hash.clone(),
                    Some(accum) => hash_node(hash, &accum),
                });
            }
        }
        root.unwrap_or_default()
    }
}

/// Combines two Merkle child hashes into a parent hash.
fn hash_node(left: &[u8], right: &[u8]) -> Vec<u8> {
    let mut buffer = Vec::with_capacity(1 + left.len() + right.len());
    buffer.push(0x01);
    buffer.extend_from_slice(left);
    buffer.extend_from_slice(right);
    hash_bytes(HashId::Sha256, &buffer)
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CheckpointLog {
    pub entries: Vec<CheckpointEntry>,
    #[serde(skip)]
    merkle_state: MerkleState,
}

impl Default for CheckpointLog {
    /// Creates an empty checkpoint log with a fresh Merkle state.
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            merkle_state: MerkleState::new(),
        }
    }
}

impl PartialEq for CheckpointLog {
    /// Compares checkpoint logs based on their entries.
    fn eq(&self, other: &Self) -> bool {
        self.entries == other.entries
    }
}

impl Eq for CheckpointLog {}

impl CheckpointLog {
    /// Creates an empty checkpoint log.
    pub fn new() -> Self {
        Self::default()
    }

    /// Appends a checkpoint entry to the log.
    pub fn append(&mut self, entry: CheckpointEntry) -> Result<(), SpexError> {
        self.append_with_root(entry).map(|_| ())
    }

    /// Appends a checkpoint entry and returns the updated Merkle root.
    pub fn append_with_root(&mut self, entry: CheckpointEntry) -> Result<Vec<u8>, SpexError> {
        self.ensure_merkle_state()?;
        let leaf_hash = Self::leaf_hash(&entry)?;
        let root = self.merkle_state.append_leaf(leaf_hash);
        self.entries.push(entry);
        Ok(root)
    }

    /// Computes the Merkle root for the current log entries.
    pub fn merkle_root(&self) -> Result<Vec<u8>, SpexError> {
        if self.entries.is_empty() {
            return Ok(Vec::new());
        }
        if self.merkle_state.leaf_count == self.entries.len() {
            return Ok(self.merkle_state.root());
        }
        Ok(MerkleState::from_entries(&self.entries)?.root())
    }

    /// Compares two logs for append-only consistency.
    pub fn compare_with(&self, other: &Self) -> LogConsistency {
        if self.entries == other.entries {
            return LogConsistency::Identical;
        }
        if self.is_prefix_of(other) {
            return LogConsistency::LocalBehind;
        }
        if other.is_prefix_of(self) {
            return LogConsistency::LocalAhead;
        }
        LogConsistency::Diverged
    }

    /// Returns true when the current log entries are a prefix of another log.
    pub fn is_prefix_of(&self, other: &Self) -> bool {
        if self.entries.len() > other.entries.len() {
            return false;
        }
        self.entries
            .iter()
            .zip(other.entries.iter())
            .all(|(left, right)| left == right)
    }

    /// Serializes the log into CTAP2 canonical CBOR bytes.
    pub fn to_cbor_bytes(&self) -> Result<Vec<u8>, SpexError> {
        cbor::to_ctap2_canonical_bytes(self)
    }

    /// Deserializes a log from CTAP2 canonical CBOR bytes.
    pub fn from_cbor_bytes(bytes: &[u8]) -> Result<Self, SpexError> {
        let mut log: CheckpointLog = serde_cbor::from_slice(bytes)?;
        log.rebuild_merkle_state()?;
        Ok(log)
    }

    /// Serializes the log as base64-encoded CBOR.
    pub fn export_base64(&self) -> Result<String, SpexError> {
        let bytes = self.to_cbor_bytes()?;
        Ok(BASE64_STANDARD.encode(bytes))
    }

    /// Deserializes a log from a base64-encoded CBOR payload.
    pub fn import_base64(encoded: &str) -> Result<Self, SpexError> {
        let bytes = BASE64_STANDARD
            .decode(encoded.as_bytes())
            .map_err(|err| SpexError::InvalidInput(err.to_string()))?;
        Self::from_cbor_bytes(&bytes)
    }

    /// Ensures the Merkle state is synchronized with the current entries.
    fn ensure_merkle_state(&mut self) -> Result<(), SpexError> {
        if self.merkle_state.leaf_count == self.entries.len() {
            return Ok(());
        }
        self.rebuild_merkle_state()
    }

    /// Rebuilds the Merkle state from the log entries.
    fn rebuild_merkle_state(&mut self) -> Result<(), SpexError> {
        self.merkle_state = MerkleState::from_entries(&self.entries)?;
        Ok(())
    }

    /// Computes the Merkle leaf hash for a checkpoint entry.
    fn leaf_hash(entry: &CheckpointEntry) -> Result<Vec<u8>, SpexError> {
        let encoded = cbor::ctap2_canonical_value_bytes(&entry.to_cbor_value())?;
        let mut buffer = Vec::with_capacity(1 + encoded.len());
        buffer.push(0x00);
        buffer.extend_from_slice(&encoded);
        Ok(hash_bytes(HashId::Sha256, &buffer))
    }
}
