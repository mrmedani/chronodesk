use anyhow::{Context, Result};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct CapturedFrame {
    pub monitor_id: u32,
    pub width: usize,
    pub height: usize,
    pub data: Vec<u8>,
    pub dirty_rects: Vec<DirtyRect>,
}

#[derive(Debug, Clone)]
pub struct DirtyRect {
    pub x: usize,
    pub y: usize,
    pub width: usize,
    pub height: usize,
}

pub struct ScreenCapture {
    prev_frames: HashMap<u32, Vec<u8>>,
}

impl ScreenCapture {
    pub fn new() -> Result<Self> {
        let monitors = xcap::Monitor::all().context("Failed to enumerate monitors")?;
        tracing::info!("Found {} monitor(s)", monitors.len());
        Ok(Self {
            prev_frames: HashMap::new(),
        })
    }

    pub fn capture_all(&mut self) -> Result<Vec<CapturedFrame>> {
        let mut frames = Vec::new();
        let monitors = xcap::Monitor::all()?;

        for monitor in &monitors {
            let id = monitor.id()?;
            let img = monitor.capture_image()?;
            let (w, h) = (img.width() as usize, img.height() as usize);
            let data = img.into_raw();

            let dirty = self.detect_changes(id, &data, w, h);
            self.prev_frames.insert(id, data.clone());

            frames.push(CapturedFrame {
                monitor_id: id,
                width: w,
                height: h,
                data,
                dirty_rects: dirty,
            });
        }

        Ok(frames)
    }

    pub fn capture_monitor(&mut self, monitor_id: u32) -> Result<Option<CapturedFrame>> {
        let monitors = xcap::Monitor::all()?;
        let monitor = match monitors.iter().find(|m| m.id().ok() == Some(monitor_id)) {
            Some(m) => m,
            None => return Ok(None),
        };

        let img = monitor.capture_image()?;
        let (w, h) = (img.width() as usize, img.height() as usize);
        let data = img.into_raw();

        let dirty = self.detect_changes(monitor_id, &data, w, h);
        self.prev_frames.insert(monitor_id, data.clone());

        Ok(Some(CapturedFrame {
            monitor_id,
            width: w,
            height: h,
            data,
            dirty_rects: dirty,
        }))
    }

    fn detect_changes(
        &mut self,
        id: u32,
        new_data: &[u8],
        width: usize,
        height: usize,
    ) -> Vec<DirtyRect> {
        let prev = match self.prev_frames.get(&id) {
            Some(p) => p,
            None => {
                return vec![DirtyRect {
                    x: 0,
                    y: 0,
                    width,
                    height,
                }]
            }
        };

        if prev.len() != new_data.len() {
            return vec![DirtyRect {
                x: 0,
                y: 0,
                width,
                height,
            }];
        }

        let mut rects = Vec::new();
        let tile_size = 64;
        let pitch = width * 4;

        for ty in (0..height).step_by(tile_size) {
            for tx in (0..width).step_by(tile_size) {
                let tile_h = tile_size.min(height - ty);
                let tile_w = tile_size.min(width - tx);

                for y in ty..ty + tile_h {
                    let offset = y * pitch + tx * 4;
                    let end = offset + tile_w * 4;
                    if end > prev.len() || end > new_data.len() {
                        rects.push(DirtyRect {
                            x: tx,
                            y: ty,
                            width: tile_w,
                            height: tile_h,
                        });
                        break;
                    }
                    let prev_row = &prev[offset..end];
                    let new_row = &new_data[offset..end];

                    if prev_row != new_row {
                        rects.push(DirtyRect {
                            x: tx,
                            y: ty,
                            width: tile_w,
                            height: tile_h,
                        });
                        break;
                    }
                }
            }
        }

        rects
    }
}
