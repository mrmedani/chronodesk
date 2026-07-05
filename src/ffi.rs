use crate::capture::ScreenCapture;
use crate::logger;
use crate::network::signaling::{SignalCommand as SigCmd, SignalEvent, SignalingClient};
use crate::network::transport::{SignalCommand as TrCmd, Transport, TransportEvent};
use crate::protocol::ChannelMessage;
use crate::video::{EncoderType, QualityController, VideoEncoder};
use ring::hmac;
use std::ffi::{CStr, CString};
use std::sync::Mutex;
use std::sync::OnceLock;
use tokio::runtime::Runtime;
use tokio::sync::mpsc;

static RUNTIME: OnceLock<Runtime> = OnceLock::new();
fn rt() -> &'static Runtime {
    RUNTIME.get_or_init(|| Runtime::new().expect("FATAL: Failed to create tokio runtime"))
}

struct AppState {
    peer_id: String,
    events: Vec<String>,
    frame_rgba: Vec<u8>,
    frame_width: u32,
    frame_height: u32,
    frame_ready: bool,
    connected: bool,
    is_host: bool,
    pending_offer: Option<(String, String)>,
    transport_tx: Option<mpsc::UnboundedSender<TrCmd>>,
    signaling_tx: Option<mpsc::UnboundedSender<SigCmd>>,
    crypto_session: Option<crate::crypto::CryptoSession>,
    file_transfer_manager: crate::file_transfer::FileTransferManager,
}

static STATE: OnceLock<Mutex<AppState>> = OnceLock::new();

fn state() -> &'static Mutex<AppState> {
    STATE.get_or_init(|| {
        Mutex::new(AppState {
            peer_id: String::new(),
            events: Vec::new(),
            frame_rgba: Vec::new(),
            frame_width: 0,
            frame_height: 0,
            frame_ready: false,
            connected: false,
            is_host: false,
            pending_offer: None,
            transport_tx: None,
            signaling_tx: None,
            crypto_session: None,
            file_transfer_manager: crate::file_transfer::FileTransferManager::new(),
        })
    })
}

fn lock_state() -> std::sync::MutexGuard<'static, AppState> {
    state().lock().unwrap_or_else(|e| e.into_inner())
}

fn push_event(json: &str) {
    lock_state().events.push(json.to_string());
}

fn push_event_obj(value: &serde_json::Value) {
    lock_state().events.push(value.to_string());
}

fn config_dir() -> std::path::PathBuf {
    let path = if cfg!(target_os = "windows") {
        std::env::var("APPDATA").unwrap_or_else(|_| ".".to_string()) + "\\chronodesk"
    } else {
        std::env::var("HOME").unwrap_or_else(|_| ".".to_string()) + "/.chronodesk"
    };
    let dir = std::path::PathBuf::from(&path);
    let _ = std::fs::create_dir_all(&dir);
    dir
}

fn load_or_create_id() -> String {
    let dir = config_dir();
    let id_file = dir.join("id");
    if let Ok(id) = std::fs::read_to_string(&id_file) {
        let id = id.trim().to_string();
        if id.len() >= 4 {
            return id;
        }
    }
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let id = format!("{:09}", nanos % 1_000_000_000);
    let _ = std::fs::write(&id_file, &id);
    id
}

fn get_or_create_auth_secret() -> String {
    let dir = config_dir();
    let secret_file = dir.join("auth_secret");
    if let Ok(s) = std::fs::read_to_string(&secret_file) {
        let s = s.trim().to_string();
        if s.len() >= 16 {
            return s;
        }
    }
    use ring::rand::SecureRandom;
    let rng = ring::rand::SystemRandom::new();
    let mut bytes = vec![0u8; 32];
    let _ = rng.fill(&mut bytes);
    let secret: String = bytes.iter().map(|b| format!("{:02x}", b)).collect();
    let _ = std::fs::write(&secret_file, &secret);
    secret
}

pub fn compute_auth_token(peer_id: &str) -> String {
    let secret = get_or_create_auth_secret();
    let key = hmac::Key::new(hmac::HMAC_SHA256, secret.as_bytes());
    let tag = hmac::sign(&key, peer_id.as_bytes());
    tag.as_ref().iter().map(|b| format!("{:02x}", b)).collect()
}

fn jpeg_to_rgba(jpeg: &[u8], _w: usize, _h: usize) -> Vec<u8> {
    if let Ok(img) = image::load_from_memory(jpeg) {
        img.to_rgba8().to_vec()
    } else {
        Vec::new()
    }
}

fn config_path() -> std::path::PathBuf {
    let path = if cfg!(target_os = "windows") {
        std::env::var("APPDATA").unwrap_or_else(|_| ".".to_string()) + "\\chronodesk"
    } else {
        std::env::var("HOME").unwrap_or_else(|_| ".".to_string()) + "/.chronodesk"
    };
    std::path::PathBuf::from(&path).join("config.json")
}

fn load_config() -> serde_json::Value {
    let path = config_path();
    if let Ok(s) = std::fs::read_to_string(&path) {
        serde_json::from_str(&s).unwrap_or(serde_json::json!({}))
    } else {
        serde_json::json!({})
    }
}

