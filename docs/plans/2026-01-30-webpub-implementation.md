# webpub Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build a static website publishing tool with CDC deduplication, supporting both server sync and standalone archives.

**Architecture:** Core library handles scanning, chunking, and merkle tree building. Two backends: archive (single file) and server sync (WebSocket). Server stores chunks in sharded SQLite DBs, serves files by reassembling chunks on-the-fly.

**Tech Stack:** Rust, fastcdc, blake3, rmp-serde, rusqlite, tokio, tokio-tungstenite, axum, clap

---

## Milestone 1: Archive Format (archive + extract)

This milestone delivers a working `webpub archive` and `webpub extract` command pair.

---

### Task 1: Project Setup

**Files:**
- Create: `Cargo.toml`
- Create: `src/main.rs`
- Create: `src/lib.rs`

**Step 1: Initialize Cargo project**

```bash
cargo init --name webpub
```

**Step 2: Set up Cargo.toml with dependencies**

Replace `Cargo.toml` with:

```toml
[package]
name = "webpub"
version = "0.1.0"
edition = "2021"

[dependencies]
fastcdc = "3"
blake3 = "1"
rmp-serde = "1"
serde = { version = "1", features = ["derive"] }
rusqlite = { version = "0.31", features = ["bundled"] }
tokio = { version = "1", features = ["full"] }
tokio-tungstenite = "0.24"
axum = "0.7"
clap = { version = "4", features = ["derive"] }
thiserror = "1"
hex = "0.4"
mime_guess = "2"

[dev-dependencies]
tempfile = "3"
```

**Step 3: Set up minimal main.rs**

```rust
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "webpub")]
#[command(about = "Static website publishing with deduplication")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Create archive from directory
    Archive {
        /// Source directory
        dir: std::path::PathBuf,
        /// Output archive file
        output: std::path::PathBuf,
    },
    /// Extract archive to directory
    Extract {
        /// Archive file
        archive: std::path::PathBuf,
        /// Output directory
        output: std::path::PathBuf,
    },
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Archive { dir, output } => {
            println!("Archive {:?} -> {:?}", dir, output);
            todo!("archive command")
        }
        Commands::Extract { archive, output } => {
            println!("Extract {:?} -> {:?}", archive, output);
            todo!("extract command")
        }
    }
}
```

**Step 4: Set up lib.rs**

```rust
pub mod chunker;
pub mod merkle;
pub mod scanner;
pub mod archive;

pub use merkle::Node;
```

**Step 5: Create empty module files**

```bash
touch src/chunker.rs src/merkle.rs src/scanner.rs src/archive.rs
```

**Step 6: Verify it compiles**

Run: `cargo build`
Expected: Compiles with warnings about empty modules

**Step 7: Commit**

```bash
git add -A
git commit -m "feat: initialize project structure with CLI skeleton"
```

---

### Task 2: Core Types - Merkle Tree Node

**Files:**
- Create: `src/merkle.rs`
- Create: `tests/merkle_tests.rs`

**Step 1: Write test for Node serialization round-trip**

Create `tests/merkle_tests.rs`:

```rust
use webpub::Node;

#[test]
fn test_file_node_roundtrip() {
    let node = Node::File {
        name: "test.txt".to_string(),
        permissions: 0o644,
        size: 100,
        chunks: vec![[0u8; 32], [1u8; 32]],
        hash: [2u8; 32],
    };

    let bytes = rmp_serde::to_vec(&node).unwrap();
    let decoded: Node = rmp_serde::from_slice(&bytes).unwrap();

    assert_eq!(node, decoded);
}

#[test]
fn test_directory_node_roundtrip() {
    let child = Node::File {
        name: "child.txt".to_string(),
        permissions: 0o644,
        size: 50,
        chunks: vec![[3u8; 32]],
        hash: [4u8; 32],
    };

    let node = Node::Directory {
        name: "mydir".to_string(),
        permissions: 0o755,
        children: vec![child],
        hash: [5u8; 32],
    };

    let bytes = rmp_serde::to_vec(&node).unwrap();
    let decoded: Node = rmp_serde::from_slice(&bytes).unwrap();

    assert_eq!(node, decoded);
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test test_file_node_roundtrip`
Expected: FAIL - Node type doesn't exist

**Step 3: Implement Node type**

In `src/merkle.rs`:

```rust
use serde::{Deserialize, Serialize};

/// A node in the merkle tree representing a file or directory.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Node {
    File {
        name: String,
        permissions: u32,
        size: u64,
        chunks: Vec<[u8; 32]>,
        hash: [u8; 32],
    },
    Directory {
        name: String,
        permissions: u32,
        children: Vec<Node>,
        hash: [u8; 32],
    },
}

impl Node {
    pub fn name(&self) -> &str {
        match self {
            Node::File { name, .. } => name,
            Node::Directory { name, .. } => name,
        }
    }

    pub fn hash(&self) -> &[u8; 32] {
        match self {
            Node::File { hash, .. } => hash,
            Node::Directory { hash, .. } => hash,
        }
    }
}
```

**Step 4: Export from lib.rs**

Update `src/lib.rs`:

```rust
pub mod archive;
pub mod chunker;
pub mod merkle;
pub mod scanner;

pub use merkle::Node;
```

**Step 5: Run tests to verify they pass**

Run: `cargo test`
Expected: PASS

**Step 6: Commit**

```bash
git add -A
git commit -m "feat: add Node type for merkle tree"
```

---

### Task 3: Chunker Module

**Files:**
- Create: `src/chunker.rs`
- Create: `tests/chunker_tests.rs`

**Step 1: Write test for chunking**

Create `tests/chunker_tests.rs`:

```rust
use webpub::chunker::{chunk_data, Chunk};

#[test]
fn test_chunk_small_data() {
    let data = b"Hello, world!";
    let chunks: Vec<Chunk> = chunk_data(data).collect();

    // Small data should produce one chunk
    assert_eq!(chunks.len(), 1);
    assert_eq!(chunks[0].data, data);

    // Hash should be BLAKE3 of data
    let expected_hash = blake3::hash(data);
    assert_eq!(chunks[0].hash, *expected_hash.as_bytes());
}

#[test]
fn test_chunk_deterministic() {
    let data = b"Some test data that we chunk";
    let chunks1: Vec<Chunk> = chunk_data(data).collect();
    let chunks2: Vec<Chunk> = chunk_data(data).collect();

    assert_eq!(chunks1.len(), chunks2.len());
    for (c1, c2) in chunks1.iter().zip(chunks2.iter()) {
        assert_eq!(c1.hash, c2.hash);
    }
}

#[test]
fn test_chunk_large_data() {
    // Create data large enough to produce multiple chunks
    // fastcdc default min is 16KB, avg 32KB, max 64KB
    let data: Vec<u8> = (0..200_000).map(|i| (i % 256) as u8).collect();
    let chunks: Vec<Chunk> = chunk_data(&data).collect();

    // Should produce multiple chunks
    assert!(chunks.len() > 1);

    // Reconstruct and verify
    let reconstructed: Vec<u8> = chunks.iter().flat_map(|c| c.data.iter().copied()).collect();
    assert_eq!(reconstructed, data);
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test test_chunk_small_data`
Expected: FAIL - chunker module empty

**Step 3: Implement chunker**

In `src/chunker.rs`:

```rust
use fastcdc::v2020::FastCDC;

/// A content-addressed chunk of data.
#[derive(Debug, Clone)]
pub struct Chunk {
    pub hash: [u8; 32],
    pub data: Vec<u8>,
}

/// Chunk sizes: min 16KB, avg 32KB, max 64KB
const MIN_SIZE: u32 = 16 * 1024;
const AVG_SIZE: u32 = 32 * 1024;
const MAX_SIZE: u32 = 64 * 1024;

/// Chunk data using FastCDC algorithm, yielding chunks with BLAKE3 hashes.
pub fn chunk_data(data: &[u8]) -> impl Iterator<Item = Chunk> + '_ {
    let chunker = FastCDC::new(data, MIN_SIZE, AVG_SIZE, MAX_SIZE);

    chunker.map(|chunk| {
        let chunk_data = data[chunk.offset..chunk.offset + chunk.length].to_vec();
        let hash = *blake3::hash(&chunk_data).as_bytes();
        Chunk {
            hash,
            data: chunk_data,
        }
    })
}
```

**Step 4: Export from lib.rs**

Already exported via `pub mod chunker;`

**Step 5: Run tests to verify they pass**

Run: `cargo test chunker`
Expected: PASS

**Step 6: Commit**

```bash
git add -A
git commit -m "feat: add chunker module with fastcdc and blake3"
```

---

### Task 4: Scanner Module

**Files:**
- Create: `src/scanner.rs`
- Create: `tests/scanner_tests.rs`

**Step 1: Write test for directory scanning**

Create `tests/scanner_tests.rs`:

