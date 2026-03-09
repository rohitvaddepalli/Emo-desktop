use crate::voice::audio_input::AudioInput;
use crate::voice::stt::SttEngine;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use tauri::{AppHandle, Emitter};

pub struct VoiceManager {
    audio_input: Arc<Mutex<AudioInput>>,
    stt: Arc<Mutex<Option<SttEngine>>>,
    is_running: Arc<Mutex<bool>>,
}

impl VoiceManager {
    pub fn new() -> Self {
        let model_dir = "../models/whisper";
        println!("VoiceManager: Initializing STT from {}", model_dir);

        let stt = match SttEngine::new(model_dir) {
            Ok(engine) => {
                println!("VoiceManager: STT Engine loaded successfully");
                Some(engine)
            }
            Err(e) => {
                eprintln!("VoiceManager: Failed to load STT Engine: {}", e);
                eprintln!("VoiceManager: Voice recognition will be unavailable");
                None
            }
        };

        Self {
            audio_input: Arc::new(Mutex::new(AudioInput::new())),
            stt: Arc::new(Mutex::new(stt)),
            is_running: Arc::new(Mutex::new(false)),
        }
    }

    pub fn start(&self, app_handle: AppHandle) {
        let is_running = self.is_running.clone();
        let audio_input = self.audio_input.clone();
        let stt = self.stt.clone();

        {
            let mut running = is_running.lock().unwrap();
            if *running {
                println!("VoiceManager: Already running");
                return;
            }
            *running = true;
        }

        // Check if STT is available
        let has_stt = stt.lock().unwrap().is_some();
        if !has_stt {
            eprintln!("VoiceManager: Cannot start - STT engine not loaded");
            *is_running.lock().unwrap() = false;
            return;
        }

        // Start audio input
        if let Ok(mut input) = audio_input.lock() {
            if let Err(e) = input.start() {
                eprintln!("VoiceManager: Failed to start audio input: {}", e);
                *is_running.lock().unwrap() = false;
                return;
            }
        }

        println!("VoiceManager: Starting voice loop...");

        // Spawn VAD Loop
        thread::spawn(move || {
            let mut sample_rate = 0u32;

            // Wait for sample rate with timeout
            let start_wait = std::time::Instant::now();
            loop {
                if !*is_running.lock().unwrap() {
                    return;
                }
                if start_wait.elapsed() > Duration::from_secs(5) {
                    eprintln!("VoiceManager: Timeout waiting for audio");
                    return;
                }
                if let Ok(input) = audio_input.lock() {
                    sample_rate = input.get_sample_rate();
                }
                if sample_rate > 0 {
                    break;
                }
                thread::sleep(Duration::from_millis(50));
            }

            println!("VoiceManager: Audio started at {} Hz", sample_rate);

            // Emit ready event
            let _ = app_handle.emit("voice-ready", true);

            let mut speech_buffer: Vec<f32> = Vec::new();
            let mut silence_frames = 0u32;
            let mut is_speaking = false;
            let mut noise_floor: f32 = 0.0;
            let mut frame_count: u32 = 0;

            // VAD Configuration
            let rms_threshold_base: f32 = 0.02; // Lower base threshold
            let silence_limit_frames: u32 = 20; // 2 seconds of silence
            let min_speech_frames: u32 = 5; // At least 500ms of speech
            let mut speech_frames: u32 = 0;

            // Calibration period (first 50 frames = 5 seconds)
            let calibration_frames: u32 = 50;

            loop {
                if !*is_running.lock().unwrap() {
                    println!("VoiceManager: Stopping voice loop");
                    break;
                }

                thread::sleep(Duration::from_millis(100));

                let raw_data = if let Ok(input) = audio_input.lock() {
                    input.flush()
                } else {
                    Vec::new()
                };

                if raw_data.is_empty() {
                    continue;
                }

                // Resample to 16kHz for Whisper
                let chunk_16k = resample_linear(&raw_data, sample_rate, 16000);

                // Calculate RMS
                let rms = calculate_rms(&chunk_16k);
                frame_count += 1;

                // Calibration: measure background noise
                if frame_count <= calibration_frames {
                    noise_floor = noise_floor * 0.9 + rms * 0.1; // Exponential moving average
                    if frame_count == calibration_frames {
                        println!("VoiceManager: Noise floor calibrated to {:.4}", noise_floor);
                    }
                    continue;
                }

                // Dynamic threshold based on noise floor
                let threshold = (noise_floor * 2.0).max(rms_threshold_base);

                if rms > threshold {
                    if !is_speaking {
                        println!(
                            "VoiceManager: Speech detected! RMS: {:.4} (threshold: {:.4})",
                            rms, threshold
                        );
                        is_speaking = true;
                    }
                    silence_frames = 0;
                    speech_frames += 1;
                    speech_buffer.extend(chunk_16k);
                } else {
                    if is_speaking {
                        silence_frames += 1;
                        speech_buffer.extend(chunk_16k);

                        if silence_frames > silence_limit_frames {
                            // Speech ended
                            if speech_frames >= min_speech_frames {
                                println!(
                                    "VoiceManager: Processing speech ({} samples, {} frames)...",
                                    speech_buffer.len(),
                                    speech_frames
                                );

                                // Emit processing event
                                let _ = app_handle.emit("voice-processing", true);

                                // Send to STT
                                if let Ok(mut stt_guard) = stt.lock() {
                                    if let Some(engine) = stt_guard.as_mut() {
                                        match engine.transcribe(&speech_buffer) {
                                            Ok(text) => {
                                                if !text.trim().is_empty() {
                                                    println!(
                                                        "VoiceManager: Transcription: '{}'",
                                                        text
                                                    );
                                                    let _ = app_handle.emit("stt-result", text);
                                                } else {
                                                    println!("VoiceManager: Empty transcription");
                                                    let _ = app_handle.emit(
                                                        "voice-error",
                                                        "Could not understand audio",
                                                    );
                                                }
                                            }
                                            Err(e) => {
                                                eprintln!(
                                                    "VoiceManager: Transcription failed: {}",
                                                    e
                                                );
                                                let _ = app_handle.emit(
                                                    "voice-error",
                                                    format!("Transcription error: {}", e),
                                                );
                                            }
                                        }
                                    }
                                }
                            } else {
                                println!(
                                    "VoiceManager: Speech too short ({} frames), ignored",
                                    speech_frames
                                );
                            }

                            // Reset state
                            is_speaking = false;
                            speech_buffer.clear();
                            silence_frames = 0;
                            speech_frames = 0;
                        }
                    }
                }
            }

            println!("VoiceManager: Voice loop ended");
            let _ = app_handle.emit("voice-ready", false);
        });
    }

