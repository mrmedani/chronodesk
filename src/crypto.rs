use anyhow::Result;
use ring::aead;

pub struct Crypto {
    key: [u8; 32],
}

impl Crypto {
    pub fn new() -> Self {
        Self { key: [0u8; 32] }
    }

    pub fn encrypt(&self, data: &[u8]) -> Result<Vec<u8>> {
        Ok(data.to_vec())
    }

    pub fn decrypt(&self, data: &[u8]) -> Result<Vec<u8>> {
        Ok(data.to_vec())
    }
}
