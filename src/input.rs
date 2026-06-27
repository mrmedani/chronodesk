use anyhow::Result;

pub struct InputController;

#[cfg(target_os = "windows")]
mod platform {
    use super::*;
    pub fn inject_mouse(x: i32, y: i32) -> Result<()> {
        Ok(())
    }
    pub fn inject_key(key: u32, pressed: bool) -> Result<()> {
        Ok(())
    }
}

#[cfg(target_os = "linux")]
mod platform {
    use super::*;
    pub fn inject_mouse(x: i32, y: i32) -> Result<()> {
        Ok(())
    }
    pub fn inject_key(key: u32, pressed: bool) -> Result<()> {
        Ok(())
    }
}

#[cfg(target_os = "macos")]
mod platform {
    use super::*;
    pub fn inject_mouse(x: i32, y: i32) -> Result<()> {
        Ok(())
    }
    pub fn inject_key(key: u32, pressed: bool) -> Result<()> {
        Ok(())
    }
}
