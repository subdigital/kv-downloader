use anyhow::Result;
use clap::{command, Parser, Subcommand};
use dotenv::dotenv;

mod commands;
mod download_progress;
mod driver;
mod keystore;
mod prompt;
mod tasks;

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
    Download(commands::DownloadArgs),
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
        Commands::Auth => commands::auth::run()?,
        Commands::Logout => commands::logout::run()?,
        Commands::Download(args) => commands::Download::run(args)?,
    }

    Ok(())
}