    pub fn stop(&self) {
        println!("VoiceManager: Stopping...");
        *self.is_running.lock().unwrap() = false;
        if let Ok(mut input) = self.audio_input.lock() {
            let _ = input.stop();
        }
    }
}

fn calculate_rms(data: &[f32]) -> f32 {
    if data.is_empty() {
        return 0.0;
    }
    let sum_sq: f32 = data.iter().map(|s| s * s).sum();
    (sum_sq / data.len() as f32).sqrt()
}

fn resample_linear(data: &[f32], from_hz: u32, to_hz: u32) -> Vec<f32> {
    if from_hz == to_hz {
        return data.to_vec();
    }
    let ratio = from_hz as f32 / to_hz as f32;
    let new_len = (data.len() as f32 / ratio).ceil() as usize;
    let mut out = Vec::with_capacity(new_len);

    for i in 0..new_len {
        let old_idx = i as f32 * ratio;
        let idx0 = old_idx.floor() as usize;
        let idx1 = (idx0 + 1).min(data.len().saturating_sub(1));
        let t = old_idx - idx0 as f32;

        let s0 = data.get(idx0).unwrap_or(&0.0);
        let s1 = data.get(idx1).unwrap_or(&0.0);

        out.push(s0 + (s1 - s0) * t);
    }
    out
}