fn save_config(config: &serde_json::Value) {
    let path = config_path();
    if let Some(parent) = path.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            logger::write_log(&format!("config mkdir error: {e}"));
        }
    }
    if let Ok(s) = serde_json::to_string(config) {
        if let Err(e) = std::fs::write(&path, &s) {
            logger::write_log(&format!("config write error: {e}"));
        }
    }
}

fn get_signaling_addr() -> String {
    let config = load_config();
    config
        .get("signaling_addr")
        .and_then(|v| v.as_str())
        .unwrap_or("144.24.201.196:21116")
        .to_string()
}

fn get_turn_config() -> Option<crate::network::transport::TurnConfig> {
    let config = load_config();
    let url = config
        .get("turn_url")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    if url.is_empty() {
        return None;
    }
    Some(crate::network::transport::TurnConfig {
        url,
        username: config
            .get("turn_username")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        credential: config
            .get("turn_password")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
    })
}

fn get_download_dir() -> std::path::PathBuf {
    let config = load_config();
    config
        .get("download_dir")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("chronodesk"))
}

#[no_mangle]
pub extern "C" fn chronodesk_init() {
    logger::init();
    logger::write_log("chronodesk_init started");
    let addr = get_signaling_addr();
    let id = load_or_create_id();
    lock_state().peer_id = id.clone();
    push_event_obj(&serde_json::json!({"type":"init","peer_id":id,"signaling_addr":addr}));
    logger::write_log(&format!("init complete — peer_id={id} addr={addr}"));
    let addr2 = addr.clone();
    let id2 = id.clone();
    rt().spawn(async move {
        logger::write_log("run_loop starting");
        if let Err(e) = run_loop(&addr2, &id2).await {
            logger::write_log(&format!("run_loop exited with error: {e}"));
            push_event_obj(
                &serde_json::json!({"type":"error","msg":format!("Internal error: {e}")}),
            );
        } else {
            logger::write_log("run_loop exited cleanly");
        }
    });
}

#[no_mangle]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "C" fn chronodesk_get_config(key: *const std::ffi::c_char) -> *mut std::ffi::c_char {
    if key.is_null() {
        return CString::new("").unwrap_or_default().into_raw();
    }
    let key = unsafe { CStr::from_ptr(key) }.to_str().unwrap_or("");
    let config = load_config();
    let val = config.get(key).and_then(|v| v.as_str()).unwrap_or("");
    CString::new(val).unwrap_or_default().into_raw()
}

#[no_mangle]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "C" fn chronodesk_set_config(
    key: *const std::ffi::c_char,
    value: *const std::ffi::c_char,
) {
    if key.is_null() || value.is_null() {
        return;
    }
    let key = unsafe { CStr::from_ptr(key) }
        .to_str()
        .unwrap_or("")
        .to_string();
    let value = unsafe { CStr::from_ptr(value) }
        .to_str()
        .unwrap_or("")
        .to_string();
    let mut config = load_config();
    if !config.is_object() {
        config = serde_json::json!({});
    }
    config[&key] = serde_json::json!(&value);
    save_config(&config);
    push_event_obj(&serde_json::json!({"type":"config_updated","key":key,"value":value}));
}

