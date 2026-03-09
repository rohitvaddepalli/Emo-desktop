use anyhow::{Error as E, Result};
use candle_core::{Device, Tensor};
use candle_core::quantized::gguf_file;
use candle_transformers::models::quantized_qwen2 as model;
use candle_transformers::generation::LogitsProcessor;
use tokenizers::Tokenizer;
use std::time::{Instant, Duration};

/// Idle timeout before a large model should be unloaded (5 minutes).
pub const LARGE_MODEL_IDLE_TIMEOUT: Duration = Duration::from_secs(300);

/// Qwen2.5 model wrapper. Supports both 0.5B and 1.5B GGUF variants.
pub struct QwenModel {
    model: model::ModelWeights,
    tokenizer: Tokenizer,
    device: Device,
    /// Timestamp of the last generate() call for idle-unload tracking.
    pub last_used: Instant,
}

impl QwenModel {
    /// Load a GGUF model from disk with explicit HF repo for tokenizer fallback.
    pub fn new_with_repo(model_path: &str, hf_repo: &str) -> Result<Self> {
        let device = Device::Cpu;

        // Load GGUF weights
        let mut file = std::fs::File::open(model_path)?;
        let content = gguf_file::Content::read(&mut file)
            .map_err(|e| E::msg(e.to_string()))?;
        let model_weights = model::ModelWeights::from_gguf(content, &mut file, &device)?;

        // Load tokenizer — prefer local file next to GGUF, fall back to HF Hub
        let model_folder = std::path::Path::new(model_path)
            .parent()
            .ok_or_else(|| E::msg("Cannot determine model folder"))?;
        let tokenizer_path = model_folder.join("tokenizer.json");

        let tokenizer = if tokenizer_path.exists() {
            println!("Loading tokenizer from {:?}", tokenizer_path);
            Tokenizer::from_file(&tokenizer_path).map_err(E::msg)?
        } else {
            println!(
                "tokenizer.json not found locally — fetching from HF Hub ({})...",
                hf_repo
            );
            let api = hf_hub::api::sync::Api::new()?;
            let repo = api.model(hf_repo.to_string());
            let fetched = repo.get("tokenizer.json")?;
            // Cache alongside the GGUF for subsequent launches (best-effort)
            let _ = std::fs::copy(&fetched, &tokenizer_path);
            Tokenizer::from_file(fetched).map_err(E::msg)?
        };

        println!("Model loaded: {}", model_path);
        Ok(Self { model: model_weights, tokenizer, device, last_used: Instant::now() })
    }

    /// Load the Qwen 0.5B model (fast chat tier).
    pub fn new(model_path: &str) -> Result<Self> {
        Self::new_with_repo(model_path, "Qwen/Qwen2.5-0.5B-Instruct")
    }

    /// Load the Qwen 1.5B model (reasoning tier). 
    pub fn new_large(model_path: &str) -> Result<Self> {
        Self::new_with_repo(model_path, "Qwen/Qwen2.5-1.5B-Instruct")
    }

    /// True if the model has been idle longer than LARGE_MODEL_IDLE_TIMEOUT.
    pub fn is_idle(&self) -> bool {
        self.last_used.elapsed() > LARGE_MODEL_IDLE_TIMEOUT
    }

    /// Run inference. sample_len caps token output. Returns generated text only.
    pub fn generate(&mut self, prompt: &str, sample_len: usize) -> Result<String> {
        self.last_used = Instant::now();

        // Qwen2.5 ChatML template
        let formatted = format!(
            "<|im_start|>user\n{}<|im_end|>\n<|im_start|>assistant\n",
            prompt
        );

        let tokens = self
            .tokenizer
            .encode(formatted, true)
            .map_err(E::msg)?
            .get_ids()
            .to_vec();

        let mut pipeline = LogitsProcessor::new(
            299_792_458, // fixed seed for reproducibility
            Some(0.7),   // temperature — balanced creativity
            Some(0.9),   // top-p
        );

        let mut output_tokens: Vec<u32> = Vec::new();
        let mut current_tokens = tokens.clone();

        for index in 0..sample_len {
            let context_size = if index > 0 { 1 } else { current_tokens.len() };
            let start_pos = current_tokens.len().saturating_sub(context_size);
            let input = Tensor::new(&current_tokens[start_pos..], &self.device)?
                .unsqueeze(0)?;
            let logits = self.model.forward(&input, start_pos)?;
            let next_token = pipeline.sample(&logits.squeeze(0)?)?;

            output_tokens.push(next_token);
            current_tokens.push(next_token);

            // Stop on Qwen EOS tokens (im_end=151645, eos=151643, pad=151644)
            if matches!(next_token, 151643 | 151644 | 151645) {
                break;
            }
        }

        let output_text = self
            .tokenizer
            .decode(&output_tokens, true)
            .map_err(E::msg)?;

        // Strip any trailing im_end markers that leak through
        let clean = output_text
            .trim_end_matches("<|im_end|>")
            .trim()
            .to_string();

        Ok(clean)
    }
}
