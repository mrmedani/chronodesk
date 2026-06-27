use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum ChannelMessage {
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
        key: u32,
        pressed: bool,
    },
    Clipboard {
        text: String,
    },
    Ping {
        timestamp: i64,
    },
    Pong {
        timestamp: i64,
    },
}
