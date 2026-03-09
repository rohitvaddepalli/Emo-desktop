use anyhow::{anyhow, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Stream, StreamConfig};
use std::sync::{Arc, Mutex};

pub struct AudioInput {
    is_listening: Arc<Mutex<bool>>,
    stream: Option<Stream>,
    buffer: Arc<Mutex<Vec<f32>>>,
    sample_rate: Arc<Mutex<u32>>,
    max_buffer_size: usize,
}

impl AudioInput {
    pub fn new() -> Self {
        Self {
            is_listening: Arc::new(Mutex::new(false)),
            stream: None,
            buffer: Arc::new(Mutex::new(Vec::new())),
            sample_rate: Arc::new(Mutex::new(0)),
            max_buffer_size: 48000 * 10, // Max 10 seconds at 48kHz
        }
    }

    pub fn start(&mut self) -> Result<()> {
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or_else(|| anyhow!("No input device available"))?;

        // Use description() instead of deprecated name()
        let device_name = device.name().unwrap_or_else(|_| "Unknown".to_string());
        println!("AudioInput: Using input device: {}", device_name);

        let default_config = device.default_input_config()?;
        let sample_rate: u32 = default_config.sample_rate();
        let config: StreamConfig = default_config.into();
        let channels = config.channels;

        *self.sample_rate.lock().unwrap() = sample_rate;
        println!(
            "AudioInput: Sample rate: {} Hz, Channels: {}",
            sample_rate, channels
        );

        let is_listening_clone = self.is_listening.clone();
        let buffer_clone = self.buffer.clone();
        let max_size = self.max_buffer_size;

        *self.is_listening.lock().unwrap() = true;

        let err_fn = move |err| {
            eprintln!("AudioInput: Stream error: {}", err);
        };

        let stream = device.build_input_stream(
            &config,
            move |data: &[f32], _: &_| {
                if !*is_listening_clone.lock().unwrap() {
                    return;
                }

                // Naive downmixing (take first channel)
                let mut mono_samples = Vec::with_capacity(data.len() / (channels as usize));
                for frame in data.chunks(channels as usize) {
                    if let Some(sample) = frame.get(0) {
                        mono_samples.push(*sample);
                    }
                }

                // Add to buffer with size limit
                if let Ok(mut buf) = buffer_clone.lock() {
                    buf.extend(&mono_samples);

                    // Prevent buffer overflow
                    if buf.len() > max_size {
                        let remove_count = buf.len() - max_size;
                        buf.drain(0..remove_count);
                    }
                }
            },
            err_fn,
            None,
        )?;

        stream.play()?;
        println!("AudioInput: Stream started successfully");
        self.stream = Some(stream);
        Ok(())
    }

    pub fn stop(&mut self) {
        println!("AudioInput: Stopping stream...");
        if let Some(stream) = self.stream.take() {
            drop(stream);
        }
        *self.is_listening.lock().unwrap() = false;

        // Clear buffer
        if let Ok(mut buf) = self.buffer.lock() {
            buf.clear();
        }
    }

    pub fn flush(&self) -> Vec<f32> {
        if let Ok(mut buf) = self.buffer.lock() {
            let data = buf.clone();
            buf.clear();
            data
        } else {
            Vec::new()
        }
    }

    pub fn get_sample_rate(&self) -> u32 {
        *self.sample_rate.lock().unwrap()
    }

    pub fn is_running(&self) -> bool {
        *self.is_listening.lock().unwrap()
    }
}
