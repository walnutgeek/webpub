use std::fs;
use std::io::{Read, Seek, SeekFrom};
use tempfile::TempDir;
use webpub::archive::{read_archive, write_archive, MAGIC};
use webpub::merkle::build_tree;
use webpub::scanner::scan_directory;

#[test]
fn test_write_archive_magic() {
    let temp = TempDir::new().unwrap();
    fs::write(temp.path().join("test.txt"), "hello").unwrap();

    let archive_path = temp.path().join("test.webpub");

    let entry = scan_directory(temp.path()).unwrap().next().unwrap();
    let (tree, chunks) = build_tree(entry);

    write_archive(&archive_path, &tree, &chunks).unwrap();

    // Verify magic bytes
    let mut file = fs::File::open(&archive_path).unwrap();
    let mut magic = [0u8; 8];
    file.read_exact(&mut magic).unwrap();
    assert_eq!(&magic, MAGIC);
}

#[test]
fn test_write_archive_version() {
    let temp = TempDir::new().unwrap();
    fs::write(temp.path().join("test.txt"), "hello").unwrap();

    let archive_path = temp.path().join("test.webpub");

    let entry = scan_directory(temp.path()).unwrap().next().unwrap();
    let (tree, chunks) = build_tree(entry);

    write_archive(&archive_path, &tree, &chunks).unwrap();

    // Verify version byte
    let mut file = fs::File::open(&archive_path).unwrap();
    file.seek(SeekFrom::Start(8)).unwrap();
    let mut version = [0u8; 1];
    file.read_exact(&mut version).unwrap();
    assert_eq!(version[0], 1);
}

#[test]
fn test_roundtrip_single_file() {
    let temp = TempDir::new().unwrap();
    let content = b"Hello, world!";
    fs::write(temp.path().join("test.txt"), content).unwrap();

    let archive_path = temp.path().join("test.webpub");
    let extract_path = temp.path().join("extracted");

    // Create archive
    let entry = scan_directory(temp.path()).unwrap().next().unwrap();
    let (tree, chunks) = build_tree(entry);
    write_archive(&archive_path, &tree, &chunks).unwrap();

    // Extract
    read_archive(&archive_path, &extract_path).unwrap();

    // Verify
    let extracted = fs::read(extract_path.join("test.txt")).unwrap();
    assert_eq!(extracted, content);
}

#[test]
fn test_roundtrip_nested_structure() {
    let temp = TempDir::new().unwrap();
    fs::create_dir(temp.path().join("subdir")).unwrap();
    fs::write(temp.path().join("root.txt"), "root").unwrap();
    fs::write(temp.path().join("subdir/nested.txt"), "nested").unwrap();

    let archive_path = temp.path().join("test.webpub");
    let extract_path = temp.path().join("extracted");

    // Create archive
    let entry = scan_directory(temp.path()).unwrap().next().unwrap();
    let (tree, chunks) = build_tree(entry);
    write_archive(&archive_path, &tree, &chunks).unwrap();

    // Extract
    read_archive(&archive_path, &extract_path).unwrap();

    // Verify
    assert_eq!(
        fs::read_to_string(extract_path.join("root.txt")).unwrap(),
        "root"
    );
    assert_eq!(
        fs::read_to_string(extract_path.join("subdir/nested.txt")).unwrap(),
        "nested"
    );
}

#[test]
fn test_roundtrip_empty_directory() {
    let temp = TempDir::new().unwrap();
    fs::create_dir(temp.path().join("empty")).unwrap();

    let archive_path = temp.path().join("test.webpub");
    let extract_path = temp.path().join("extracted");

    // Create archive
    let entry = scan_directory(temp.path()).unwrap().next().unwrap();
    let (tree, chunks) = build_tree(entry);
    write_archive(&archive_path, &tree, &chunks).unwrap();

    // Extract
    read_archive(&archive_path, &extract_path).unwrap();

    // Verify empty dir exists
    assert!(extract_path.join("empty").is_dir());
}
