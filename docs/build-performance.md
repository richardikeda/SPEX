# Rust Build Performance

## Protocol Alignment (Normative)

SPEX means **Secure Permissioned Exchange**.
SPEX is a **protocol**, not just an application.
Security comes before convenience.
Build optimizations must not remove validation, tests, supported targets, or
cryptographic invariants.

## Purpose

This document records the reproducible baseline for item 9 of `docs/p0.md` and
defines the supported developer workflow. Raw Cargo timing reports and `target/`
contents are intentionally not committed.

## Measurement Environment

Baseline date: 2026-07-26

Baseline source: `fdc569f6e60eee8942fbe0f734d26dccde3730db`

| Property | Value |
| --- | --- |
| OS | Windows 11 Pro 64-bit, version 10.0.26200 |
| CPU | Intel Core i7-10750H, 6 cores / 12 logical processors |
| Memory | 32,603 MiB visible physical memory |
| Filesystem | NTFS |
| Rust | `rustc 1.95.0 (59807616e 2026-04-14)` |
| Cargo | `cargo 1.95.0 (f2d3ce0bd 2026-03-21)` |
| Target | `x86_64-pc-windows-msvc` |
| Linker | MSVC linker selected by the Rust target |
| Profile | Cargo `dev` / `test`, project defaults |

The before and after measurements use separate empty directories under the
Windows temporary directory through `CARGO_TARGET_DIR`. They do not delete or
reuse the contributor's normal workspace cache.

## Before-and-After Results

| Scenario | Before | After | Change |
| --- | ---: | ---: | ---: |
| Clean workspace build | 202.542 s | 388.663 s | +91.9% |
| Warm no-change build | 1.057 s | 2.218 s | +109.8% |
| Core-only incremental edit | 11.285 s | 30.823 s | +173.1% |
| Client-only incremental edit | 8.515 s | 20.089 s | +135.9% |
| CLI-only incremental edit | 6.327 s | 13.544 s | +114.1% |
| Default-feature test compilation | 158.728 s | 197.106 s | +24.2% |
| All-feature test compilation | 203.748 s | 271.268 s | +33.1% |
| Final isolated target size | 14,591,443,045 bytes | 13,172,864,490 bytes | -9.7% |
| Ubuntu stable Rust CI job | 539 s | 559 s | +3.7% |

The local wall-clock comparison is recorded for transparency but is not
accepted as evidence of a compile-time regression or improvement. Host CPU load
was 94-100% during the after run, compared with materially lower load at the
start of the before run. Background load and real-time file scanning were not
controlled. The deterministic result is the 1,418,578,555-byte reduction in
the final isolated target (9.7%). After default-feature test compilation alone,
the target fell from 8,630,469,440 to 7,644,446,770 bytes (11.4%), while
Criterion and Native TLS were absent from the compilation log. The hosted CI
comparison confirms that the redundant build step was removed, but its single
wall-clock sample does not demonstrate a net speedup: runner variance in the
default-feature test and cache-finalization steps outweighed that saved step.

The all-feature test value measures the transition after default-feature test
compilation in the same isolated target. Enabling the optional `blake3_hash`
feature causes a separate set of workspace artifacts, so it is intentionally
reported rather than treated as a duplicate.