```rust
use std::fs;
use tempfile::TempDir;
use webpub::scanner::{scan_directory, ScannedEntry};

#[test]
fn test_scan_empty_directory() {
    let temp = TempDir::new().unwrap();
    let entries: Vec<ScannedEntry> = scan_directory(temp.path()).unwrap().collect();

    // Root directory only
    assert_eq!(entries.len(), 1);
    match &entries[0] {
        ScannedEntry::Directory { name, .. } => {
            assert_eq!(name, "");  // root has empty name
        }
        _ => panic!("Expected directory"),
    }
}

#[test]
fn test_scan_with_files() {
    let temp = TempDir::new().unwrap();
    fs::write(temp.path().join("a.txt"), "hello").unwrap();
    fs::write(temp.path().join("b.txt"), "world").unwrap();

    let entries: Vec<ScannedEntry> = scan_directory(temp.path()).unwrap().collect();

    // Root + 2 files
    assert_eq!(entries.len(), 3);
}

#[test]
fn test_scan_nested_directories() {
    let temp = TempDir::new().unwrap();
    fs::create_dir(temp.path().join("subdir")).unwrap();
    fs::write(temp.path().join("subdir/file.txt"), "content").unwrap();

    let entries: Vec<ScannedEntry> = scan_directory(temp.path()).unwrap().collect();

    // Root + subdir + file
    assert_eq!(entries.len(), 3);
}

#[test]
fn test_scan_sorted_by_name() {
    let temp = TempDir::new().unwrap();
    fs::write(temp.path().join("z.txt"), "z").unwrap();
    fs::write(temp.path().join("a.txt"), "a").unwrap();
    fs::write(temp.path().join("m.txt"), "m").unwrap();

    let entries: Vec<ScannedEntry> = scan_directory(temp.path()).unwrap().collect();

    // Skip root, check file order
    let names: Vec<&str> = entries.iter().skip(1).map(|e| e.name()).collect();
    assert_eq!(names, vec!["a.txt", "m.txt", "z.txt"]);
}

#[test]
fn test_scan_ignores_symlinks() {
    let temp = TempDir::new().unwrap();
    fs::write(temp.path().join("real.txt"), "content").unwrap();

    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(
            temp.path().join("real.txt"),
            temp.path().join("link.txt"),
        ).unwrap();
    }

    let entries: Vec<ScannedEntry> = scan_directory(temp.path()).unwrap().collect();

    // Root + real file only (symlink ignored)
    #[cfg(unix)]
    assert_eq!(entries.len(), 2);
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test test_scan_empty`
Expected: FAIL - scanner module empty

**Step 3: Implement scanner**

In `src/scanner.rs`:

```rust
use std::fs;
use std::io;
use std::path::Path;

/// A scanned filesystem entry.
#[derive(Debug)]
pub enum ScannedEntry {
    File {
        name: String,
        permissions: u32,
        size: u64,
        data: Vec<u8>,
    },
    Directory {
        name: String,
        permissions: u32,
        children: Vec<ScannedEntry>,
    },
}

impl ScannedEntry {
    pub fn name(&self) -> &str {
        match self {
            ScannedEntry::File { name, .. } => name,
            ScannedEntry::Directory { name, .. } => name,
        }
    }
}

/// Scan a directory recursively, returning entries sorted by name.
/// Ignores symlinks and special files.
pub fn scan_directory(path: &Path) -> io::Result<impl Iterator<Item = ScannedEntry>> {
    let entry = scan_entry(path, "")?;
    Ok(std::iter::once(entry))
}

fn scan_entry(path: &Path, name: &str) -> io::Result<ScannedEntry> {
    let metadata = fs::metadata(path)?;

    #[cfg(unix)]
    let permissions = {
        use std::os::unix::fs::PermissionsExt;
        metadata.permissions().mode()
    };
    #[cfg(not(unix))]
    let permissions = if metadata.permissions().readonly() { 0o444 } else { 0o644 };

    if metadata.is_file() {
        let data = fs::read(path)?;
        Ok(ScannedEntry::File {
            name: name.to_string(),
            permissions,
            size: metadata.len(),
            data,
        })
    } else if metadata.is_dir() {
        let mut children = Vec::new();

        for entry in fs::read_dir(path)? {
            let entry = entry?;
            let file_type = entry.file_type()?;

            // Skip symlinks and special files
            if file_type.is_symlink() {
                continue;
            }

            let child_name = entry.file_name().to_string_lossy().to_string();
            let child_path = entry.path();

            // Skip if we can't read metadata (broken symlink, permission denied, etc.)
            if let Ok(child_entry) = scan_entry(&child_path, &child_name) {
                children.push(child_entry);
            }
        }

        // Sort by name for determinism
        children.sort_by(|a, b| a.name().cmp(b.name()));

        Ok(ScannedEntry::Directory {
            name: name.to_string(),
            permissions,
            children,
        })
    } else {
        // Special file - treat as empty directory to skip
        Err(io::Error::new(io::ErrorKind::Other, "special file"))
    }
}
```

**Step 4: Run tests to verify they pass**

Run: `cargo test scanner`
Expected: PASS

**Step 5: Commit**

```bash
git add -A
git commit -m "feat: add scanner module for directory walking"
```

---

### Task 5: Merkle Tree Builder

**Files:**
- Modify: `src/merkle.rs`
- Create: `tests/merkle_builder_tests.rs`

**Step 1: Write test for building merkle tree**

Create `tests/merkle_builder_tests.rs`:

```rust
use std::fs;
use tempfile::TempDir;
use webpub::merkle::build_tree;
use webpub::scanner::scan_directory;
use webpub::Node;

#[test]
fn test_build_tree_single_file() {
    let temp = TempDir::new().unwrap();
    fs::write(temp.path().join("test.txt"), "hello").unwrap();

    let entry = scan_directory(temp.path()).unwrap().next().unwrap();
    let (tree, chunks) = build_tree(entry);

    // Root should be a directory
    match &tree {
        Node::Directory { children, .. } => {
            assert_eq!(children.len(), 1);
            match &children[0] {
                Node::File { name, size, .. } => {
                    assert_eq!(name, "test.txt");
                    assert_eq!(*size, 5);
                }
                _ => panic!("Expected file"),
            }
        }
        _ => panic!("Expected directory"),
    }

    // Should have one chunk for "hello"
    assert_eq!(chunks.len(), 1);
}

#[test]
fn test_build_tree_deterministic() {
    let temp = TempDir::new().unwrap();
    fs::write(temp.path().join("a.txt"), "aaa").unwrap();
    fs::write(temp.path().join("b.txt"), "bbb").unwrap();

    let entry1 = scan_directory(temp.path()).unwrap().next().unwrap();
    let (tree1, _) = build_tree(entry1);

    let entry2 = scan_directory(temp.path()).unwrap().next().unwrap();
    let (tree2, _) = build_tree(entry2);

    // Same content should produce same hash
    assert_eq!(tree1.hash(), tree2.hash());
}

#[test]
fn test_build_tree_empty_dir_preserved() {
    let temp = TempDir::new().unwrap();
    fs::create_dir(temp.path().join("empty")).unwrap();

    let entry = scan_directory(temp.path()).unwrap().next().unwrap();
    let (tree, chunks) = build_tree(entry);

    match &tree {
        Node::Directory { children, .. } => {
            assert_eq!(children.len(), 1);
            match &children[0] {
                Node::Directory { name, children: subchildren, .. } => {
                    assert_eq!(name, "empty");
                    assert!(subchildren.is_empty());
                }
                _ => panic!("Expected empty directory"),
            }
        }
        _ => panic!("Expected directory"),
    }

    // No chunks for empty directory
    assert!(chunks.is_empty());
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test test_build_tree`
Expected: FAIL - build_tree doesn't exist

**Step 3: Implement build_tree**

Add to `src/merkle.rs`:

```rust
use crate::chunker::{chunk_data, Chunk};
use crate::scanner::ScannedEntry;

/// Build a merkle tree from a scanned entry, returning the tree and all chunks.
pub fn build_tree(entry: ScannedEntry) -> (Node, Vec<Chunk>) {
    let mut all_chunks = Vec::new();
    let node = build_node(entry, &mut all_chunks);
    (node, all_chunks)
}

fn build_node(entry: ScannedEntry, all_chunks: &mut Vec<Chunk>) -> Node {
    match entry {
        ScannedEntry::File { name, permissions, size, data } => {
            let chunks: Vec<Chunk> = chunk_data(&data).collect();
            let chunk_hashes: Vec<[u8; 32]> = chunks.iter().map(|c| c.hash).collect();

            // File hash = BLAKE3(concatenated chunk hashes)
            let mut hasher = blake3::Hasher::new();
            for hash in &chunk_hashes {
                hasher.update(hash);
            }
            let hash = *hasher.finalize().as_bytes();

            all_chunks.extend(chunks);

            Node::File {
                name,
                permissions,
                size,
                chunks: chunk_hashes,
                hash,
            }
        }
        ScannedEntry::Directory { name, permissions, children } => {
            let child_nodes: Vec<Node> = children
                .into_iter()
                .map(|c| build_node(c, all_chunks))
                .collect();

            // Directory hash = BLAKE3(sorted children's (name, permissions, hash) tuples)
            let mut hasher = blake3::Hasher::new();
            for child in &child_nodes {
                hasher.update(child.name().as_bytes());
                hasher.update(&match child {
                    Node::File { permissions, .. } => permissions.to_le_bytes(),
                    Node::Directory { permissions, .. } => permissions.to_le_bytes(),
                });
                hasher.update(child.hash());
            }
            let hash = *hasher.finalize().as_bytes();

            Node::Directory {
                name,
                permissions,
                children: child_nodes,
                hash,
            }
        }
    }
}
```

