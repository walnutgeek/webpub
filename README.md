# webpub

Static website publishing with content-based deduplication.

## Features

- **CDC deduplication**: Only changed chunks are transferred using content-defined chunking
- **Atomic deployments**: Snapshot pointer swapped atomically; no partial states visible
- **Instant rollback**: Revert to any previous snapshot instantly
- **Multi-site hosting**: Single server hosts multiple sites, routed by Host header
- **Archive format**: Create standalone `.webpub` files with built-in deduplication

## Installation

```bash
cargo install --path .
```

## Quick Start

### Archive Mode

Create and extract deduplicated archives:

```bash
# Create archive from directory
webpub archive ./my-site site.webpub

# Extract archive
webpub extract site.webpub ./output
```

### Server Mode

Run a webpub server:

```bash
# Initialize and start server
webpub serve --data ./data --http-port 8080 --sync-port 9000

# Generate auth token
webpub token add --data ./data
# Output: abc123...
```

### Client Mode

Deploy sites to a server:

```bash
# Set auth token
export WEBPUB_TOKEN=abc123...

# Push site
webpub push ./dist ws://server:9000 --host example.com

# List snapshots
webpub list ws://server:9000 --host example.com

# Rollback to previous
webpub rollback ws://server:9000 --host example.com

# Rollback to specific snapshot
webpub rollback ws://server:9000 --host example.com --to 3
```

## Commands

| Command | Description |
|---------|-------------|
| `archive <dir> <output>` | Create .webpub archive from directory |
| `extract <archive> <dir>` | Extract .webpub archive to directory |
| `serve` | Run server (HTTP + sync) |
| `push <dir> <url> --host <name>` | Deploy directory to server |
| `list <url> --host <name>` | List snapshots for a site |
| `rollback <url> --host <name>` | Rollback to previous snapshot |
| `token add\|list\|revoke` | Manage auth tokens |
| `gc` | Garbage collect unreferenced chunks |

## Server Options

```
webpub serve [OPTIONS]

Options:
  --http-port <PORT>    HTTP port for serving [default: 8080]
  --sync-port <PORT>    WebSocket port for sync [default: 9000]
  --data <PATH>         Data directory [default: ./data]
  --keep <N>            Snapshots to keep per site [default: 5]
```

## How It Works

1. **Scanning**: Client walks directory tree, reads file contents
2. **Chunking**: Files split into chunks using FastCDC (content-defined chunking)
3. **Hashing**: Each chunk hashed with BLAKE3
4. **Deduplication**: Client sends chunk hashes; server responds with which it needs
5. **Transfer**: Only missing chunks are sent
6. **Commit**: Full merkle tree sent; server verifies all chunks exist, creates snapshot
7. **Serving**: HTTP requests resolved via merkle tree, files reassembled from chunks

## Storage Layout

```
./data/
├── chunks/
│   ├── 00.db    # Chunks where hash starts with 00
│   ├── 01.db    # Chunks where hash starts with 01
│   └── ...      # 256 databases total
└── index.db     # Sites, snapshots, tokens
```

## Archive Format

```
┌────────────────────────────────┐
│ Header (25 bytes)              │
│ - magic: "WEBPUB\0\0"          │
│ - version: u8                  │
│ - index_offset: u64            │
│ - index_size: u64              │
├────────────────────────────────┤
│ Chunks (variable)              │
│ - chunk data concatenated      │
├────────────────────────────────┤
│ Index (msgpack)                │
│ - merkle tree                  │
│ - chunk offset map             │
└────────────────────────────────┘
```

## Development

```bash
# Build
cargo build

# Run tests
cargo test

# Run integration test
cargo test test_push_and_serve -- --ignored

# Check for issues
cargo clippy
```

## License

MIT
