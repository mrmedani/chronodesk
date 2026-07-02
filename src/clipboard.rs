use std::time::Duration;
use tokio::sync::mpsc;

pub struct ClipboardSync;

impl ClipboardSync {
    pub fn start() -> (Self, mpsc::UnboundedReceiver<String>) {
        let (tx, rx) = mpsc::unbounded_channel();

        std::thread::Builder::new()
            .name("chronodesk-clipboard".into())
            .spawn(move || {
                let mut clipboard = match arboard::Clipboard::new() {
                    Ok(cb) => cb,
                    Err(e) => {
                        tracing::error!("Failed to init clipboard: {e}");
                        return;
                    }
                };

                let mut last = String::new();
                if let Ok(initial) = clipboard.get_text() {
                    last = initial;
                }

                loop {
                    std::thread::sleep(Duration::from_millis(500));
                    match clipboard.get_text() {
                        Ok(text) if text != last => {
                            let _ = tx.send(text.clone());
                            last = text;
                        }
                        Err(e) => {
                            tracing::debug!("Clipboard read error: {e}");
                        }
                        _ => {}
                    }
                }
            })
            .ok();

        (Self, rx)
    }

    pub fn write(text: &str) {
        if let Ok(mut clipboard) = arboard::Clipboard::new() {
            let _ = clipboard.set_text(text);
        }
    }
}
