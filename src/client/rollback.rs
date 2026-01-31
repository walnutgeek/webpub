use crate::protocol::{ClientMessage, ServerMessage};
use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::{connect_async, tungstenite::Message};

pub async fn rollback(
    server_url: &str,
    hostname: &str,
    token: &str,
    snapshot_id: Option<u64>,
) -> Result<u64, Box<dyn std::error::Error>> {
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

    // Request rollback
    let rollback_msg = rmp_serde::to_vec(&ClientMessage::Rollback {
        hostname: hostname.to_string(),
        snapshot_id,
    })?;
    ws.send(Message::Binary(rollback_msg)).await?;

    let response = ws.next().await.ok_or("Connection closed")??;
    let server_msg: ServerMessage = match response {
        Message::Binary(data) => rmp_serde::from_slice(&data)?,
        _ => return Err("Expected binary message".into()),
    };

    match server_msg {
        ServerMessage::RollbackOk { snapshot_id } => Ok(snapshot_id),
        ServerMessage::RollbackFailed { reason } => {
            Err(format!("Rollback failed: {}", reason).into())
        }
        _ => Err("Unexpected response".into()),
    }
}