**Step 4: Update lib.rs exports**

```rust
pub mod archive;
pub mod chunker;
pub mod merkle;
pub mod scanner;

pub use chunker::Chunk;
pub use merkle::{build_tree, Node};
pub use scanner::{scan_directory, ScannedEntry};
```

**Step 5: Run tests to verify they pass**

Run: `cargo test merkle`
Expected: PASS

**Step 6: Commit**

```bash
git add -A
git commit -m "feat: add merkle tree builder"
```

---

### Task 6: Archive Writer

**Files:**
- Create: `src/archive.rs`
- Create: `tests/archive_tests.rs`

**Step 1: Write test for archive creation**

Create `tests/archive_tests.rs`:

```rust
use std::fs;
use std::io::{Read, Seek, SeekFrom};
use tempfile::TempDir;
use webpub::archive::{write_archive, MAGIC};
use webpub::merkle::build_tree;
use webpub::scanner::scan_directory;

#[test]
fn test_write_archive_magic() {
    let temp = TempDir::new().unwrap();
    fs::write(temp.path().join("test.txt"), "hello").unwrap();

    let archive_path = temp.path().join("test.webpub");

    let entry = scan_directory(temp.path()).unwrap().next().unwrap();
    let (tree, chunks) = build_tree(entry);

    write_archive(&archive_path, &tree, &chunks).unwrap();

    // Verify magic bytes
    let mut file = fs::File::open(&archive_path).unwrap();
    let mut magic = [0u8; 8];
    file.read_exact(&mut magic).unwrap();
    assert_eq!(&magic, MAGIC);
}

#[test]
fn test_write_archive_version() {
    let temp = TempDir::new().unwrap();
    fs::write(temp.path().join("test.txt"), "hello").unwrap();

    let archive_path = temp.path().join("test.webpub");

    let entry = scan_directory(temp.path()).unwrap().next().unwrap();
    let (tree, chunks) = build_tree(entry);

    write_archive(&archive_path, &tree, &chunks).unwrap();

    // Verify version byte
    let mut file = fs::File::open(&archive_path).unwrap();
    file.seek(SeekFrom::Start(8)).unwrap();
    let mut version = [0u8; 1];
    file.read_exact(&mut version).unwrap();
    assert_eq!(version[0], 1);
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test test_write_archive`
Expected: FAIL - archive module empty

**Step 3: Implement archive writer**

In `src/archive.rs`:

```rust
use crate::chunker::Chunk;
use crate::merkle::Node;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::{self, BufWriter, Read, Seek, SeekFrom, Write};
use std::path::Path;

pub const MAGIC: &[u8; 8] = b"WEBPUB\0\0";
pub const VERSION: u8 = 1;

/// Header size: magic (8) + version (1) + index_offset (8) + index_size (8) = 25 bytes
const HEADER_SIZE: u64 = 25;

/// Archive index stored at the end of the file.
#[derive(Serialize, Deserialize)]
pub struct ArchiveIndex {
    pub tree: Node,
    pub chunk_offsets: HashMap<[u8; 32], (u64, u64)>, // hash -> (offset, size)
}

/// Write an archive file.
pub fn write_archive(path: &Path, tree: &Node, chunks: &[Chunk]) -> io::Result<()> {
    let file = File::create(path)?;
    let mut writer = BufWriter::new(file);

    // Write placeholder header
    writer.write_all(MAGIC)?;
    writer.write_all(&[VERSION])?;
    writer.write_all(&[0u8; 16])?; // placeholder for index_offset and index_size

    // Write chunks, tracking offsets (deduplicate by hash)
    let mut chunk_offsets: HashMap<[u8; 32], (u64, u64)> = HashMap::new();
    let mut offset = HEADER_SIZE;

    for chunk in chunks {
        if chunk_offsets.contains_key(&chunk.hash) {
            continue; // Skip duplicate
        }

        writer.write_all(&chunk.data)?;
        chunk_offsets.insert(chunk.hash, (offset, chunk.data.len() as u64));
        offset += chunk.data.len() as u64;
    }

    // Write index
    let index = ArchiveIndex {
        tree: tree.clone(),
        chunk_offsets,
    };
    let index_bytes = rmp_serde::to_vec(&index)
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
    let index_offset = offset;
    let index_size = index_bytes.len() as u64;
    writer.write_all(&index_bytes)?;

    // Seek back and write actual header
    writer.flush()?;
    let mut file = writer.into_inner()?;
    file.seek(SeekFrom::Start(9))?; // After magic + version
    file.write_all(&index_offset.to_le_bytes())?;
    file.write_all(&index_size.to_le_bytes())?;

    Ok(())
}
```

**Step 4: Run tests to verify they pass**

Run: `cargo test archive`
Expected: PASS

**Step 5: Commit**

```bash
git add -A
git commit -m "feat: add archive writer"
```

---

### Task 7: Archive Reader (Extract)

**Files:**
- Modify: `src/archive.rs`
- Add to: `tests/archive_tests.rs`

**Step 1: Write test for archive extraction**

Add to `tests/archive_tests.rs`:

```rust
use webpub::archive::read_archive;

#[test]
fn test_roundtrip_single_file() {
    let temp = TempDir::new().unwrap();
    let content = b"Hello, world!";
    fs::write(temp.path().join("test.txt"), content).unwrap();

    let archive_path = temp.path().join("test.webpub");
    let extract_path = temp.path().join("extracted");

    // Create archive
    let entry = scan_directory(temp.path()).unwrap().next().unwrap();
    let (tree, chunks) = build_tree(entry);
    write_archive(&archive_path, &tree, &chunks).unwrap();

    // Extract
    read_archive(&archive_path, &extract_path).unwrap();

    // Verify
    let extracted = fs::read(extract_path.join("test.txt")).unwrap();
    assert_eq!(extracted, content);
}

#[test]
fn test_roundtrip_nested_structure() {
    let temp = TempDir::new().unwrap();
    fs::create_dir(temp.path().join("subdir")).unwrap();
    fs::write(temp.path().join("root.txt"), "root").unwrap();
    fs::write(temp.path().join("subdir/nested.txt"), "nested").unwrap();

    let archive_path = temp.path().join("test.webpub");
    let extract_path = temp.path().join("extracted");

    // Create archive
    let entry = scan_directory(temp.path()).unwrap().next().unwrap();
    let (tree, chunks) = build_tree(entry);
    write_archive(&archive_path, &tree, &chunks).unwrap();

    // Extract
    read_archive(&archive_path, &extract_path).unwrap();

    // Verify
    assert_eq!(fs::read_to_string(extract_path.join("root.txt")).unwrap(), "root");
    assert_eq!(fs::read_to_string(extract_path.join("subdir/nested.txt")).unwrap(), "nested");
}

#[test]
fn test_roundtrip_empty_directory() {
    let temp = TempDir::new().unwrap();
    fs::create_dir(temp.path().join("empty")).unwrap();

    let archive_path = temp.path().join("test.webpub");
    let extract_path = temp.path().join("extracted");

    // Create archive
    let entry = scan_directory(temp.path()).unwrap().next().unwrap();
    let (tree, chunks) = build_tree(entry);
    write_archive(&archive_path, &tree, &chunks).unwrap();

    // Extract
    read_archive(&archive_path, &extract_path).unwrap();

    // Verify empty dir exists
    assert!(extract_path.join("empty").is_dir());
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test test_roundtrip`
Expected: FAIL - read_archive doesn't exist

**Step 3: Implement archive reader**

Add to `src/archive.rs`:

