use webpub::server::http::find_node;
use webpub::Node;

#[test]
fn test_find_node_in_tree() {
    let tree = Node::Directory {
        name: "".to_string(),
        permissions: 0o755,
        children: vec![
            Node::File {
                name: "index.html".to_string(),
                permissions: 0o644,
                size: 100,
                chunks: vec![[1u8; 32]],
                hash: [2u8; 32],
            },
            Node::Directory {
                name: "css".to_string(),
                permissions: 0o755,
                children: vec![
                    Node::File {
                        name: "style.css".to_string(),
                        permissions: 0o644,
                        size: 50,
                        chunks: vec![[3u8; 32]],
                        hash: [4u8; 32],
                    },
                ],
                hash: [5u8; 32],
            },
        ],
        hash: [6u8; 32],
    };

    // Find root index.html
    let node = find_node(&tree, "/index.html");
    assert!(node.is_some());
    assert_eq!(node.unwrap().name(), "index.html");

    // Find nested file
    let node = find_node(&tree, "/css/style.css");
    assert!(node.is_some());
    assert_eq!(node.unwrap().name(), "style.css");

    // Directory with trailing slash -> look for index.html
    let node = find_node(&tree, "/");
    assert!(node.is_some());

    // Not found
    let node = find_node(&tree, "/missing.txt");
    assert!(node.is_none());
}
