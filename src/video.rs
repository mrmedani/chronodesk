use anyhow::Result;

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
}

impl EncoderType {
    pub fn name(&self) -> &'static str {
        match self {
            EncoderType::Auto => "auto",
            EncoderType::Nvenc => "h264_nvenc",
            EncoderType::QuickSync => "h264_qsv",
            EncoderType::Amf => "h264_amf",
            EncoderType::Software => "jpeg",
        }
    }
}

pub struct VideoEncoder {
    width: u32,
    height: u32,
    encoder_type: EncoderType,
    frame_count: i64,
    quality: u8,
}

impl VideoEncoder {
    pub fn new(encoder_type: EncoderType, width: u32, height: u32) -> Result<Self> {
        let detected = detect_best_encoder();

        let actual = if encoder_type == EncoderType::Auto {
            if cfg!(feature = "ffmpeg") {
                detected
            } else {
                EncoderType::Software
            }
        } else {
            encoder_type
        };

        tracing::info!("Video encoder: {} ({}x{})", actual.name(), width, height);

        Ok(Self {
            width,
            height,
            encoder_type: actual,
            frame_count: 0,
            quality: 85,
        })
    }

    pub fn encode(&mut self, bgra_data: &[u8]) -> Result<Vec<EncodedPacket>> {
        self.frame_count += 1;

        #[cfg(feature = "ffmpeg")]
        {
            return encode_ffmpeg(
                bgra_data,
                self.width,
                self.height,
                self.encoder_type,
                self.frame_count,
            );
        }

        #[cfg(not(feature = "ffmpeg"))]
        {
            encode_jpeg(bgra_data, self.width, self.height, self.frame_count, self.quality)
        }
    }

    pub fn flush(&mut self) -> Result<Vec<EncodedPacket>> {
        Ok(Vec::new())
    }

    pub fn request_keyframe(&mut self) {}

    pub fn set_quality(&mut self, quality: u8) {
        self.quality = quality;
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
}

#[cfg(not(feature = "ffmpeg"))]
fn encode_jpeg(
    bgra_data: &[u8],
    width: u32,
    height: u32,
    pts: i64,
    quality: u8,
) -> Result<Vec<EncodedPacket>> {
    use image::codecs::jpeg::JpegEncoder;
    use image::ColorType;

    let mut rgb = Vec::with_capacity((width * height * 3) as usize);
    for pixel in bgra_data.chunks(4) {
        rgb.push(pixel[2]);
        rgb.push(pixel[1]);
        rgb.push(pixel[0]);
    }

    let mut jpeg_data = Vec::new();
    let mut encoder = JpegEncoder::new_with_quality(&mut jpeg_data, quality);
    encoder.encode(&rgb, width, height, ColorType::Rgb8.into())?;

    Ok(vec![EncodedPacket {
        data: jpeg_data,
        keyframe: true,
        pts,
        codec: "jpeg",
    }])
}

#[cfg(feature = "ffmpeg")]
fn encode_ffmpeg(
    bgra_data: &[u8],
    width: u32,
    height: u32,
    encoder_type: EncoderType,
    pts: i64,
) -> Result<Vec<EncodedPacket>> {
    use ffmpeg_next::format as ff;

    let encoder_name = encoder_type.name();

    let codec = ffmpeg_next::encoder::find_encoder_by_name(encoder_name)
        .or_else(|_| {
            tracing::warn!("{encoder_name} not available, falling back to libx264");
            ffmpeg_next::encoder::find_encoder_by_name("libx264")
        })
        .map_err(|_| anyhow::anyhow!("No H.264 encoder found (install FFmpeg with h264 support)"))?;

    let mut encoder = ffmpeg_next::encoder::Encoder::new(codec)?;
    let ctx = encoder.encoder();

    ctx.set_width(width as i32);
    ctx.set_height(height as i32);
    ctx.set_format(ff::Pixel::YUV420P);
    ctx.set_time_base(ffmpeg_next::Rational::new(1, 60));
    ctx.set_frame_rate(Some(ffmpeg_next::Rational::new(60, 1)));

    let mut opts = ffmpeg_next::Dictionary::new();

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

    encoder.open_with_codec_options(opts)?;

    let mut sws = ffmpeg_next::software::scaling::Context::get(
        ff::Pixel::BGRA,
        width as i32,
        height as i32,
        ff::Pixel::YUV420P,
        width as i32,
        height as i32,
        ffmpeg_next::software::scaling::Flags::BILINEAR,
    )?;

    let mut src = ffmpeg_next::frame::Video::new(ff::Pixel::BGRA, width as i32, height as i32);
    src.data_mut(0).copy_from_slice(bgra_data);
    src.set_pts(Some(pts));

    let mut dst = ffmpeg_next::frame::Video::new(ff::Pixel::YUV420P, width as i32, height as i32);
    sws.run(&src, &mut dst)?;

    let mut packets = Vec::new();
    encoder.send_frame(&dst)?;

    loop {
        let mut packet = ffmpeg_next::Packet::empty();
        match encoder.receive_packet(&mut packet) {
            Ok(()) => {
                packets.push(EncodedPacket {
                    data: packet.data().to_vec(),
                    keyframe: packet.is_key(),
                    pts,
                    codec: "h264",
                });
            }
            Err(ffmpeg_next::Error::EAGAIN) => break,
            Err(e) => return Err(e.into()),
        }
    }

    Ok(packets)
}

fn detect_best_encoder() -> EncoderType {
    #[cfg(feature = "ffmpeg")]
    {
        if let Ok(_) = ffmpeg_next::encoder::find_encoder_by_name("h264_nvenc") {
            return EncoderType::Nvenc;
        }
        if let Ok(_) = ffmpeg_next::encoder::find_encoder_by_name("h264_qsv") {
            return EncoderType::QuickSync;
        }
        if let Ok(_) = ffmpeg_next::encoder::find_encoder_by_name("h264_amf") {
            return EncoderType::Amf;
        }
    }
    EncoderType::Software
}