```rust
use std::fs;
use std::io::BufReader;

/// Read and extract an archive file.
pub fn read_archive(archive_path: &Path, output_path: &Path) -> io::Result<()> {
    let file = File::open(archive_path)?;
    let mut reader = BufReader::new(file);

    // Read and verify header
    let mut magic = [0u8; 8];
    reader.read_exact(&mut magic)?;
    if &magic != MAGIC {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "invalid magic"));
    }

    let mut version = [0u8; 1];
    reader.read_exact(&mut version)?;
    if version[0] != VERSION {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "unsupported version"));
    }

    let mut offset_bytes = [0u8; 8];
    reader.read_exact(&mut offset_bytes)?;
    let index_offset = u64::from_le_bytes(offset_bytes);

    let mut size_bytes = [0u8; 8];
    reader.read_exact(&mut size_bytes)?;
    let index_size = u64::from_le_bytes(size_bytes);

    // Read index
    reader.seek(SeekFrom::Start(index_offset))?;
    let mut index_bytes = vec![0u8; index_size as usize];
    reader.read_exact(&mut index_bytes)?;

    let index: ArchiveIndex = rmp_serde::from_slice(&index_bytes)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

    // Extract tree
    fs::create_dir_all(output_path)?;
    extract_node(&index.tree, output_path, &mut reader, &index.chunk_offsets)?;

    Ok(())
}

fn extract_node(
    node: &Node,
    base_path: &Path,
    reader: &mut BufReader<File>,
    chunk_offsets: &HashMap<[u8; 32], (u64, u64)>,
) -> io::Result<()> {
    match node {
        Node::File { name, chunks, permissions, .. } => {
            let file_path = base_path.join(name);
            let mut file = File::create(&file_path)?;

            for hash in chunks {
                let (offset, size) = chunk_offsets.get(hash)
                    .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "missing chunk"))?;

                reader.seek(SeekFrom::Start(*offset))?;
                let mut data = vec![0u8; *size as usize];
                reader.read_exact(&mut data)?;
                file.write_all(&data)?;
            }

            // Set permissions
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(&file_path, fs::Permissions::from_mode(*permissions))?;
            }
        }
        Node::Directory { name, children, permissions, .. } => {
            let dir_path = if name.is_empty() {
                base_path.to_path_buf()
            } else {
                base_path.join(name)
            };

            fs::create_dir_all(&dir_path)?;

            for child in children {
                extract_node(child, &dir_path, reader, chunk_offsets)?;
            }

            // Set permissions
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(&dir_path, fs::Permissions::from_mode(*permissions))?;
            }
        }
    }
    Ok(())
}
```

**Step 4: Run tests to verify they pass**

Run: `cargo test archive`
Expected: PASS

**Step 5: Commit**

```bash
git add -A
git commit -m "feat: add archive reader for extraction"
```

---

### Task 8: CLI Integration - Archive and Extract Commands

**Files:**
- Modify: `src/main.rs`

**Step 1: Write integration test**

Create `tests/cli_tests.rs`:

```rust
use std::fs;
use std::process::Command;
use tempfile::TempDir;

fn webpub_cmd() -> Command {
    Command::new(env!("CARGO_BIN_EXE_webpub"))
}

#[test]
fn test_cli_archive_and_extract() {
    let temp = TempDir::new().unwrap();
    let source = temp.path().join("source");
    let archive = temp.path().join("test.webpub");
    let dest = temp.path().join("dest");

    // Create source
    fs::create_dir(&source).unwrap();
    fs::write(source.join("hello.txt"), "Hello!").unwrap();
    fs::create_dir(source.join("subdir")).unwrap();
    fs::write(source.join("subdir/world.txt"), "World!").unwrap();

    // Archive
    let status = webpub_cmd()
        .args(["archive", source.to_str().unwrap(), archive.to_str().unwrap()])
        .status()
        .unwrap();
    assert!(status.success());
    assert!(archive.exists());

    // Extract
    let status = webpub_cmd()
        .args(["extract", archive.to_str().unwrap(), dest.to_str().unwrap()])
        .status()
        .unwrap();
    assert!(status.success());

    // Verify
    assert_eq!(fs::read_to_string(dest.join("hello.txt")).unwrap(), "Hello!");
    assert_eq!(fs::read_to_string(dest.join("subdir/world.txt")).unwrap(), "World!");
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test test_cli_archive`
Expected: FAIL - commands not implemented

**Step 3: Implement CLI commands**

Update `src/main.rs`:

```rust
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use webpub::{archive, build_tree, scan_directory};

#[derive(Parser)]
#[command(name = "webpub")]
#[command(about = "Static website publishing with deduplication")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Create archive from directory
    Archive {
        /// Source directory
        dir: PathBuf,
        /// Output archive file
        output: PathBuf,
    },
    /// Extract archive to directory
    Extract {
        /// Archive file
        archive: PathBuf,
        /// Output directory
        output: PathBuf,
    },
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Archive { dir, output } => {
            let entry = scan_directory(&dir)?
                .next()
                .ok_or("Failed to scan directory")?;
            let (tree, chunks) = build_tree(entry);
            archive::write_archive(&output, &tree, &chunks)?;
            println!("Created archive: {}", output.display());
            println!("  Tree hash: {}", hex::encode(tree.hash()));
            println!("  Chunks: {}", chunks.len());
        }
        Commands::Extract { archive: archive_path, output } => {
            archive::read_archive(&archive_path, &output)?;
            println!("Extracted to: {}", output.display());
        }
    }

    Ok(())
}
```

**Step 4: Run tests to verify they pass**

Run: `cargo test cli`
Expected: PASS

**Step 5: Run manual test**

```bash
mkdir -p /tmp/testsite
echo "Hello" > /tmp/testsite/index.html
cargo run -- archive /tmp/testsite /tmp/test.webpub
cargo run -- extract /tmp/test.webpub /tmp/extracted
cat /tmp/extracted/index.html
```

Expected: "Hello"

**Step 6: Commit**

```bash
git add -A
git commit -m "feat: implement archive and extract CLI commands"
```

---

## Milestone 2: Server

This milestone delivers `webpub serve` with HTTP serving and sync protocol.

---

### Task 9: Server Storage Module

**Files:**
- Create: `src/server/mod.rs`
- Create: `src/server/storage.rs`
- Create: `tests/storage_tests.rs`

**Step 1: Write test for storage operations**

Create `tests/storage_tests.rs`:

```rust
use tempfile::TempDir;
use webpub::server::storage::Storage;
use webpub::Node;

#[test]
fn test_storage_init() {
    let temp = TempDir::new().unwrap();
    let storage = Storage::open(temp.path()).unwrap();

    // Should create index.db
    assert!(temp.path().join("index.db").exists());
}

#[test]
fn test_storage_chunks() {
    let temp = TempDir::new().unwrap();
    let storage = Storage::open(temp.path()).unwrap();

    let hash = [1u8; 32];
    let data = b"test chunk data".to_vec();

    // Store chunk
    storage.store_chunk(&hash, &data).unwrap();

    // Retrieve chunk
    let retrieved = storage.get_chunk(&hash).unwrap();
    assert_eq!(retrieved, Some(data));

    // Non-existent chunk
    let missing = storage.get_chunk(&[2u8; 32]).unwrap();
    assert_eq!(missing, None);
}

#[test]
fn test_storage_has_chunks() {
    let temp = TempDir::new().unwrap();
    let storage = Storage::open(temp.path()).unwrap();

    let hash1 = [1u8; 32];
    let hash2 = [2u8; 32];
    let hash3 = [3u8; 32];

    storage.store_chunk(&hash1, b"data1").unwrap();
    storage.store_chunk(&hash2, b"data2").unwrap();

    let have = storage.has_chunks(&[hash1, hash2, hash3]).unwrap();
    assert_eq!(have, vec![hash1, hash2]);
}

#[test]
fn test_storage_tokens() {
    let temp = TempDir::new().unwrap();
    let storage = Storage::open(temp.path()).unwrap();

    let token = storage.add_token().unwrap();
    assert!(storage.verify_token(&token).unwrap());
    assert!(!storage.verify_token("invalid").unwrap());

    storage.revoke_token(&token).unwrap();
    assert!(!storage.verify_token(&token).unwrap());
}

#[test]
fn test_storage_snapshots() {
    let temp = TempDir::new().unwrap();
    let storage = Storage::open(temp.path()).unwrap();

    let tree = Node::Directory {
        name: "".to_string(),
        permissions: 0o755,
        children: vec![],
        hash: [0u8; 32],
    };

    // Create snapshot
    let id = storage.create_snapshot("example.com", &tree).unwrap();
    assert_eq!(id, 1);

    // Get current snapshot
    let current = storage.get_current_snapshot("example.com").unwrap();
    assert!(current.is_some());
    assert_eq!(current.unwrap().0, id);

    // List snapshots
    let list = storage.list_snapshots("example.com").unwrap();
    assert_eq!(list.len(), 1);
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test test_storage`
Expected: FAIL - storage module doesn't exist

**Step 3: Create server module structure**

Create `src/server/mod.rs`:

```rust
pub mod storage;
```

Update `src/lib.rs`:

