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
