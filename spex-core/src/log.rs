use crate::{
    cbor,
    error::SpexError,
    hash::{hash_bytes, HashId},
    types::Ctap2Cbor,
};
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

#[derive(Clone, Debug, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct CheckpointLog {
    pub entries: Vec<CheckpointEntry>,
}

impl CheckpointLog {
    pub fn new() -> Self {
        Self { entries: Vec::new() }
    }

    pub fn append(&mut self, entry: CheckpointEntry) {
        self.entries.push(entry);
    }

    pub fn merkle_root(&self) -> Result<Vec<u8>, SpexError> {
        if self.entries.is_empty() {
            return Ok(Vec::new());
        }

        let mut level: Vec<Vec<u8>> = self
            .entries
            .iter()
            .map(Self::leaf_hash)
            .collect::<Result<_, _>>()?;

        while level.len() > 1 {
            let mut next = Vec::with_capacity((level.len() + 1) / 2);
            for pair in level.chunks(2) {
                let left = &pair[0];
                let right = if pair.len() == 2 { &pair[1] } else { &pair[0] };
                let mut buffer = Vec::with_capacity(1 + left.len() + right.len());
                buffer.push(0x01);
                buffer.extend_from_slice(left);
                buffer.extend_from_slice(right);
                next.push(hash_bytes(HashId::Sha256, &buffer));
            }
            level = next;
        }

        Ok(level
            .pop()
            .unwrap_or_else(|| hash_bytes(HashId::Sha256, &[])))
    }

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

    pub fn is_prefix_of(&self, other: &Self) -> bool {
        if self.entries.len() > other.entries.len() {
            return false;
        }
        self.entries
            .iter()
            .zip(other.entries.iter())
            .all(|(left, right)| left == right)
    }

    pub fn to_cbor_bytes(&self) -> Result<Vec<u8>, SpexError> {
        cbor::to_ctap2_canonical_bytes(self)
    }

    pub fn from_cbor_bytes(bytes: &[u8]) -> Result<Self, SpexError> {
        Ok(serde_cbor::from_slice(bytes)?)
    }

    fn leaf_hash(entry: &CheckpointEntry) -> Result<Vec<u8>, SpexError> {
        let encoded = cbor::ctap2_canonical_value_bytes(&entry.to_cbor_value())?;
        let mut buffer = Vec::with_capacity(1 + encoded.len());
        buffer.push(0x00);
        buffer.extend_from_slice(&encoded);
        Ok(hash_bytes(HashId::Sha256, &buffer))
    }
}
