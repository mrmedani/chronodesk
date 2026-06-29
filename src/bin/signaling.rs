use anyhow::Result;
use clap::Parser;
use dashmap::DashMap;
use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio_tungstenite::accept_async;
use tokio_tungstenite::tungstenite::Message;

#[derive(Parser)]
#[command(name = "signaling-server")]
struct Args {
    #[arg(short, long, default_value = "0.0.0.0:21116")]
    bind: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
enum SignalMessage {
    Register {
        peer_id: String,
    },
    Offer {
        from: String,
        to: String,
        sdp: String,
    },
    Answer {
        from: String,
        to: String,
        sdp: String,
    },
    IceCandidate {
        from: String,
        to: String,
        candidate: String,
        sdp_mid: String,
        sdp_mline_index: u16,
    },
    PeerList {
        peers: Vec<String>,
    },
    Error {
        msg: String,
    },
}

type PeerMap = Arc<DashMap<String, tokio::sync::mpsc::UnboundedSender<SignalMessage>>>;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    let args = Args::parse();

    let peers: PeerMap = Arc::new(DashMap::new());
    let listener = TcpListener::bind(&args.bind).await?;
    tracing::info!("Signaling server listening on {}", args.bind);

    while let Ok((stream, addr)) = listener.accept().await {
        let peers = peers.clone();
        tokio::spawn(handle_connection(stream, addr, peers));
    }

    Ok(())
}

async fn handle_connection(
    stream: tokio::net::TcpStream,
    addr: std::net::SocketAddr,
    peers: PeerMap,
) {
    let ws = match accept_async(stream).await {
        Ok(ws) => ws,
        Err(e) => {
            tracing::error!("WebSocket handshake failed from {addr}: {e}");
            return;
        }
    };

    let (mut ws_sender, mut ws_receiver) = ws.split();
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<SignalMessage>();

    let mut peer_id: Option<String> = None;

    let send_task = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if let Ok(payload) = serde_json::to_string(&msg) {
                if ws_sender.send(Message::Text(payload)).await.is_err() {
                    break;
                }
            }
        }
    });

    while let Some(Ok(msg)) = ws_receiver.next().await {
        let text = match msg {
            Message::Text(t) => t,
            Message::Close(_) => break,
            _ => continue,
        };

        let signal: SignalMessage = match serde_json::from_str(&text) {
            Ok(s) => s,
            Err(_) => continue,
        };

        match signal {
            SignalMessage::Register { peer_id: id } => {
                peers.insert(id.clone(), tx.clone());
                peer_id = Some(id.clone());
                tracing::info!("Peer registered: {id} from {addr}");

                let peer_list: Vec<String> = peers.iter().map(|e| e.key().clone()).collect();
                let _ = tx.send(SignalMessage::PeerList { peers: peer_list });
            }
            SignalMessage::Offer { from, to, sdp } => {
                if let Some(peer) = peers.get(&to) {
                    let _ = peer.send(SignalMessage::Offer { from, to, sdp });
                } else if let Some(peer) = peers.get(&from) {
                    let _ = peer.send(SignalMessage::Error {
                        msg: format!("Peer {to} not found"),
                    });
                }
            }
            SignalMessage::Answer { from, to, sdp } => {
                if let Some(peer) = peers.get(&to) {
                    let _ = peer.send(SignalMessage::Answer { from, to, sdp });
                } else if let Some(peer) = peers.get(&from) {
                    let _ = peer.send(SignalMessage::Error {
                        msg: format!("Peer {to} not found"),
                    });
                }
            }
            SignalMessage::IceCandidate {
                from,
                to,
                candidate,
                sdp_mid,
                sdp_mline_index,
            } => {
                if let Some(peer) = peers.get(&to) {
                    let _ = peer.send(SignalMessage::IceCandidate {
                        from,
                        to,
                        candidate,
                        sdp_mid,
                        sdp_mline_index,
                    });
                }
            }
            _ => {}
        }
    }

    if let Some(id) = peer_id {
        peers.remove(&id);
        tracing::info!("Peer unregistered: {id}");
    }

    send_task.abort();
}