The `main` CI evidence is
[run 30205588446](https://github.com/richardikeda/SPEX/actions/runs/30205588446):
the Ubuntu stable Rust job took 539 seconds, including a 13-second workspace
build, 250-second default-feature test step, and 255-second all-feature test
step. `Swatinem/rust-cache` reported a 534 MB cache hit.

The optimized PR evidence is
[run 30209196954](https://github.com/richardikeda/SPEX/actions/runs/30209196954):
the same job class took 559 seconds with the redundant workspace build skipped,
a 275-second default-feature test step, and a 254-second all-feature test step.
The cache again reported a 534 MB hit. The controlled workflow change therefore
saved the 13-second duplicate build by construction, while the total job was
20 seconds slower in this one sample. This is treated as runner noise rather
than evidence that dependency reduction regressed compilation. Future
optimization decisions require repeated comparable CI runs rather than this
single paired observation.

## Measurement Method

The clean and warm build commands are:

```powershell
$env:CARGO_TARGET_DIR = Join-Path $env:TEMP 'spex-build-before'
cargo build --workspace --locked --timings
cargo build --workspace --locked
```

The after run uses the same commands with `spex-build-after`. Both directories
were absent before their respective clean build.

Incremental measurements update only the modification time of the named
implementation file, build the workspace, and then restore the original
timestamp. No file content changes during measurement:

- `spex-core/src/lib.rs`
- `crates/spex-client/src/lib.rs`
- `crates/spex-cli/main.rs`

Test compilation is measured without executing tests:

```powershell
cargo test --workspace --locked --no-run
cargo test --workspace --locked --all-features --no-run
```

The raw HTML produced by `--timings` remains in the isolated temporary target
only. The summarized values above are the versioned evidence.

## Baseline Cost Centers

The before timing report identifies the following longest individual compile
units. Durations overlap because Cargo builds independent units in parallel:

| Unit | Duration |
| --- | ---: |
| `windows-sys` 0.61.2 | 42.56 s |
| Tokio 1.52.4 | 41.69 s |
| `futures-util` 0.3.32 | 38.28 s |
| ICU properties data 2.2.0 | 37.87 s |
| Syn 2.0.119 | 37.75 s |
| Ring 0.17.14 | 34.12 s |
| Regex automata 0.4.16 | 31.91 s |
| Rustls 0.23.42 | 29.55 s |
| `spex-transport` | 19.63 s |

The longest build-script executions were Ring at 14.21 seconds and bundled
SQLite (`libsqlite3-sys`) at 8.04 seconds. The CLI binary unit took 12.14
seconds and the bridge binary unit took 5.61 seconds. This evidence supports
continued attention to networking/TLS, platform bindings, procedural macros,
bundled SQLite, and link work; it does not justify replacing security
dependencies without a separate compatibility analysis.

## Developer Feedback Workflow

Use `cargo check` for the default edit feedback loop. Scope it to the package
being changed whenever downstream integration is not yet needed:

```bash
cargo check -p spex-core --locked
cargo check -p spex-client --locked
cargo check -p spex-cli --locked
```

Keep the same toolchain, target triple, feature set, `RUSTFLAGS`, and target
directory between comparable runs. Keep `Cargo.lock` committed and use
`--locked` for reproducible checks.

Run the full project gates before commit and whenever a change affects public
behavior, security logic, dependency manifests, feature selection, workflows,
or cross-crate integration:

```bash
cargo test --workspace --locked
cargo test --workspace --locked --all-features
cargo fmt --all -- --check
cargo clippy --workspace --locked --all-targets --all-features -- -D warnings
```

Package-scoped checks accelerate iteration; they do not replace the full
security and release gates.

## Troubleshooting Repeated Downloads or Full Rebuilds

If Cargo downloads or rebuilds everything on every run:

1. Confirm the toolchain and target triple did not change.
2. Confirm `RUSTFLAGS`, features, profiles, and environment variables are stable.
3. Confirm the same `target/` location is being used.
4. Check whether antivirus, cleanup tools, containers, or ephemeral runners are
   deleting or locking build artifacts.
5. Check timestamps and free disk space.
6. Use `cargo tree -d` and Cargo timings to identify dependency or feature
   changes before modifying profiles.

Project scripts must not run `cargo clean` automatically. A clean rebuild is a
deliberate diagnostic action only, preferably performed in a new isolated
`CARGO_TARGET_DIR`, because deleting the normal cache makes subsequent feedback
slower and destroys useful evidence.

## Dependency and Feature Findings

- CLI `reqwest` tests use only Rustls. Disabling default features removes the
  Native TLS, OpenSSL/Schannel, `hyper-tls`, and `tokio-native-tls` paths.
- Criterion is needed only by `spex-bridge` benchmarks. The `benchmarks` feature
  and the benchmark's `required-features` gate keep it out of ordinary tests.
- The CLI directly uses `libp2p::identity::Keypair` and `libp2p::Multiaddr`;
  removing the direct dependency now would only move those concrete types
  behind another public boundary and would not avoid compiling transport's
  libp2p graph.
- The enabled libp2p behaviours are used by the transport implementation:
  Gossipsub, Identify, Kademlia, Noise, TCP, Tokio, Yamux, and derive macros.
- `keyring` v4's compatibility feature selects the supported Windows, Apple,
  and Secret Service stores. Replacing that compatibility API with manually
  initialized target stores is security-sensitive and is deferred until
  platform-specific behavior tests exist.
- Fuzzing remains an independent workspace. Its lockfile is validated and
  audited separately from the main workspace.
- Compatible duplicates are reviewed with `cargo tree -d`; incompatible
  security or protocol APIs are not forced into one version solely to reduce
  compile counts.

## Deferred Experiments

Faster linkers, `sccache`, nextest, profile changes, and monomorphization
refactors remain measurement-driven follow-ups. They are not required defaults
until cross-platform results demonstrate a benefit without changing test or
release semantics.

Secure. Permissioned. Explicit.
