use std::fs;
use std::io::{Read, Seek, SeekFrom};
use tempfile::TempDir;
use webpub::archive::{write_archive, MAGIC};
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
