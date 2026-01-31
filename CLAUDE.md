# CLAUDE.md

Instructions for Claude when working on this project.

## Project Overview

**webpub** is a static website publishing tool with content-based deduplication. It has two main use cases:

1. **Server sync**: Push website files to a server with CDC chunking, atomic deployment switches, and rollback capability
2. **Archive format**: Create standalone `.webpub` archive files with built-in deduplication

## Architecture

```
src/
├── main.rs           # CLI entry point (clap)
├── lib.rs            # Public library API
├── chunker.rs        # CDC chunking with fastcdc + BLAKE3
├── scanner.rs        # Directory walking
├── merkle.rs         # Node type and tree building
├── archive.rs        # .webpub file format read/write
├── protocol.rs       # WebSocket message types
├── client/
│   ├── push.rs       # Push to server
│   ├── list.rs       # List snapshots
│   └── rollback.rs   # Rollback to snapshot
└── server/
    ├── storage.rs    # SQLite storage (sharded chunks + index)
    ├── http.rs       # Static file serving via axum
    └── sync.rs       # WebSocket sync handler
```

## Key Design Decisions

- **Chunking**: fastcdc (16KB min, 32KB avg, 64KB max) with BLAKE3 hashing
- **Merkle tree**: Hierarchical, mirrors directory structure, children sorted by name for determinism
- **Storage**: 256 sharded SQLite databases by first byte of chunk hash, plus index.db for snapshots/tokens
- **Protocol**: Binary msgpack over WebSocket
- **Serving**: Files reassembled from chunks on each request (correctness over performance)

## Commands

```bash
# Build
cargo build

# Run tests
cargo test

# Run integration test (spawns servers)
cargo test test_push_and_serve -- --ignored

# Check for issues
cargo clippy
```

## Testing

Tests are in `tests/` directory:
- `archive_tests.rs` - Archive read/write roundtrips
- `chunker_tests.rs` - CDC chunking behavior
- `scanner_tests.rs` - Directory walking
- `merkle_builder_tests.rs` - Tree construction
- `storage_tests.rs` - SQLite storage operations
- `protocol_tests.rs` - Message serialization
- `http_tests.rs` - Path lookup in merkle tree
- `cli_tests.rs` - CLI archive/extract flow
- `integration_test.rs` - Full push/serve flow (marked `#[ignore]`)

## Code Patterns

- Error handling: Use `thiserror` for custom errors, `Box<dyn Error>` for async boundaries
- Async: tokio runtime, futures-util for WebSocket streams
- Serialization: serde + rmp-serde (msgpack)
- CLI: clap with derive macros

## Important Files

- `docs/plans/2026-01-30-webpub-design.md` - Original design document
- `docs/plans/2026-01-30-webpub-implementation.md` - Implementation plan with all tasks

## TODOs

- `src/server/sync.rs`: cleanup_old_snapshots() is a placeholder
- `src/main.rs`: GC command is a placeholder
- Consider adding connection timeouts to client functions
- Consider extracting common auth logic in client modules
