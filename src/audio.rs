use anyhow::Result;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use opus::{Application, Channels, Decoder, Encoder};
use std::sync::mpsc as std_mpsc;
use tokio::sync::mpsc;

pub const SAMPLE_RATE: u32 = 48000;
pub const FRAME_SIZE: usize = 960;
pub const CHANNELS: usize = 2;

pub struct AudioCapture {
    _shutdown_tx: Option<std_mpsc::Sender<()>>,
    _thread: Option<std::thread::JoinHandle<()>>,
}

impl AudioCapture {
    pub fn new() -> Result<(Self, mpsc::UnboundedReceiver<Vec<f32>>)> {
        let (tx, rx) = mpsc::unbounded_channel();
        let (shutdown_tx, shutdown_rx) = std_mpsc::channel();

        let thread = std::thread::Builder::new()
            .name("chronodesk-audio-capture".into())
            .spawn(move || {
                let host = cpal::default_host();
                let device = match host.default_input_device() {
                    Some(d) => d,
                    None => {
                        tracing::error!("No audio input device available");
                        return;
                    }
                };
                let config = match device.default_input_config() {
                    Ok(c) => c,
                    Err(e) => {
                        tracing::error!("Failed to get input config: {e}");
                        return;
                    }
                };

                tracing::info!(
                    "Audio input: {} ({:?})",
                    device.name().unwrap_or_default(),
                    config
                );

                let tx = tx.clone();
                let err_fn = move |err| {
                    tracing::error!("Audio capture error: {err}");
                };

                let stream_result = match config.sample_format() {
                    cpal::SampleFormat::F32 => device.build_input_stream(
                        &config.into(),
                        move |data: &[f32], _| {
                            let _ = tx.send(data.to_vec());
                        },
                        err_fn,
                        None,
                    ),
                    cpal::SampleFormat::I16 => device.build_input_stream(
                        &config.into(),
                        move |data: &[i16], _| {
                            let float_data: Vec<f32> =
                                data.iter().map(|&s| s as f32 / i16::MAX as f32).collect();
                            let _ = tx.send(float_data);
                        },
                        err_fn,
                        None,
                    ),
                    _ => device.build_input_stream(
                        &config.into(),
                        move |data: &[f32], _| {
                            let _ = tx.send(data.to_vec());
                        },
                        err_fn,
                        None,
                    ),
                };

                let stream = match stream_result {
                    Ok(s) => s,
                    Err(e) => {
                        tracing::error!("Failed to build input stream: {e}");
                        return;
                    }
                };

                if let Err(e) = stream.play() {
                    tracing::error!("Failed to start audio capture: {e}");
                    return;
                }

                let _ = shutdown_rx.recv();
                let _ = stream.pause();
            })?;

        Ok((
            Self {
                _shutdown_tx: Some(shutdown_tx),
                _thread: Some(thread),
            },
            rx,
        ))
    }
}

pub struct AudioPlayer {
    _shutdown_tx: Option<std_mpsc::Sender<()>>,
    _thread: Option<std::thread::JoinHandle<()>>,
    audio_tx: std_mpsc::Sender<Vec<f32>>,
}

