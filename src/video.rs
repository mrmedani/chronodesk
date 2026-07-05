use anyhow::Result;
use std::collections::VecDeque;

pub struct EncodedPacket {
    pub data: Vec<u8>,
    pub keyframe: bool,
    pub pts: i64,
    pub codec: &'static str,
}

#[derive(Clone, Copy, PartialEq)]
pub enum EncoderType {
    Auto,
    Nvenc,
    QuickSync,
    Amf,
    Software,
    Vp8,
}

impl EncoderType {
    pub fn name(&self) -> &'static str {
        match self {
            EncoderType::Auto => "auto",
            EncoderType::Nvenc => "h264_nvenc",
            EncoderType::QuickSync => "h264_qsv",
            EncoderType::Amf => "h264_amf",
            EncoderType::Software => "webp",
            EncoderType::Vp8 => "webp",
        }
    }
}

pub struct VideoEncoder {
    width: u32,
    height: u32,
    encoder_type: EncoderType,
    frame_count: i64,
    quality: f32,
    target_bitrate: u32,
    target_fps: u32,
}

impl VideoEncoder {
    pub fn new(encoder_type: EncoderType, width: u32, height: u32) -> Result<Self> {
        let detected = detect_best_encoder();
        let actual = if encoder_type == EncoderType::Auto {
            detected
        } else {
            encoder_type
        };
        tracing::info!("Video encoder: {} ({}x{})", actual.name(), width, height);
        Ok(Self {
            width,
            height,
            encoder_type: actual,
            frame_count: 0,
            quality: 85.0,
            target_bitrate: 5_000_000,
            target_fps: 30,
        })
    }

    pub fn encode(&mut self, bgra_data: &[u8], width: u32, height: u32) -> Result<Vec<EncodedPacket>> {
        self.frame_count += 1;
        // Keep internal resolution aligned with actual frame dimensions
        self.width = width;
        self.height = height;
        match self.encoder_type {
            EncoderType::Vp8 | EncoderType::Software | EncoderType::Auto
            | EncoderType::Nvenc | EncoderType::QuickSync | EncoderType::Amf => {
                // All hardware encoders map to WebP until H.264 viewer support is added
                encode_webp(bgra_data, width, height, self.frame_count, self.quality)
            }
        }
    }

    pub fn flush(&mut self) -> Result<Vec<EncodedPacket>> {
        Ok(Vec::new())
    }

    pub fn request_keyframe(&mut self) {}

    pub fn set_quality(&mut self, quality: f32) {
        self.quality = quality.clamp(1.0, 100.0);
    }

    pub fn set_bitrate(&mut self, bitrate: u32) {
        self.target_bitrate = bitrate;
    }

    pub fn set_target_fps(&mut self, fps: u32) {
        self.target_fps = fps.clamp(1, 60);
    }

    pub fn set_resolution(&mut self, width: u32, height: u32) {
        self.width = width;
        self.height = height;
    }

    pub fn width(&self) -> u32 {
        self.width
    }
    pub fn height(&self) -> u32 {
        self.height
    }
    pub fn encoder_type(&self) -> EncoderType {
        self.encoder_type
    }
    pub fn quality(&self) -> f32 {
        self.quality
    }
    pub fn target_fps(&self) -> u32 {
        self.target_fps
    }
}

fn rgba_to_rgb(bgra: &[u8]) -> Vec<u8> {
    let mut rgb = Vec::with_capacity(bgra.len() / 4 * 3);
    for pixel in bgra.chunks_exact(4) {
        rgb.push(pixel[2]);
        rgb.push(pixel[1]);
        rgb.push(pixel[0]);
    }
    rgb
}

fn encode_webp(
    bgra_data: &[u8],
    width: u32,
    height: u32,
    pts: i64,
    quality: f32,
) -> Result<Vec<EncodedPacket>> {
    let rgb = rgba_to_rgb(bgra_data);
    let encoder = webp::Encoder::new(&rgb, webp::PixelLayout::Rgb, width, height);
    let mem = encoder.encode(quality);
    Ok(vec![EncodedPacket {
        data: mem.to_vec(),
        keyframe: true,
        pts,
        codec: "webp",
    }])
}

