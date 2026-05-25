use anyhow::Result;
use clap::{command, Parser, Subcommand};
use dotenv::dotenv;

#[derive(Debug, Parser)]
#[command(name = "kv-downloader")]
#[command(version, about, long_about)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    #[arg(global = true, long, help = "enable debug logging")]
    debug: bool,
}

#[derive(Debug, Subcommand)]
enum Commands {
    Auth,
    Logout,
    #[command(arg_required_else_help = true)]
    Download(kv_core::commands::DownloadArgs),
}

fn main() -> Result<()> {
    dotenv().ok();
    let cli = Cli::parse();
    tracing_subscriber::fmt()
        .with_max_level(if cli.debug {
            tracing::Level::DEBUG
        } else {
            tracing::Level::INFO
        })
        .init();
    match cli.command {
        Commands::Auth => kv_core::commands::auth::run()?,
        Commands::Logout => kv_core::commands::logout::run()?,
        Commands::Download(args) => kv_core::commands::Download::run(args)?,
    }

    Ok(())
}
