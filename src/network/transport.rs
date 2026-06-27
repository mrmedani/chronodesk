use crate::capture::CapturedFrame;
use crate::network::signaling::SignalEvent;
use anyhow::Result;
use std::sync::Arc;
use tokio::sync::mpsc;
use webrtc::api::interceptor_registry::register_default_interceptors;
use webrtc::api::media_engine::MediaEngine;
use webrtc::api::APIBuilder;
use webrtc::ice_transport::ice_candidate::RTCIceCandidateInit;
use webrtc::ice_transport::ice_server::RTCIceServer;
use webrtc::interceptor::registry::Registry;
use webrtc::peer_connection::configuration::RTCConfiguration;
use webrtc::peer_connection::peer_connection_state::RTCPeerConnectionState;
use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;
use webrtc::peer_connection::RTCPeerConnection;

pub enum TransportEvent {
    Connected { peer_id: String },
    Disconnected { peer_id: String },
    DataReceived { data: Vec<u8> },
    Error { msg: String },
}

pub struct Transport {
    peer_id: String,
    pc: Arc<RTCPeerConnection>,
    event_tx: mpsc::UnboundedSender<TransportEvent>,
    signal_tx: mpsc::UnboundedSender<SignalCommand>,
}

enum SignalCommand {
    CreateOffer(String),
    HandleOffer(String, String),
    HandleAnswer(String, String),
    HandleIceCandidate(String, String, String, u16),
    Disconnect,
}

impl Transport {
    pub async fn new(
        peer_id: &str,
        stun_addr: &str,
    ) -> Result<(Self, mpsc::UnboundedReceiver<TransportEvent>)> {
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let (signal_tx, mut signal_rx) = mpsc::unbounded_channel::<SignalCommand>();

        let mut m = MediaEngine::default();
        m.register_default_codecs()?;

        let mut registry = Registry::new();
        registry = register_default_interceptors(registry, &mut m)?;

        let api = APIBuilder::new()
            .with_media_engine(m)
            .with_interceptor_registry(registry)
            .build();

        let config = RTCConfiguration {
            ice_servers: vec![RTCIceServer {
                urls: vec![stun_addr.to_string()],
                ..Default::default()
            }],
            ..Default::default()
        };

        let pc = Arc::new(api.new_peer_connection(config).await?);
        let pc_clone = pc.clone();

        let event_tx_clone = event_tx.clone();
        pc.on_peer_connection_state_change(Box::new(move |state| {
            let tx = event_tx_clone.clone();
            Box::pin(async move {
                match state {
                    RTCPeerConnectionState::Connected => {
                        let _ = tx.send(TransportEvent::Connected {
                            peer_id: String::new(),
                        });
                    }
                    RTCPeerConnectionState::Disconnected
                    | RTCPeerConnectionState::Failed
                    | RTCPeerConnectionState::Closed => {
                        let _ = tx.send(TransportEvent::Disconnected {
                            peer_id: String::new(),
                        });
                    }
                    _ => {}
                }
            })
        }));

        let event_tx_for_signal = event_tx.clone();
        let self_signal_tx = signal_tx.clone();
        let signal_tx_for_spawn = signal_tx.clone();

        tokio::spawn(async move {
            while let Some(cmd) = signal_rx.recv().await {
                match cmd {
                    SignalCommand::CreateOffer(target) => {
                        match create_and_send_offer(&pc_clone, &target, &signal_tx_for_spawn).await {
                            Ok(_) => {}
                            Err(e) => {
                                let _ = event_tx_for_signal
                                    .send(TransportEvent::Error { msg: e.to_string() });
                            }
                        }
                    }
                    SignalCommand::HandleOffer(from, sdp) => {
                        match handle_incoming_offer(&pc_clone, &from, &sdp, &signal_tx_for_spawn).await {
                            Ok(_) => {}
                            Err(e) => {
                                let _ = event_tx_for_signal
                                    .send(TransportEvent::Error { msg: e.to_string() });
                            }
                        }
                    }
                    SignalCommand::HandleAnswer(_from, sdp) => {
                        let desc = RTCSessionDescription::answer(sdp).unwrap();
                        if let Err(e) = pc_clone.set_remote_description(desc).await {
                            let _ = event_tx_for_signal
                                .send(TransportEvent::Error { msg: e.to_string() });
                        }
                    }
                    SignalCommand::HandleIceCandidate(
                        _from,
                        candidate,
                        sdp_mid,
                        sdp_mline_index,
                    ) => {
                        let c = RTCIceCandidateInit {
                            candidate,
                            sdp_mid: Some(sdp_mid),
                            sdp_mline_index: Some(sdp_mline_index),
                            username_fragment: None,
                        };
                        if let Err(e) = pc_clone.add_ice_candidate(c).await {
                            let _ = event_tx_for_signal
                                .send(TransportEvent::Error { msg: e.to_string() });
                        }
                    }
                    SignalCommand::Disconnect => {
                        let _ = pc_clone.close().await;
                        break;
                    }
                }
            }
        });

        let transport = Self {
            peer_id: peer_id.to_string(),
            pc,
            event_tx,
            signal_tx: self_signal_tx,
        };

        Ok((transport, event_rx))
    }

