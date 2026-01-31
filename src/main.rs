use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::net::TcpListener;
use webpub::{archive, build_tree, scan_directory, server::storage::Storage};

#[derive(Parser)]
#[command(name = "webpub")]
#[command(about = "Static website publishing with deduplication")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Create archive from directory
    Archive {
        /// Source directory
        dir: PathBuf,
        /// Output archive file
        output: PathBuf,
    },
    /// Extract archive to directory
    Extract {
        /// Archive file
        archive: PathBuf,
        /// Output directory
        output: PathBuf,
    },
    /// Run the server
    Serve {
        /// HTTP port for serving websites
        #[arg(long, default_value = "8080")]
        http_port: u16,
        /// Sync port for WebSocket deployments
        #[arg(long, default_value = "9000")]
        sync_port: u16,
        /// Data directory for storage
        #[arg(long, default_value = "./data")]
        data: PathBuf,
        /// Number of snapshots to keep per site
        #[arg(long, default_value = "5")]
        keep: usize,
    },
    /// Manage authentication tokens
    Token {
        #[command(subcommand)]
        action: TokenAction,
        /// Data directory for storage
        #[arg(long, default_value = "./data")]
        data: PathBuf,
    },
    /// Garbage collect unused chunks
    Gc {
        /// Data directory for storage
        #[arg(long, default_value = "./data")]
        data: PathBuf,
    },
}

#[derive(Subcommand)]
enum TokenAction {
    /// Add a new token
    Add,
    /// List all tokens
    List,
    /// Revoke a token
    Revoke {
        /// Token to revoke
        token: String,
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Archive { dir, output } => {
            let entry = scan_directory(&dir)?
                .next()
                .ok_or("Failed to scan directory")?;
            let (tree, chunks) = build_tree(entry);
            archive::write_archive(&output, &tree, &chunks)?;
            println!("Created archive: {}", output.display());
            println!("  Tree hash: {}", hex::encode(tree.hash()));
            println!("  Chunks: {}", chunks.len());
        }
        Commands::Extract {
            archive: archive_path,
            output,
        } => {
            archive::read_archive(&archive_path, &output)?;
            println!("Extracted to: {}", output.display());
        }
        Commands::Serve {
            http_port,
            sync_port,
            data,
            keep,
        } => {
            let storage = Arc::new(Storage::open(&data)?);

            // Create HTTP server
            let http_router = webpub::server::http::create_router(storage.clone());
            let http_addr = format!("0.0.0.0:{}", http_port);
            let http_listener = TcpListener::bind(&http_addr).await?;
            println!("HTTP server listening on {}", http_addr);

            // Create sync server
            let sync_addr = format!("0.0.0.0:{}", sync_port);
            let sync_listener = TcpListener::bind(&sync_addr).await?;
            println!("Sync server listening on {}", sync_addr);

            // Run both servers concurrently
            let http_server = async {
                axum::serve(http_listener, http_router).await.unwrap();
            };

            let sync_storage = storage.clone();
            let sync_server = async move {
                loop {
                    match sync_listener.accept().await {
                        Ok((stream, addr)) => {
                            println!("Sync connection from {}", addr);
                            let storage = sync_storage.clone();
                            tokio::spawn(webpub::server::sync::handle_connection(
                                stream, storage, keep,
                            ));
                        }
                        Err(e) => {
                            eprintln!("Failed to accept sync connection: {}", e);
                        }
                    }
                }
            };

            tokio::select! {
                _ = http_server => {},
                _ = sync_server => {},
            }
        }
        Commands::Token { action, data } => {
            let storage = Storage::open(&data)?;

            match action {
                TokenAction::Add => {
                    let token = storage.add_token()?;
                    println!("{}", token);
                }
                TokenAction::List => {
                    let tokens = storage.list_tokens()?;
                    if tokens.is_empty() {
                        println!("No tokens found");
                    } else {
                        for token in tokens {
                            println!("{}", token);
                        }
                    }
                }
                TokenAction::Revoke { token } => {
                    storage.revoke_token(&token)?;
                    println!("Token revoked");
                }
            }
        }
        Commands::Gc { data: _ } => {
            println!("Garbage collection not yet implemented");
        }
    }

    Ok(())
}