```rust
pub mod archive;
pub mod chunker;
pub mod merkle;
pub mod scanner;
pub mod server;

pub use chunker::Chunk;
pub use merkle::{build_tree, Node};
pub use scanner::{scan_directory, ScannedEntry};
```

**Step 4: Implement storage module**

Create `src/server/storage.rs`:

```rust
use crate::Node;
use rusqlite::{params, Connection, OptionalExtension};
use std::path::Path;
use std::sync::Mutex;

pub struct Storage {
    index: Mutex<Connection>,
    chunks_path: std::path::PathBuf,
}

impl Storage {
    pub fn open(data_path: &Path) -> rusqlite::Result<Self> {
        std::fs::create_dir_all(data_path)?;
        std::fs::create_dir_all(data_path.join("chunks"))?;

        let index = Connection::open(data_path.join("index.db"))?;

        index.execute_batch(
            "CREATE TABLE IF NOT EXISTS sites (
                hostname TEXT PRIMARY KEY
            );
            CREATE TABLE IF NOT EXISTS snapshots (
                id INTEGER PRIMARY KEY,
                hostname TEXT NOT NULL,
                root_hash BLOB NOT NULL,
                tree BLOB NOT NULL,
                created_at INTEGER NOT NULL,
                is_current INTEGER DEFAULT 0,
                FOREIGN KEY (hostname) REFERENCES sites(hostname)
            );
            CREATE TABLE IF NOT EXISTS tokens (
                token TEXT PRIMARY KEY,
                created_at INTEGER NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_snapshots_hostname ON snapshots(hostname);
            CREATE INDEX IF NOT EXISTS idx_snapshots_current ON snapshots(hostname, is_current);"
        )?;

        Ok(Self {
            index: Mutex::new(index),
            chunks_path: data_path.join("chunks"),
        })
    }

    fn chunk_db_path(&self, hash: &[u8; 32]) -> std::path::PathBuf {
        let prefix = format!("{:02x}", hash[0]);
        self.chunks_path.join(format!("{}.db", prefix))
    }

    fn get_chunk_conn(&self, hash: &[u8; 32]) -> rusqlite::Result<Connection> {
        let path = self.chunk_db_path(hash);
        let conn = Connection::open(&path)?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS chunks (
                hash BLOB PRIMARY KEY,
                data BLOB NOT NULL
            )",
            [],
        )?;
        Ok(conn)
    }

    pub fn store_chunk(&self, hash: &[u8; 32], data: &[u8]) -> rusqlite::Result<()> {
        let conn = self.get_chunk_conn(hash)?;
        conn.execute(
            "INSERT OR IGNORE INTO chunks (hash, data) VALUES (?1, ?2)",
            params![hash.as_slice(), data],
        )?;
        Ok(())
    }

    pub fn get_chunk(&self, hash: &[u8; 32]) -> rusqlite::Result<Option<Vec<u8>>> {
        let conn = self.get_chunk_conn(hash)?;
        conn.query_row(
            "SELECT data FROM chunks WHERE hash = ?1",
            params![hash.as_slice()],
            |row| row.get(0),
        ).optional()
    }

    pub fn has_chunks(&self, hashes: &[[u8; 32]]) -> rusqlite::Result<Vec<[u8; 32]>> {
        let mut found = Vec::new();
        for hash in hashes {
            let conn = self.get_chunk_conn(hash)?;
            let exists: bool = conn.query_row(
                "SELECT 1 FROM chunks WHERE hash = ?1",
                params![hash.as_slice()],
                |_| Ok(true),
            ).optional()?.unwrap_or(false);
            if exists {
                found.push(*hash);
            }
        }
        Ok(found)
    }

    pub fn add_token(&self) -> rusqlite::Result<String> {
        use std::time::{SystemTime, UNIX_EPOCH};

        let token: String = (0..32)
            .map(|_| format!("{:02x}", rand::random::<u8>()))
            .collect();
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let conn = self.index.lock().unwrap();
        conn.execute(
            "INSERT INTO tokens (token, created_at) VALUES (?1, ?2)",
            params![&token, now],
        )?;

        Ok(token)
    }

    pub fn verify_token(&self, token: &str) -> rusqlite::Result<bool> {
        let conn = self.index.lock().unwrap();
        let exists: bool = conn.query_row(
            "SELECT 1 FROM tokens WHERE token = ?1",
            params![token],
            |_| Ok(true),
        ).optional()?.unwrap_or(false);
        Ok(exists)
    }

    pub fn revoke_token(&self, token: &str) -> rusqlite::Result<()> {
        let conn = self.index.lock().unwrap();
        conn.execute("DELETE FROM tokens WHERE token = ?1", params![token])?;
        Ok(())
    }

    pub fn list_tokens(&self) -> rusqlite::Result<Vec<String>> {
        let conn = self.index.lock().unwrap();
        let mut stmt = conn.prepare("SELECT token FROM tokens")?;
        let tokens = stmt.query_map([], |row| row.get(0))?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(tokens)
    }

    pub fn create_snapshot(&self, hostname: &str, tree: &Node) -> rusqlite::Result<u64> {
        use std::time::{SystemTime, UNIX_EPOCH};

        let tree_bytes = rmp_serde::to_vec(tree).unwrap();
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let conn = self.index.lock().unwrap();

        // Ensure site exists
        conn.execute(
            "INSERT OR IGNORE INTO sites (hostname) VALUES (?1)",
            params![hostname],
        )?;

        // Clear current flag for this hostname
        conn.execute(
            "UPDATE snapshots SET is_current = 0 WHERE hostname = ?1",
            params![hostname],
        )?;

        // Insert new snapshot as current
        conn.execute(
            "INSERT INTO snapshots (hostname, root_hash, tree, created_at, is_current)
             VALUES (?1, ?2, ?3, ?4, 1)",
            params![hostname, tree.hash().as_slice(), tree_bytes, now],
        )?;

        Ok(conn.last_insert_rowid() as u64)
    }

    pub fn get_current_snapshot(&self, hostname: &str) -> rusqlite::Result<Option<(u64, Node)>> {
        let conn = self.index.lock().unwrap();
        conn.query_row(
            "SELECT id, tree FROM snapshots WHERE hostname = ?1 AND is_current = 1",
            params![hostname],
            |row| {
                let id: i64 = row.get(0)?;
                let tree_bytes: Vec<u8> = row.get(1)?;
                let tree: Node = rmp_serde::from_slice(&tree_bytes).unwrap();
                Ok((id as u64, tree))
            },
        ).optional()
    }

    pub fn list_snapshots(&self, hostname: &str) -> rusqlite::Result<Vec<(u64, i64, bool)>> {
        let conn = self.index.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, created_at, is_current FROM snapshots WHERE hostname = ?1 ORDER BY id DESC"
        )?;
        let snapshots = stmt.query_map(params![hostname], |row| {
            Ok((row.get::<_, i64>(0)? as u64, row.get(1)?, row.get::<_, i64>(2)? == 1))
        })?.collect::<Result<Vec<_>, _>>()?;
        Ok(snapshots)
    }

    pub fn set_current_snapshot(&self, hostname: &str, id: u64) -> rusqlite::Result<bool> {
        let conn = self.index.lock().unwrap();

        // Verify snapshot exists
        let exists: bool = conn.query_row(
            "SELECT 1 FROM snapshots WHERE hostname = ?1 AND id = ?2",
            params![hostname, id as i64],
            |_| Ok(true),
        ).optional()?.unwrap_or(false);

        if !exists {
            return Ok(false);
        }

        conn.execute(
            "UPDATE snapshots SET is_current = 0 WHERE hostname = ?1",
            params![hostname],
        )?;
        conn.execute(
            "UPDATE snapshots SET is_current = 1 WHERE hostname = ?1 AND id = ?2",
            params![hostname, id as i64],
        )?;

        Ok(true)
    }
}

// Add rand to Cargo.toml for token generation
```

**Step 5: Add rand dependency**

Update `Cargo.toml` to add:
```toml
rand = "0.8"
```

**Step 6: Run tests to verify they pass**

Run: `cargo test storage`
Expected: PASS

**Step 7: Commit**

```bash
git add -A
git commit -m "feat: add server storage module with SQLite"
```

---

### Task 10: Sync Protocol Types

**Files:**
- Create: `src/protocol.rs`
- Create: `tests/protocol_tests.rs`

**Step 1: Write test for protocol message serialization**

Create `tests/protocol_tests.rs`:

