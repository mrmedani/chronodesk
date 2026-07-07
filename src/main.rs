use anyhow::Result;
use chronodesk::capture::ScreenCapture;
use chronodesk::crypto;
use chronodesk::input::{logical_key_to_enigo, InputController};
use chronodesk::network::signaling::SignalEvent;
use chronodesk::network::signaling::SignalingClient;
use chronodesk::network::transport::{Transport, TransportEvent};
use chronodesk::protocol::ChannelMessage;
use chronodesk::video::{EncoderType, VideoEncoder};
use clap::Parser;

#[derive(Parser)]
#[command(name = "CHRONODESK")]
#[command(about = "Open-source remote desktop software")]
enum Cli {
    Host {
        #[arg(short, long, default_value = "82.70.239.217:21116")]
        signaling: String,

        #[arg(short, long)]
        peer_id: Option<String>,
    },
    Client {
        #[arg(short, long, default_value = "82.70.239.217:21116")]
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

    let auth_token = chronodesk::ffi::compute_auth_token(&my_id);
    let (signaling, mut signal_events) = SignalingClient::new(signaling_addr, &my_id, &auth_token);

    let (transport, mut transport_events) = Transport::new(
        &my_id,
        "stun:stun.l.google.com:19302",
        None,
        Some(signaling.channel()),
    )
    .await?;

    tokio::spawn(async move {
        if let Err(e) = signaling.run().await {
            tracing::error!("Signaling error: {e}");
        }
    });

    let mut capture = ScreenCapture::new()?;
    let mut encoder = VideoEncoder::new(EncoderType::Auto, 1920, 1080)?;
    let mut connected = false;
    let mut local_private_key: Option<ring::agreement::EphemeralPrivateKey> = None;
    let mut crypto_session: Option<crypto::CryptoSession> = None;

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
                        match crypto::generate_keypair() {
                            Ok((priv_key, pub_key)) => {
                                local_private_key = Some(priv_key);
                                let _ = transport.send_message(&ChannelMessage::Handshake { public_key: pub_key }).await;
                                tracing::info!("sent crypto handshake");
                            }
                            Err(e) => tracing::error!("keypair generation failed: {e}"),
                        }
                    }
                    TransportEvent::Disconnected { .. } => {
                        tracing::info!("Disconnected");
                        break;
                    }
                    TransportEvent::MessageReceived { msg } => {
                        handle_host_message_crypto(msg, &mut local_private_key, &mut crypto_session, &transport).await?;
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
                    match encoder.encode(&frame.data, frame.width as u32, frame.height as u32) {
                        Ok(packets) => {
                            for pkt in &packets {
                                let msg = ChannelMessage::VideoFrame {
                                    width: frame.width as u32,
                                    height: frame.height as u32,
                                    codec: if pkt.codec == "webp" {
                                        2
                                    } else if pkt.codec == "jpeg" {
                                        0
                                    } else {
                                        1
                                    },
                                    data: pkt.data.clone(),
                                };
                                if let Some(ref session) = crypto_session {
                                    if let Ok(data) = bincode::serialize(&msg) {
                                        if let Ok(encrypted) = session.encrypt(&data) {
                                            let _ = transport
                                                .send_message(&ChannelMessage::Encrypted {
                                                    data: encrypted,
                                                })
                                                .await;
                                        }
                                    }
                                } else {
                                    let _ = transport.send_message(&msg).await;
                                }
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

    let auth_token = chronodesk::ffi::compute_auth_token(&my_id);
    let (signaling, mut signal_events) = SignalingClient::new(signaling_addr, &my_id, &auth_token);

    let (mut transport, mut transport_events) = Transport::new(
        &my_id,
        "stun:stun.l.google.com:19302",
        None,
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

    let mut crypto_session: Option<crypto::CryptoSession> = None;

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
                        handle_client_message_crypto(msg, &mut crypto_session, &transport).await;
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

async fn handle_client_message_crypto(
    msg: ChannelMessage,
    crypto_session: &mut Option<crypto::CryptoSession>,
    transport: &Transport,
) {
    if let ChannelMessage::Handshake { public_key } = &msg {
        match crypto::generate_keypair() {
            Ok((priv_key, my_pub_key)) => {
                match crypto::compute_shared_secret(priv_key, public_key) {
                    Ok(shared) => match crypto::derive_session_key(&shared) {
                        Ok(key) => match crypto::CryptoSession::new(&key) {
                            Ok(session) => {
                                tracing::info!("crypto handshake complete (client)");
                                *crypto_session = Some(session);
                                let resp = ChannelMessage::Handshake {
                                    public_key: my_pub_key,
                                };
                                let _ = transport.send_message(&resp).await;
                            }
                            Err(e) => tracing::error!("crypto session init failed: {e}"),
                        },
                        Err(e) => tracing::error!("crypto key derivation failed: {e}"),
                    },
                    Err(e) => tracing::error!("crypto shared secret failed: {e}"),
                }
            }
            Err(e) => tracing::error!("crypto viewer keypair failed: {e}"),
        }
        return;
    }

    let inner = match msg {
        ChannelMessage::Encrypted { data } => {
            if let Some(ref session) = crypto_session {
                match session.decrypt(&data) {
                    Ok(plaintext) => match bincode::deserialize::<ChannelMessage>(&plaintext) {
                        Ok(decoded) => decoded,
                        Err(_) => return,
                    },
                    Err(_) => return,
                }
            } else {
                return;
            }
        }
        other => other,
    };

    handle_client_message(inner);
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

async fn handle_host_message_crypto(
    msg: ChannelMessage,
    local_private_key: &mut Option<ring::agreement::EphemeralPrivateKey>,
    crypto_session: &mut Option<crypto::CryptoSession>,
    _transport: &Transport,
) -> Result<()> {
    if let ChannelMessage::Handshake { public_key } = &msg {
        if let Some(priv_key) = local_private_key.take() {
            if let Ok(shared) = crypto::compute_shared_secret(priv_key, public_key) {
                if let Ok(key) = crypto::derive_session_key(&shared) {
                    if let Ok(session) = crypto::CryptoSession::new(&key) {
                        tracing::info!("crypto handshake complete (host)");
                        *crypto_session = Some(session);
                    }
                }
            }
        }
        return Ok(());
    }

    let inner = match msg {
        ChannelMessage::Encrypted { data } => {
            if let Some(ref session) = crypto_session {
                if let Ok(plaintext) = session.decrypt(&data) {
                    if let Ok(decoded) = bincode::deserialize::<ChannelMessage>(&plaintext) {
                        decoded
                    } else {
                        return Ok(());
                    }
                } else {
                    return Ok(());
                }
            } else {
                return Ok(());
            }
        }
        other => other,
    };

    handle_host_message(inner).await
}

async fn handle_host_message(msg: ChannelMessage) -> Result<()> {
    match msg {
        ChannelMessage::InputMove { x, y } => {
            let mut inp = InputController::new()?;
            inp.mouse_move(x, y)?;
        }
        ChannelMessage::InputClick { button, pressed } => {
            let mut inp = InputController::new()?;
            if pressed {
                inp.mouse_down(button)?;
            } else {
                inp.mouse_up(button)?;
            }
        }
        ChannelMessage::InputKey { key, pressed } => {
            if let Some(enigo_key) = logical_key_to_enigo(key) {
                let dir = if pressed {
                    enigo::Direction::Press
                } else {
                    enigo::Direction::Release
                };
                if let Ok(mut inp) = InputController::new() {
                    let _ = inp.key_press(enigo_key, dir);
                }
            }
        }
        ChannelMessage::Clipboard { text } => {
            chronodesk::clipboard::ClipboardSync::write(&text);
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
