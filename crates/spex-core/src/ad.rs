use crate::error::SpexError;

/// Big-endian helpers (explicit = deterministic)
fn u16be(v: u16) -> [u8; 2] { v.to_be_bytes() }
fn u32be(v: u32) -> [u8; 4] { v.to_be_bytes() }
fn u64be(v: u64) -> [u8; 8] { v.to_be_bytes() }

#[derive(Clone, Copy, Debug)]
pub struct ProtoSuite {
    pub major: u16,
    pub minor: u16,
    pub ciphersuite_id: u16,
}

/// AD = thread_id(32) || epoch(u32be) || cfg_hash(32) ||
///      proto_suite(major u16be || minor u16be || ciphersuite u16be) ||
///      seq(u64be) || sender_userid(20)
pub fn build_ad(
    thread_id: &[u8; 32],
    epoch: u32,
    cfg_hash: &[u8; 32],
    suite: ProtoSuite,
    seq: u64,
    sender_userid: &[u8; 20],
) -> Vec<u8> {
    let mut out = Vec::with_capacity(32 + 4 + 32 + 6 + 8 + 20);
    out.extend_from_slice(thread_id);
    out.extend_from_slice(&u32be(epoch));
    out.extend_from_slice(cfg_hash);
    out.extend_from_slice(&u16be(suite.major));
    out.extend_from_slice(&u16be(suite.minor));
    out.extend_from_slice(&u16be(suite.ciphersuite_id));
    out.extend_from_slice(&u64be(seq));
    out.extend_from_slice(sender_userid);
    out
}

/// Convenience for parsing fixed-size hex inputs in tests.
pub fn to_fixed<const N: usize>(bytes: &[u8]) -> Result<[u8; N], SpexError> {
    if bytes.len() != N {
        return Err(SpexError::InvalidLength("fixed array"));
    }
    let mut arr = [0u8; N];
    arr.copy_from_slice(bytes);
    Ok(arr)
}
