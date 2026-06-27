mod capture;
mod crypto;
mod input;
mod network;
mod video;

use anyhow::Result;
use clap::Parser;
use network::signaling::SignalEvent;
use network::transport::{Transport, TransportEvent};
use tracing_subscriber;

#[derive(Parser)]
#[command(name = "CHRONODESK")]
#[command(about = "Open-source remote desktop software")]
enum Cli {
    Client {
        #[arg(short, long, default_value = "127.0.0.1:21116")]
        signaling: String,

        #[arg(short, long)]
        peer_id: Option<String>,

        #[arg(short, long)]
        connect: Option<String>,
    },
    Server {
        #[arg(short, long, default_value = "0.0.0.0:21116")]
        bind: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    match Cli::parse() {
        Cli::Client {
            signaling,
            peer_id,
            connect,
        } => run_client(&signaling, peer_id, connect).await?,
        Cli::Server { bind } => run_server(&bind).await?,
    }

    Ok(())
}

async fn run_client(
    signaling_addr: &str,
    peer_id: Option<String>,
    connect_to: Option<String>,
) -> Result<()> {
    let my_id = peer_id.unwrap_or_else(|| {
        let id = uuid::Uuid::new_v4();
        id.to_string()[..8].to_string()
    });

    tracing::info!("Starting CHRONODESK client as: {my_id}");

    let (mut transport, mut transport_events) =
        Transport::new(&my_id, "stun:stun.l.google.com:19302").await?;

    let (signaling, mut signal_events) =
        network::signaling::SignalingClient::new(signaling_addr, &my_id);

    tokio::spawn(async move {
        if let Err(e) = signaling.run().await {
            tracing::error!("Signaling error: {e}");
        }
    });

    if let Some(target) = connect_to {
        tracing::info!("Connecting to {target}...");
        transport.connect_to(&target).await?;
    }

    loop {
        tokio::select! {
            Some(event) = signal_events.recv() => {
                match &event {
                    SignalEvent::Offer { from, .. } => {
                        tracing::info!("Got offer from: {from}");
                    }
                    SignalEvent::Answer { from, .. } => {
                        tracing::info!("Got answer from: {from}");
                    }
                    SignalEvent::PeerList(peers) => {
                        tracing::info!("Peers online: {:?}", peers);
                    }
                    SignalEvent::Error(msg) => {
                        tracing::error!("Signal error: {msg}");
                    }
                    _ => {}
                }
                transport.handle_signal_event(event);
            }
            Some(event) = transport_events.recv() => {
                match event {
                    TransportEvent::Connected { .. } => {
                        tracing::info!("P2P connection established!");
                    }
                    TransportEvent::Disconnected { .. } => {
                        tracing::info!("Disconnected");
                        break;
                    }
                    TransportEvent::DataReceived { data } => {
                        tracing::info!("Received {} bytes", data.len());
                    }
                    TransportEvent::Error { msg } => {
                        tracing::error!("Transport error: {msg}");
                    }
                }
            }
            else => break,
        }
    }

    Ok(())
}

async fn run_server(_bind: &str) -> Result<()> {
    tracing::warn!("Run 'cargo run --bin signaling-server -- --bind {_bind}' for the signaling server");
    Ok(())
}
