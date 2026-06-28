use anyhow::Result;
use enigo::{
    Button, Coordinate, Direction,
    Direction::{Click, Press, Release},
    Enigo, Key, Keyboard, Mouse, Settings,
};

pub struct InputController {
    enigo: Enigo,
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
