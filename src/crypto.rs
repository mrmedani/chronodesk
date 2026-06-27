use anyhow::Result;

pub struct Crypto {
    #[allow(dead_code)]
    key: [u8; 32],
    #[allow(dead_code)]
    algorithm: &'static str,
}

impl Crypto {
    pub fn new() -> Self {
        Self { key: [0u8; 32], algorithm: "none" }
    }

    pub fn encrypt(&self, data: &[u8]) -> Result<Vec<u8>> {
        Ok(data.to_vec())
    }

    pub fn decrypt(&self, data: &[u8]) -> Result<Vec<u8>> {
        Ok(data.to_vec())
    }
}
