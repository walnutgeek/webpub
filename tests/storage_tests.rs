use tempfile::TempDir;
use webpub::server::storage::Storage;
use webpub::Node;

#[test]
fn test_storage_init() {
    let temp = TempDir::new().unwrap();
    let _storage = Storage::open(temp.path()).unwrap();

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
