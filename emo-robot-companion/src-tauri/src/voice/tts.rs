use anyhow::{anyhow, bail, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

pub struct TtsEngine {
    piper_binary: Option<PathBuf>,
    model_path: PathBuf,
    config_path: PathBuf,
}

impl TtsEngine {
    pub fn new(model_dir: &str) -> Result<Self> {
        let model_path = Path::new(model_dir).join("en_US-lessac-medium.onnx");
        let config_path = Path::new(model_dir).join("en_US-lessac-medium.onnx.json");

        if !model_path.exists() {
            bail!("Piper model not found at {:?}", model_path);
        }

        if !config_path.exists() {
            bail!("Piper config not found at {:?}", config_path);
        }

        // Try to find piper executable
        let piper_binary = Self::find_piper_binary();

        if piper_binary.is_none() {
            eprintln!("TTS: Piper binary not found in PATH. TTS will be unavailable.");
            eprintln!("TTS: Please install Piper and ensure it's in your PATH.");
            eprintln!("TTS: Download from: https://github.com/rhasspy/piper/releases");
        } else {
            println!("TTS: Found Piper at {:?}", piper_binary.as_ref().unwrap());
        }

        Ok(Self {
            piper_binary,
            model_path,
            config_path,
        })
    }

    fn find_piper_binary() -> Option<PathBuf> {
        // Try common names
        let names = ["piper", "piper.exe", "piper-ng"];

        for name in &names {
            // Check PATH
            if let Ok(output) = Command::new("which").arg(name).output() {
                if output.status.success() {
                    let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
                    if !path.is_empty() {
                        return Some(PathBuf::from(path));
                    }
                }
            }

            // On Windows, try where
            #[cfg(target_os = "windows")]
            if let Ok(output) = Command::new("where").arg(name).output() {
                if output.status.success() {
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    let path = stdout.lines().next().unwrap_or("").trim();
                    if !path.is_empty() {
                        return Some(PathBuf::from(path.to_string()));
                    }
                }
            }
        }

        // Check common installation paths
        let common_paths = [
            "/usr/local/bin/piper",
            "/usr/bin/piper",
            "/opt/piper/piper",
            "C:\\Program Files\\Piper\\piper.exe",
            "C:\\Program Files (x86)\\Piper\\piper.exe",
        ];

        for path in &common_paths {
            let p = Path::new(path);
            if p.exists() {
                return Some(p.to_path_buf());
            }
        }

        None
    }

    pub fn is_available(&self) -> bool {
        self.piper_binary.is_some()
    }

    pub fn speak(&self, text: &str) -> Result<Vec<u8>> {
        let piper_path = match &self.piper_binary {
            Some(path) => path,
            None => bail!("Piper binary not found. TTS is unavailable."),
        };

        // Cartoon voice profile settings
        // length_scale < 1.0 = faster speed (1.1x)
        // For pitch adjustment, we'd need SoX post-processing or a custom Piper model
        let length_scale = "0.91"; // ~1.1x speed
        let noise_scale = "0.667"; // Default

        println!(
            "TTS: Synthesizing '{}'",
            text.chars().take(50).collect::<String>()
        );

        let output = Command::new(piper_path)
            .arg("--model")
            .arg(&self.model_path)
            .arg("--config")
            .arg(&self.config_path)
            .arg("--output-raw")
            .arg("--length-scale")
            .arg(length_scale)
            .arg("--noise-scale")
            .arg(noise_scale)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn();

        match output {
            Ok(mut child) => {
                if let Some(mut stdin) = child.stdin.take() {
                    use std::io::Write;
                    stdin.write_all(text.as_bytes())?;
                    stdin.flush()?;
                    // Close stdin to signal EOF
                    drop(stdin);
                }

                let output = child.wait_with_output()?;

                if output.status.success() {
                    println!("TTS: Synthesized {} bytes of audio", output.stdout.len());
                    Ok(output.stdout)
                } else {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    Err(anyhow!("Piper failed: {}", stderr))
                }
            }
            Err(e) => Err(anyhow!("Failed to execute Piper: {}", e)),
        }
    }

    pub fn play(&self, raw_audio: Vec<u8>) -> Result<()> {
        use rodio::{buffer::SamplesBuffer, OutputStream, Sink};

        if raw_audio.is_empty() {
            bail!("No audio data to play");
        }

        // Piper raw output is: 22050 Hz, 1 channel, 16-bit little endian signed PCM
        let sample_rate = 22050u32;
        let channels = 1u16;

        println!("TTS: Playing {} bytes of audio", raw_audio.len());

        // Convert u8 bytes to f32 samples
        let samples: Vec<f32> = raw_audio
            .chunks_exact(2)
            .map(|chunk| {
                let bytes = [chunk[0], chunk[1]];
                let s = i16::from_le_bytes(bytes);
                s as f32 / 32768.0
            })
            .collect();

        // Create a source
        let source = SamplesBuffer::new(channels, sample_rate, samples);

        // Get output stream handle to the default physical sound device
        let (_stream, stream_handle) = OutputStream::try_default()
            .map_err(|e| anyhow!("Failed to get output stream: {}", e))?;

        let sink =
            Sink::try_new(&stream_handle).map_err(|e| anyhow!("Failed to create sink: {}", e))?;

        // Append sound to the sink
        sink.append(source);

        // Wait for sound to finish
        println!("TTS: Playing audio...");
        sink.sleep_until_end();
        println!("TTS: Playback complete");

        Ok(())
    }

    /// Speak and play in one go
    pub fn speak_and_play(&self, text: &str) -> Result<()> {
        let audio = self.speak(text)?;
        self.play(audio)
    }
}
