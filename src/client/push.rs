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
) -> Result<u64, Box<dyn std::error::Error>> {
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
    ws.send(Message::Binary(auth_msg)).await?;

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
        ws.send(Message::Binary(msg)).await?;

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
        ws.send(Message::Binary(msg)).await?;

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
    ws.send(Message::Binary(msg)).await?;

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
