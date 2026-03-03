use rusqlite::Connection;
use tempfile::tempdir;
use std::time::{Instant, Duration};

#[test]
fn test_connection_open_performance() {
    let tmp = tempdir().expect("tempdir");
    let db_path = tmp.path().join("bench.db");

    // Create the DB
    Connection::open(&db_path).unwrap();

    let iterations = 100;
    println!("\n--- Performance Test ({} iterations) ---", iterations);

    let start = Instant::now();
    for _ in 0..iterations {
        let _conn = Connection::open(&db_path).unwrap();
    }
    let duration = start.elapsed();
    println!("Connection::open baseline: {:?} total ({:?} per open)",
        duration, duration / iterations);

    // Simple pool logic simulation
    let mut pool = Vec::new();
    for _ in 0..10 {
        pool.push(Connection::open(&db_path).unwrap());
    }

    let start = Instant::now();
    for _ in 0..iterations {
        let conn = pool.pop().unwrap_or_else(|| Connection::open(&db_path).unwrap());
        // simulate use
        let _ = &conn;
        pool.push(conn);
    }
    let duration_pool = start.elapsed();
    println!("Simulated pool: {:?} total ({:?} per acquire/release)",
        duration_pool, duration_pool / iterations);

    if duration_pool > Duration::ZERO {
        println!("Estimated improvement: {:.2}x", duration.as_secs_f64() / duration_pool.as_secs_f64());
    }
}
