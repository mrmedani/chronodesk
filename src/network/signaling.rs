use anyhow::Result;
use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum SignalMessage {
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

pub enum SignalEvent {
    Offer {
        from: String,
        sdp: String,
    },
    Answer {
        from: String,
        sdp: String,
    },
    IceCandidate {
        from: String,
        candidate: String,
        sdp_mid: String,
        sdp_mline_index: u16,
    },
    PeerList(Vec<String>),
    Error(String),
}

#[allow(dead_code)]
pub(crate) enum SignalCommand {
    SendOffer {
        to: String,
        sdp: String,
    },
    SendAnswer {
        to: String,
        sdp: String,
    },
    SendIceCandidate {
        to: String,
        candidate: String,
        sdp_mid: String,
        sdp_mline_index: u16,
    },
    Disconnect,
}

pub struct SignalingClient {
    server_url: String,
    peer_id: String,
    cmd_tx: mpsc::UnboundedSender<SignalCommand>,
    event_tx: mpsc::UnboundedSender<SignalEvent>,
    cmd_rx: mpsc::UnboundedReceiver<SignalCommand>,
}

impl SignalingClient {
    pub fn new(server_url: &str, peer_id: &str) -> (Self, mpsc::UnboundedReceiver<SignalEvent>) {
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();
        (
            Self {
                server_url: server_url.to_string(),
                peer_id: peer_id.to_string(),
                cmd_tx,
                event_tx,
                cmd_rx,
            },
            event_rx,
        )
    }

    #[allow(dead_code)]
    pub(crate) fn channel(&self) -> mpsc::UnboundedSender<SignalCommand> {
        self.cmd_tx.clone()
    }

    pub async fn run(mut self) -> Result<()> {
        let url = format!("ws://{}/ws", self.server_url);
        let (ws, _) = connect_async(&url).await?;
        let (mut write, mut read) = ws.split();

        let register = SignalMessage::Register {
            peer_id: self.peer_id.clone(),
        };
        write
            .send(Message::Text(serde_json::to_string(&register)?))
            .await?;

        loop {
            tokio::select! {
                msg = read.next() => {
                    match msg {
                        Some(Ok(Message::Text(text))) => {
                            if let Ok(signal) = serde_json::from_str::<SignalMessage>(&text) {
                                self.handle_signal(signal);
                            }
                        }
                        Some(Ok(Message::Close(_))) | None => break,
                        _ => continue,
                    }
                }
                cmd = self.cmd_rx.recv() => {
                    match cmd {
                        Some(cmd) => {
                            match cmd {
                                SignalCommand::SendOffer { to, sdp } => {
                                    let msg = SignalMessage::Offer {
                                        from: self.peer_id.clone(),
                                        to, sdp,
                                    };
                                    write.send(Message::Text(serde_json::to_string(&msg)?)).await?;
                                }
                                SignalCommand::SendAnswer { to, sdp } => {
                                    let msg = SignalMessage::Answer {
                                        from: self.peer_id.clone(),
                                        to, sdp,
                                    };
                                    write.send(Message::Text(serde_json::to_string(&msg)?)).await?;
                                }
                                SignalCommand::SendIceCandidate { to, candidate, sdp_mid, sdp_mline_index } => {
                                    let msg = SignalMessage::IceCandidate {
                                        from: self.peer_id.clone(),
                                        to, candidate, sdp_mid, sdp_mline_index,
                                    };
                                    write.send(Message::Text(serde_json::to_string(&msg)?)).await?;
                                }
                                SignalCommand::Disconnect => break,
                            }
                        }
                        None => break,
                    }
                }
            }
        }

        Ok(())
    }

    fn handle_signal(&self, msg: SignalMessage) {
        match msg {
            SignalMessage::Offer { from, sdp, .. } => {
                let _ = self.event_tx.send(SignalEvent::Offer { from, sdp });
            }
            SignalMessage::Answer { from, sdp, .. } => {
                let _ = self.event_tx.send(SignalEvent::Answer { from, sdp });
            }
            SignalMessage::IceCandidate {
                from,
                candidate,
                sdp_mid,
                sdp_mline_index,
                ..
            } => {
                let _ = self.event_tx.send(SignalEvent::IceCandidate {
                    from,
                    candidate,
                    sdp_mid,
                    sdp_mline_index,
                });
            }
            SignalMessage::PeerList { peers } => {
                let _ = self.event_tx.send(SignalEvent::PeerList(peers));
            }
            SignalMessage::Error { msg } => {
                let _ = self.event_tx.send(SignalEvent::Error(msg));
            }
            _ => {}
        }
    }
}
