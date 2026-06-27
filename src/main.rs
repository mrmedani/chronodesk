mod capture;
mod crypto;
mod input;
mod network;
mod video;

use anyhow::Result;
use clap::Parser;
use tracing_subscriber;

#[derive(Parser)]
#[command(name = "CHRONODESK")]
#[command(about = "Open-source remote desktop software")]
enum Cli {
    Client,
    Server {
        #[arg(short, long, default_value = "0.0.0.0:21116")]
        bind: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    match Cli::parse() {
        Cli::Client => run_client().await?,
        Cli::Server { bind } => run_server(&bind).await?,
    }

    Ok(())
}

async fn run_client() -> Result<()> {
    tracing::info!("Starting CHRONODESK client...");
    Ok(())
}

async fn run_server(bind: &str) -> Result<()> {
    tracing::info!("Starting CHRONODESK server on {bind}...");
    Ok(())
}
