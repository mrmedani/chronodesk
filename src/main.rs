#![allow(dead_code)]

mod capture;
mod input;
mod network;
mod protocol;
mod video;

use anyhow::Result;
use clap::Parser;
use network::signaling::SignalEvent;
use network::transport::{Transport, TransportEvent};
use protocol::ChannelMessage;

#[derive(Parser)]
#[command(name = "CHRONODESK")]
#[command(about = "Open-source remote desktop software")]
enum Cli {
    Host {
        #[arg(short, long, default_value = "144.24.201.196:21116")]
        signaling: String,

        #[arg(short, long)]
        peer_id: Option<String>,
    },
    Client {
        #[arg(short, long, default_value = "144.24.201.196:21116")]
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
        Cli::Host { signaling, peer_id } => run_host(&signaling, peer_id).await?,
        Cli::Client {
            signaling,
            peer_id,
            connect,
        } => run_client(&signaling, peer_id, connect).await?,
        Cli::Server { bind } => run_server(&bind).await?,
    }

    Ok(())
}

async fn run_host(signaling_addr: &str, peer_id: Option<String>) -> Result<()> {
    let my_id = peer_id.unwrap_or_else(|| {
        let id = uuid::Uuid::new_v4();
        id.to_string()[..8].to_string()
    });

    tracing::info!("CHRONODESK host starting as: {my_id}");

    let (signaling, mut signal_events) =
        network::signaling::SignalingClient::new(signaling_addr, &my_id);

    let (transport, mut transport_events) = Transport::new(
        &my_id,
        "stun:stun.l.google.com:19302",
        Some(signaling.channel()),
    )
    .await?;

    tokio::spawn(async move {
        if let Err(e) = signaling.run().await {
            tracing::error!("Signaling error: {e}");
        }
    });

    let mut capture = capture::ScreenCapture::new()?;
    let mut encoder = video::VideoEncoder::new(video::EncoderType::Auto, 1920, 1080)?;
    let mut connected = false;

    loop {
        tokio::select! {
            Some(event) = signal_events.recv() => {
                trace_signal_event(&event);
                transport.handle_signal_event(event);
            }
            Some(event) = transport_events.recv() => {
                match event {
                    TransportEvent::Connected { .. } => {
                        tracing::info!("P2P connected - streaming screen");
                        connected = true;
                    }
                    TransportEvent::Disconnected { .. } => {
                        tracing::info!("Disconnected");
                        break;
                    }
                    TransportEvent::MessageReceived { msg } => {
                        handle_host_message(msg).await?;
                    }
                    TransportEvent::Error { msg } => {
                        tracing::error!("Transport error: {msg}");
                    }
                }
            }
            else => break,
        }

        if connected {
            if let Ok(frames) = capture.capture_all() {
                for frame in &frames {
                    if frame.dirty_rects.is_empty() {
                        continue;
                    }
                    match encoder.encode(&frame.data) {
                        Ok(packets) => {
                            for pkt in &packets {
                                let msg = ChannelMessage::VideoFrame {
                                    width: frame.width as u32,
                                    height: frame.height as u32,
                                    codec: if pkt.codec == "jpeg" { 0 } else { 1 },
                                    data: pkt.data.clone(),
                                };
                                let _ = transport.send_message(&msg).await;
                            }
                        }
                        Err(e) => tracing::warn!("Encode error: {e}"),
                    }
                }
            }
        }
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

    tracing::info!("CHRONODESK client starting as: {my_id}");

    let (signaling, mut signal_events) =
        network::signaling::SignalingClient::new(signaling_addr, &my_id);

    let (mut transport, mut transport_events) = Transport::new(
        &my_id,
        "stun:stun.l.google.com:19302",
        Some(signaling.channel()),
    )
    .await?;

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
                trace_signal_event(&event);
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
                    TransportEvent::MessageReceived { msg } => {
                        handle_client_message(msg);
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
    tracing::warn!(
        "Run 'cargo run --bin signaling-server -- --bind {_bind}' for the signaling server"
    );
    Ok(())
}

fn trace_signal_event(event: &SignalEvent) {
    match event {
        SignalEvent::Offer { from, .. } => tracing::info!("Signal: offer from {from}"),
        SignalEvent::Answer { from, .. } => tracing::info!("Signal: answer from {from}"),
        SignalEvent::IceCandidate { from, .. } => tracing::debug!("Signal: ICE from {from}"),
        SignalEvent::PeerList(peers) => tracing::info!("Peers online: {:?}", peers),
        SignalEvent::Error(msg) => tracing::error!("Signal error: {msg}"),
    }
}

async fn handle_host_message(msg: ChannelMessage) -> Result<()> {
    match msg {
        ChannelMessage::InputMove { x, y } => {
            let mut inp = input::InputController::new()?;
            inp.mouse_move(x, y)?;
        }
        ChannelMessage::InputClick { button, pressed } => {
            let mut inp = input::InputController::new()?;
            if pressed {
                inp.mouse_down(button)?;
            } else {
                inp.mouse_up(button)?;
            }
        }
        ChannelMessage::InputKey { key: _, pressed: _ } => {
            tracing::debug!("Key event ignored (stub)");
        }
        ChannelMessage::Clipboard { text } => {
            tracing::info!("Clipboard received: {} chars", text.len());
        }
        ChannelMessage::Ping { timestamp } => {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as i64;
            tracing::debug!("Ping: {}ms", now - timestamp);
        }
        _ => {}
    }
    Ok(())
}

fn handle_client_message(msg: ChannelMessage) {
    #[allow(clippy::single_match)]
    match msg {
        ChannelMessage::VideoFrame {
            width,
            height,
            codec,
            data,
        } => {
            let codec_name = if codec == 0 { "JPEG" } else { "H.264" };
            tracing::info!(
                "Video frame: {}x{}, codec={}, size={} bytes",
                width,
                height,
                codec_name,
                data.len()
            );
        }
        _ => {}
    }
}
