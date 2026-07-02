use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum ChannelMessage {
    Handshake {
        public_key: Vec<u8>,
    },
    Encrypted {
        data: Vec<u8>,
    },
    VideoFrame {
        width: u32,
        height: u32,
        codec: u8,
        data: Vec<u8>,
    },
    InputMove {
        x: i32,
        y: i32,
    },
    InputClick {
        button: u8,
        pressed: bool,
    },
    InputKey {
        key: u64,
        pressed: bool,
    },
    Clipboard {
        text: String,
    },
    AudioData {
        data: Vec<u8>,
        sample_rate: u32,
        channels: u16,
    },
    Ping {
        timestamp: i64,
    },
    Pong {
        timestamp: i64,
    },
    FileTransferRequest {
        id: String,
        name: String,
        size: u64,
    },
    FileTransferAccept {
        id: String,
    },
    FileTransferReject {
        id: String,
    },
    FileTransferChunk {
        id: String,
        offset: u64,
        data: Vec<u8>,
    },
    FileTransferComplete {
        id: String,
    },
    FileTransferError {
        id: String,
        message: String,
    },
}
