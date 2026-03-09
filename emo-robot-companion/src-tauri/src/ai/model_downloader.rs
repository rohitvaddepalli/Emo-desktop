use anyhow::{Error as E, Result};
use std::path::{Path, PathBuf};
use std::fs;
use tauri::{AppHandle, Emitter};

/// Information about a single model to download.
#[derive(Clone, serde::Serialize)]
pub struct ModelInfo {
    pub id: String,
    pub name: String,
    pub description: String,
    pub size_label: String,
    pub required: bool,
}

/// Progress event payload sent to the frontend.
#[derive(Clone, serde::Serialize)]
pub struct DownloadProgress {
    pub model_id: String,
    pub model_name: String,
    pub status: String,       // "downloading" | "complete" | "error" | "checking"
    pub file_name: String,
    pub files_done: usize,
    pub files_total: usize,
    pub message: String,
}

/// Returns the models directory next to the src-tauri folder.
pub fn get_models_dir() -> Result<PathBuf> {
    let base = std::env::current_dir()?;
    let models_dir = base.join("../models");
    Ok(models_dir)
}

/// Check which models are already downloaded.
pub fn check_model_status(models_dir: &Path) -> Vec<(String, bool)> {
    let checks = vec![
        (
            "qwen-0.5b".to_string(),
            models_dir
                .join("qwen2.5-0.5b/qwen2.5-0.5b-instruct-q4_k_m.gguf")
                .exists()
                && models_dir.join("qwen2.5-0.5b/tokenizer.json").exists(),
        ),
        (
            "whisper-tiny".to_string(),
            models_dir.join("whisper/model.safetensors").exists()
                && models_dir.join("whisper/tokenizer.json").exists(),
        ),
        (
            "piper-tts".to_string(),
            models_dir
                .join("piper/en/en_US/lessac/medium/en_US-lessac-medium.onnx")
                .exists(),
        ),
    ];
    checks
}

/// Get the list of all models available for download.
pub fn get_model_list() -> Vec<ModelInfo> {
    vec![
        ModelInfo {
            id: "qwen-0.5b".to_string(),
            name: "Qwen 2.5-0.5B".to_string(),
            description: "Core chat model • Fast inference on CPU".to_string(),
            size_label: "~400 MB".to_string(),
            required: true,
        },
        ModelInfo {
            id: "whisper-tiny".to_string(),
            name: "Whisper Tiny".to_string(),
            description: "Speech-to-text • Voice recognition".to_string(),
            size_label: "~75 MB".to_string(),
            required: true,
        },
        ModelInfo {
            id: "piper-tts".to_string(),
            name: "Piper TTS".to_string(),
            description: "Text-to-speech • Voice output".to_string(),
            size_label: "~65 MB".to_string(),
            required: true,
        },
    ]
}

/// Download all required models, emitting progress events to the frontend.
pub fn download_all_models(app_handle: &AppHandle, models_dir: &Path) -> Result<()> {
    let api = hf_hub::api::sync::Api::new()?;

    // ── 1. Qwen 2.5-0.5B (GGUF + tokenizer) ────────────────────────────────
    download_qwen(&api, app_handle, models_dir)?;

    // ── 2. Whisper Tiny ──────────────────────────────────────────────────────
    download_whisper(&api, app_handle, models_dir)?;

    // ── 3. Piper TTS ─────────────────────────────────────────────────────────
    download_piper(&api, app_handle, models_dir)?;

    Ok(())
}

fn emit_progress(app_handle: &AppHandle, progress: DownloadProgress) {
    let _ = app_handle.emit("model-download-progress", progress);
}

fn download_qwen(api: &hf_hub::api::sync::Api, app: &AppHandle, models_dir: &Path) -> Result<()> {
    let model_id = "qwen-0.5b";
    let model_name = "Qwen 2.5-0.5B";
    let target_dir = models_dir.join("qwen2.5-0.5b");
    fs::create_dir_all(&target_dir)?;

    let gguf_path = target_dir.join("qwen2.5-0.5b-instruct-q4_k_m.gguf");
    let tokenizer_path = target_dir.join("tokenizer.json");

    let files_total = 2;

    // Check if already exists
    if gguf_path.exists() && tokenizer_path.exists() {
        emit_progress(app, DownloadProgress {
            model_id: model_id.to_string(),
            model_name: model_name.to_string(),
            status: "complete".to_string(),
            file_name: String::new(),
            files_done: files_total,
            files_total,
            message: "Already downloaded".to_string(),
        });
        return Ok(());
    }

    // Download GGUF
    if !gguf_path.exists() {
        emit_progress(app, DownloadProgress {
            model_id: model_id.to_string(),
            model_name: model_name.to_string(),
            status: "downloading".to_string(),
            file_name: "qwen2.5-0.5b-instruct-q4_k_m.gguf".to_string(),
            files_done: 0,
            files_total,
            message: "Downloading GGUF weights (~400 MB)...".to_string(),
        });

        let repo = api.model("Qwen/Qwen2.5-0.5B-Instruct-GGUF".to_string());
        let downloaded = repo.get("qwen2.5-0.5b-instruct-q4_k_m.gguf")
            .map_err(|e| E::msg(format!("Failed to download Qwen GGUF: {}", e)))?;
        fs::copy(&downloaded, &gguf_path)?;
    }

    emit_progress(app, DownloadProgress {
        model_id: model_id.to_string(),
        model_name: model_name.to_string(),
        status: "downloading".to_string(),
        file_name: "tokenizer.json".to_string(),
        files_done: 1,
        files_total,
        message: "Downloading tokenizer...".to_string(),
    });

    // Download tokenizer
    if !tokenizer_path.exists() {
        let repo = api.model("Qwen/Qwen2.5-0.5B-Instruct".to_string());
        let downloaded = repo.get("tokenizer.json")
            .map_err(|e| E::msg(format!("Failed to download tokenizer: {}", e)))?;
        fs::copy(&downloaded, &tokenizer_path)?;
    }

    emit_progress(app, DownloadProgress {
        model_id: model_id.to_string(),
        model_name: model_name.to_string(),
        status: "complete".to_string(),
        file_name: String::new(),
        files_done: files_total,
        files_total,
        message: "Qwen model ready!".to_string(),
    });

    Ok(())
}

