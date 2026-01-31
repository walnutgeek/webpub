use clap::{Parser, Subcommand};
use std::path::PathBuf;
use webpub::{archive, build_tree, scan_directory};

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
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
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
        Commands::Extract { archive: archive_path, output } => {
            archive::read_archive(&archive_path, &output)?;
            println!("Extracted to: {}", output.display());
        }
    }

    Ok(())
}
