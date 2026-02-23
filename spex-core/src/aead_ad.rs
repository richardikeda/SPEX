use crate::types::ProtoSuite;
use crate::utils::{u16be, u32be, u64be};

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_vectors;
    use crate::types::{self, ProtoSuite};

    #[test]
    fn test_build_ad_layout() {
        let thread_id = [0x11u8; 32];
        let epoch = 0x12345678;
        let cfg_hash = [0x22u8; 32];
        let suite = ProtoSuite {
            major: 0x1122,
            minor: 0x3344,
            ciphersuite_id: 0x5566,
        };
        let seq = 0x1122334455667788;
        let sender_userid = [0x33u8; 20];

        let ad = build_ad(&thread_id, epoch, &cfg_hash, suite, seq, &sender_userid);

        assert_eq!(ad.len(), 102);

        // AD = thread_id(32) || epoch(u32be) || cfg_hash(32) ||
        //      proto_suite(major u16be || minor u16be || ciphersuite u16be) ||
        //      seq(u64be) || sender_userid(20)

        assert_eq!(&ad[0..32], &thread_id, "thread_id mismatch");
        assert_eq!(&ad[32..36], &epoch.to_be_bytes(), "epoch mismatch");
        assert_eq!(&ad[36..68], &cfg_hash, "cfg_hash mismatch");
        assert_eq!(&ad[68..70], &suite.major.to_be_bytes(), "major mismatch");
        assert_eq!(&ad[70..72], &suite.minor.to_be_bytes(), "minor mismatch");
        assert_eq!(
            &ad[72..74],
            &suite.ciphersuite_id.to_be_bytes(),
            "ciphersuite_id mismatch"
        );
        assert_eq!(&ad[74..82], &seq.to_be_bytes(), "seq mismatch");
        assert_eq!(&ad[82..102], &sender_userid, "sender_userid mismatch");
    }

    #[test]
    fn test_build_ad_boundary_values() {
        let thread_id = [0xFFu8; 32];
        let epoch = u32::MAX;
        let cfg_hash = [0xFFu8; 32];
        let suite = ProtoSuite {
            major: u16::MAX,
            minor: u16::MAX,
            ciphersuite_id: u16::MAX,
        };
        let seq = u64::MAX;
        let sender_userid = [0xFFu8; 20];

        let ad = build_ad(&thread_id, epoch, &cfg_hash, suite, seq, &sender_userid);

        assert_eq!(ad.len(), 102);
        assert_eq!(&ad[32..36], &[0xFF, 0xFF, 0xFF, 0xFF]);
        assert_eq!(&ad[68..70], &[0xFF, 0xFF]);
        assert_eq!(
            &ad[74..82],
            &[0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]
        );
    }

    #[test]
    fn test_build_ad_against_tv4() {
        let thread_id =
            types::to_fixed::<32>(&hex::decode(test_vectors::TV4_THREAD_ID_HEX).unwrap()).unwrap();
        let cfg_hash =
            types::to_fixed::<32>(&hex::decode(test_vectors::TV4_CFG_HASH_SHA256_HEX).unwrap())
                .unwrap();
        let sender_userid =
            types::to_fixed::<20>(&hex::decode(test_vectors::TV4_SENDER_USERID_HEX).unwrap())
                .unwrap();

        let suite = ProtoSuite {
            major: test_vectors::TV4_PROTO_SUITE_MAJOR,
            minor: test_vectors::TV4_PROTO_SUITE_MINOR,
            ciphersuite_id: test_vectors::TV4_PROTO_SUITE_CIPHERSUITE_ID,
        };

        let got = build_ad(
            &thread_id,
            test_vectors::TV4_EPOCH,
            &cfg_hash,
            suite,
            test_vectors::TV4_SEQ,
            &sender_userid,
        );

        let expected_hex = test_vectors::TV4_AEAD_AD_HEX;
        assert_eq!(hex::encode(got), expected_hex);
    }
}