fn download_whisper(api: &hf_hub::api::sync::Api, app: &AppHandle, models_dir: &Path) -> Result<()> {
    let model_id = "whisper-tiny";
    let model_name = "Whisper Tiny";
    let target_dir = models_dir.join("whisper");
    fs::create_dir_all(&target_dir)?;

    let files = vec![
        ("config.json", "openai/whisper-tiny"),
        ("model.safetensors", "openai/whisper-tiny"),
        ("tokenizer.json", "openai/whisper-tiny"),
        ("preprocessor_config.json", "openai/whisper-tiny"),
        ("mel_filters.safetensors", "lmz/candle-whisper"),
    ];

    let files_total = files.len();

    // Quick check if all exist
    let all_exist = files.iter().all(|(name, _)| target_dir.join(name).exists());
    if all_exist {
        emit_progress(app, DownloadProgress {
            model_id: model_id.to_string(),
            model_name: model_name.to_string(),
            status: "complete".to_string(),
            file_name: String::new(),
            files_done: files_total,
            files_total,
            message: "Already downloaded".to_string(),
        });
        return Ok(());
    }

    for (i, (file_name, repo_id)) in files.iter().enumerate() {
        let dest = target_dir.join(file_name);
        if dest.exists() {
            continue;
        }

        emit_progress(app, DownloadProgress {
            model_id: model_id.to_string(),
            model_name: model_name.to_string(),
            status: "downloading".to_string(),
            file_name: file_name.to_string(),
            files_done: i,
            files_total,
            message: format!("Downloading {}...", file_name),
        });

        let repo = api.model(repo_id.to_string());
        let downloaded = repo.get(file_name)
            .map_err(|e| E::msg(format!("Failed to download {}: {}", file_name, e)))?;
        fs::copy(&downloaded, &dest)?;
    }

    emit_progress(app, DownloadProgress {
        model_id: model_id.to_string(),
        model_name: model_name.to_string(),
        status: "complete".to_string(),
        file_name: String::new(),
        files_done: files_total,
        files_total,
        message: "Whisper model ready!".to_string(),
    });

    Ok(())
}

fn download_piper(api: &hf_hub::api::sync::Api, app: &AppHandle, models_dir: &Path) -> Result<()> {
    let model_id = "piper-tts";
    let model_name = "Piper TTS";
    let target_dir = models_dir.join("piper/en/en_US/lessac/medium");
    fs::create_dir_all(&target_dir)?;

    let files = vec![
        "en/en_US/lessac/medium/en_US-lessac-medium.onnx",
        "en/en_US/lessac/medium/en_US-lessac-medium.onnx.json",
    ];

    let files_total = files.len();

    // Quick check
    let all_exist = files.iter().all(|f| {
        let name = Path::new(f).file_name().unwrap();
        target_dir.join(name).exists()
    });
    if all_exist {
        emit_progress(app, DownloadProgress {
            model_id: model_id.to_string(),
            model_name: model_name.to_string(),
            status: "complete".to_string(),
            file_name: String::new(),
            files_done: files_total,
            files_total,
            message: "Already downloaded".to_string(),
        });
        return Ok(());
    }

    let repo = api.model("rhasspy/piper-voices".to_string());

    for (i, file_path) in files.iter().enumerate() {
        let file_name = Path::new(file_path).file_name().unwrap().to_str().unwrap();
        let dest = target_dir.join(file_name);
        if dest.exists() {
            continue;
        }

        emit_progress(app, DownloadProgress {
            model_id: model_id.to_string(),
            model_name: model_name.to_string(),
            status: "downloading".to_string(),
            file_name: file_name.to_string(),
            files_done: i,
            files_total,
            message: format!("Downloading {}...", file_name),
        });

        let downloaded = repo.get(file_path)
            .map_err(|e| E::msg(format!("Failed to download {}: {}", file_name, e)))?;
        fs::copy(&downloaded, &dest)?;
    }

    emit_progress(app, DownloadProgress {
        model_id: model_id.to_string(),
        model_name: model_name.to_string(),
        status: "complete".to_string(),
        file_name: String::new(),
        files_done: files_total,
        files_total,
        message: "Piper TTS ready!".to_string(),
    });

    Ok(())
}
