use std::fs;
use std::process::Command;
use tempfile::TempDir;

fn webpub_cmd() -> Command {
    Command::new(env!("CARGO_BIN_EXE_webpub"))
}

#[test]
fn test_cli_archive_and_extract() {
    let temp = TempDir::new().unwrap();
    let source = temp.path().join("source");
    let archive = temp.path().join("test.webpub");
    let dest = temp.path().join("dest");

    // Create source
    fs::create_dir(&source).unwrap();
    fs::write(source.join("hello.txt"), "Hello!").unwrap();
    fs::create_dir(source.join("subdir")).unwrap();
    fs::write(source.join("subdir/world.txt"), "World!").unwrap();

    // Archive
    let status = webpub_cmd()
        .args([
            "archive",
            source.to_str().unwrap(),
            archive.to_str().unwrap(),
        ])
        .status()
        .unwrap();
    assert!(status.success());
    assert!(archive.exists());

    // Extract
    let status = webpub_cmd()
        .args(["extract", archive.to_str().unwrap(), dest.to_str().unwrap()])
        .status()
        .unwrap();
    assert!(status.success());

    // Verify
    assert_eq!(
        fs::read_to_string(dest.join("hello.txt")).unwrap(),
        "Hello!"
    );
    assert_eq!(
        fs::read_to_string(dest.join("subdir/world.txt")).unwrap(),
        "World!"
    );
}
