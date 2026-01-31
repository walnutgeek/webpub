use std::fs;
use tempfile::TempDir;
use webpub::scanner::{scan_directory, ScannedEntry};

#[test]
fn test_scan_empty_directory() {
    let temp = TempDir::new().unwrap();
    let entries: Vec<ScannedEntry> = scan_directory(temp.path()).unwrap().collect();

    // Root directory only
    assert_eq!(entries.len(), 1);
    match &entries[0] {
        ScannedEntry::Directory { name, .. } => {
            assert_eq!(name, "");  // root has empty name
        }
        _ => panic!("Expected directory"),
    }
}

#[test]
fn test_scan_with_files() {
    let temp = TempDir::new().unwrap();
    fs::write(temp.path().join("a.txt"), "hello").unwrap();
    fs::write(temp.path().join("b.txt"), "world").unwrap();

    let entries: Vec<ScannedEntry> = scan_directory(temp.path()).unwrap().collect();

    // Root + 2 files
    assert_eq!(entries.len(), 3);
}

#[test]
fn test_scan_nested_directories() {
    let temp = TempDir::new().unwrap();
    fs::create_dir(temp.path().join("subdir")).unwrap();
    fs::write(temp.path().join("subdir/file.txt"), "content").unwrap();

    let entries: Vec<ScannedEntry> = scan_directory(temp.path()).unwrap().collect();

    // Root + subdir + file
    assert_eq!(entries.len(), 3);
}

#[test]
fn test_scan_sorted_by_name() {
    let temp = TempDir::new().unwrap();
    fs::write(temp.path().join("z.txt"), "z").unwrap();
    fs::write(temp.path().join("a.txt"), "a").unwrap();
    fs::write(temp.path().join("m.txt"), "m").unwrap();

    let entries: Vec<ScannedEntry> = scan_directory(temp.path()).unwrap().collect();

    // Skip root, check file order
    let names: Vec<&str> = entries.iter().skip(1).map(|e| e.name()).collect();
    assert_eq!(names, vec!["a.txt", "m.txt", "z.txt"]);
}

#[test]
fn test_scan_ignores_symlinks() {
    let temp = TempDir::new().unwrap();
    fs::write(temp.path().join("real.txt"), "content").unwrap();

    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(
            temp.path().join("real.txt"),
            temp.path().join("link.txt"),
        ).unwrap();
    }

    let entries: Vec<ScannedEntry> = scan_directory(temp.path()).unwrap().collect();

    // Root + real file only (symlink ignored)
    #[cfg(unix)]
    assert_eq!(entries.len(), 2);
}