```rust
use webpub::protocol::*;

#[test]
fn test_auth_message_roundtrip() {
    let msg = ClientMessage::Auth { token: "secret123".to_string() };
    let bytes = rmp_serde::to_vec(&msg).unwrap();
    let decoded: ClientMessage = rmp_serde::from_slice(&bytes).unwrap();

    match decoded {
        ClientMessage::Auth { token } => assert_eq!(token, "secret123"),
        _ => panic!("Wrong variant"),
    }
}

#[test]
fn test_have_chunks_message() {
    let msg = ClientMessage::HaveChunks {
        hashes: vec![[1u8; 32], [2u8; 32]],
    };
    let bytes = rmp_serde::to_vec(&msg).unwrap();
    let decoded: ClientMessage = rmp_serde::from_slice(&bytes).unwrap();

    match decoded {
        ClientMessage::HaveChunks { hashes } => {
            assert_eq!(hashes.len(), 2);
        }
        _ => panic!("Wrong variant"),
    }
}

#[test]
fn test_server_messages() {
    let msg = ServerMessage::NeedChunks { hashes: vec![[1u8; 32]] };
    let bytes = rmp_serde::to_vec(&msg).unwrap();
    let _: ServerMessage = rmp_serde::from_slice(&bytes).unwrap();

    let msg = ServerMessage::CommitOk { snapshot_id: 42 };
    let bytes = rmp_serde::to_vec(&msg).unwrap();
    let _: ServerMessage = rmp_serde::from_slice(&bytes).unwrap();
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test test_auth_message`
Expected: FAIL - protocol module doesn't exist

**Step 3: Implement protocol types**

Create `src/protocol.rs`:

```rust
use crate::Node;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ClientMessage {
    Auth { token: String },
    HaveChunks { hashes: Vec<[u8; 32]> },
    ChunkData { hash: [u8; 32], data: Vec<u8> },
    CommitTree { hostname: String, tree: Node },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ServerMessage {
    AuthOk,
    AuthFailed,
    NeedChunks { hashes: Vec<[u8; 32]> },
    ChunkAck { hash: [u8; 32] },
    CommitOk { snapshot_id: u64 },
    CommitFailed { reason: String },
}
```

**Step 4: Export from lib.rs**

Add to `src/lib.rs`:
```rust
pub mod protocol;
```

**Step 5: Run tests to verify they pass**

Run: `cargo test protocol`
Expected: PASS

**Step 6: Commit**

```bash
git add -A
git commit -m "feat: add sync protocol message types"
```

---

### Task 11: Server HTTP Handler

**Files:**
- Create: `src/server/http.rs`
- Modify: `src/server/mod.rs`

**Step 1: Write test for HTTP serving**

Create `tests/http_tests.rs`:

```rust
use axum::http::StatusCode;
use tempfile::TempDir;
use webpub::server::storage::Storage;
use webpub::Node;
use std::sync::Arc;

// Integration test - requires running server
// For now, test the path lookup logic

#[test]
fn test_find_node_in_tree() {
    use webpub::server::http::find_node;

    let tree = Node::Directory {
        name: "".to_string(),
        permissions: 0o755,
        children: vec![
            Node::File {
                name: "index.html".to_string(),
                permissions: 0o644,
                size: 100,
                chunks: vec![[1u8; 32]],
                hash: [2u8; 32],
            },
            Node::Directory {
                name: "css".to_string(),
                permissions: 0o755,
                children: vec![
                    Node::File {
                        name: "style.css".to_string(),
                        permissions: 0o644,
                        size: 50,
                        chunks: vec![[3u8; 32]],
                        hash: [4u8; 32],
                    },
                ],
                hash: [5u8; 32],
            },
        ],
        hash: [6u8; 32],
    };

    // Find root index.html
    let node = find_node(&tree, "/index.html");
    assert!(node.is_some());
    assert_eq!(node.unwrap().name(), "index.html");

    // Find nested file
    let node = find_node(&tree, "/css/style.css");
    assert!(node.is_some());
    assert_eq!(node.unwrap().name(), "style.css");

    // Directory with trailing slash -> look for index.html
    let node = find_node(&tree, "/");
    assert!(node.is_some());

    // Not found
    let node = find_node(&tree, "/missing.txt");
    assert!(node.is_none());
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test test_find_node`
Expected: FAIL - http module doesn't exist

**Step 3: Implement HTTP handler**

Create `src/server/http.rs`:

```rust
use crate::server::storage::Storage;
use crate::Node;
use axum::{
    body::Body,
    extract::{Host, Path, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use std::sync::Arc;

pub struct AppState {
    pub storage: Arc<Storage>,
}

pub fn create_router(storage: Arc<Storage>) -> Router {
    let state = AppState { storage };

    Router::new()
        .route("/", get(handle_request))
        .route("/*path", get(handle_request))
        .with_state(Arc::new(state))
}

async fn handle_request(
    State(state): State<Arc<AppState>>,
    Host(host): Host,
    path: Option<Path<String>>,
) -> Response {
    let path_str = path.map(|p| format!("/{}", p.0)).unwrap_or_else(|| "/".to_string());

    // Strip port from host if present
    let hostname = host.split(':').next().unwrap_or(&host);

    // Get current snapshot for this host
    let snapshot = match state.storage.get_current_snapshot(hostname) {
        Ok(Some((_, tree))) => tree,
        Ok(None) => return (StatusCode::NOT_FOUND, "Site not found").into_response(),
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    };

    // Find the node for this path
    let node = match find_node(&snapshot, &path_str) {
        Some(n) => n,
        None => return (StatusCode::NOT_FOUND, "Not found").into_response(),
    };

    // Must be a file
    let (chunks, name) = match node {
        Node::File { chunks, name, .. } => (chunks, name),
        Node::Directory { .. } => {
            // Try index.html
            if let Some(Node::File { chunks, name, .. }) = find_node(&snapshot, &format!("{}index.html",
                if path_str.ends_with('/') { &path_str } else { &format!("{}/", path_str) })) {
                (chunks, name)
            } else {
                return (StatusCode::NOT_FOUND, "Not found").into_response();
            }
        }
    };

    // Reassemble file from chunks
    let mut data = Vec::new();
    for hash in chunks {
        match state.storage.get_chunk(hash) {
            Ok(Some(chunk_data)) => data.extend(chunk_data),
            Ok(None) => return (StatusCode::INTERNAL_SERVER_ERROR, "Missing chunk").into_response(),
            Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
        }
    }

    // Guess content type from extension
    let content_type = mime_guess::from_path(name)
        .first_or_octet_stream()
        .to_string();

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, content_type)
        .body(Body::from(data))
        .unwrap()
}

pub fn find_node<'a>(tree: &'a Node, path: &str) -> Option<&'a Node> {
    let path = path.trim_start_matches('/');

    if path.is_empty() || path == "/" {
        // Root directory - look for index.html
        if let Node::Directory { children, .. } = tree {
            return children.iter().find(|c| c.name() == "index.html");
        }
        return None;
    }

    let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    find_node_recursive(tree, &parts)
}

fn find_node_recursive<'a>(node: &'a Node, parts: &[&str]) -> Option<&'a Node> {
    if parts.is_empty() {
        return Some(node);
    }

    match node {
        Node::Directory { children, .. } => {
            for child in children {
                if child.name() == parts[0] {
                    return find_node_recursive(child, &parts[1..]);
                }
            }
            None
        }
        Node::File { .. } => None,
    }
}
```

**Step 4: Update server mod.rs**

```rust
pub mod http;
pub mod storage;
```

**Step 5: Run tests to verify they pass**

Run: `cargo test http`
Expected: PASS

**Step 6: Commit**

```bash
git add -A
git commit -m "feat: add HTTP handler for static file serving"
```

---

### Task 12: Server WebSocket Sync Handler

**Files:**
- Create: `src/server/sync.rs`
- Modify: `src/server/mod.rs`

**Step 1: Implement sync handler**

Create `src/server/sync.rs`:

