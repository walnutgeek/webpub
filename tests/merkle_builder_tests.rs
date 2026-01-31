use std::fs;
use tempfile::TempDir;
use webpub::merkle::build_tree;
use webpub::scanner::scan_directory;
use webpub::Node;

#[test]
fn test_build_tree_single_file() {
    let temp = TempDir::new().unwrap();
    fs::write(temp.path().join("test.txt"), "hello").unwrap();

    let entry = scan_directory(temp.path()).unwrap().next().unwrap();
    let (tree, chunks) = build_tree(entry);

    // Root should be a directory
    match &tree {
        Node::Directory { children, .. } => {
            assert_eq!(children.len(), 1);
            match &children[0] {
                Node::File { name, size, .. } => {
                    assert_eq!(name, "test.txt");
                    assert_eq!(*size, 5);
                }
                _ => panic!("Expected file"),
            }
        }
        _ => panic!("Expected directory"),
    }

    // Should have one chunk for "hello"
    assert_eq!(chunks.len(), 1);
}

#[test]
fn test_build_tree_deterministic() {
    let temp = TempDir::new().unwrap();
    fs::write(temp.path().join("a.txt"), "aaa").unwrap();
    fs::write(temp.path().join("b.txt"), "bbb").unwrap();

    let entry1 = scan_directory(temp.path()).unwrap().next().unwrap();
    let (tree1, _) = build_tree(entry1);

    let entry2 = scan_directory(temp.path()).unwrap().next().unwrap();
    let (tree2, _) = build_tree(entry2);

    // Same content should produce same hash
    assert_eq!(tree1.hash(), tree2.hash());
}

#[test]
fn test_build_tree_empty_dir_preserved() {
    let temp = TempDir::new().unwrap();
    fs::create_dir(temp.path().join("empty")).unwrap();

    let entry = scan_directory(temp.path()).unwrap().next().unwrap();
    let (tree, chunks) = build_tree(entry);

    match &tree {
        Node::Directory { children, .. } => {
            assert_eq!(children.len(), 1);
            match &children[0] {
                Node::Directory { name, children: subchildren, .. } => {
                    assert_eq!(name, "empty");
                    assert!(subchildren.is_empty());
                }
                _ => panic!("Expected empty directory"),
            }
        }
        _ => panic!("Expected directory"),
    }

    // No chunks for empty directory
    assert!(chunks.is_empty());
}
