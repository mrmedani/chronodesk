use crate::network::signaling::SignalCommand as SignalingCommand;
use crate::network::signaling::SignalEvent;
use crate::protocol::ChannelMessage;
use anyhow::Result;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::sync::Mutex;
use webrtc::api::interceptor_registry::register_default_interceptors;
use webrtc::api::media_engine::MediaEngine;
use webrtc::api::APIBuilder;
use webrtc::data_channel::RTCDataChannel;
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
    MessageReceived { msg: ChannelMessage },
    Error { msg: String },
}

pub struct Transport {
    peer_id: String,
    _pc: Arc<RTCPeerConnection>,
    _event_tx: mpsc::UnboundedSender<TransportEvent>,
    signal_tx: mpsc::UnboundedSender<SignalCommand>,
    data_channel: Arc<Mutex<Option<Arc<RTCDataChannel>>>>,
}

pub struct TurnConfig {
    pub url: String,
    pub username: String,
    pub credential: String,
}

pub(crate) enum SignalCommand {
    CreateOffer(String),
    HandleOffer(String, String),
    HandleAnswer(String, String),
    HandleIceCandidate(String, String, String, u16),
    SendMessage(ChannelMessage),
    Disconnect,
}

impl Transport {
    pub async fn new(
        peer_id: &str,
        stun_addr: &str,
        turn: Option<TurnConfig>,
        signaling_tx: Option<mpsc::UnboundedSender<SignalingCommand>>,
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

        let mut ice_servers = vec![RTCIceServer {
            urls: vec![stun_addr.to_string()],
            ..Default::default()
        }];
        if let Some(turn_cfg) = turn {
            ice_servers.push(RTCIceServer {
                urls: vec![turn_cfg.url],
                username: turn_cfg.username,
                credential: turn_cfg.credential,
            });
        }

        let config = RTCConfiguration {
            ice_servers,
            ..Default::default()
        };

        let pc = Arc::new(api.new_peer_connection(config).await?);
        let pc_clone = pc.clone();
        let data_channel: Arc<Mutex<Option<Arc<RTCDataChannel>>>> = Arc::new(Mutex::new(None));
        let dc_store = data_channel.clone();

        let current_peer: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
        let current_peer_for_state = current_peer.clone();

        let event_tx_clone = event_tx.clone();
        pc.on_peer_connection_state_change(Box::new(move |state| {
            let tx = event_tx_clone.clone();
            let cp = current_peer_for_state.clone();
            Box::pin(async move {
                let peer = cp.lock().await.clone().unwrap_or_default();
                match state {
                    RTCPeerConnectionState::Connected => {
                        let _ = tx.send(TransportEvent::Connected { peer_id: peer });
                    }
                    RTCPeerConnectionState::Disconnected
                    | RTCPeerConnectionState::Failed
                    | RTCPeerConnectionState::Closed => {
                        let _ = tx.send(TransportEvent::Disconnected { peer_id: peer });
                    }
                    _ => {}
                }
            })
        }));

        let event_tx_dc = event_tx.clone();
        pc.on_data_channel(Box::new(move |dc: Arc<RTCDataChannel>| {
            let tx = event_tx_dc.clone();
            let dc_store = dc_store.clone();
            Box::pin(async move {
                *dc_store.lock().await = Some(dc.clone());
                let tx2 = tx.clone();
                dc.on_message(Box::new(move |msg| {
                    let tx = tx2.clone();
                    Box::pin(async move {
                        if let Ok(cmsg) = bincode::deserialize::<ChannelMessage>(&msg.data) {
                            let _ = tx.send(TransportEvent::MessageReceived { msg: cmsg });
                        }
                    })
                }));
            })
        }));

        let signaling_tx_for_ice = signaling_tx.clone();
        let current_peer_for_ice = current_peer.clone();
        pc.on_ice_candidate(Box::new(move |candidate| {
            let sig_tx = signaling_tx_for_ice.clone();
            let cp = current_peer_for_ice.clone();
            Box::pin(async move {
                if let Some(c) = candidate {
                    if let Some(ref tx) = sig_tx {
                        let target = match cp.lock().await.clone() {
                            Some(t) => t,
                            None => return,
                        };
                        let candidate_str = c.to_string();
                        let _ = tx.send(SignalingCommand::SendIceCandidate {
                            to: target,
                            candidate: candidate_str,
                            sdp_mid: "0".to_string(),
                            sdp_mline_index: 0,
                        });
                    }
                }
            })
        }));

        let event_tx_for_signal = event_tx.clone();
        let signal_tx_for_spawn = signal_tx.clone();
        let dc_for_spawn = data_channel.clone();
        let current_peer_for_spawn = current_peer.clone();
        let signaling_tx_for_spawn = signaling_tx.clone();

        tokio::spawn(async move {
            while let Some(cmd) = signal_rx.recv().await {
                match cmd {
                    SignalCommand::CreateOffer(target) => {
                        *current_peer_for_spawn.lock().await = Some(target.clone());
                        match create_and_send_offer(
                            &pc_clone,
                            &target,
                            &signal_tx_for_spawn,
                            &signaling_tx_for_spawn,
                            &dc_for_spawn,
                            &event_tx_for_signal,
                        )
                        .await
                        {
                            Ok(_) => {}
                            Err(e) => {
                                let _ = event_tx_for_signal
                                    .send(TransportEvent::Error { msg: e.to_string() });
                            }
                        }
                    }
                    SignalCommand::HandleOffer(from, sdp) => {
                        *current_peer_for_spawn.lock().await = Some(from.clone());
                        match handle_incoming_offer(
                            &pc_clone,
                            &from,
                            &sdp,
                            &signaling_tx_for_spawn,
                            &event_tx_for_signal,
                        )
                        .await
                        {
                            Ok(_) => {}
                            Err(e) => {
                                let _ = event_tx_for_signal
                                    .send(TransportEvent::Error { msg: e.to_string() });
                            }
                        }
                    }
                    SignalCommand::HandleAnswer(_from, sdp) => {
                        match RTCSessionDescription::answer(sdp) {
                            Ok(desc) => {
                                if let Err(e) = pc_clone.set_remote_description(desc).await {
                                    let _ = event_tx_for_signal
                                        .send(TransportEvent::Error { msg: e.to_string() });
                                }
                            }
                            Err(e) => {
                                let _ = event_tx_for_signal
                                    .send(TransportEvent::Error { msg: e.to_string() });
                            }
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
                    SignalCommand::SendMessage(msg) => {
                        let dc = dc_for_spawn.lock().await;
                        if let Some(dc) = dc.as_ref() {
                            if let Ok(data) = bincode::serialize(&msg) {
                                let _ = dc.send(&bytes::Bytes::from(data)).await;
                            }
                        }
                    }
                    SignalCommand::Disconnect => {
                        let _ = pc_clone.close().await;
                        break;
                    }
                }
            }
        });

        let self_signal_tx = signal_tx.clone();
        let transport = Self {
            peer_id: peer_id.to_string(),
            _pc: pc,
            _event_tx: event_tx,
            signal_tx: self_signal_tx,
            data_channel,
        };

        Ok((transport, event_rx))
    }

    pub(crate) fn signal_tx(&self) -> mpsc::UnboundedSender<SignalCommand> {
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
                let _ = self.signal_tx.send(SignalCommand::HandleAnswer(from, sdp));
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

    pub async fn send_message(&self, msg: &ChannelMessage) -> Result<()> {
        let dc = self.data_channel.lock().await;
        if let Some(dc) = dc.as_ref() {
            let data = bincode::serialize(msg)?;
            dc.send(&bytes::Bytes::from(data)).await?;
        }
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
    _: &mpsc::UnboundedSender<SignalCommand>,
    signaling_tx: &Option<mpsc::UnboundedSender<SignalingCommand>>,
    dc_store: &Arc<Mutex<Option<Arc<RTCDataChannel>>>>,
    event_tx: &mpsc::UnboundedSender<TransportEvent>,
) -> Result<()> {
    let dc = pc.create_data_channel("chronodesk", None).await?;
    *dc_store.lock().await = Some(dc.clone());

    let tx = event_tx.clone();
    dc.on_message(Box::new(move |msg| {
        let tx = tx.clone();
        Box::pin(async move {
            if let Ok(cmsg) = bincode::deserialize::<ChannelMessage>(&msg.data) {
                let _ = tx.send(TransportEvent::MessageReceived { msg: cmsg });
            }
        })
    }));

    let offer = pc.create_offer(None).await?;
    pc.set_local_description(offer.clone()).await?;

    if let Some(desc) = pc.local_description().await {
        if let Some(ref sig_tx) = signaling_tx {
            let _ = sig_tx.send(SignalingCommand::SendOffer {
                to: target.to_string(),
                sdp: desc.sdp,
            });
        }
    }

    Ok(())
}

async fn handle_incoming_offer(
    pc: &RTCPeerConnection,
    _from: &str,
    sdp: &str,
    signaling_tx: &Option<mpsc::UnboundedSender<SignalingCommand>>,
    _event_tx: &mpsc::UnboundedSender<TransportEvent>,
) -> Result<()> {
    let desc = RTCSessionDescription::offer(sdp.to_string())?;
    pc.set_remote_description(desc).await?;

    let answer = pc.create_answer(None).await?;
    pc.set_local_description(answer.clone()).await?;

    if let Some(desc) = pc.local_description().await {
        if let Some(ref sig_tx) = signaling_tx {
            let _ = sig_tx.send(SignalingCommand::SendAnswer {
                to: _from.to_string(),
                sdp: desc.sdp,
            });
        }
    }

    Ok(())
}
