use anyhow::Result;
use crate::network::signaling::{SignalEvent, SignalingClient};
use crate::network::transport::{Transport, TransportEvent};
use crate::protocol::ChannelMessage;

pub async fn run_host(signaling_addr: &str, peer_id: Option<String>) -> Result<()> {
    let my_id = peer_id.unwrap_or_else(|| {
        let id = uuid::Uuid::new_v4();
        id.to_string()[..8].to_string()
    });

    tracing::info!("CHRONODESK host starting as: {my_id}");

    let (transport, mut transport_events) =
        Transport::new(&my_id, "stun:stun.l.google.com:19302").await?;

    let (signaling, mut signal_events) =
        SignalingClient::new(signaling_addr, &my_id);

    tokio::spawn(async move {
        if let Err(e) = signaling.run().await {
            tracing::error!("Signaling error: {e}");
        }
    });

    let mut capture = crate::capture::ScreenCapture::new()?;
    let mut encoder = crate::video::VideoEncoder::new(crate::video::EncoderType::Auto, 1920, 1080)?;
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

pub async fn run_client(
    signaling_addr: &str,
    peer_id: Option<String>,
    connect_to: Option<String>,
) -> Result<()> {
    let my_id = peer_id.unwrap_or_else(|| {
        let id = uuid::Uuid::new_v4();
        id.to_string()[..8].to_string()
    });

    tracing::info!("CHRONODESK client starting as: {my_id}");

    let (mut transport, mut transport_events) =
        Transport::new(&my_id, "stun:stun.l.google.com:19302").await?;

    let (signaling, mut signal_events) =
        SignalingClient::new(signaling_addr, &my_id);

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
            let mut input = crate::input::InputController::new()?;
            input.mouse_move(x, y)?;
        }
        ChannelMessage::InputClick { button, pressed } => {
            let mut input = crate::input::InputController::new()?;
            if pressed {
                input.mouse_down(button)?;
            } else {
                input.mouse_up(button)?;
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
