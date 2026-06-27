use anyhow::Result;

pub struct ScreenCapture;

impl ScreenCapture {
    pub fn new() -> Self {
        Self
    }

    pub fn capture_frame(&self) -> Result<Vec<u8>> {
        Ok(Vec::new())
    }
}
