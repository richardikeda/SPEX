// SPDX-License-Identifier: MPL-2.0
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use rusqlite::Connection;
use std::path::PathBuf;
use tempfile::tempdir;

fn open_connection(path: &PathBuf) {
    let _conn = Connection::open(path).unwrap();
}

fn criterion_benchmark(c: &mut Criterion) {
    let tmp = tempdir().unwrap();
    let db_path = tmp.path().join("bench.db");

    // Create the DB
    Connection::open(&db_path).unwrap();

    c.bench_function("open_connection", |b| {
        b.iter(|| open_connection(black_box(&db_path)))
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