```rust
use crate::protocol::{ClientMessage, ServerMessage};
use crate::server::storage::Storage;
use crate::Node;
use futures_util::{SinkExt, StreamExt};
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio_tungstenite::{accept_async, tungstenite::Message, WebSocketStream};

pub async fn handle_connection(stream: TcpStream, storage: Arc<Storage>, keep: usize) {
    let ws_stream = match accept_async(stream).await {
        Ok(ws) => ws,
        Err(e) => {
            eprintln!("WebSocket handshake failed: {}", e);
            return;
        }
    };

    if let Err(e) = handle_sync(ws_stream, storage, keep).await {
        eprintln!("Sync error: {}", e);
    }
}

async fn handle_sync(
    mut ws: WebSocketStream<TcpStream>,
    storage: Arc<Storage>,
    keep: usize,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Wait for auth
    let msg = ws.next().await.ok_or("Connection closed")??;
    let client_msg: ClientMessage = match msg {
        Message::Binary(data) => rmp_serde::from_slice(&data)?,
        _ => return Err("Expected binary message".into()),
    };

    let token = match client_msg {
        ClientMessage::Auth { token } => token,
        _ => return Err("Expected Auth message".into()),
    };

    if !storage.verify_token(&token)? {
        let response = rmp_serde::to_vec(&ServerMessage::AuthFailed)?;
        ws.send(Message::Binary(response.into())).await?;
        return Err("Invalid token".into());
    }

    let response = rmp_serde::to_vec(&ServerMessage::AuthOk)?;
    ws.send(Message::Binary(response.into())).await?;

    // Handle sync messages
    let mut pending_tree: Option<(String, Node)> = None;

    while let Some(msg) = ws.next().await {
        let msg = msg?;
        let data = match msg {
            Message::Binary(data) => data,
            Message::Close(_) => break,
            _ => continue,
        };

        let client_msg: ClientMessage = rmp_serde::from_slice(&data)?;

        match client_msg {
            ClientMessage::HaveChunks { hashes } => {
                let have = storage.has_chunks(&hashes)?;
                let need: Vec<[u8; 32]> = hashes
                    .into_iter()
                    .filter(|h| !have.contains(h))
                    .collect();

                let response = rmp_serde::to_vec(&ServerMessage::NeedChunks { hashes: need })?;
                ws.send(Message::Binary(response.into())).await?;
            }
            ClientMessage::ChunkData { hash, data } => {
                storage.store_chunk(&hash, &data)?;

                let response = rmp_serde::to_vec(&ServerMessage::ChunkAck { hash })?;
                ws.send(Message::Binary(response.into())).await?;
            }
            ClientMessage::CommitTree { hostname, tree } => {
                // Verify all chunks exist
                if let Err(missing) = verify_tree_chunks(&tree, &storage) {
                    let response = rmp_serde::to_vec(&ServerMessage::CommitFailed {
                        reason: format!("Missing {} chunks", missing),
                    })?;
                    ws.send(Message::Binary(response.into())).await?;
                    continue;
                }

                let snapshot_id = storage.create_snapshot(&hostname, &tree)?;

                // Cleanup old snapshots
                cleanup_old_snapshots(&storage, &hostname, keep)?;

                let response = rmp_serde::to_vec(&ServerMessage::CommitOk { snapshot_id })?;
                ws.send(Message::Binary(response.into())).await?;

                println!("Deployed {} snapshot {}", hostname, snapshot_id);
            }
            _ => {}
        }
    }

    Ok(())
}

fn verify_tree_chunks(tree: &Node, storage: &Storage) -> Result<(), usize> {
    let mut missing = 0;
    verify_node_chunks(tree, storage, &mut missing);
    if missing > 0 {
        Err(missing)
    } else {
        Ok(())
    }
}

fn verify_node_chunks(node: &Node, storage: &Storage, missing: &mut usize) {
    match node {
        Node::File { chunks, .. } => {
            for hash in chunks {
                if storage.get_chunk(hash).ok().flatten().is_none() {
                    *missing += 1;
                }
            }
        }
        Node::Directory { children, .. } => {
            for child in children {
                verify_node_chunks(child, storage, missing);
            }
        }
    }
}

fn cleanup_old_snapshots(storage: &Storage, hostname: &str, keep: usize) -> rusqlite::Result<()> {
    let snapshots = storage.list_snapshots(hostname)?;
    if snapshots.len() > keep {
        // TODO: Delete old snapshots (keeping `keep` most recent)
        // For now, just leave them - GC will clean up chunks
    }
    Ok(())
}
```

**Step 2: Add futures-util dependency**

Update `Cargo.toml`:
```toml
futures-util = "0.3"
```

**Step 3: Update server mod.rs**

```rust
pub mod http;
pub mod storage;
pub mod sync;
```

**Step 4: Verify it compiles**

Run: `cargo build`
Expected: Compiles

**Step 5: Commit**

```bash
git add -A
git commit -m "feat: add WebSocket sync handler"
```

---

### Task 13: Server CLI Command

**Files:**
- Modify: `src/main.rs`

**Step 1: Add serve command to CLI**

Update `src/main.rs`:

```rust
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::sync::Arc;
use webpub::{archive, build_tree, scan_directory};
use webpub::server::storage::Storage;

#[derive(Parser)]
#[command(name = "webpub")]
#[command(about = "Static website publishing with deduplication")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Create archive from directory
    Archive {
        /// Source directory
        dir: PathBuf,
        /// Output archive file
        output: PathBuf,
    },
    /// Extract archive to directory
    Extract {
        /// Archive file
        archive: PathBuf,
        /// Output directory
        output: PathBuf,
    },
    /// Run server
    Serve {
        /// HTTP port for serving websites
        #[arg(long, default_value = "8080")]
        http_port: u16,
        /// WebSocket port for sync
        #[arg(long, default_value = "9000")]
        sync_port: u16,
        /// Data directory
        #[arg(long, default_value = "./data")]
        data: PathBuf,
        /// Number of snapshots to keep per site
        #[arg(long, default_value = "5")]
        keep: usize,
    },
    /// Manage tokens
    Token {
        #[command(subcommand)]
        action: TokenAction,
    },
    /// Garbage collect unreferenced chunks
    Gc {
        /// Data directory
        #[arg(long, default_value = "./data")]
        data: PathBuf,
    },
}

#[derive(Subcommand)]
enum TokenAction {
    /// Add a new token
    Add {
        #[arg(long, default_value = "./data")]
        data: PathBuf,
    },
    /// List all tokens
    List {
        #[arg(long, default_value = "./data")]
        data: PathBuf,
    },
    /// Revoke a token
    Revoke {
        token: String,
        #[arg(long, default_value = "./data")]
        data: PathBuf,
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Archive { dir, output } => {
            let entry = scan_directory(&dir)?
                .next()
                .ok_or("Failed to scan directory")?;
            let (tree, chunks) = build_tree(entry);
            archive::write_archive(&output, &tree, &chunks)?;
            println!("Created archive: {}", output.display());
            println!("  Tree hash: {}", hex::encode(tree.hash()));
            println!("  Chunks: {}", chunks.len());
        }
        Commands::Extract { archive: archive_path, output } => {
            archive::read_archive(&archive_path, &output)?;
            println!("Extracted to: {}", output.display());
        }
        Commands::Serve { http_port, sync_port, data, keep } => {
            let storage = Arc::new(Storage::open(&data)?);

            println!("Starting webpub server...");
            println!("  HTTP: http://0.0.0.0:{}", http_port);
            println!("  Sync: ws://0.0.0.0:{}", sync_port);
            println!("  Data: {}", data.display());
            println!("  Keep: {} snapshots per site", keep);

            // Start HTTP server
            let http_storage = Arc::clone(&storage);
            let http_handle = tokio::spawn(async move {
                let app = webpub::server::http::create_router(http_storage);
                let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", http_port))
                    .await
                    .unwrap();
                axum::serve(listener, app).await.unwrap();
            });

            // Start sync server
            let sync_storage = Arc::clone(&storage);
            let sync_handle = tokio::spawn(async move {
                let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", sync_port))
                    .await
                    .unwrap();

                while let Ok((stream, addr)) = listener.accept().await {
                    println!("Sync connection from {}", addr);
                    let storage = Arc::clone(&sync_storage);
                    tokio::spawn(async move {
                        webpub::server::sync::handle_connection(stream.into_std().unwrap().try_into().unwrap(), storage, keep).await;
                    });
                }
            });

            tokio::select! {
                _ = http_handle => {}
                _ = sync_handle => {}
            }
        }
        Commands::Token { action } => {
            match action {
                TokenAction::Add { data } => {
                    let storage = Storage::open(&data)?;
                    let token = storage.add_token()?;
                    println!("{}", token);
                }
                TokenAction::List { data } => {
                    let storage = Storage::open(&data)?;
                    for token in storage.list_tokens()? {
                        // Show partial token for identification
                        println!("{}...", &token[..16]);
                    }
                }
                TokenAction::Revoke { token, data } => {
                    let storage = Storage::open(&data)?;
                    storage.revoke_token(&token)?;
                    println!("Token revoked");
                }
            }
        }
        Commands::Gc { data } => {
            println!("Garbage collection not yet implemented");
            // TODO: Implement GC
        }
    }

    Ok(())
}
```

**Step 2: Fix the sync handler TcpStream issue**

The sync handler needs a tokio TcpStream. Update `src/server/sync.rs`:

```rust
use tokio::net::TcpStream;
// Change the function signature and implementation to use tokio's TcpStream properly
```

Actually, let's fix the main.rs spawn to pass the tokio stream directly:

Update the sync spawn in main.rs:
```rust
let sync_handle = tokio::spawn(async move {
    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", sync_port))
        .await
        .unwrap();

    while let Ok((stream, addr)) = listener.accept().await {
        println!("Sync connection from {}", addr);
        let storage = Arc::clone(&sync_storage);
        tokio::spawn(webpub::server::sync::handle_connection(stream, storage, keep));
    }
});
```

And update `src/server/sync.rs` to use `tokio::net::TcpStream`:

```rust
use tokio::net::TcpStream;
use tokio_tungstenite::{accept_async, tungstenite::Message};
// ... rest remains same
```

