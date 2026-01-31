use crate::protocol::{ClientMessage, ServerMessage};
use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::{connect_async, tungstenite::Message};

pub async fn list(
    server_url: &str,
    hostname: &str,
    token: &str,
) -> Result<Vec<(u64, String, bool)>, Box<dyn std::error::Error>> {
    let (mut ws, _) = connect_async(server_url).await?;

    // Authenticate
    let auth_msg = rmp_serde::to_vec(&ClientMessage::Auth {
        token: token.to_string(),
    })?;
    ws.send(Message::Binary(auth_msg)).await?;

    let response = ws.next().await.ok_or("Connection closed")??;
    let server_msg: ServerMessage = match response {
        Message::Binary(data) => rmp_serde::from_slice(&data)?,
        _ => return Err("Expected binary message".into()),
    };

    match server_msg {
        ServerMessage::AuthOk => {}
        ServerMessage::AuthFailed => return Err("Authentication failed".into()),
        _ => return Err("Unexpected response".into()),
    }

    // Request list
    let list_msg = rmp_serde::to_vec(&ClientMessage::ListSnapshots {
        hostname: hostname.to_string(),
    })?;
    ws.send(Message::Binary(list_msg)).await?;

    let response = ws.next().await.ok_or("Connection closed")??;
    let server_msg: ServerMessage = match response {
        Message::Binary(data) => rmp_serde::from_slice(&data)?,
        _ => return Err("Expected binary message".into()),
    };

    match server_msg {
        ServerMessage::SnapshotList { snapshots } => Ok(snapshots),
        _ => Err("Unexpected response".into()),
    }
}