impl AudioPlayer {
    pub fn new() -> Result<Self> {
        let (audio_tx, audio_rx) = std_mpsc::channel::<Vec<f32>>();
        let (shutdown_tx, shutdown_rx) = std_mpsc::channel();

        let thread = std::thread::Builder::new()
            .name("chronodesk-audio-playback".into())
            .spawn(move || {
                let host = cpal::default_host();
                let device = match host.default_output_device() {
                    Some(d) => d,
                    None => {
                        tracing::error!("No audio output device available");
                        return;
                    }
                };
                let config = match device.default_output_config() {
                    Ok(c) => c,
                    Err(e) => {
                        tracing::error!("Failed to get output config: {e}");
                        return;
                    }
                };

                tracing::info!(
                    "Audio output: {} ({:?})",
                    device.name().unwrap_or_default(),
                    config
                );

                use std::collections::VecDeque;
                let buffer: std::sync::Arc<std::sync::Mutex<VecDeque<f32>>> =
                    std::sync::Arc::new(std::sync::Mutex::new(VecDeque::new()));
                let buf = buffer.clone();
                let err_fn = move |err| {
                    tracing::error!("Audio playback error: {err}");
                };

                let stream_result = match config.sample_format() {
                    cpal::SampleFormat::F32 => device.build_output_stream(
                        &config.into(),
                        move |data: &mut [f32], _| {
                            let mut b = buf.lock().unwrap();
                            for sample in data.iter_mut() {
                                *sample = b.pop_front().unwrap_or(0.0);
                            }
                        },
                        err_fn,
                        None,
                    ),
                    _ => device.build_output_stream(
                        &config.into(),
                        move |data: &mut [f32], _| {
                            let mut b = buf.lock().unwrap();
                            for sample in data.iter_mut() {
                                *sample = b.pop_front().unwrap_or(0.0);
                            }
                        },
                        err_fn,
                        None,
                    ),
                };

                let stream = match stream_result {
                    Ok(s) => s,
                    Err(e) => {
                        tracing::error!("Failed to build output stream: {e}");
                        return;
                    }
                };

                if let Err(e) = stream.play() {
                    tracing::error!("Failed to start audio playback: {e}");
                    return;
                }

                loop {
                    match shutdown_rx.try_recv() {
                        Ok(_) | Err(std_mpsc::TryRecvError::Disconnected) => break,
                        _ => {}
                    }
                    match audio_rx.try_recv() {
                        Ok(samples) => {
                            let mut b = buffer.lock().unwrap();
                            b.extend(samples);
                        }
                        Err(std_mpsc::TryRecvError::Disconnected) => break,
                        _ => {}
                    }
                    std::thread::sleep(std::time::Duration::from_millis(5));
                }
                let _ = stream.pause();
            })?;

        Ok(Self {
            _shutdown_tx: Some(shutdown_tx),
            _thread: Some(thread),
            audio_tx,
        })
    }

    pub fn feed(&self, samples: &[f32]) {
        let _ = self.audio_tx.send(samples.to_vec());
    }
}

pub struct AudioCodec {
    encoder: Encoder,
    decoder: Decoder,
}

impl AudioCodec {
    pub fn new() -> Result<Self> {
        let encoder = Encoder::new(SAMPLE_RATE, Channels::Stereo, Application::Audio)
            .map_err(|e| anyhow::anyhow!("Failed to create Opus encoder: {e}"))?;
        let decoder = Decoder::new(SAMPLE_RATE, Channels::Stereo)
            .map_err(|e| anyhow::anyhow!("Failed to create Opus decoder: {e}"))?;

        Ok(Self { encoder, decoder })
    }

    pub fn encode(&mut self, pcm: &[f32]) -> Result<Vec<u8>> {
        let mut encoded = vec![0u8; 4000];
        let len = self
            .encoder
            .encode_float(pcm, &mut encoded)
            .map_err(|e| anyhow::anyhow!("Opus encode error: {e}"))?;
        encoded.truncate(len);
        Ok(encoded)
    }

    pub fn decode(&mut self, encoded: &[u8], out_pcm: &mut [f32]) -> Result<usize> {
        let len = self
            .decoder
            .decode_float(encoded, out_pcm, false)
            .map_err(|e| anyhow::anyhow!("Opus decode error: {e}"))?;
        Ok(len)
    }
}

pub fn resample_to_48k_stereo(input: &[f32], input_rate: u32, input_channels: u16) -> Vec<f32> {
    if input_rate == SAMPLE_RATE && input_channels as usize == CHANNELS {
        return input.to_vec();
    }

    let frame_count = input.len() / input_channels as usize;
    let target_len = (frame_count as u64 * SAMPLE_RATE as u64 / input_rate as u64) as usize;
    let mut output = vec![0.0f32; target_len * CHANNELS];

    for ch in 0..CHANNELS.min(input_channels as usize) {
        for i in 0..target_len {
            let src_idx = (i as u64 * input_rate as u64 / SAMPLE_RATE as u64) as usize;
            let src_pos = src_idx.min(frame_count - 1) * input_channels as usize + ch;
            output[i * CHANNELS + ch] = input[src_pos];
        }
    }

    output
}