async fn run_loop(signaling_addr: &str, my_id: &str) -> Result<(), anyhow::Error> {
    logger::write_log("run_loop started");
    let auth_token = compute_auth_token(my_id);
    let (signaling_client, mut signal_events) =
        SignalingClient::new(signaling_addr, my_id, &auth_token);
    let signaling_tx = signaling_client.channel();

    let stun_addr = format!(
        "stun:{}",
        signaling_addr.split(':').next().unwrap_or("144.24.201.196")
    );

    let turn_cfg = get_turn_config();
    let (transport, mut transport_events) =
        match Transport::new(my_id, &stun_addr, turn_cfg, Some(signaling_tx.clone())).await {
            Ok(t) => t,
            Err(e) => {
                logger::write_log(&format!("Transport init FAILED: {e}"));
                push_event_obj(
                    &serde_json::json!({"type":"error","msg":format!("Transport init: {e}")}),
                );
                return Ok(());
            }
        };
    let transport_tx = transport.signal_tx();

    {
        let mut s = lock_state();
        s.transport_tx = Some(transport_tx.clone());
        s.signaling_tx = Some(signaling_tx.clone());
        logger::write_log("transport_tx and signaling_tx stored in state");
    }

    tokio::spawn(async move {
        if let Err(e) = signaling_client.run().await {
            push_event_obj(&serde_json::json!({"type":"error","msg":format!("Signaling: {e}")}));
        }
    });

    let mut capture = ScreenCapture::new().ok();
    let mut encoder = VideoEncoder::new(EncoderType::Auto, 1920, 1080).ok();
    let mut capture_active = false;
    let mut audio_capture: Option<(crate::audio::AudioCapture, crate::audio::AudioCodec)> = None;
    let mut audio_player: Option<(crate::audio::AudioPlayer, crate::audio::AudioCodec)> = None;

    let mut audio_rx: Option<mpsc::UnboundedReceiver<Vec<f32>>> = None;
    let mut clipboard_rx: Option<mpsc::UnboundedReceiver<String>> = None;

    let mut quality_ctrl = QualityController::new();
    let mut frame_count: i64 = 0;
    let mut ping_interval: i32 = 0;

    let mut local_private_key: Option<ring::agreement::EphemeralPrivateKey> = None;
    let mut crypto_session: Option<crate::crypto::CryptoSession> = None;

    loop {
        tokio::select! {
            Some(event) = signal_events.recv() => {
                match event {
                    SignalEvent::Offer { from, sdp } => {
                        let s = lock_state();
                        if s.connected || s.pending_offer.is_some() {
                            continue;
                        }
                        drop(s);
                        lock_state().pending_offer = Some((from.clone(), sdp));
                        push_event_obj(&serde_json::json!({"type":"connection_request","from":from}));
                    }
                    SignalEvent::Answer { from, sdp } => {
                        let _ = transport_tx.send(TrCmd::HandleAnswer(from, sdp));
                    }
                    SignalEvent::IceCandidate { from, candidate, sdp_mid, sdp_mline_index } => {
                        let _ = transport_tx.send(TrCmd::HandleIceCandidate(from, candidate, sdp_mid, sdp_mline_index));
                    }
                    SignalEvent::PeerList(_) => {}
                    SignalEvent::Error(msg) => {
                        push_event_obj(&serde_json::json!({"type":"error","msg":msg}));
                    }
                }
            }
            Some(event) = transport_events.recv() => {
                match event {
                    TransportEvent::Connected { .. } => {
                        let was_host = {
                            let mut s = lock_state();
                            let h = s.is_host;
                            s.connected = true;
                            h
                        };
                        capture_active = was_host;

                        if was_host {
                            match crate::audio::AudioCapture::new() {
                                Ok((cap, rx)) => {
                                    audio_rx = Some(rx);
                                    match crate::audio::AudioCodec::new() {
                                        Ok(codec) => {
                                            audio_capture = Some((cap, codec));
                                            logger::write_log("audio capture started (host)");
                                        }
                                        Err(e) => logger::write_log(&format!("audio codec init failed: {e}")),
                                    }
                                }
                                Err(e) => logger::write_log(&format!("audio capture failed: {e}")),
                            }
                        } else {
                            match crate::audio::AudioPlayer::new() {
                                Ok(player) => {
                                    match crate::audio::AudioCodec::new() {
                                        Ok(codec) => {
                                            audio_player = Some((player, codec));
                                            logger::write_log("audio player started (viewer)");
                                        }
                                        Err(e) => logger::write_log(&format!("audio codec init failed: {e}")),
                                    }
                                    logger::write_log("audio player started (viewer)");
                                }
                                Err(e) => logger::write_log(&format!("audio player failed: {e}")),
                            }
                        }

                        if was_host {
                            match crate::crypto::generate_keypair() {
                                Ok((priv_key, pub_key)) => {
                                    local_private_key = Some(priv_key);
                                    let handshake = ChannelMessage::Handshake { public_key: pub_key };
                                    if let Err(e) = transport.send_message(&handshake).await {
                                        logger::write_log(&format!("failed to send crypto handshake: {e}"));
                                    }
                                    logger::write_log("sent crypto handshake (host)");
                                }
                                Err(e) => logger::write_log(&format!("crypto keypair generation failed: {e}")),
                            }
                        }

                        let (_clip, clip_rx) = crate::clipboard::ClipboardSync::start();
                        clipboard_rx = Some(clip_rx);
                        logger::write_log("clipboard sync started");

                        logger::write_log(&format!("transport connected — is_host={was_host}"));
                        push_event(r#"{"type":"connected"}"#);
                    }
                    TransportEvent::Disconnected { .. } => {
                        capture_active = false;
                        audio_capture = None;
                        audio_player = None;
                        audio_rx = None;
                        clipboard_rx = None;
                        {
                            let mut s = lock_state();
                            s.connected = false;
                            s.is_host = false;
                            s.crypto_session = None;
                            // Clean up any in-progress file transfers
                            let part_files: Vec<std::path::PathBuf> = s.file_transfer_manager.incoming.values().map(|t| t.file_path.clone()).collect();
                            s.file_transfer_manager.incoming.clear();
                            s.file_transfer_manager.outgoing.clear();
                            for p in part_files {
                                let _ = std::fs::remove_file(&p);
                            }
                        }
                        logger::write_log("transport disconnected");
                        push_event(r#"{"type":"disconnected"}"#);
                    }
                    TransportEvent::MessageReceived { msg } => {
                        // Handle key exchange handshake (always unencrypted, before crypto_session is set)
                        if let ChannelMessage::Handshake { public_key } = &msg {
                            if let Some(priv_key) = local_private_key.take() {
                                match crate::crypto::compute_shared_secret(priv_key, public_key) {
                                    Ok(shared) => {
                                        match crate::crypto::derive_session_key(&shared) {
                                            Ok(key) => {
                                                match crate::crypto::CryptoSession::new(&key) {
                                                    Ok(session) => {
                                                        logger::write_log("crypto handshake complete (host)");
                                                        crypto_session = Some(session.clone());
                                                        lock_state().crypto_session = Some(session);
                                                    }
                                                    Err(e) => logger::write_log(&format!("crypto session init failed: {e}")),
                                                }
                                            }
                                            Err(e) => logger::write_log(&format!("crypto key derivation failed: {e}")),
                                        }
                                    }
                                    Err(e) => logger::write_log(&format!("crypto shared secret failed: {e}")),
                                }
                            } else {
                                match crate::crypto::generate_keypair() {
                                    Ok((priv_key, my_pub_key)) => {
                                        match crate::crypto::compute_shared_secret(priv_key, public_key) {
                                            Ok(shared) => {
                                                match crate::crypto::derive_session_key(&shared) {
                                                    Ok(key) => {
                                                        match crate::crypto::CryptoSession::new(&key) {
                                                            Ok(session) => {
                                                                logger::write_log("crypto handshake complete (viewer)");
                                                                crypto_session = Some(session.clone());
                                                                lock_state().crypto_session = Some(session);
                                                                let resp = ChannelMessage::Handshake { public_key: my_pub_key };
                                                                if let Err(e) = transport.send_message(&resp).await {
                                                                    logger::write_log(&format!("failed to send crypto response: {e}"));
                                                                }
                                                            }
                                                            Err(e) => logger::write_log(&format!("crypto session init failed: {e}")),
                                                        }
                                                    }
                                                    Err(e) => logger::write_log(&format!("crypto key derivation failed: {e}")),
                                                }
                                            }
                                            Err(e) => logger::write_log(&format!("crypto shared secret failed: {e}")),
                                        }
                                    }
                                    Err(e) => logger::write_log(&format!("crypto viewer keypair failed: {e}")),
                                }
                            }
                            continue;
                        }

                        // Decrypt if encrypted; otherwise use as-is (backward compat)
                        let inner = match msg {
                            ChannelMessage::Encrypted { data } => {
                                if let Some(ref session) = crypto_session {
                                    match session.decrypt(&data) {
                                        Ok(plaintext) => match bincode::deserialize::<ChannelMessage>(&plaintext) {
                                            Ok(decoded) => decoded,
                                            Err(_) => continue,
                                        },
                                        Err(_) => continue,
                                    }
                                } else {
                                    continue;
                                }
                            }
                            other => other,
                        };

                        match inner {
                            ChannelMessage::VideoFrame { width, height, codec, data } => {
                                let rgba = match codec {
                                    0 | 2 => jpeg_to_rgba(&data, width as usize, height as usize),
                                    _ => {
                                        logger::write_log(&format!("unsupported video codec {codec}, size={}", data.len()));
                                        Vec::new()
                                    }
                                };
                                if !rgba.is_empty() {
                                    let mut s = lock_state();
                                    s.frame_rgba = rgba;
                                    s.frame_width = width;
                                    s.frame_height = height;
                                    s.frame_ready = true;
                                }
                                push_event(&format!(r#"{{"type":"frame","w":{},"h":{},"codec":{},"size":{}}}"#, width, height, codec, data.len()));
                            }
                            ChannelMessage::AudioData { data, sample_rate, channels } => {
                                if let Some((ref player, ref mut codec)) = audio_player {
                                    let mut pcm = vec![0.0f32; crate::audio::FRAME_SIZE * crate::audio::CHANNELS];
                                    if let Ok(len) = codec.decode(&data, &mut pcm) {
                                        let resampled = crate::audio::resample_to_48k_stereo(&pcm[..len], sample_rate, channels);
                                        player.feed(&resampled);
                                    }
                                }
                            }
                            ChannelMessage::InputMove { x, y } => {
                                if let Ok(mut inp) = crate::input::InputController::new() {
                                    let _ = inp.mouse_move(x, y);
                                }
                            }
                            ChannelMessage::InputClick { button, pressed } => {
                                if let Ok(mut inp) = crate::input::InputController::new() {
                                    if pressed {
                                        let _ = inp.mouse_down(button);
                                    } else {
                                        let _ = inp.mouse_up(button);
                                    }
                                }
                            }
                            ChannelMessage::InputKey { key, pressed } => {
                                if let Some(enigo_key) = crate::input::logical_key_to_enigo(key) {
                                    if let Ok(mut inp) = crate::input::InputController::new() {
                                        let dir = if pressed { enigo::Direction::Press } else { enigo::Direction::Release };
                                        let _ = inp.key_press(enigo_key, dir);
                                    }
                                }
                            }
                            ChannelMessage::Clipboard { text } => {
                                crate::clipboard::ClipboardSync::write(&text);
                                push_event_obj(&serde_json::json!({"type":"clipboard","text":text}));
                            }
                            ChannelMessage::Ping { .. } => {
                                let pong = ChannelMessage::Pong { timestamp: 0 };
                                if let Some(ref session) = crypto_session {
                                    if let Ok(data) = bincode::serialize(&pong) {
                                        if let Ok(encrypted) = session.encrypt(&data) {
                                            let _ = transport.send_message(&ChannelMessage::Encrypted { data: encrypted }).await;
                                        }
                                    }
                                } else {
                                    let _ = transport.send_message(&pong).await;
                                }
                            }
                            ChannelMessage::Pong { .. } => {
                                quality_ctrl.record_pong_received();
                            }
                            ChannelMessage::FileTransferRequest { id, name, size } => {
                                let safe_name = crate::file_transfer::sanitize_filename(&name);
                                let dir = get_download_dir();
                                if let Err(e) = std::fs::create_dir_all(&dir) {
                                    logger::write_log(&format!("file transfer mkdir error: {e}"));
                                }
                                let part_path = dir.join(format!("{id}_{safe_name}.part"));
                                match std::fs::File::create(&part_path) {
                                    Ok(_file) => {
                                        let transfer = crate::file_transfer::IncomingTransfer {
                                            file_path: part_path,
                                            name: safe_name.clone(),
                                            total_size: size,
                                            bytes_received: 0,
                                        };
                                        lock_state().file_transfer_manager.incoming.insert(id.clone(), transfer);
                                        push_event_obj(&serde_json::json!({"type":"file_request","id":id,"name":name,"size":size}));
                                    }
                                    Err(e) => {
                                        logger::write_log(&format!("file transfer create error: {e}"));
                                        push_event_obj(&serde_json::json!({"type":"file_error","id":id,"msg":format!("cannot create file for {name}")}));
                                    }
                                }
                            }
                            ChannelMessage::FileTransferAccept { id } => {
                                let mut outgoing = lock_state().file_transfer_manager.outgoing.remove(&id);
                                if let Some(ref mut outgoing) = outgoing {
                                    let total = outgoing.total_size;
                                    let name = outgoing.name.clone();
                                    let mut idx = 0u64;
                                    while let Some((offset, data)) = crate::file_transfer::read_chunk(outgoing) {
                                        let chunk = ChannelMessage::FileTransferChunk {
                                            id: id.clone(),
                                            offset,
                                            data,
                                        };
                                        if let Some(ref session) = crypto_session {
                                            if let Ok(ser) = bincode::serialize(&chunk) {
                                                if let Ok(enc) = session.encrypt(&ser) {
                                                    let _ = transport.send_message(&ChannelMessage::Encrypted { data: enc }).await;
                                                }
                                            }
                                        } else {
                                            let _ = transport.send_message(&chunk).await;
                                        }
                                        if idx.is_multiple_of(16) || outgoing.offset >= total {
                                            push_event_obj(&serde_json::json!({"type":"file_progress","id":id,"bytes_sent":outgoing.offset.min(total),"total_size":total}));
                                        }
                                        idx += 1;
                                    }
                                    let complete = ChannelMessage::FileTransferComplete { id: id.clone() };
                                    if let Some(ref session) = crypto_session {
                                        if let Ok(ser) = bincode::serialize(&complete) {
                                            if let Ok(enc) = session.encrypt(&ser) {
                                                let _ = transport.send_message(&ChannelMessage::Encrypted { data: enc }).await;
                                            }
                                        }
                                    } else {
                                        let _ = transport.send_message(&complete).await;
                                    }
                                    push_event_obj(&serde_json::json!({"type":"file_sent","id":id,"name":name,"size":total}));
                                }
                            }
                            ChannelMessage::FileTransferReject { id } => {
                                lock_state().file_transfer_manager.outgoing.remove(&id);
                                push_event_obj(&serde_json::json!({"type":"file_rejected","id":id}));
                            }
                            ChannelMessage::FileTransferChunk { id, offset, data } => {
                                let mut s = lock_state();
                                let mut io_failed = false;
                                if let Some(ref mut t) = s.file_transfer_manager.incoming.get_mut(&id) {
                                    use std::io::{SeekFrom, Write};
                                    match std::fs::OpenOptions::new().write(true).open(&t.file_path) {
                                        Ok(mut file) => {
                                            if let Err(e) = std::io::Seek::seek(&mut file, SeekFrom::Start(offset)).and_then(|_| file.write_all(&data)) {
                                                logger::write_log(&format!("file chunk write error: {e}"));
                                                io_failed = true;
                                            }
                                        }
                                        Err(e) => {
                                            logger::write_log(&format!("file chunk open error: {e}"));
                                            io_failed = true;
                                        }
                                    }
                                    if io_failed {
                                        let name = t.name.clone();
                                        let _ = std::fs::remove_file(&t.file_path);
                                        s.file_transfer_manager.incoming.remove(&id);
                                        drop(s);
                                        push_event_obj(&serde_json::json!({"type":"file_error","id":id,"msg":format!("IO error writing chunk for {name}")}));
                                    } else {
                                        t.bytes_received += data.len() as u64;
                                        let done = t.bytes_received >= t.total_size;
                                        if done {
                                            if let Some(t) = s.file_transfer_manager.incoming.remove(&id) {
                                                let final_path = get_download_dir().join(&t.name);
                                                if let Err(e) = std::fs::rename(&t.file_path, &final_path) {
                                                    logger::write_log(&format!("file rename error: {e}"));
                                                }
                                                drop(s);
                                                push_event_obj(&serde_json::json!({"type":"file_complete","id":id,"name":t.name,"size":t.total_size,"path":final_path.to_string_lossy()}));
                                            }
                                        } else {
                                            let bytes_received = t.bytes_received;
                                            let total_size = t.total_size;
                                            drop(s);
                                            push_event_obj(&serde_json::json!({"type":"file_progress","id":id,"bytes_received":bytes_received,"total_size":total_size}));
                                        }
                                    }
                                }
                            }
                            ChannelMessage::FileTransferComplete { id } => {
                                let mut s = lock_state();
                                if let Some(t) = s.file_transfer_manager.incoming.remove(&id) {
                                    let final_path = get_download_dir().join(&t.name);
                                    if let Err(e) = std::fs::rename(&t.file_path, &final_path) {
                                        logger::write_log(&format!("file rename error: {e}"));
                                    }
                                    drop(s);
                                    push_event_obj(&serde_json::json!({"type":"file_complete","id":id,"name":t.name,"size":t.total_size,"path":final_path.to_string_lossy()}));
                                }
                            }
                            ChannelMessage::FileTransferError { id, message } => {
                                lock_state().file_transfer_manager.outgoing.remove(&id);
                                push_event_obj(&serde_json::json!({"type":"file_error","id":id,"msg":message}));
                            }
                            _ => {}
                        }
                    }
                    TransportEvent::Error { msg } => {
                        logger::write_log(&format!("Transport error: {msg}"));
                        push_event_obj(&serde_json::json!({"type":"error","msg":msg}));
                    }
                }
            }
            Some(audio_samples) = async {
                if let Some(ref mut rx) = audio_rx.as_mut() {
                    rx.recv().await
                } else {
                    std::future::pending::<Option<Vec<f32>>>().await
                }
            } => {
                if let Some((ref _cap, ref mut codec)) = audio_capture {
                    let resampled = crate::audio::resample_to_48k_stereo(&audio_samples, 48000, 2);
                    if let Ok(encoded) = codec.encode(&resampled) {
                        let msg = ChannelMessage::AudioData {
                            data: encoded,
                            sample_rate: crate::audio::SAMPLE_RATE,
                            channels: 2,
                        };
                        if let Some(ref session) = crypto_session {
                            if let Ok(data) = bincode::serialize(&msg) {
                                if let Ok(encrypted) = session.encrypt(&data) {
                                    let _ = transport.send_message(&ChannelMessage::Encrypted { data: encrypted }).await;
                                }
                            }
                        } else {
                            let _ = transport.send_message(&msg).await;
                        }
                    }
                }
            }
            Some(clip_text) = async {
                if let Some(ref mut rx) = clipboard_rx.as_mut() {
                    rx.recv().await
                } else {
                    std::future::pending::<Option<String>>().await
                }
            } => {
                if capture_active {
                    let msg = ChannelMessage::Clipboard { text: clip_text };
                    if let Some(ref session) = crypto_session {
                        if let Ok(data) = bincode::serialize(&msg) {
                            if let Ok(encrypted) = session.encrypt(&data) {
                                let _ = transport.send_message(&ChannelMessage::Encrypted { data: encrypted }).await;
                            }
                        }
                    } else {
                        let _ = transport.send_message(&msg).await;
                    }
                }
            }
            _ = tokio::time::sleep(tokio::time::Duration::from_millis(16)) => {
                ping_interval += 1;
                if ping_interval >= 125 {
                    ping_interval = 0;
                    quality_ctrl.record_ping_sent();
                    let ping = ChannelMessage::Ping { timestamp: 0 };
                    if let Some(ref session) = crypto_session {
                        if let Ok(data) = bincode::serialize(&ping) {
                            if let Ok(encrypted) = session.encrypt(&data) {
                                let _ = transport.send_message(&ChannelMessage::Encrypted { data: encrypted }).await;
                            }
                        }
                    } else {
                        let _ = transport.send_message(&ping).await;
                    }
                }

                if capture_active {
                    if let (Some(ref mut cap), Some(ref mut enc)) = (&mut capture, &mut encoder) {
                        frame_count += 1;
                        let fps = enc.target_fps();
                        let skip = (60 / fps.max(1)) as i64;
                        if skip > 1 && frame_count % skip != 0 {
                            continue;
                        }
                        quality_ctrl.adapt(enc, frame_count);
                        if quality_ctrl.rtt_ms() > 0.0 {
                            let ev = format!(
                                r#"{{"type":"quality","rtt":{},"quality":{},"fps":{},"scale":{}}}"#,
                                quality_ctrl.rtt_ms() as i64,
                                enc.quality() as u8,
                                enc.target_fps(),
                                1.0,
                            );
                            push_event(&ev);
                        }

                        if let Ok(frames) = cap.capture_all() {
                            for frame in &frames {
                                if frame.dirty_rects.is_empty() { continue; }
                                if let Ok(packets) = enc.encode(&frame.data, frame.width as u32, frame.height as u32) {
                                    for pkt in &packets {
                                        quality_ctrl.record_frame_size(pkt.data.len());
                                        let codec_id = match pkt.codec {
                                            "jpeg" => 0,
                                            "h264" => 1,
                                            "webp" => 2,
                                            _ => 0,
                                        };
                                        let msg = ChannelMessage::VideoFrame {
                                            width: frame.width as u32,
                                            height: frame.height as u32,
                                            codec: codec_id,
                                            data: pkt.data.clone(),
                                        };
                                        if let Some(ref session) = crypto_session {
                                            if let Ok(data) = bincode::serialize(&msg) {
                                                if let Ok(encrypted) = session.encrypt(&data) {
                                                    let _ = transport.send_message(&ChannelMessage::Encrypted { data: encrypted }).await;
                                                }
                                            }
                                        } else {
                                            let _ = transport.send_message(&msg).await;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[no_mangle]
pub extern "C" fn chronodesk_get_peer_id() -> *mut std::ffi::c_char {
    let id = state()
        .lock()
        .map(|s| s.peer_id.clone())
        .unwrap_or_default();
    CString::new(id).unwrap_or_default().into_raw()
}

#[no_mangle]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "C" fn chronodesk_free_string(ptr: *mut std::ffi::c_char) {
    if !ptr.is_null() {
        unsafe {
            let _ = CString::from_raw(ptr);
        }
    }
}

#[no_mangle]
pub extern "C" fn chronodesk_poll_event() -> *mut std::ffi::c_char {
    let ev = state()
        .lock()
        .map(|mut s| s.events.pop())
        .ok()
        .flatten()
        .unwrap_or_default();
    CString::new(ev).unwrap_or_default().into_raw()
}

#[no_mangle]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "C" fn chronodesk_connect(peer_id: *const std::ffi::c_char) {
    if peer_id.is_null() {
        return;
    }
    let target = unsafe { CStr::from_ptr(peer_id) }
        .to_str()
        .unwrap_or("")
        .to_string();
    if target.is_empty() {
        logger::write_log("chronodesk_connect called with empty target");
        return;
    }
    logger::write_log(&format!("chronodesk_connect called — target={target}"));
    rt().spawn(async move {
        let s = lock_state();
        if let Some(ref tx) = s.transport_tx {
            logger::write_log(&format!("sending CreateOffer to {target}"));
            push_event_obj(&serde_json::json!({"type":"connecting","to":target}));
            if let Err(e) = tx.send(TrCmd::CreateOffer(target)) {
                logger::write_log(&format!("CreateOffer send failed: {e}"));
                push_event(
                    r#"{"type":"error","msg":"Connection failed — transport channel closed"}"#,
                );
            }
        } else {
            logger::write_log("transport_tx is None — not ready yet");
            push_event(r#"{"type":"error","msg":"Transport not ready — still initializing"}"#);
        }
    });
}

#[no_mangle]
pub extern "C" fn chronodesk_accept() {
    let (pending, tx) = {
        let mut s = lock_state();
        s.is_host = true;
        (s.pending_offer.take(), s.transport_tx.clone())
    };
    if let Some((from, sdp)) = pending {
        if let Some(ref tx) = tx {
            if let Err(e) = tx.send(TrCmd::HandleOffer(from, sdp)) {
                logger::write_log(&format!("HandleOffer send failed: {e}"));
            }
        }
        push_event(r#"{"type":"accepted"}"#);
    }
}

#[no_mangle]
pub extern "C" fn chronodesk_deny() {
    let had_pending = lock_state().pending_offer.take().is_some();
    if !had_pending {
        logger::write_log("chronodesk_deny called but no pending offer");
    }
    push_event(r#"{"type":"denied"}"#);
}

#[no_mangle]
pub extern "C" fn chronodesk_disconnect() {
    rt().spawn(async {
        let tx = lock_state().transport_tx.clone();
        if let Some(ref tx) = tx {
            if let Err(e) = tx.send(TrCmd::Disconnect) {
                logger::write_log(&format!("Disconnect send failed: {e}"));
            }
        }
    });
}

#[no_mangle]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "C" fn chronodesk_get_frame(
    out_data: *mut *mut u8,
    out_len: *mut i32,
    out_width: *mut i32,
    out_height: *mut i32,
) -> i32 {
    if out_data.is_null() || out_len.is_null() || out_width.is_null() || out_height.is_null() {
        return 0;
    }
    let mut s = lock_state();
    if !s.frame_ready {
        return 0;
    }
    let len = s.frame_rgba.len();
    let width = s.frame_width;
    let height = s.frame_height;
    unsafe {
        let ptr = libc::malloc(len) as *mut u8;
        if ptr.is_null() {
            return 0;
        }
        std::ptr::copy_nonoverlapping(s.frame_rgba.as_ptr(), ptr, len);
        *out_data = ptr;
        *out_len = len as i32;
        *out_width = width as i32;
        *out_height = height as i32;
    }
    s.frame_ready = false;
    1
}

#[no_mangle]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "C" fn chronodesk_free_frame(ptr: *mut u8) {
    if !ptr.is_null() {
        unsafe {
            libc::free(ptr as *mut std::ffi::c_void);
        }
    }
}

fn encrypt_input_msg(s: &AppState, inner: ChannelMessage) -> ChannelMessage {
    if let Some(ref session) = s.crypto_session {
        if let Ok(data) = bincode::serialize(&inner) {
            if let Ok(encrypted) = session.encrypt(&data) {
                return ChannelMessage::Encrypted { data: encrypted };
            }
        }
    }
    inner
}

#[no_mangle]
pub extern "C" fn chronodesk_send_input_move(x: i32, y: i32) {
    let s = lock_state();
    let tx = s.transport_tx.clone();
    if let Some(ref tx) = tx {
        let msg = encrypt_input_msg(&s, ChannelMessage::InputMove { x, y });
        let _ = tx.send(TrCmd::SendMessage(msg));
    }
}

#[no_mangle]
pub extern "C" fn chronodesk_send_input_click(button: u8, pressed: bool) {
    let s = lock_state();
    let tx = s.transport_tx.clone();
    if let Some(ref tx) = tx {
        let msg = encrypt_input_msg(&s, ChannelMessage::InputClick { button, pressed });
        let _ = tx.send(TrCmd::SendMessage(msg));
    }
}

#[no_mangle]
pub extern "C" fn chronodesk_send_input_key(key: u64, pressed: bool) {
    let s = lock_state();
    let tx = s.transport_tx.clone();
    if let Some(ref tx) = tx {
        let msg = encrypt_input_msg(&s, ChannelMessage::InputKey { key, pressed });
        let _ = tx.send(TrCmd::SendMessage(msg));
    }
}

#[no_mangle]
pub extern "C" fn chronodesk_get_log() -> *mut std::ffi::c_char {
    let log = logger::read_log();
    CString::new(log).unwrap_or_default().into_raw()
}

#[no_mangle]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "C" fn chronodesk_send_file(path: *const std::ffi::c_char) -> *mut std::ffi::c_char {
    if path.is_null() {
        return CString::new("").unwrap_or_default().into_raw();
    }
    let path = match unsafe { CStr::from_ptr(path) }.to_str() {
        Ok(s) => s.to_string(),
        Err(_) => return CString::new("").unwrap_or_default().into_raw(),
    };
    let result = match crate::file_transfer::FileTransferManager::prepare_outgoing(&path) {
        Ok((id, outgoing)) => {
            let total_size = outgoing.total_size;
            let name = outgoing.name.clone();
            let s = lock_state();
            let owned_id = id.clone();
            let msg = ChannelMessage::FileTransferRequest {
                id: owned_id,
                name: name.clone(),
                size: total_size,
            };
            let request = if let Some(ref session) = s.crypto_session {
                if let Ok(data) = bincode::serialize(&msg) {
                    if let Ok(encrypted) = session.encrypt(&data) {
                        ChannelMessage::Encrypted { data: encrypted }
                    } else {
                        msg
                    }
                } else {
                    msg
                }
            } else {
                msg
            };
            if let Some(ref tx) = s.transport_tx {
                let _ = tx.send(TrCmd::SendMessage(request));
            }
            drop(s);
            lock_state()
                .file_transfer_manager
                .outgoing
                .insert(id.clone(), outgoing);
            id
        }
        Err(e) => {
            logger::write_log(&format!("send_file failed: {e}"));
            String::new()
        }
    };
    CString::new(result).unwrap_or_default().into_raw()
}

#[no_mangle]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "C" fn chronodesk_accept_file_transfer(id: *const std::ffi::c_char) {
    if id.is_null() {
        return;
    }
    let id = unsafe { CStr::from_ptr(id) }
        .to_str()
        .unwrap_or("")
        .to_string();
    if id.is_empty() {
        return;
    }
    let msg = ChannelMessage::FileTransferAccept { id };
    let s = lock_state();
    if let Some(ref tx) = s.transport_tx {
        let send_msg = if let Some(ref session) = s.crypto_session {
            if let Ok(data) = bincode::serialize(&msg) {
                if let Ok(encrypted) = session.encrypt(&data) {
                    ChannelMessage::Encrypted { data: encrypted }
                } else {
                    msg
                }
            } else {
                msg
            }
        } else {
            msg
        };
        let _ = tx.send(TrCmd::SendMessage(send_msg));
    }
}

#[no_mangle]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "C" fn chronodesk_reject_file_transfer(id: *const std::ffi::c_char) {
    if id.is_null() {
        return;
    }
    let id = unsafe { CStr::from_ptr(id) }
        .to_str()
        .unwrap_or("")
        .to_string();
    if id.is_empty() {
        return;
    }
    let msg = ChannelMessage::FileTransferReject { id };
    let s = lock_state();
    if let Some(ref tx) = s.transport_tx {
        let send_msg = if let Some(ref session) = s.crypto_session {
            if let Ok(data) = bincode::serialize(&msg) {
                if let Ok(encrypted) = session.encrypt(&data) {
                    ChannelMessage::Encrypted { data: encrypted }
                } else {
                    msg
                }
            } else {
                msg
            }
        } else {
            msg
        };
        let _ = tx.send(TrCmd::SendMessage(send_msg));
    }
}

#[no_mangle]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "C" fn chronodesk_cancel_file_transfer(id: *const std::ffi::c_char) {
    if id.is_null() {
        return;
    }
    let id = unsafe { CStr::from_ptr(id) }
        .to_str()
        .unwrap_or("")
        .to_string();
    if id.is_empty() {
        return;
    }
    let mut s = lock_state();
    let dir = get_download_dir();
    if let Some((name, _was_incoming)) = s.file_transfer_manager.cancel_transfer(&id, &dir) {
        let error_msg = ChannelMessage::FileTransferError {
            id: id.clone(),
            message: "cancelled".to_string(),
        };
        if let Some(ref tx) = s.transport_tx {
            let send_msg = if let Some(ref session) = s.crypto_session {
                if let Ok(data) = bincode::serialize(&error_msg) {
                    if let Ok(encrypted) = session.encrypt(&data) {
                        ChannelMessage::Encrypted { data: encrypted }
                    } else {
                        error_msg
                    }
                } else {
                    error_msg
                }
            } else {
                error_msg
            };
            let _ = tx.send(TrCmd::SendMessage(send_msg));
        }
        drop(s);
        push_event_obj(&serde_json::json!({"type":"file_cancelled","id":id,"name":name}));
    }
}

#[no_mangle]
pub extern "C" fn chronodesk_get_version() -> *mut std::ffi::c_char {
    CString::new(env!("CARGO_PKG_VERSION"))
        .unwrap_or_default()
        .into_raw()
}
