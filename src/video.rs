use anyhow::Result;

pub enum EncoderType {
    Nvenc,
    QuickSync,
    Amf,
    Software,
}

pub struct VideoEncoder;

impl VideoEncoder {
    pub fn new(encoder: EncoderType) -> Self {
        Self
    }

    pub fn encode(&self, frame: &[u8]) -> Result<Vec<u8>> {
        Ok(Vec::new())
    }
}