**Step 3: Verify it compiles**

Run: `cargo build`
Expected: Compiles

**Step 4: Commit**

```bash
git add -A
git commit -m "feat: add serve command with HTTP and sync servers"
```

---

## Milestone 3: Client

This milestone delivers `webpub push`, `rollback`, and `list` commands.

---

### Task 14: Client Push Module

**Files:**
- Create: `src/client/mod.rs`
- Create: `src/client/push.rs`

**Step 1: Implement push client**

Create `src/client/mod.rs`:
```rust
pub mod push;
```

Create `src/client/push.rs`:

```rust
use crate::protocol::{ClientMessage, ServerMessage};
use crate::{build_tree, scan_directory, Chunk};
use futures_util::{SinkExt, StreamExt};
use std::path::Path;
use tokio_tungstenite::{connect_async, tungstenite::Message};

pub async fn push(
    dir: &Path,
    server_url: &str,
    hostname: &str,
    token: &str,
) -> Result<u64, Box<dyn std::error::Error + Send + Sync>> {
    // Scan directory and build tree
    println!("Scanning {}...", dir.display());
    let entry = scan_directory(dir)?
        .next()
        .ok_or("Failed to scan directory")?;
    let (tree, chunks) = build_tree(entry);

    println!("  Files: {} chunks", chunks.len());
    println!("  Root hash: {}", hex::encode(tree.hash()));

    // Connect to server
    println!("Connecting to {}...", server_url);
    let (mut ws, _) = connect_async(server_url).await?;

    // Authenticate
    let auth_msg = rmp_serde::to_vec(&ClientMessage::Auth { token: token.to_string() })?;
    ws.send(Message::Binary(auth_msg.into())).await?;

    let response = ws.next().await.ok_or("Connection closed")??;
    let server_msg: ServerMessage = match response {
        Message::Binary(data) => rmp_serde::from_slice(&data)?,
        _ => return Err("Expected binary message".into()),
    };

    match server_msg {
        ServerMessage::AuthOk => println!("Authenticated"),
        ServerMessage::AuthFailed => return Err("Authentication failed".into()),
        _ => return Err("Unexpected response".into()),
    }

    // Send chunk hashes in batches
    const BATCH_SIZE: usize = 100;
    let mut chunks_to_send: Vec<&Chunk> = Vec::new();

    for batch in chunks.chunks(BATCH_SIZE) {
        let hashes: Vec<[u8; 32]> = batch.iter().map(|c| c.hash).collect();
        let msg = rmp_serde::to_vec(&ClientMessage::HaveChunks { hashes })?;
        ws.send(Message::Binary(msg.into())).await?;

        // Get response
        let response = ws.next().await.ok_or("Connection closed")??;
        let server_msg: ServerMessage = match response {
            Message::Binary(data) => rmp_serde::from_slice(&data)?,
            _ => return Err("Expected binary message".into()),
        };

        match server_msg {
            ServerMessage::NeedChunks { hashes: needed } => {
                for chunk in batch {
                    if needed.contains(&chunk.hash) {
                        chunks_to_send.push(chunk);
                    }
                }
            }
            _ => return Err("Unexpected response".into()),
        }
    }

    // Send needed chunks
    println!("Sending {} chunks...", chunks_to_send.len());
    for chunk in chunks_to_send {
        let msg = rmp_serde::to_vec(&ClientMessage::ChunkData {
            hash: chunk.hash,
            data: chunk.data.clone(),
        })?;
        ws.send(Message::Binary(msg.into())).await?;

        // Wait for ack
        let response = ws.next().await.ok_or("Connection closed")??;
        let _: ServerMessage = match response {
            Message::Binary(data) => rmp_serde::from_slice(&data)?,
            _ => return Err("Expected binary message".into()),
        };
    }

    // Commit tree
    println!("Committing...");
    let msg = rmp_serde::to_vec(&ClientMessage::CommitTree {
        hostname: hostname.to_string(),
        tree,
    })?;
    ws.send(Message::Binary(msg.into())).await?;

    let response = ws.next().await.ok_or("Connection closed")??;
    let server_msg: ServerMessage = match response {
        Message::Binary(data) => rmp_serde::from_slice(&data)?,
        _ => return Err("Expected binary message".into()),
    };

    match server_msg {
        ServerMessage::CommitOk { snapshot_id } => {
            println!("Deployed snapshot {}", snapshot_id);
            Ok(snapshot_id)
        }
        ServerMessage::CommitFailed { reason } => {
            Err(format!("Commit failed: {}", reason).into())
        }
        _ => Err("Unexpected response".into()),
    }
}
```

**Step 2: Update lib.rs**

Add to `src/lib.rs`:
```rust
pub mod client;
```

**Step 3: Add push command to CLI**

Add to Commands enum in `src/main.rs`:
```rust
/// Push directory to server
Push {
    /// Source directory
    dir: PathBuf,
    /// Server WebSocket URL
    server: String,
    /// Hostname to publish as
    #[arg(long)]
    host: String,
},
```

Add to match in main():
```rust
Commands::Push { dir, server, host } => {
    let token = std::env::var("WEBPUB_TOKEN")
        .map_err(|_| "WEBPUB_TOKEN environment variable not set")?;

    let snapshot_id = webpub::client::push::push(&dir, &server, &host, &token).await?;
    println!("Successfully deployed snapshot {}", snapshot_id);
}
```

**Step 4: Verify it compiles**

Run: `cargo build`
Expected: Compiles

**Step 5: Commit**

```bash
git add -A
git commit -m "feat: add push client command"
```

---

### Task 15: End-to-End Integration Test

**Files:**
- Create: `tests/integration_test.rs`

**Step 1: Write integration test**

Create `tests/integration_test.rs`:

```rust
use std::fs;
use std::process::{Command, Stdio};
use std::time::Duration;
use tempfile::TempDir;

#[tokio::test]
async fn test_push_and_serve() {
    let temp = TempDir::new().unwrap();
    let data_dir = temp.path().join("data");
    let site_dir = temp.path().join("site");

    // Create site content
    fs::create_dir(&site_dir).unwrap();
    fs::write(site_dir.join("index.html"), "<h1>Hello</h1>").unwrap();
    fs::create_dir(site_dir.join("css")).unwrap();
    fs::write(site_dir.join("css/style.css"), "body { color: red; }").unwrap();

    // Start server
    let mut server = Command::new(env!("CARGO_BIN_EXE_webpub"))
        .args([
            "serve",
            "--http-port", "18080",
            "--sync-port", "19000",
            "--data", data_dir.to_str().unwrap(),
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();

    // Wait for server to start
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Add token
    let output = Command::new(env!("CARGO_BIN_EXE_webpub"))
        .args(["token", "add", "--data", data_dir.to_str().unwrap()])
        .output()
        .unwrap();
    let token = String::from_utf8(output.stdout).unwrap().trim().to_string();

    // Push site
    let status = Command::new(env!("CARGO_BIN_EXE_webpub"))
        .args([
            "push",
            site_dir.to_str().unwrap(),
            "ws://127.0.0.1:19000",
            "--host", "test.local",
        ])
        .env("WEBPUB_TOKEN", &token)
        .status()
        .unwrap();
    assert!(status.success());

    // Fetch via HTTP
    let response = reqwest::Client::new()
        .get("http://127.0.0.1:18080/index.html")
        .header("Host", "test.local")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    assert!(response.text().await.unwrap().contains("Hello"));

    // Cleanup
    server.kill().unwrap();
}
```

**Step 2: Add reqwest dev dependency**

Add to `Cargo.toml`:
```toml
[dev-dependencies]
tempfile = "3"
reqwest = { version = "0.11", features = ["json"] }
tokio = { version = "1", features = ["full", "test-util"] }
```

**Step 3: Run integration test**

Run: `cargo test test_push_and_serve -- --ignored`
(Mark as ignored for CI, run manually)

**Step 4: Commit**

```bash
git add -A
git commit -m "test: add end-to-end integration test"
```

---

### Task 16: List and Rollback Commands

**Files:**
- Create: `src/client/list.rs`
- Create: `src/client/rollback.rs`
- Modify: `src/main.rs`

Implementation follows same pattern as push - connect via WebSocket, send request, receive response.

**Commit after implementation:**

```bash
git add -A
git commit -m "feat: add list and rollback client commands"
```

---

## Summary

**Milestone 1 (Archive):** Tasks 1-8
- Project setup, core types, chunker, scanner, merkle builder
- Archive write/read, CLI commands

**Milestone 2 (Server):** Tasks 9-13
- Storage module, protocol types
- HTTP serving, WebSocket sync handler
- Serve CLI command

**Milestone 3 (Client):** Tasks 14-16
- Push client, integration tests
- List and rollback commands

Each task follows TDD: write failing test, implement, verify pass, commit.
