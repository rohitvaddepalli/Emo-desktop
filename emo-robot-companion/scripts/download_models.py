#!/usr/bin/env python3
"""
Download and prepare all required models for Emo Robot Companion
"""

import os
from pathlib import Path
from huggingface_hub import snapshot_download, hf_hub_download

# Base project directory is parent of scripts/
BASE_DIR = Path(__file__).parent.parent
MODELS_DIR = BASE_DIR / "models"
MODELS_DIR.mkdir(exist_ok=True)

def download_qwen_models():
    """Download Qwen models in GGUF format"""
    print("📦 Downloading Qwen2.5-0.5B-Instruct (GGUF)...")
    snapshot_download(
        repo_id="Qwen/Qwen2.5-0.5B-Instruct-GGUF",
        local_dir=MODELS_DIR / "qwen2.5-0.5b",
        allow_patterns=["*q4_k_m.gguf"],
    )
    
    # Download tokenizer from main repo (GGUF repo often lacks it)
    print("📦 Downloading Qwen Tokenizer...")
    hf_hub_download(
        repo_id="Qwen/Qwen2.5-0.5B-Instruct",
        filename="tokenizer.json",
        local_dir=MODELS_DIR / "qwen2.5-0.5b",
    )
    
    # Optional: Download 1.5B (commented out for V1 Foundation Phase to save bandwidth, can enable later)
    # print("📦 Downloading Qwen2.5-1.5B-Instruct (GGUF)...")
    # snapshot_download(
    #     repo_id="Qwen/Qwen2.5-1.5B-Instruct-GGUF",
    #     local_dir=MODELS_DIR / "qwen2.5-1.5b",
    #     allow_patterns=["*q4_k_m.gguf"],
    # )

def download_whisper():
    """Download Whisper-tiny for STT"""
    print("🎤 Downloading Whisper-tiny (SafeTensors)...")
    snapshot_download(
        repo_id="openai/whisper-tiny",
        local_dir=MODELS_DIR / "whisper",
        allow_patterns=["config.json", "model.safetensors", "tokenizer.json", "preprocessor_config.json"],
    )
    # Download mel filters required for Candle
    hf_hub_download(
        repo_id="lmz/candle-whisper",
        filename="mel_filters.safetensors",
        local_dir=MODELS_DIR / "whisper",
    )

def download_piper():
    """Download Piper TTS model"""
    print("🔊 Downloading Piper TTS (cartoon voice)...")
    # Using a high quality voice suitable for robot - en_US-lessac-medium is standard recommendation
    # Alternative: en_US-amy-medium or others. Sticking to PRD suggestion.
    PIPER_DIR = MODELS_DIR / "piper"
    PIPER_DIR.mkdir(exist_ok=True)
    
    hf_hub_download(
        repo_id="rhasspy/piper-voices",
        filename="en/en_US/lessac/medium/en_US-lessac-medium.onnx",
        local_dir=PIPER_DIR,
    )
    hf_hub_download(
        repo_id="rhasspy/piper-voices",
        filename="en/en_US/lessac/medium/en_US-lessac-medium.onnx.json",
        local_dir=PIPER_DIR,
    )

if __name__ == "__main__":
    print("🤖 Emo Robot - Model Download Script")
    print("=" * 50)
    
    try:
        download_qwen_models()
        download_whisper()
        download_piper()
        
        print("\n✅ All models downloaded successfully!")
        print(f"📁 Models location: {MODELS_DIR.absolute()}")
        
    except Exception as e:
        print(f"❌ Error: {e}")
        exit(1)
