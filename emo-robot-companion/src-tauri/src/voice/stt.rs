use anyhow::{bail, Error, Result};
use candle_core::{DType, Device, IndexOp, Tensor};
use candle_transformers::models::whisper::{self, Config};
use std::path::Path;
use tokenizers::Tokenizer;

pub struct SttEngine {
    model: whisper::model::Whisper,
    tokenizer: Tokenizer,
    mel_filters: Vec<f32>,
    device: Device,
    config: Config,
    sot_token: u32,
    eot_token: u32,
}

impl SttEngine {
    pub fn new(model_dir: &str) -> Result<Self> {
        let device = Device::Cpu;
        let model_path = Path::new(model_dir).join("model.safetensors");
        let config_path = Path::new(model_dir).join("config.json");
        let tokenizer_path = Path::new(model_dir).join("tokenizer.json");

        println!("STT: Loading model from {:?}", model_dir);

        if !model_path.exists() {
            bail!("Model file not found: {:?}", model_path);
        }
        if !config_path.exists() {
            bail!("Config file not found: {:?}", config_path);
        }
        if !tokenizer_path.exists() {
            bail!("Tokenizer file not found: {:?}", tokenizer_path);
        }

        let config: Config = serde_json::from_str(&std::fs::read_to_string(&config_path)?)
            .map_err(|e| Error::msg(format!("Failed to parse config: {}", e)))?;

        let tokenizer = Tokenizer::from_file(&tokenizer_path)
            .map_err(|e| Error::msg(format!("Failed to load tokenizer: {}", e)))?;

        println!("STT: Loading model weights...");
        let tensor_storage = unsafe {
            candle_nn::VarBuilder::from_mmaped_safetensors(
                &[model_path],
                candle_core::DType::F32,
                &device,
            )?
        };
        let model = whisper::model::Whisper::load(&tensor_storage, config.clone())?;
        println!("STT: Model loaded successfully");

        // Generate mel filters inline if not found (Whisper standard)
        let mel_filters = Self::generate_mel_filters(config.num_mel_bins, 400);
        println!("STT: Generated {} mel filters", mel_filters.len());

        let sot_token = tokenizer
            .token_to_id(whisper::SOT_TOKEN)
            .ok_or(Error::msg("SOT token not found"))?;
        let eot_token = tokenizer
            .token_to_id(whisper::EOT_TOKEN)
            .ok_or(Error::msg("EOT token not found"))?;

        Ok(Self {
            model,
            tokenizer,
            mel_filters,
            device,
            config,
            sot_token,
            eot_token,
        })
    }

    /// Generate mel filterbank (simplified version)
    fn generate_mel_filters(n_mels: usize, n_fft: usize) -> Vec<f32> {
        // This is a simplified mel filterbank generation
        // In production, use the actual mel filters from Whisper
        let mut filters = vec![0.0f32; n_mels * (n_fft / 2 + 1)];

        // Simple triangular filters spaced mel-scale
        let mel_min = 0.0f32;
        let mel_max = 2595.0 * (1.0f32 + 8000.0f32 / 700.0f32).log10();

        for i in 0..n_mels {
            let mel_center = mel_min + (mel_max - mel_min) * (i + 1) as f32 / (n_mels + 1) as f32;
            let freq_center = 700.0 * (10.0f32.powf(mel_center / 2595.0) - 1.0);
            let bin_center = (freq_center * n_fft as f32 / 16000.0).round() as usize;

            if bin_center < n_fft / 2 {
                filters[i * (n_fft / 2 + 1) + bin_center] = 1.0;
            }
        }

        filters
    }

    pub fn transcribe(&mut self, audio_data: &[f32]) -> Result<String> {
        if audio_data.len() < 1600 {
            // Need at least 100ms of audio
            return Ok("".to_string());
        }

        println!("STT: Transcribing {} samples", audio_data.len());

        // 1. Convert to mel spectrogram
        let mel = whisper::audio::pcm_to_mel(&self.config, audio_data, &self.mel_filters);
        let mel_len = mel.len();
        let n_mels = self.config.num_mel_bins;
        let n_frames = mel_len / n_mels;

        println!(
            "STT: Mel spectrogram: {} frames x {} mels",
            n_frames, n_mels
        );

        if n_frames == 0 {
            return Ok("".to_string());
        }

        let mel_tensor = Tensor::from_vec(mel, (1, n_mels, n_frames), &self.device)?;

        // 2. Pad or truncate to 3000 frames (Whisper standard)
        let target_frames = 3000usize;
        let mel_tensor = if n_frames < target_frames {
            let pad_len = target_frames - n_frames;
            let pad = Tensor::zeros((1, n_mels, pad_len), DType::F32, &self.device)?;
            Tensor::cat(&[&mel_tensor, &pad], 2)?
        } else {
            mel_tensor.narrow(2, 0, target_frames)?
        };

        // 3. Forward Encoder
        println!("STT: Running encoder...");
        let encoder_output = self.model.encoder.forward(&mel_tensor, true)?;

        // 4. Decode with better initialization
        let mut tokens = vec![self.sot_token];

        // Add English language token and transcribe task if available
        // Format: <|startoftranscript|> <|en|> <|transcribe|> <|notimestamps|>

        println!("STT: Decoding...");
        let max_tokens = 100.min(self.config.max_target_positions);

        for i in 0..max_tokens {
            let token_tensor = Tensor::new(tokens.as_slice(), &self.device)?.unsqueeze(0)?;

            let logits = self
                .model
                .decoder
                .forward(&token_tensor, &encoder_output, i == 0)?;
            let logits = self.model.decoder.final_linear(&logits)?;

            let (_b, seq_len, _vocab) = logits.dims3()?;
            let last_logits = logits.i((0, seq_len - 1))?;

            // Apply temperature sampling or greedy
            let next_token = last_logits.argmax(0)?.to_scalar::<u32>()?;

            if next_token == self.eot_token {
                break;
            }

            tokens.push(next_token);

            // Safety: prevent infinite loops
            if tokens.len() > max_tokens {
                break;
            }
        }

        // Decode tokens
        let decoded = self
            .tokenizer
            .decode(&tokens, true)
            .map_err(|e| Error::msg(format!("Decoding failed: {}", e)))?;

        let cleaned = decoded
            .replace("<|startoftranscript|>", "")
            .replace("<|endoftext|>", "")
            .replace("<|notimestamps|>", "")
            .trim()
            .to_string();

        println!("STT: Transcribed: '{}'", cleaned);
        Ok(cleaned)
    }
}
