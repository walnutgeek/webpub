# webpub Design

Static website publishing tool with content-based deduplication.

## Overview

**webpub** syncs static websites to a server with CDC (content-defined chunking) deduplication. Only changed chunks are transferred. Deployments are atomic with instant rollback support.

Secondary use case: create standalone `.webpub` archives with built-in deduplication.

## High-Level Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                       webpub CLI                            │
│  push | serve | rollback | list | archive | extract | gc    │
└─────────────────────────────────────────────────────────────┘
                              │
┌─────────────────────────────────────────────────────────────┐
│                      Core Library                           │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌─────────────┐ │
│  │ Scanner  │  │ Chunker  │  │  Merkle  │  │ Serializer  │ │
│  │(file tree)│  │(fastcdc) │  │  Tree    │  │ (msgpack)   │ │
│  └──────────┘  └──────────┘  └──────────┘  └─────────────┘ │
└─────────────────────────────────────────────────────────────┘
                              │
        ┌─────────────────────┴─────────────────────┐
        ▼                                           ▼
┌───────────────────┐                   ┌───────────────────┐
│   Sync Backend    │                   │  Archive Backend  │
│   (WebSocket)     │                   │  (single file)    │
└───────────────────┘                   └───────────────────┘
```

**Data flow:**
1. Scanner walks directory, yields files/dirs with metadata (name, permissions)
2. Chunker splits files via fastcdc, produces chunks with BLAKE3 hashes
3. Merkle tree builder creates hierarchical structure (children sorted by name)
4. Backend writes to destination (server via WebSocket, or archive file)

## Data Structures

### Chunk

```rust
struct Chunk {
    hash: [u8; 32],  // BLAKE3 hash
    data: Vec<u8>,   // raw content
}
```

### Merkle Tree Nodes

```rust
enum Node {
    File {
        name: String,
        permissions: u32,
        size: u64,
        chunks: Vec<[u8; 32]>,  // ordered chunk hashes
        hash: [u8; 32],         // BLAKE3(chunks concatenated)
    },
    Directory {
        name: String,
        permissions: u32,
        children: Vec<Node>,    // sorted by name
        hash: [u8; 32],         // BLAKE3(children's (name, perm, hash) tuples)
    },
}
```

**Determinism:** Children sorted by name ensures identical content produces identical hashes.

### Snapshot

```rust
struct Snapshot {
    id: u64,
    hostname: String,
    root_hash: [u8; 32],
    tree: Node,
    created_at: u64,
}
```

## Sync Protocol (WebSocket)

Binary msgpack-encoded messages over WebSocket.

### Connection Flow

1. Client connects to `ws://server:port/sync`
2. Client sends `Auth { token }`
3. Server responds `AuthOk` or closes connection

### Sync Flow

```
Client                              Server
   │                                   │
   │─── Auth { token } ───────────────>│
   │<── AuthOk ────────────────────────│
   │                                   │
   │─── HaveChunks { hashes: [...] } ─>│  (batch of chunk hashes)
   │<── NeedChunks { hashes: [...] } ──│  (which ones server wants)
   │                                   │
   │─── ChunkData { hash, data } ─────>│  (repeat for each needed)
   │<── ChunkAck { hash } ─────────────│
   │        ...                        │
   │                                   │
   │─── CommitTree { host, tree } ────>│  (full merkle tree)
   │<── CommitOk { snapshot_id } ──────│  (atomic flip done)
   │                                   │
```

- `HaveChunks` sent in batches as client scans (streaming)
- Server streams `NeedChunks` responses
- Client sends chunk data only for needed chunks
- `CommitTree` includes full tree; server verifies all chunks exist before accepting

## HTTP Serving

Server runs two listeners:
- WebSocket for sync (e.g., `--sync-port 9000`)
- HTTP for serving websites (e.g., `--http-port 8080`)

### Request Handling

```
GET /path/to/file (Host: example.com)
    │
    ▼
┌─────────────────────────────┐
│ Look up hostname's current  │
│ snapshot in index.db        │
└─────────────────────────────┘
    │
    ▼
┌─────────────────────────────┐
│ Look up path in snapshot's  │
│ merkle tree                 │
└─────────────────────────────┘
    │
    ▼ (found File node)
┌─────────────────────────────┐
│ Read chunks by hash from    │
│ chunk DBs, concatenate      │
└─────────────────────────────┘
    │
    ▼
┌─────────────────────────────┐
│ Return with Content-Type    │
│ (guess from extension)      │
└─────────────────────────────┘
```

- **Directory requests:** `GET /foo/` looks for `index.html`
- **404:** Path not in tree
- **Atomicity:** Snapshot pointer swapped atomically; no partial states visible

## Archive Format

Single `.webpub` file for standalone archives.

