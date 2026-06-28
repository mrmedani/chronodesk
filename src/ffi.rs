use crate::capture::ScreenCapture;
use crate::network::signaling::{SignalCommand as SigCmd, SignalEvent, SignalingClient};
use crate::network::transport::{SignalCommand as TrCmd, Transport, TransportEvent};
use crate::protocol::ChannelMessage;
use crate::video::{EncoderType, VideoEncoder};
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

fn push_event(json: &str) {
    if let Ok(mut s) = state().lock() {
        s.events.push(json.to_string());
    }
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
    let _ = std::fs::create_dir_all(path.parent().unwrap());
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

#[no_mangle]
pub extern "C" fn chronodesk_init() {
    let addr = get_signaling_addr();
    let id = load_or_create_id();
    {
        let mut s = state().lock().unwrap();
        s.peer_id = id.clone();
    }
    push_event(&format!(
        r#"{{"type":"init","peer_id":"{}","signaling_addr":"{}"}}"#,
        id, addr
    ));
    rt().spawn(async move {
        run_loop(&addr, &id).await;
    });
}

#[no_mangle]
pub extern "C" fn chronodesk_get_config(key: *const std::ffi::c_char) -> *mut std::ffi::c_char {
    let key = unsafe { CStr::from_ptr(key) }.to_str().unwrap_or("");
    let config = load_config();
    let val = config.get(key).and_then(|v| v.as_str()).unwrap_or("");
    CString::new(val).unwrap_or_default().into_raw()
}

#[no_mangle]
pub extern "C" fn chronodesk_set_config(
    key: *const std::ffi::c_char,
    value: *const std::ffi::c_char,
) {
    let key = unsafe { CStr::from_ptr(key) }
        .to_str()
        .unwrap_or("")
        .to_string();
    let value = unsafe { CStr::from_ptr(value) }
        .to_str()
        .unwrap_or("")
        .to_string();
    let mut config = load_config();
    config[&key] = serde_json::json!(&value);
    save_config(&config);
    push_event(&format!(
        r#"{{"type":"config_updated","key":"{}","value":"{}"}}"#,
        key, value
    ));
}

async fn run_loop(signaling_addr: &str, my_id: &str) {
    let (signaling_client, mut signal_events) = SignalingClient::new(signaling_addr, my_id);
    let signaling_tx = signaling_client.channel();

    let stun_addr = format!(
        "stun:{}",
        signaling_addr.split(':').next().unwrap_or("144.24.201.196")
    );

    let (transport, mut transport_events) =
        match Transport::new(my_id, &stun_addr, Some(signaling_tx.clone())).await {
            Ok(t) => t,
            Err(e) => {
                push_event(&format!(
                    r#"{{"type":"error","msg":"Transport init: {}"}}"#,
                    e
                ));
                return;
            }
        };
    let transport_tx = transport.signal_tx();

    {
        let mut s = state().lock().unwrap();
        s.transport_tx = Some(transport_tx.clone());
        s.signaling_tx = Some(signaling_tx.clone());
    }

    tokio::spawn(async move {
        if let Err(e) = signaling_client.run().await {
            push_event(&format!(r#"{{"type":"error","msg":"Signaling: {}"}}"#, e));
        }
    });

    let mut capture = ScreenCapture::new().ok();
    let mut encoder = VideoEncoder::new(EncoderType::Auto, 1920, 1080).ok();
    let mut capture_active = false;

    loop {
        tokio::select! {
            Some(event) = signal_events.recv() => {
                match event {
                    SignalEvent::Offer { from, sdp } => {
                        let s = state().lock().unwrap();
                        if s.connected || s.pending_offer.is_some() {
                            continue;
                        }
                        drop(s);
                        {
                            let mut s2 = state().lock().unwrap();
                            s2.pending_offer = Some((from.clone(), sdp));
                        }
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
                        let mut s = state().lock().unwrap();
                        s.connected = true;
                        capture_active = s.is_host;
                        push_event(r#"{"type":"connected"}"#);
                    }
                    TransportEvent::Disconnected { .. } => {
                        capture_active = false;
                        {
                            let mut s = state().lock().unwrap();
                            s.connected = false;
                            s.is_host = false;
                        }
                        push_event(r#"{"type":"disconnected"}"#);
                    }
                    TransportEvent::MessageReceived { msg } => {
                        match msg {
                            ChannelMessage::VideoFrame { width, height, codec, data } => {
                                let rgba = if codec == 0 {
                                    jpeg_to_rgba(&data, width as usize, height as usize)
                                } else {
                                    Vec::new()
                                };
                                if !rgba.is_empty() {
                                    let mut s = state().lock().unwrap();
                                    s.frame_rgba = rgba;
                                    s.frame_width = width;
                                    s.frame_height = height;
                                    s.frame_ready = true;
                                }
                                push_event(&format!(r#"{{"type":"frame","w":{},"h":{},"codec":{},"size":{}}}"#, width, height, codec, data.len()));
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
                            ChannelMessage::InputKey { .. } => {}
                            ChannelMessage::Clipboard { text } => {
                                push_event(&format!(r#"{{"type":"clipboard","text":"{}"}}"#, text));
                            }
                            ChannelMessage::Ping { .. } => {}
                            ChannelMessage::Pong { .. } => {}
                        }
                    }
                    TransportEvent::Error { msg } => {
                        push_event(&format!(r#"{{"type":"error","msg":"{}"}}"#, msg));
                    }
                }
            }
            _ = tokio::time::sleep(tokio::time::Duration::from_millis(16)) => {
                if capture_active {
                    if let (Some(ref mut cap), Some(ref mut enc)) = (&mut capture, &mut encoder) {
                        if let Ok(frames) = cap.capture_all() {
                            for frame in &frames {
                                if frame.dirty_rects.is_empty() { continue; }
                                if let Ok(packets) = enc.encode(&frame.data) {
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
pub extern "C" fn chronodesk_connect(peer_id: *const std::ffi::c_char) {
    let target = unsafe { CStr::from_ptr(peer_id) }
        .to_str()
        .unwrap_or("")
        .to_string();
    if target.is_empty() {
        return;
    }
    push_event(&format!(r#"{{"type":"connecting","to":"{}"}}"#, target));
    rt().spawn(async move {
        let s = state().lock().unwrap();
        if let Some(ref tx) = s.transport_tx {
            let _ = tx.send(TrCmd::CreateOffer(target));
        }
    });
}

#[no_mangle]
pub extern "C" fn chronodesk_accept() {
    let pending = state()
        .lock()
        .map(|mut s| s.pending_offer.take())
        .ok()
        .flatten();
    if let Some((from, sdp)) = pending {
        let mut s = state().lock().unwrap();
        s.is_host = true;
        if let Some(ref tx) = s.transport_tx {
            let _ = tx.send(TrCmd::HandleOffer(from, sdp));
        }
        drop(s);
        push_event(r#"{"type":"accepted"}"#);
    }
}

#[no_mangle]
pub extern "C" fn chronodesk_deny() {
    let _ = state().lock().map(|mut s| {
        s.pending_offer = None;
    });
    push_event(r#"{"type":"denied"}"#);
}

#[no_mangle]
pub extern "C" fn chronodesk_disconnect() {
    rt().spawn(async {
        let s = state().lock().unwrap();
        if let Some(ref tx) = s.transport_tx {
            let _ = tx.send(TrCmd::Disconnect);
        }
    });
}

#[no_mangle]
pub extern "C" fn chronodesk_get_frame(
    out_data: *mut *mut u8,
    out_len: *mut i32,
    out_width: *mut i32,
    out_height: *mut i32,
) -> i32 {
    let mut s = state().lock().unwrap();
    if !s.frame_ready {
        return 0;
    }
    let len = s.frame_rgba.len() as i32;
    let width = s.frame_width as i32;
    let height = s.frame_height as i32;
    unsafe {
        let ptr = libc::malloc(len as usize) as *mut u8;
        std::ptr::copy_nonoverlapping(s.frame_rgba.as_ptr(), ptr, len as usize);
        *out_data = ptr;
        *out_len = len;
        *out_width = width;
        *out_height = height;
    }
    s.frame_ready = false;
    1
}

#[no_mangle]
pub extern "C" fn chronodesk_free_frame(ptr: *mut u8) {
    if !ptr.is_null() {
        unsafe {
            libc::free(ptr as *mut std::ffi::c_void);
        }
    }
}

#[no_mangle]
pub extern "C" fn chronodesk_send_input_move(x: i32, y: i32) {
    let s = state().lock().unwrap();
    if let Some(ref tx) = s.transport_tx {
        let _ = tx.send(TrCmd::SendMessage(ChannelMessage::InputMove { x, y }));
    }
}

#[no_mangle]
pub extern "C" fn chronodesk_send_input_click(button: u8, pressed: bool) {
    let s = state().lock().unwrap();
    if let Some(ref tx) = s.transport_tx {
        let _ = tx.send(TrCmd::SendMessage(ChannelMessage::InputClick {
            button,
            pressed,
        }));
    }
}