#[cfg(feature = "ffmpeg")]
#[allow(dead_code)]
fn encode_ffmpeg(
    bgra_data: &[u8],
    width: u32,
    height: u32,
    encoder_type: EncoderType,
    pts: i64,
) -> Result<Vec<EncodedPacket>> {
    use ffmpeg_next::{codec, encoder, format as ff, frame, Dictionary, Packet};

    let encoder_name = encoder_type.name();
    let codec = encoder::find_by_name(encoder_name)
        .or_else(|| {
            tracing::warn!("{encoder_name} not available, falling back to libx264");
            encoder::find_by_name("libx264")
        })
        .ok_or_else(|| {
            anyhow::anyhow!("No H.264 encoder found (install FFmpeg with h264 support)")
        })?;

    let mut enc = codec::context::Context::new_with_codec(codec)
        .encoder()
        .video()?;
    enc.set_width(width);
    enc.set_height(height);
    enc.set_format(ff::Pixel::YUV420P);

    let mut opts = Dictionary::new();
    match encoder_type {
        EncoderType::Nvenc => {
            opts.set("preset", "p4");
            opts.set("tune", "ll");
            opts.set("rc", "vbr");
            opts.set("cq", "23");
            opts.set("zerolatency", "1");
            opts.set("g", "120");
        }
        EncoderType::QuickSync => {
            opts.set("preset", "veryfast");
            opts.set("rc", "vbr");
            opts.set("global_quality", "23");
        }
        EncoderType::Amf => {
            opts.set("quality", "speed");
            opts.set("usage", "lowlatency");
        }
        _ => {
            opts.set("preset", "ultrafast");
            opts.set("tune", "zerolatency");
            opts.set("crf", "23");
        }
    }

    let mut encoder = enc.open_with(opts)?;
    let mut sws = ffmpeg_next::software::scaling::Context::get(
        ff::Pixel::BGRA,
        width,
        height,
        ff::Pixel::YUV420P,
        width,
        height,
        ffmpeg_next::software::scaling::Flags::BILINEAR,
    )?;

    let mut src = frame::Video::new(ff::Pixel::BGRA, width, height);
    src.data_mut(0).copy_from_slice(bgra_data);
    src.set_pts(Some(pts));

    let mut dst = frame::Video::new(ff::Pixel::YUV420P, width, height);
    sws.run(&src, &mut dst)?;

    let mut packets = Vec::new();
    encoder.send_frame(&dst)?;
    let mut packet = Packet::empty();
    loop {
        match encoder.receive_packet(&mut packet) {
            Ok(()) => {
                packets.push(EncodedPacket {
                    data: packet.data().unwrap_or_default().to_vec(),
                    keyframe: packet.is_key(),
                    pts,
                    codec: "h264",
                });
            }
            Err(ffmpeg_next::Error::Other {
                errno: ffmpeg_next::error::EAGAIN,
            }) => break,
            Err(e) => return Err(e.into()),
        }
    }
    Ok(packets)
}

fn detect_best_encoder() -> EncoderType {
    EncoderType::Software
}

/// Adaptive quality controller — adjusts quality, fps, and resolution based on RTT and frame sizes.
pub struct QualityController {
    pub quality: f32,
    pub target_fps: u32,
    pub resolution_scale: f32,
    rtt_estimate: f64,
    rtt_samples: VecDeque<f64>,
    frame_size_samples: VecDeque<usize>,
    adapt_count: u64,
    last_ping_time: Option<std::time::Instant>,
    last_adapt_frame: i64,
}

impl Default for QualityController {
    fn default() -> Self {
        Self::new()
    }
}

impl QualityController {
    pub fn new() -> Self {
        Self {
            quality: 85.0,
            target_fps: 30,
            resolution_scale: 1.0,
            rtt_estimate: 10.0,
            rtt_samples: VecDeque::with_capacity(10),
            frame_size_samples: VecDeque::new(),
            adapt_count: 0,
            last_ping_time: None,
            last_adapt_frame: 0,
        }
    }

    pub fn record_ping_sent(&mut self) {
        self.last_ping_time = Some(std::time::Instant::now());
    }

    pub fn record_pong_received(&mut self) {
        if let Some(t) = self.last_ping_time {
            let rtt = t.elapsed().as_secs_f64() * 1000.0;
            self.rtt_samples.push_back(rtt);
            if self.rtt_samples.len() > 10 {
                self.rtt_samples.pop_front();
            }
            self.rtt_estimate =
                self.rtt_samples.iter().sum::<f64>() / self.rtt_samples.len() as f64;
        }
    }

    pub fn record_frame_size(&mut self, size: usize) {
        self.frame_size_samples.push_back(size);
        if self.frame_size_samples.len() > 30 {
            self.frame_size_samples.pop_front();
        }
    }

    pub fn adapt(&mut self, encoder: &mut VideoEncoder, current_frame: i64) {
        if current_frame - self.last_adapt_frame < 15 {
            return;
        }
        self.last_adapt_frame = current_frame;
        self.adapt_count += 1;

        let avg_rtt = self.rtt_estimate;
        let avg_frame_size = self.frame_size_samples.iter().sum::<usize>() as f64
            / self.frame_size_samples.len().max(1) as f64;

        // Estimate bandwidth in Mbps from average frame size and target FPS
        let fps_actual = self.target_fps as f64;
        let bandwidth_mbps = avg_frame_size * fps_actual * 8.0 / 1_000_000.0;

        tracing::debug!(
            "Quality adapt #{}: rtt={:.0}ms, frame={:.0}B, bw={:.1}Mbps, quality={:.0}, fps={}, scale={:.2}",
            self.adapt_count, avg_rtt, avg_frame_size, bandwidth_mbps,
            self.quality, self.target_fps, self.resolution_scale,
        );

        if avg_rtt > 500.0 {
            self.quality = 15.0;
            self.resolution_scale = 0.5;
            self.target_fps = 10;
        } else if avg_rtt > 200.0 {
            self.quality = 30.0;
            self.resolution_scale = 0.75;
            self.target_fps = 15;
        } else if avg_rtt > 100.0 {
            self.quality = 50.0;
            self.resolution_scale = 0.85;
            self.target_fps = 24;
        } else if avg_rtt > 50.0 {
            self.quality = 65.0;
            self.resolution_scale = 1.0;
            self.target_fps = 30;
        } else {
            self.quality = (self.quality * 1.05).min(85.0);
            self.resolution_scale = 1.0;
            self.target_fps = 30;
        }

        encoder.set_quality(self.quality);
        encoder.set_target_fps(self.target_fps);
    }

    pub fn rtt_ms(&self) -> f64 {
        self.rtt_estimate
    }
}