    pub fn signal_tx(&self) -> mpsc::UnboundedSender<SignalCommand> {
        self.signal_tx.clone()
    }

    pub async fn connect_to(&mut self, target_id: &str) -> Result<()> {
        let _ = self
            .signal_tx
            .send(SignalCommand::CreateOffer(target_id.to_string()));
        Ok(())
    }

    pub fn handle_signal_event(&self, event: SignalEvent) {
        match event {
            SignalEvent::Offer { from, sdp } => {
                let _ = self.signal_tx.send(SignalCommand::HandleOffer(from, sdp));
            }
            SignalEvent::Answer { from, sdp } => {
                let _ = self
                    .signal_tx
                    .send(SignalCommand::HandleAnswer(from, sdp));
            }
            SignalEvent::IceCandidate {
                from,
                candidate,
                sdp_mid,
                sdp_mline_index,
            } => {
                let _ = self.signal_tx.send(SignalCommand::HandleIceCandidate(
                    from,
                    candidate,
                    sdp_mid,
                    sdp_mline_index,
                ));
            }
            _ => {}
        }
    }

    pub async fn send_data(&self, data: &[u8]) -> Result<()> {
        let _ = data;
        Ok(())
    }

    pub async fn send_frame(&self, _frame: &CapturedFrame) -> Result<()> {
        Ok(())
    }

    pub async fn disconnect(&mut self) -> Result<()> {
        let _ = self.signal_tx.send(SignalCommand::Disconnect);
        Ok(())
    }

    pub fn peer_id(&self) -> &str {
        &self.peer_id
    }
}

async fn create_and_send_offer(
    pc: &RTCPeerConnection,
    target: &str,
    signal_tx: &mpsc::UnboundedSender<SignalCommand>,
) -> Result<()> {
    let _dc = pc.create_data_channel("chronodesk", None).await?;

    let offer = pc.create_offer(None).await?;
    pc.set_local_description(offer.clone()).await?;

    if let Some(desc) = pc.local_description().await {
        let _ = signal_tx.send(SignalCommand::HandleOffer(target.to_string(), desc.sdp));
    }

    Ok(())
}

async fn handle_incoming_offer(
    pc: &RTCPeerConnection,
    _from: &str,
    sdp: &str,
    signal_tx: &mpsc::UnboundedSender<SignalCommand>,
) -> Result<()> {
    let desc = RTCSessionDescription::offer(sdp.to_string())?;
    pc.set_remote_description(desc).await?;

    let answer = pc.create_answer(None).await?;
    pc.set_local_description(answer.clone()).await?;

    if let Some(desc) = pc.local_description().await {
        let _ = signal_tx.send(SignalCommand::HandleOffer(String::new(), desc.sdp));
    }

    Ok(())
}
