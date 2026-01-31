use std::fs;
use std::process::{Command, Stdio};
use std::time::Duration;
use tempfile::TempDir;

#[tokio::test]
#[ignore] // Requires spawning servers; flaky in CI
async fn test_push_and_serve() {
    let temp = TempDir::new().unwrap();
    let data_dir = temp.path().join("data");
    let site_dir = temp.path().join("site");

    // Create site content
    fs::create_dir(&site_dir).unwrap();
    fs::write(site_dir.join("index.html"), "<h1>Hello</h1>").unwrap();
    fs::create_dir(site_dir.join("css")).unwrap();
    fs::write(site_dir.join("css/style.css"), "body { color: red; }").unwrap();

    // Start server
    let mut server = Command::new(env!("CARGO_BIN_EXE_webpub"))
        .args([
            "serve",
            "--http-port", "18080",
            "--sync-port", "19000",
            "--data", data_dir.to_str().unwrap(),
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();

    // Wait for server to start
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Add token
    let output = Command::new(env!("CARGO_BIN_EXE_webpub"))
        .args(["token", "add", "--data", data_dir.to_str().unwrap()])
        .output()
        .unwrap();
    let token = String::from_utf8(output.stdout).unwrap().trim().to_string();

    // Push site
    let status = Command::new(env!("CARGO_BIN_EXE_webpub"))
        .args([
            "push",
            site_dir.to_str().unwrap(),
            "ws://127.0.0.1:19000",
            "--host", "test.local",
        ])
        .env("WEBPUB_TOKEN", &token)
        .status()
        .unwrap();
    assert!(status.success());

    // Fetch via HTTP
    let response = reqwest::Client::new()
        .get("http://127.0.0.1:18080/index.html")
        .header("Host", "test.local")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    assert!(response.text().await.unwrap().contains("Hello"));

    // Cleanup
    server.kill().unwrap();
}
