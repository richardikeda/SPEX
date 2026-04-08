// SPDX-License-Identifier: MPL-2.0
use rusqlite::{params, Connection};
use spex_bridge::{export_abuse_logs, init_state, AbuseLogFilter};

/// Inserts deterministic request log rows for abuse export tests.
fn seed_logs(db_path: &std::path::Path) {
    let conn = Connection::open(db_path).expect("open db");
    conn.execute(
        "INSERT INTO request_logs (timestamp, identity, ip, slot_id, bytes, request_kind, outcome) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![100_i64, "id-a", "10.0.0.0", Option::<String>::None, 20_i64, "card", "accepted"],
    )
    .expect("insert row 1");
    conn.execute(
        "INSERT INTO request_logs (timestamp, identity, ip, slot_id, bytes, request_kind, outcome) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![200_i64, "id-b", "10.0.1.0", Option::<String>::None, 40_i64, "inbox", "rejected"],
    )
    .expect("insert row 2");
}

/// Verifies that abuse export applies filters and emits a stable minimized payload.
#[test]
fn test_export_abuse_logs_filters() {
    let temp_dir = tempfile::tempdir().expect("temp dir");
    let db_path = temp_dir.path().join("bridge.db");
    let _state = init_state(&db_path).expect("init state");
    seed_logs(&db_path);

    let filtered = export_abuse_logs(
        &db_path,
        &AbuseLogFilter {
            request_kind: Some("inbox".to_string()),
            outcome: Some("rejected".to_string()),
            since_timestamp: Some(150),
            until_timestamp: Some(250),
            limit: Some(1),
            ..AbuseLogFilter::default()
        },
    )
    .expect("export filtered logs");

    assert_eq!(filtered.len(), 1);
    let record = &filtered[0];
    assert_eq!(record.timestamp, 200);
    assert_eq!(record.request_kind, "inbox");
    assert_eq!(record.outcome, "rejected");
    assert_eq!(record.ip_prefix, "10.0.1.0");
    assert_eq!(record.bytes, 40);
    assert_ne!(record.identity_hash_hex, "id-b");
}
