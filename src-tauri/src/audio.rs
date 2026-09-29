//! Microphone capture. cpal streams aren't Send on every platform, so one dedicated
//! thread owns the stream and the rest of the app talks to it over a channel.

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, Sample, SizedSample};
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};

pub const RATE: u32 = 16_000;

enum Cmd {
    Start(mpsc::Sender<Result<(), String>>),
    Stop(mpsc::Sender<Vec<f32>>),
}

pub struct Recorder {
    tx: Mutex<mpsc::Sender<Cmd>>,
}

struct Active {
    _stream: cpal::Stream,
    buf: Arc<Mutex<Vec<f32>>>,
    rate: u32,
}

impl Recorder {
    pub fn spawn(app: AppHandle) -> Self {
        let (tx, rx) = mpsc::channel::<Cmd>();
        std::thread::spawn(move || {
            let mut active: Option<Active> = None;
            while let Ok(cmd) = rx.recv() {
                match cmd {
                    Cmd::Start(reply) => {
                        active = None;
                        let res = open(&app).map(|a| active = Some(a));
                        let _ = reply.send(res);
                    }
                    Cmd::Stop(reply) => {
                        let samples = match active.take() {
                            Some(a) => {
                                let rate = a.rate;
                                let buf = std::mem::take(&mut *a.buf.lock().unwrap());
                                drop(a);
                                resample(&buf, rate)
                            }
                            None => Vec::new(),
                        };
                        let _ = reply.send(samples);
                    }
                }
            }
        });
        Self { tx: Mutex::new(tx) }
    }

    pub fn start(&self) -> Result<(), String> {
        let (reply, rx) = mpsc::channel();
        self.tx.lock().unwrap().send(Cmd::Start(reply)).map_err(|e| e.to_string())?;
        rx.recv().map_err(|e| e.to_string())?
    }

    /// Stops recording and returns 16 kHz mono samples.
    pub fn stop(&self) -> Vec<f32> {
        let (reply, rx) = mpsc::channel();
        if self.tx.lock().unwrap().send(Cmd::Stop(reply)).is_err() {
            return Vec::new();
        }
        rx.recv().unwrap_or_default()
    }
}

/// Collects mono samples and streams a mic level to the overlay for the waveform.
struct Sink {
    buf: Arc<Mutex<Vec<f32>>>,
    app: AppHandle,
    sum_sq: f32,
    n: usize,
    last_emit: Instant,
}

impl Sink {
    fn push(&mut self, mono: impl Iterator<Item = f32>) {
        let mut buf = self.buf.lock().unwrap();
        for s in mono {
            buf.push(s);
            self.sum_sq += s * s;
            self.n += 1;
        }
        drop(buf);
        if self.last_emit.elapsed() >= Duration::from_millis(40) && self.n > 0 {
            let rms = (self.sum_sq / self.n as f32).sqrt();
            let _ = self.app.emit_to("overlay", "yap://level", rms);
            self.sum_sq = 0.0;
            self.n = 0;
            self.last_emit = Instant::now();
        }
    }
}

fn open(app: &AppHandle) -> Result<Active, String> {
    let host = cpal::default_host();
    let device = host
        .default_input_device()
        .ok_or("No microphone found. Plug one in or check your sound settings.")?;
    let supported = device.default_input_config().map_err(|e| format!("Microphone unavailable: {e}"))?;
    let rate = supported.sample_rate().0;
    let channels = supported.channels() as usize;
    let config: cpal::StreamConfig = supported.config();
    let buf = Arc::new(Mutex::new(Vec::with_capacity(rate as usize * 30)));
    let sink = Sink { buf: buf.clone(), app: app.clone(), sum_sq: 0.0, n: 0, last_emit: Instant::now() };

    let stream = match supported.sample_format() {
        cpal::SampleFormat::F32 => build::<f32>(&device, &config, channels, sink),
        cpal::SampleFormat::I16 => build::<i16>(&device, &config, channels, sink),
        cpal::SampleFormat::U16 => build::<u16>(&device, &config, channels, sink),
        cpal::SampleFormat::I32 => build::<i32>(&device, &config, channels, sink),
        other => return Err(format!("Unsupported microphone format: {other:?}")),
    }
    .map_err(|e| format!("Couldn't open the microphone: {e}"))?;
    stream.play().map_err(|e| format!("Couldn't start the microphone: {e}"))?;
    Ok(Active { _stream: stream, buf, rate })
}

