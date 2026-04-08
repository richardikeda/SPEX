// SPDX-License-Identifier: MPL-2.0
use crate::utils::u16be;

/// SPEX MLS extension types (private range)
pub const EXT_SPEX_PROTO_SUITE: u16 = 0xF0A0;
pub const EXT_SPEX_CFG_HASH: u16 = 0xF0A1;

/// ext_spex_proto_suite (0xF0A0)
/// extension_data = major(u16) || minor(u16) || ciphersuite_id(u16) || flags(u8)
pub fn ext_proto_suite_bytes(major: u16, minor: u16, ciphersuite_id: u16, flags: u8) -> Vec<u8> {
    let mut data = Vec::with_capacity(7);
    data.extend_from_slice(&u16be(major));
    data.extend_from_slice(&u16be(minor));
    data.extend_from_slice(&u16be(ciphersuite_id));
    data.push(flags);

    let mut out = Vec::with_capacity(2 + 2 + data.len());
    out.extend_from_slice(&u16be(EXT_SPEX_PROTO_SUITE));
    out.extend_from_slice(&u16be(data.len() as u16));
    out.extend_from_slice(&data);
    out
}

/// ext_spex_cfg_hash (0xF0A1)
/// extension_data = hash_id(u16) || len(u8) || cfg_hash(len bytes)
pub fn ext_cfg_hash_bytes(hash_id: u16, cfg_hash: &[u8]) -> Vec<u8> {
    assert!(cfg_hash.len() <= 255);
    let mut data = Vec::with_capacity(2 + 1 + cfg_hash.len());
    data.extend_from_slice(&u16be(hash_id));
    data.push(cfg_hash.len() as u8);
    data.extend_from_slice(cfg_hash);

    let mut out = Vec::with_capacity(2 + 2 + data.len());
    out.extend_from_slice(&u16be(EXT_SPEX_CFG_HASH));
    out.extend_from_slice(&u16be(data.len() as u16));
    out.extend_from_slice(&data);
    out
}
