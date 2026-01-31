use clap::{Parser, Subcommand};

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
        dir: std::path::PathBuf,
        /// Output archive file
        output: std::path::PathBuf,
    },
    /// Extract archive to directory
    Extract {
        /// Archive file
        archive: std::path::PathBuf,
        /// Output directory
        output: std::path::PathBuf,
    },
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Archive { dir, output } => {
            println!("Archive {:?} -> {:?}", dir, output);
            todo!("archive command")
        }
        Commands::Extract { archive, output } => {
            println!("Extract {:?} -> {:?}", archive, output);
            todo!("extract command")
        }
    }
}
