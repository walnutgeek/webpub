use crate::Node;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ClientMessage {
    Auth { token: String },
    HaveChunks { hashes: Vec<[u8; 32]> },
    ChunkData { hash: [u8; 32], data: Vec<u8> },
    CommitTree { hostname: String, tree: Node },
    ListSnapshots { hostname: String },
    Rollback { hostname: String, snapshot_id: Option<u64> },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ServerMessage {
    AuthOk,
    AuthFailed,
    NeedChunks { hashes: Vec<[u8; 32]> },
    ChunkAck { hash: [u8; 32] },
    CommitOk { snapshot_id: u64 },
    CommitFailed { reason: String },
    SnapshotList { snapshots: Vec<(u64, String, bool)> }, // (id, created_at, is_current)
    RollbackOk { snapshot_id: u64 },
    RollbackFailed { reason: String },
}
