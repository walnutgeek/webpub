use std::fs;
use tempfile::TempDir;
use webpub::scanner::{scan_directory, ScannedEntry};

#[test]
fn test_scan_empty_directory() {
    let temp = TempDir::new().unwrap();
    let entry = scan_directory(temp.path()).unwrap().next().unwrap();

    // Root directory with no children
    match &entry {
        ScannedEntry::Directory { name, children, .. } => {
            assert_eq!(name, ""); // root has empty name
            assert!(children.is_empty());
        }
        _ => panic!("Expected directory"),
    }
}

#[test]
fn test_scan_with_files() {
    let temp = TempDir::new().unwrap();
    fs::write(temp.path().join("a.txt"), "hello").unwrap();
    fs::write(temp.path().join("b.txt"), "world").unwrap();

    let entry = scan_directory(temp.path()).unwrap().next().unwrap();

    // Root with 2 file children
    match &entry {
        ScannedEntry::Directory { children, .. } => {
            assert_eq!(children.len(), 2);
        }
        _ => panic!("Expected directory"),
    }
}

#[test]
fn test_scan_nested_directories() {
    let temp = TempDir::new().unwrap();
    fs::create_dir(temp.path().join("subdir")).unwrap();
    fs::write(temp.path().join("subdir/file.txt"), "content").unwrap();

    let entry = scan_directory(temp.path()).unwrap().next().unwrap();

    // Root with subdir, subdir has file
    match &entry {
        ScannedEntry::Directory { children, .. } => {
            assert_eq!(children.len(), 1);
            match &children[0] {
                ScannedEntry::Directory {
                    name,
                    children: subchildren,
                    ..
                } => {
                    assert_eq!(name, "subdir");
                    assert_eq!(subchildren.len(), 1);
                }
                _ => panic!("Expected subdirectory"),
            }
        }
        _ => panic!("Expected directory"),
    }
}

#[test]
fn test_scan_sorted_by_name() {
    let temp = TempDir::new().unwrap();
    fs::write(temp.path().join("z.txt"), "z").unwrap();
    fs::write(temp.path().join("a.txt"), "a").unwrap();
    fs::write(temp.path().join("m.txt"), "m").unwrap();

    let entry = scan_directory(temp.path()).unwrap().next().unwrap();

    // Check file order in children
    match &entry {
        ScannedEntry::Directory { children, .. } => {
            let names: Vec<&str> = children.iter().map(|e| e.name()).collect();
            assert_eq!(names, vec!["a.txt", "m.txt", "z.txt"]);
        }
        _ => panic!("Expected directory"),
    }
}

#[test]
fn test_scan_ignores_symlinks() {
    let temp = TempDir::new().unwrap();
    fs::write(temp.path().join("real.txt"), "content").unwrap();

    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(temp.path().join("real.txt"), temp.path().join("link.txt"))
            .unwrap();
    }

    let entry = scan_directory(temp.path()).unwrap().next().unwrap();

    // Root with real file only (symlink ignored)
    match &entry {
        ScannedEntry::Directory { children, .. } => {
            #[cfg(unix)]
            assert_eq!(children.len(), 1);
        }
        _ => panic!("Expected directory"),
    }
}