fn build<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    channels: usize,
    mut sink: Sink,
) -> Result<cpal::Stream, cpal::BuildStreamError>
where
    T: SizedSample,
    f32: FromSample<T>,
{
    device.build_input_stream(
        config,
        move |data: &[T], _: &cpal::InputCallbackInfo| {
            sink.push(
                data.chunks(channels)
                    .map(|frame| frame.iter().map(|s| f32::from_sample(*s)).sum::<f32>() / channels as f32),
            );
        },
        |err| eprintln!("yap: mic error: {err}"),
        None,
    )
}

/// A mic stream that hands each chunk of mono samples to `push` as it arrives, for Notes, which
/// transcribes while you're still talking. Separate from the push-to-talk `Recorder`, so a note
/// can keep running while you dictate. Samples come at the device's rate, returned alongside;
/// dropping the stream stops it. Not Send: keep it on the thread that opened it.
pub fn open_streaming(push: impl FnMut(&[f32]) + Send + 'static) -> Result<(cpal::Stream, u32), String> {
    let device = cpal::default_host()
        .default_input_device()
        .ok_or("No microphone found. Plug one in or check your sound settings.")?;
    let supported = device.default_input_config().map_err(|e| format!("Microphone unavailable: {e}"))?;
    let rate = supported.sample_rate().0;
    let channels = supported.channels() as usize;
    let config: cpal::StreamConfig = supported.config();
    let stream = match supported.sample_format() {
        cpal::SampleFormat::F32 => build_streaming::<f32>(&device, &config, channels, push),
        cpal::SampleFormat::I16 => build_streaming::<i16>(&device, &config, channels, push),
        cpal::SampleFormat::U16 => build_streaming::<u16>(&device, &config, channels, push),
        cpal::SampleFormat::I32 => build_streaming::<i32>(&device, &config, channels, push),
        other => return Err(format!("Unsupported microphone format: {other:?}")),
    }
    .map_err(|e| format!("Couldn't open the microphone: {e}"))?;
    stream.play().map_err(|e| format!("Couldn't start the microphone: {e}"))?;
    Ok((stream, rate))
}

fn build_streaming<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    channels: usize,
    mut push: impl FnMut(&[f32]) + Send + 'static,
) -> Result<cpal::Stream, cpal::BuildStreamError>
where
    T: SizedSample,
    f32: FromSample<T>,
{
    let mut mono = Vec::new();
    device.build_input_stream(
        config,
        move |data: &[T], _: &cpal::InputCallbackInfo| {
            mono.clear();
            mono.extend(
                data.chunks(channels)
                    .map(|frame| frame.iter().map(|s| f32::from_sample(*s)).sum::<f32>() / channels as f32),
            );
            push(&mono);
        },
        |err| eprintln!("yap: notes mic error: {err}"),
        None,
    )
}

/// Box-filter + nearest resample to 16 kHz. Crude, but plenty for speech recognition.
pub(crate) fn resample(input: &[f32], rate: u32) -> Vec<f32> {
    if rate == RATE || input.is_empty() {
        return input.to_vec();
    }
    let ratio = rate as f64 / RATE as f64;
    let half = (ratio / 2.0).max(0.5);
    let out_len = (input.len() as f64 / ratio) as usize;
    let last = input.len() - 1;
    (0..out_len)
        .map(|i| {
            let center = i as f64 * ratio;
            let lo = ((center - half).floor().max(0.0) as usize).min(last);
            let hi = ((center + half).ceil() as usize).min(last).max(lo);
            let window = &input[lo..=hi];
            window.iter().sum::<f32>() / window.len() as f32
        })
        .collect()
}

pub fn rms(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    (samples.iter().map(|s| s * s).sum::<f32>() / samples.len() as f32).sqrt()
}