```
┌────────────────────────────────┐
│ Header (fixed size)            │
│ - magic: "WEBPUB\0\0" (8 bytes)│
│ - version: u8                  │
│ - index_offset: u64            │
│ - index_size: u64              │
├────────────────────────────────┤
│ Chunks (variable)              │
│ - chunk 1 data                 │
│ - chunk 2 data                 │
│ - ...                          │
├────────────────────────────────┤
│ Index (msgpack)                │
│ - tree: Node (full merkle)     │
│ - chunk_offsets: Map<hash,     │
│     (offset, size)>            │
└────────────────────────────────┘
```

**Writing:**
1. Write header with placeholder offsets
2. Stream chunks, track offsets in memory
3. Write msgpack index at end
4. Seek back, update header with final offsets

**Reading:**
1. Read header, seek to index
2. Deserialize tree and chunk map
3. Walk tree, read chunks by offset, write files

**Deduplication:** Within archive, identical chunks stored once.

## Server Storage

```
./data/
├── chunks/
│   ├── 00.db    (chunks where hash starts with 00)
│   ├── 01.db    (chunks where hash starts with 01)
│   ├── ...
│   └── ff.db    (256 databases total)
└── index.db     (sites, snapshots, tokens)
```

### Chunk Databases

Each `XX.db` (where XX is first two hex chars of hash):

```sql
CREATE TABLE chunks (
    hash BLOB PRIMARY KEY,  -- 32 bytes
    data BLOB NOT NULL
);
```

### Index Database

```sql
CREATE TABLE sites (
    hostname TEXT PRIMARY KEY
);

CREATE TABLE snapshots (
    id INTEGER PRIMARY KEY,
    hostname TEXT NOT NULL,
    root_hash BLOB NOT NULL,
    tree BLOB NOT NULL,        -- msgpack-encoded Node
    created_at INTEGER NOT NULL,
    is_current BOOLEAN DEFAULT FALSE,
    FOREIGN KEY (hostname) REFERENCES sites(hostname)
);

CREATE TABLE tokens (
    token TEXT PRIMARY KEY,
    created_at INTEGER NOT NULL
);
```

**Atomic flip:** Single transaction: insert snapshot, update `is_current` flags.

**GC:** Load all snapshots, collect referenced hashes, delete unreferenced chunks from all DBs.

## CLI

```
webpub push <dir> <server-url> --host <hostname>
    Push directory to server for specific hostname.
    Reads WEBPUB_TOKEN from env.
    Example: webpub push ./dist ws://deploy.example.com:9000 --host blog.example.com

webpub serve --http-port 8080 --sync-port 9000 --data ./data --keep 5
    Run server. Routes HTTP requests by Host header.
    --keep N: snapshots to retain per site (default: 5)

webpub rollback <server-url> --host <hostname> [--to <snapshot-id>]
    Rollback site to previous snapshot (or specific ID).

webpub list <server-url> --host <hostname>
    List snapshots for a site.

webpub archive <dir> <output.webpub>
    Create archive from directory.

webpub extract <archive.webpub> <output-dir>
    Extract archive to directory.

webpub gc --data ./data
    Remove unreferenced chunks across all sites.

webpub token add --data ./data
    Generate new token, print to stdout.

webpub token list --data ./data
    List tokens (partial hash for identification).

webpub token revoke <token> --data ./data
    Remove token.
```

## Authentication

- **Client:** `WEBPUB_TOKEN` environment variable
- **Server:** `tokens.txt` in data directory, one token per line

Tokens are global (not per-site).

## File Handling

- **Empty directories:** Preserved
- **Symlinks:** Ignored
- **Special files:** Ignored (devices, sockets, etc.)

## Dependencies

```toml
[dependencies]
fastcdc = "3"
blake3 = "1"
rmp-serde = "1"
serde = { version = "1", features = ["derive"] }
rusqlite = { version = "0.31", features = ["bundled"] }
tokio = { version = "1", features = ["full"] }
tokio-tungstenite = "0.21"
axum = "0.7"
clap = { version = "4", features = ["derive"] }
```

## Module Structure

```
src/
├── main.rs           # CLI entry point
├── lib.rs            # public library API
├── scanner.rs        # directory walking
├── chunker.rs        # fastcdc wrapper
├── merkle.rs         # tree building & hashing
├── protocol.rs       # sync message types
├── client/
│   ├── mod.rs
│   ├── push.rs       # sync to server
│   └── archive.rs    # write .webpub file
├── server/
│   ├── mod.rs
│   ├── sync.rs       # WebSocket handler
│   ├── http.rs       # static file serving
│   └── storage.rs    # SQLite operations
└── extract.rs        # read .webpub file
```

## Design Principles

- **Simplicity over performance:** Reassemble files on every request; optimize later if needed
- **Correctness first:** Atomic operations, no partial states
- **No encryption:** Public websites don't need it (archive encryption out of scope for now)
- **No resume logic:** Re-run sync; server has partial chunks, client re-scans and sends missing
