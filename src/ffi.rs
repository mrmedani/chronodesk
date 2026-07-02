use crate::capture::ScreenCapture;
use crate::logger;
use crate::network::signaling::{SignalCommand as SigCmd, SignalEvent, SignalingClient};
use crate::network::transport::{SignalCommand as TrCmd, Transport, TransportEvent};
use crate::protocol::ChannelMessage;
use crate::video::{EncoderType, QualityController, VideoEncoder};
use std::ffi::{CStr, CString};
use std::sync::Mutex;
use std::sync::OnceLock;
use tokio::runtime::Runtime;
use tokio::sync::mpsc;

static RUNTIME: OnceLock<Runtime> = OnceLock::new();
fn rt() -> &'static Runtime {
    RUNTIME.get_or_init(|| Runtime::new().expect("Failed to create tokio runtime"))
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
        })
    })
}

fn lock_state() -> std::sync::MutexGuard<'static, AppState> {
    state().lock().unwrap_or_else(|e| e.into_inner())
}

fn push_event(json: &str) {
    lock_state().events.push(json.to_string());
}

fn load_or_create_id() -> String {
    let path = if cfg!(target_os = "windows") {
        std::env::var("APPDATA").unwrap_or_else(|_| ".".to_string()) + "\\chronodesk"
    } else {
        std::env::var("HOME").unwrap_or_else(|_| ".".to_string()) + "/.chronodesk"
    };
    let _ = std::fs::create_dir_all(&path);
    let id_file = std::path::Path::new(&path).join("id");
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
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(s) = serde_json::to_string(config) {
        let _ = std::fs::write(&path, &s);
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

#[no_mangle]
pub extern "C" fn chronodesk_init() {
    logger::init();
    logger::write_log("chronodesk_init started");
    let addr = get_signaling_addr();
    let id = load_or_create_id();
    lock_state().peer_id = id.clone();
    push_event(&format!(
        r#"{{"type":"init","peer_id":"{}","signaling_addr":"{}"}}"#,
        id, addr
    ));
    logger::write_log(&format!("init complete — peer_id={id} addr={addr}"));
    let addr2 = addr.clone();
    let id2 = id.clone();
    rt().spawn(async move {
        logger::write_log("run_loop starting");
        if let Err(e) = run_loop(&addr2, &id2).await {
            logger::write_log(&format!("run_loop exited with error: {e}"));
            push_event(&format!(
                r#"{{"type":"error","msg":"Internal error: {e}"}}"#
            ));
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
    push_event(&format!(
        r#"{{"type":"config_updated","key":"{}","value":"{}"}}"#,
        key, value
    ));
}

async fn run_loop(signaling_addr: &str, my_id: &str) -> Result<(), anyhow::Error> {
    logger::write_log("run_loop started");
    let (signaling_client, mut signal_events) = SignalingClient::new(signaling_addr, my_id);
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
                push_event(&format!(
                    r#"{{"type":"error","msg":"Transport init: {}"}}"#,
                    e
                ));
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
            push_event(&format!(r#"{{"type":"error","msg":"Signaling: {}"}}"#, e));
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
                        push_event(&format!(r#"{{"type":"connection_request","from":"{}"}}"#, from));
                    }
                    SignalEvent::Answer { from, sdp } => {
                        let _ = transport_tx.send(TrCmd::HandleAnswer(from, sdp));
                    }
                    SignalEvent::IceCandidate { from, candidate, sdp_mid, sdp_mline_index } => {
                        let _ = transport_tx.send(TrCmd::HandleIceCandidate(from, candidate, sdp_mid, sdp_mline_index));
                    }
                    SignalEvent::PeerList(_) => {}
                    SignalEvent::Error(msg) => {
                        push_event(&format!(r#"{{"type":"error","msg":"{}"}}"#, msg));
                    }
                }
            }
            Some(event) = transport_events.recv() => {
                match event {
                    TransportEvent::Connected { .. } => {
                        let was_host = lock_state().is_host;
                        lock_state().connected = true;
                        capture_active = was_host;

                        if was_host {
                            match crate::audio::AudioCapture::new() {
                                Ok((cap, rx)) => {
                                    audio_rx = Some(rx);
                                    audio_capture = Some((cap, crate::audio::AudioCodec::new().unwrap()));
                                    logger::write_log("audio capture started (host)");
                                }
                                Err(e) => logger::write_log(&format!("audio capture failed: {e}")),
                            }
                        } else {
                            match crate::audio::AudioPlayer::new() {
                                Ok(player) => {
                                    let codec = crate::audio::AudioCodec::new().unwrap();
                                    audio_player = Some((player, codec));
                                    logger::write_log("audio player started (viewer)");
                                }
                                Err(e) => logger::write_log(&format!("audio player failed: {e}")),
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
                        lock_state().connected = false;
                        lock_state().is_host = false;
                        logger::write_log("transport disconnected");
                        push_event(r#"{"type":"disconnected"}"#);
                    }
                    TransportEvent::MessageReceived { msg } => {
                        match msg {
                            ChannelMessage::VideoFrame { width, height, codec, data } => {
                                let rgba = match codec {
                                    0 | 2 => jpeg_to_rgba(&data, width as usize, height as usize),
                                    _ => Vec::new(),
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
                                push_event(&format!(r#"{{"type":"clipboard","text":"{}"}}"#, text));
                            }
                            ChannelMessage::Ping { .. } => {
                                let pong = ChannelMessage::Pong { timestamp: 0 };
                                let _ = transport.send_message(&pong).await;
                            }
                            ChannelMessage::Pong { .. } => {
                                quality_ctrl.record_pong_received();
                            }
                        }
                    }
                    TransportEvent::Error { msg } => {
                        logger::write_log(&format!("Transport error: {msg}"));
                        push_event(&format!(r#"{{"type":"error","msg":"{}"}}"#, msg));
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
                        let _ = transport.send_message(&msg).await;
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
                    let _ = transport.send_message(&msg).await;
                }
            }
            _ = tokio::time::sleep(tokio::time::Duration::from_millis(16)) => {
                ping_interval += 1;
                if ping_interval >= 125 {
                    ping_interval = 0;
                    quality_ctrl.record_ping_sent();
                    let ping = ChannelMessage::Ping { timestamp: 0 };
                    let _ = transport.send_message(&ping).await;
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
                                if let Ok(packets) = enc.encode(&frame.data) {
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
            push_event(&format!(r#"{{"type":"connecting","to":"{}"}}"#, target));
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
    let pending = lock_state().pending_offer.take();
    if let Some((from, sdp)) = pending {
        lock_state().is_host = true;
        let tx = lock_state().transport_tx.clone();
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
    lock_state().pending_offer = None;
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

#[no_mangle]
pub extern "C" fn chronodesk_send_input_move(x: i32, y: i32) {
    let tx = lock_state().transport_tx.clone();
    if let Some(ref tx) = tx {
        let _ = tx.send(TrCmd::SendMessage(ChannelMessage::InputMove { x, y }));
    }
}

#[no_mangle]
pub extern "C" fn chronodesk_send_input_click(button: u8, pressed: bool) {
    let tx = lock_state().transport_tx.clone();
    if let Some(ref tx) = tx {
        let _ = tx.send(TrCmd::SendMessage(ChannelMessage::InputClick {
            button,
            pressed,
        }));
    }
}

#[no_mangle]
pub extern "C" fn chronodesk_send_input_key(key: u64, pressed: bool) {
    let tx = lock_state().transport_tx.clone();
    if let Some(ref tx) = tx {
        let _ = tx.send(TrCmd::SendMessage(ChannelMessage::InputKey {
            key,
            pressed,
        }));
    }
}

#[no_mangle]
pub extern "C" fn chronodesk_get_log() -> *mut std::ffi::c_char {
    let log = logger::read_log();
    CString::new(log).unwrap_or_default().into_raw()
}
