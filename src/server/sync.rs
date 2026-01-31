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
        ws.send(Message::Binary(response)).await?;
        return Err("Invalid token".into());
    }

    let response = rmp_serde::to_vec(&ServerMessage::AuthOk)?;
    ws.send(Message::Binary(response.into())).await?;

    // Handle sync messages
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
                ws.send(Message::Binary(response)).await?;
            }
            ClientMessage::ChunkData { hash, data } => {
                storage.store_chunk(&hash, &data)?;

                let response = rmp_serde::to_vec(&ServerMessage::ChunkAck { hash })?;
                ws.send(Message::Binary(response)).await?;
            }
            ClientMessage::CommitTree { hostname, tree } => {
                // Verify all chunks exist
                if let Err(missing) = verify_tree_chunks(&tree, &storage) {
                    let response = rmp_serde::to_vec(&ServerMessage::CommitFailed {
                        reason: format!("Missing {} chunks", missing),
                    })?;
                    ws.send(Message::Binary(response)).await?;
                    continue;
                }

                let snapshot_id = storage.create_snapshot(&hostname, &tree)?;

                // Cleanup old snapshots
                cleanup_old_snapshots(&storage, &hostname, keep)?;

                let response = rmp_serde::to_vec(&ServerMessage::CommitOk {
                    snapshot_id: snapshot_id as u64,
                })?;
                ws.send(Message::Binary(response)).await?;

                println!("Deployed {} snapshot {}", hostname, snapshot_id);
            }
            ClientMessage::ListSnapshots { hostname } => {
                let snapshots = storage.list_snapshots(&hostname)?;
                // Convert from (i64, bool, String) to (u64, String, bool)
                let snapshots: Vec<(u64, String, bool)> = snapshots
                    .into_iter()
                    .map(|(id, is_current, created_at)| (id as u64, created_at, is_current))
                    .collect();
                let response = rmp_serde::to_vec(&ServerMessage::SnapshotList { snapshots })?;
                ws.send(Message::Binary(response)).await?;
            }
            ClientMessage::Rollback { hostname, snapshot_id } => {
                // If no snapshot_id given, use previous (second most recent)
                let snapshots = storage.list_snapshots(&hostname)?;
                let target_id = match snapshot_id {
                    Some(id) => id as i64,
                    None => {
                        // Find previous (second in list since list is sorted by id DESC)
                        if snapshots.len() < 2 {
                            let response = rmp_serde::to_vec(&ServerMessage::RollbackFailed {
                                reason: "No previous snapshot to rollback to".to_string(),
                            })?;
                            ws.send(Message::Binary(response)).await?;
                            continue;
                        }
                        snapshots[1].0 // Second snapshot (previous)
                    }
                };

                if storage.set_current_snapshot(&hostname, target_id)? {
                    let response = rmp_serde::to_vec(&ServerMessage::RollbackOk {
                        snapshot_id: target_id as u64,
                    })?;
                    ws.send(Message::Binary(response)).await?;
                    println!("Rolled back {} to snapshot {}", hostname, target_id);
                } else {
                    let response = rmp_serde::to_vec(&ServerMessage::RollbackFailed {
                        reason: "Snapshot not found".to_string(),
                    })?;
                    ws.send(Message::Binary(response)).await?;
                }
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

fn cleanup_old_snapshots(
    storage: &Storage,
    hostname: &str,
    keep: usize,
) -> crate::server::storage::Result<()> {
    let snapshots = storage.list_snapshots(hostname)?;
    if snapshots.len() > keep {
        // TODO: Delete old snapshots (keeping `keep` most recent)
        // For now, just leave them - GC will clean up chunks
    }
    Ok(())
}
