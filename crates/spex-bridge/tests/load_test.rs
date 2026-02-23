use axum::{body::Body, http::{Request, StatusCode}};
use spex_bridge::{app, init_state};
use tempfile::tempdir;
use tower::ServiceExt;
use rusqlite::{params, Connection};

#[tokio::test]
async fn stress_test_concurrent_inbox_reads() {
    let tmp = tempdir().expect("tempdir");
    let db_path = tmp.path().join("bridge.db");

    // Initialize state (creates tables)
    let state = init_state(db_path.clone()).unwrap();
    let app = app(state);

    // Seed data
    {
        let conn = Connection::open(&db_path).unwrap();
        conn.execute("INSERT OR IGNORE INTO inbox_keys (inbox_key) VALUES (?1)", params!["stress-inbox"]).unwrap();
        for i in 0..10 {
            let item = format!("item-{}", i);
            conn.execute(
                "INSERT INTO inbox_items (inbox_key, item) VALUES (?1, ?2)",
                params!["stress-inbox", item.as_bytes()]
            ).unwrap();
        }
    }

    let mut handles = Vec::new();
    let concurrency = 50;

    for _ in 0..concurrency {
        let app_clone = app.clone();
        handles.push(tokio::spawn(async move {
            let response = app_clone.oneshot(
                Request::builder()
                    .uri("/inbox/stress-inbox")
                    .body(Body::empty())
                    .unwrap()
            ).await.expect("request failed");

            assert_eq!(response.status(), StatusCode::OK);
        }));
    }

    for handle in handles {
        handle.await.expect("task failed");
    }
}
