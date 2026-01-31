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
