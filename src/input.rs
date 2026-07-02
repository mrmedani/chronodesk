use anyhow::Result;
use enigo::{
    Button, Coordinate, Direction,
    Direction::{Click, Press, Release},
    Enigo, Key, Keyboard, Mouse, Settings,
};

pub struct InputController {
    enigo: Enigo,
}

pub fn logical_key_to_enigo(key_id: u64) -> Option<Key> {
    if (0x20..=0xFFFF).contains(&key_id) && key_id != 0x7F {
        return char::from_u32(key_id as u32).map(Key::Unicode);
    }
    match key_id {
        0x100000008 => Some(Key::Backspace),
        0x100000009 => Some(Key::Tab),
        0x10000000D => Some(Key::Return),
        0x10000001B => Some(Key::Escape),
        0x000000020 => Some(Key::Space),
        0x10000007F => Some(Key::Delete),
        0x100000039 => Some(Key::CapsLock),
        0x10000003A => Some(Key::F1),
        0x10000003B => Some(Key::F2),
        0x10000003C => Some(Key::F3),
        0x10000003D => Some(Key::F4),
        0x10000003E => Some(Key::F5),
        0x10000003F => Some(Key::F6),
        0x100000040 => Some(Key::F7),
        0x100000041 => Some(Key::F8),
        0x100000042 => Some(Key::F9),
        0x100000043 => Some(Key::F10),
        0x100000044 => Some(Key::F11),
        0x100000045 => Some(Key::F12),
        0x100000049 => Some(Key::Insert),
        0x10000004A => Some(Key::Home),
        0x10000004B => Some(Key::PageUp),
        0x10000004D => Some(Key::End),
        0x10000004E => Some(Key::PageDown),
        0x10000004F => Some(Key::RightArrow),
        0x100000050 => Some(Key::LeftArrow),
        0x100000051 => Some(Key::DownArrow),
        0x100000052 => Some(Key::UpArrow),
        0x1000000E0 | 0x1000000E4 => Some(Key::Control),
        0x1000000E1 | 0x1000000E5 => Some(Key::Shift),
        0x1000000E2 | 0x1000000E6 => Some(Key::Alt),
        0x1000000E3 | 0x1000000E7 => Some(Key::Meta),
        _ => None,
    }
}

impl InputController {
    pub fn new() -> Result<Self> {
        Ok(Self {
            enigo: Enigo::new(&Settings::default())
                .map_err(|e| anyhow::anyhow!("Failed to init enigo: {:?}", e))?,
        })
    }

    pub fn mouse_move(&mut self, x: i32, y: i32) -> Result<()> {
        self.enigo
            .move_mouse(x, y, Coordinate::Abs)
            .map_err(|e| anyhow::anyhow!("mouse_move: {:?}", e))?;
        Ok(())
    }

    pub fn mouse_click(&mut self, button: u8) -> Result<()> {
        let btn = match_button(button);
        self.enigo
            .button(btn, Click)
            .map_err(|e| anyhow::anyhow!("mouse_click: {:?}", e))?;
        Ok(())
    }

    pub fn mouse_down(&mut self, button: u8) -> Result<()> {
        let btn = match_button(button);
        self.enigo
            .button(btn, Press)
            .map_err(|e| anyhow::anyhow!("mouse_down: {:?}", e))?;
        Ok(())
    }

    pub fn mouse_up(&mut self, button: u8) -> Result<()> {
        let btn = match_button(button);
        self.enigo
            .button(btn, Release)
            .map_err(|e| anyhow::anyhow!("mouse_up: {:?}", e))?;
        Ok(())
    }

    pub fn key_press(&mut self, key: Key, direction: Direction) -> Result<()> {
        self.enigo
            .key(key, direction)
            .map_err(|e| anyhow::anyhow!("key: {:?}", e))?;
        Ok(())
    }
}

fn match_button(b: u8) -> Button {
    match b {
        1 => Button::Left,
        2 => Button::Right,
        3 => Button::Middle,
        _ => Button::Left,
    }
}
