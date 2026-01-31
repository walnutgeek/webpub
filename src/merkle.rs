use serde::{Deserialize, Serialize};

use crate::chunker::{chunk_data, Chunk};
use crate::scanner::ScannedEntry;

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
