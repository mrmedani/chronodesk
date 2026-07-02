use anyhow::{Context, Result};
use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use uuid::Uuid;

use crate::logger;

pub const CHUNK_SIZE: u64 = 64 * 1024;

pub fn generate_transfer_id() -> String {
    Uuid::new_v4().to_string()
}

pub struct OutgoingTransfer {
    pub file: std::fs::File,
    pub name: String,
    pub total_size: u64,
    pub offset: u64,
}

impl OutgoingTransfer {
    pub fn is_complete(&self) -> bool {
        self.offset >= self.total_size
    }
}

pub struct IncomingTransfer {
    pub file_path: PathBuf,
    pub name: String,
    pub total_size: u64,
    pub bytes_received: u64,
}

impl IncomingTransfer {
    pub fn is_complete(&self) -> bool {
        self.bytes_received >= self.total_size
    }
}

pub struct FileTransferManager {
    pub outgoing: HashMap<String, OutgoingTransfer>,
    pub incoming: HashMap<String, IncomingTransfer>,
}

impl Default for FileTransferManager {
    fn default() -> Self {
        Self::new()
    }
}

impl FileTransferManager {
    pub fn new() -> Self {
        Self {
            outgoing: HashMap::new(),
            incoming: HashMap::new(),
        }
    }

    pub fn prepare_outgoing(path: &str) -> Result<(String, OutgoingTransfer)> {
        let file = std::fs::File::open(path).context("opening file for transfer")?;
        let total_size = file
            .metadata()
            .context("getting file metadata")?
            .len();
        let name = Path::new(path)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "unknown".to_string());
        let id = generate_transfer_id();
        Ok((id, OutgoingTransfer { file, name, total_size, offset: 0 }))
    }

    pub fn incoming_progress(&self, id: &str) -> Option<(u64, u64)> {
        let t = self.incoming.get(id)?;
        Some((t.bytes_received, t.total_size))
    }

    pub fn cancel_transfer(&mut self, id: &str, download_dir: &Path) -> Option<(String, bool)> {
        if let Some(t) = self.outgoing.remove(id) {
            Some((t.name, false))
        } else if let Some(t) = self.incoming.remove(id) {
            let name = t.name.clone();
            let path = t.file_path.clone();
            if path.starts_with(download_dir) {
                if let Err(e) = std::fs::remove_file(&path) {
                    logger::write_log(&format!("failed to remove .part file: {e}"));
                }
            } else {
                logger::write_log(&format!("blocked path traversal: {path:?} not under {download_dir:?}"));
            }
            Some((name, true))
        } else {
            None
        }
    }
}

pub fn read_chunk(transfer: &mut OutgoingTransfer) -> Option<(u64, Vec<u8>)> {
    if transfer.offset >= transfer.total_size {
        return None;
    }
    let remaining = transfer.total_size.saturating_sub(transfer.offset);
    let to_read = CHUNK_SIZE.min(remaining);
    let mut buf = vec![0u8; to_read as usize];
    if transfer.file.read_exact(&mut buf).is_err() {
        logger::write_log(&format!(
            "read_chunk: read_exact failed at offset {}, size {}",
            transfer.offset, to_read
        ));
        return None;
    }
    let offset = transfer.offset;
    transfer.offset += to_read;
    Some((offset, buf))
}

pub fn sanitize_filename(name: &str) -> String {
    let name = name.trim();
    let name = name
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '.' || *c == '-' || *c == '_')
        .collect::<String>();
    if name.is_empty() || name == "." || name == ".." {
        "unknown".to_string()
    } else {
        name
    }
}
