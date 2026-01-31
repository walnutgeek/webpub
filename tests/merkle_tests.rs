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
