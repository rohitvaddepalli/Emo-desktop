# Emo Desktop Companion - TODO List

Based on `PRD.md`

## Phase 1: Foundation (Weeks 1-3)
**Goal:** Working desktop app with text chat and animated eyes.

- [x] **Project Setup**
    - [x] Initialize Tauri + React project structure (`npm create tauri-app`)
    - [x] Configure `tauri.conf.json` (window size, permissions, transparent background)
    - [x] Install Rust dependencies (`candle-core`, `candle-nn`, `tokio`, `serde`, etc.)
    - [x] Install Frontend dependencies (`framer-motion`, `zustand`, `lucide-react`, `tailwindcss`)

- [x] **UI Implementation**
    - [x] Create `EmoRobot.jsx` main widget component
    - [x] Implement draggable, always-on-top window functionality
    - [x] Design and implement `Eyes.jsx` using HTML5 Canvas or SVG
        - [x] Idle state (slow blinking)
        - [x] Happy state
        - [x] Thinking state
    - [x] Implement `Chat.jsx` interface (expandable/collapsible)

- [x] **Local AI Integration (Text-only)**
    - [x] Implement `download_models.py` script to fetch Qwen and Whisper models
    - [x] Run model download script to populate `models/` directory
    - [x] Create `src-tauri/src/ai/model_manager.rs` for loading GGUF models
    - [x] Implement Qwen 2.5-0.5B inference logic in Rust
    - [x] specific IPC commands to send text to backend and receive AI response

## Phase 2: Voice Integration (Weeks 4-5)
**Goal:** Fully voice-controlled assistant.

- [x] **Speech-to-Text (STT)**
    - [x] Integrate `whisper-rs` or `whisper.cpp` bindings in Rust (Using `candle-transformers`)
    - [x] Implement `src-tauri/src/voice/stt.rs` to handle audio input stream
    - [x] Connect microphone input to Whisper model

- [x] **Voice Activity Detection (VAD)**
    - [x] Implement VAD logic in `src-tauri/src/voice/vad.rs` (Implemented inline in `voice_manager.rs` for efficiency)
    - [x] Add noise suppression (RNNoise) if feasible (Basic RMS threshold implemented)

- [x] **Text-to-Speech (TTS)**
    - [x] Integrate Piper TTS in `src-tauri/src/voice/tts.rs` (Basic wrapper implemented)
    - [x] Configure Piper with cartoon voice profile (speed 1.1x via length_scale=0.9)
    - [x] Implement audio playback for TTS output

- [x] **Interaction Loop**
    - [x] Sync Eye animations with TTS audio output (Basic state syncing in EmoRobot.jsx)
    - [x] Implement Wake Word detection ("Hey Emo", currently via STT)
    - [x] Add continuous listening mode toggle (Implemented via toggle button and loop logic)

## Phase 3: Automation Tools (Weeks 6-7)
**Goal:** Working task automation capabilities.
- [x] **Local AI Integration**
    - [x] Connect LLM to file system (Dependency walkdir added, ToolManager list_directory implemented)
    - [x] Implement system command execution (sysinfo integrated for metrics)
    - [x] Create "Tool Use" system for LLM (Context injection implemented in lib.rs)

- [x] **Task Implementation**
    - [x] Implement file search/manipulation tools (`file_search`, `file_read`, `file_write`, `file_move`, `file_delete`)
    - [x] Add basic "agentic" loops for multi-step tasks
    - [x] Implement `folder_organize` with confirmation

- [x] **Application Control**
    - [x] Implement `app_launch`, `app_close`, `app_list`
    - [x] Implement `window_focus`

- [x] **System Utilities**
    - [x] Implement `system_info` (CPU/RAM stats via `sysinfo` crate)
    - [x] Implement `screenshot` capability
    - [x] Implement `clipboard_read` and `clipboard_write`

- [x] **Web & Productivity**
    - [x] Implement `web_open` and `web_search`
    - [x] Implement `timer_set` and `reminder_create`

## Phase 4: Agent Intelligence (Week 8)
**Goal:** Intelligent agentic behavior.

- [x] **Advanced AI Logic**
    - [x] Integrate Qwen 2.5-1.5B model for complex reasoning
    - [x] Implement `src-tauri/src/ai/router.rs` to select between 0.5B and 1.5B models based on intent
    - [x] Create system prompts for tool usage (`src-tauri/src/ai/prompts.rs`)

- [x] **Agent Loop Implementation**
    - [x] Implement REPL-like loop: User Input -> Intent -> Tool Call -> Tool Result -> Final Response
    - [x] Handle multi-step reasoning (chaining tools)

- [x] **Memory System**
    - [x] Set up SQLite using `rusqlite` (integrated in `ai/tools.rs`)
    - [x] Schema for conversation history and settings
    - [x] Implement context injection into AI prompts from recent history

## Phase 5: Polish & Optimization (Week 9-10)
**Goal:** Production-ready v1.0 release.

- [x] **Performance Tuning**
    - [x] Profile memory usage and CPU load
    - [x] Implement model quantizations (ensure Q4_K_M is used)
    - [x] Implement model unloading (lazy loading) for 1.5B model

- [ ] **UI/UX Refinement**
    - [x] complete `Settings.jsx` panel (Model selection, Voice settings, Appearance)
    - [x] Add visual feedback for "Listening", "Thinking", "Working" states
    - [x] Smooth transitions and error handling animations

- [x] **Packaging & Distribution**
    - [x] Create application icons and assets
    - [x] Configure build scripts for Windows (msi), macOS (dmg), Linux (deb)
    - [x] Write `README.md` and documentation

## Future Enhancements (v2.0+)
- [ ] Mobile App support
- [ ] Vision capabilities (screen understanding)
- [ ] Plugin system for community tools
