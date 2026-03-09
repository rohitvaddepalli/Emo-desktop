use std::sync::{Arc, Mutex};
use tauri::State;
use anyhow::Result;

pub mod ai;
pub mod voice;

use ai::model_manager::QwenModel;
use ai::tools::ToolManager;
use ai::prompts::{build_agent_prompt, build_chat_prompt, parse_tool_call};
use ai::router::{route, requires_tool, ModelTier};
use voice::voice_manager::VoiceManager;

// ─── App State ────────────────────────────────────────────────────────────────

struct AppState {
    model_small: Mutex<Option<QwenModel>>,   // Qwen 0.5B — always-on
    model_large: Mutex<Option<QwenModel>>,   // Qwen 1.5B — lazy loaded
    voice_manager: Arc<VoiceManager>,
    tool_manager: Arc<ToolManager>,
}

// ─── Model Management ─────────────────────────────────────────────────────────

#[tauri::command]
async fn load_model(state: State<'_, AppState>) -> Result<String, String> {
    println!("Loading Qwen 0.5B model...");
    let base_path = std::env::current_dir().map_err(|e| e.to_string())?;
    let model_path = base_path.join("../models/qwen2.5-0.5b/qwen2.5-0.5b-instruct-q4_k_m.gguf");

    if !model_path.exists() {
        return Err(format!(
            "Model not found at {:?}. Please run scripts/download_models.py first.",
            model_path
        ));
    }

    let path_string = model_path.to_str().ok_or("Invalid path encoding")?.to_string();
    let model = tauri::async_runtime::spawn_blocking(move || {
        QwenModel::new(&path_string).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())??;

    let mut state_model = state.model_small.lock().map_err(|_| "Mutex poisoned")?;
    *state_model = Some(model);
    Ok("Qwen 0.5B loaded successfully!".to_string())
}

#[tauri::command]
async fn load_large_model(state: State<'_, AppState>) -> Result<String, String> {
    println!("Loading Qwen 1.5B model...");
    let base_path = std::env::current_dir().map_err(|e| e.to_string())?;
    let model_path = base_path.join("../models/qwen2.5-1.5b/qwen2.5-1.5b-instruct-q4_k_m.gguf");

    if !model_path.exists() {
        return Err(format!(
            "Qwen 1.5B model not found at {:?}. Please run scripts/download_models.py first.",
            model_path
        ));
    }

    let path_string = model_path.to_str().ok_or("Invalid path encoding")?.to_string();
    let model = tauri::async_runtime::spawn_blocking(move || {
        QwenModel::new_large(&path_string).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())??;

    let mut state_model = state.model_large.lock().map_err(|_| "Mutex poisoned")?;
    *state_model = Some(model);
    Ok("Qwen 1.5B loaded successfully!".to_string())
}

#[tauri::command]
async fn unload_large_model(state: State<'_, AppState>) -> Result<String, String> {
    let mut state_model = state.model_large.lock().map_err(|_| "Mutex poisoned")?;
    *state_model = None;
    Ok("Qwen 1.5B unloaded to free memory.".to_string())
}

/// Returns boolean: whether each model is currently loaded.
#[tauri::command]
fn get_model_status(state: State<'_, AppState>) -> serde_json::Value {
    let small_loaded = state.model_small.lock().map(|m| m.is_some()).unwrap_or(false);
    let large_loaded = state.model_large.lock().map(|m| m.is_some()).unwrap_or(false);
    serde_json::json!({
        "small_loaded": small_loaded,
        "large_loaded": large_loaded,
    })
}

/// Check if the large model has been idle long enough to warrant unloading.
#[tauri::command]
fn is_large_model_idle(state: State<'_, AppState>) -> bool {
    state.model_large
        .lock()
        .ok()
        .and_then(|guard| guard.as_ref().map(|m| m.is_idle()))
        .unwrap_or(false)
}

// ─── Legacy: simple text generation (Chat.jsx compatibility) ──────────────────

#[tauri::command]
async fn generate_text(prompt: String, state: State<'_, AppState>) -> Result<String, String> {
    let _ = state.tool_manager.store_conversation("user", &prompt);

    let sys_status = state.tool_manager.get_system_status();
    let current_time = state.tool_manager.get_current_time();
    let history = state
        .tool_manager
        .get_conversation_history(6)
        .unwrap_or_default();

    let formatted = build_chat_prompt(&prompt, &current_time, &history);

    let mut state_model = state.model_small.lock().map_err(|_| "Mutex poisoned")?;
    if let Some(ref mut model) = *state_model {
        match model.generate(&formatted, 200) {
            Ok(output) => {
                let _ = state.tool_manager.store_conversation("assistant", &output);
                Ok(output)
            }
            Err(e) => Err(e.to_string()),
        }
    } else {
        Err("Model not loaded. Call load_model first.".to_string())
    }
}

// ─── Agent Loop ───────────────────────────────────────────────────────────────

/// AgentStep is returned to the frontend so it can show the intermediate steps.
#[derive(serde::Serialize, Clone)]
pub struct AgentStep {
    pub step_type: String,  // "thinking" | "tool_call" | "tool_result" | "response"
    pub content: String,
}

/// The full agentic command: routes to correct model, optionally calls tools,
/// and returns a structured result the frontend can render step-by-step.
#[tauri::command]
async fn agent_run(
    prompt: String,
    state: State<'_, AppState>,
) -> Result<Vec<AgentStep>, String> {
    let mut steps: Vec<AgentStep> = Vec::new();

    // Store user message
    let _ = state.tool_manager.store_conversation("user", &prompt);

    // Context
    let sys_status = state.tool_manager.get_system_status();
    let current_time = state.tool_manager.get_current_time();
    let history = state
        .tool_manager
        .get_conversation_history(6)
        .unwrap_or_default();

    // Route to appropriate model tier
    let tier = route(&prompt);
    let needs_tool = requires_tool(&prompt);

    steps.push(AgentStep {
        step_type: "thinking".to_string(),
        content: format!(
            "Using {} model{}",
            if tier == ModelTier::Large { "1.5B" } else { "0.5B" },
            if needs_tool { " with tool access" } else { "" }
        ),
    });

    // Build prompt
    let formatted_prompt = if needs_tool || tier == ModelTier::Large {
        build_agent_prompt(&prompt, &sys_status, &current_time, &history, None)
    } else {
        build_chat_prompt(&prompt, &current_time, &history)
    };

    // Run inference on the appropriate model
    let llm_output: String = {
        // Try large model first if needed, then fall back to small
        let use_large = tier == ModelTier::Large;

        if use_large {
            let mut model_guard = state.model_large.lock().map_err(|_| "Mutex poisoned")?;
            if let Some(ref mut model) = *model_guard {
                model.generate(&formatted_prompt, 300).map_err(|e| e.to_string())?
            } else {
                // 1.5B not loaded — fall back to 0.5B gracefully
                drop(model_guard);
                let mut small_guard = state.model_small.lock().map_err(|_| "Mutex poisoned")?;
                if let Some(ref mut model) = *small_guard {
                    model.generate(&formatted_prompt, 300).map_err(|e| e.to_string())?
                } else {
                    return Err("No model loaded. Call load_model first.".to_string());
                }
            }
        } else {
            let mut small_guard = state.model_small.lock().map_err(|_| "Mutex poisoned")?;
            if let Some(ref mut model) = *small_guard {
                model.generate(&formatted_prompt, 250).map_err(|e| e.to_string())?
            } else {
                return Err("Model not loaded. Call load_model first.".to_string());
            }
        }
    };

    // Check if the LLM wants to call a tool
    if let Some((tool_name, args)) = parse_tool_call(&llm_output) {
        steps.push(AgentStep {
            step_type: "tool_call".to_string(),
            content: format!("📟 Calling `{}` with args: {}", tool_name, args),
        });

        // Dispatch tool
        let tool_result = dispatch_tool(&tool_name, &args, &state.tool_manager);

        steps.push(AgentStep {
            step_type: "tool_result".to_string(),
            content: tool_result.clone(),
        });

        // Second LLM pass: summarise the tool result in natural language
        let follow_up_prompt = build_agent_prompt(
            &prompt,
            &sys_status,
            &current_time,
            &history,
            Some(&tool_result),
        );

        let final_output = {
            let mut small_guard = state.model_small.lock().map_err(|_| "Mutex poisoned")?;
            if let Some(ref mut model) = *small_guard {
                model.generate(&follow_up_prompt, 150).map_err(|e| e.to_string())?
            } else {
                tool_result.clone() // just echo if model gone
            }
        };

        let _ = state.tool_manager.store_conversation("assistant", &final_output);
        steps.push(AgentStep {
            step_type: "response".to_string(),
            content: final_output,
        });
    } else {
        // Pure conversational response — no tool needed
        let clean_output = llm_output.trim().to_string();
        let _ = state.tool_manager.store_conversation("assistant", &clean_output);
        steps.push(AgentStep {
            step_type: "response".to_string(),
            content: clean_output,
        });
    }

    Ok(steps)
}

/// Dispatch a tool call from the agent loop.
fn dispatch_tool(tool_name: &str, args: &serde_json::Value, tm: &ToolManager) -> String {
    let get_str = |key: &str| args.get(key).and_then(|v| v.as_str()).unwrap_or("");
    let get_u64 = |key: &str| args.get(key).and_then(|v| v.as_u64()).unwrap_or(0);

    match tool_name {
        "file_search"     => tm.file_search(get_str("dir"), get_str("query")),
        "file_read"       => tm.file_read(get_str("path")),
        "file_write"      => tm.file_write(get_str("path"), get_str("content")),
        "file_move"       => tm.file_move(get_str("source"), get_str("dest")),
        "file_delete"     => tm.file_delete(get_str("path")),
        "list_directory"  => tm.list_directory(get_str("path")),
        "folder_organize" => tm.folder_organize(get_str("path"), get_str("method")),
        "app_launch"      => tm.app_launch(get_str("app_name")),
        "app_close"       => tm.app_close(get_str("app_name")),
        "app_list"        => tm.app_list(),
        "window_focus"    => tm.window_focus(get_str("title")),
        "system_info"     => tm.get_system_status(),
        "screenshot"      => {
            let path = args.get("path").and_then(|v| v.as_str());
            tm.screenshot(path)
        }
        "clipboard_read"  => tm.clipboard_read(),
        "clipboard_write" => tm.clipboard_write(get_str("text")),
        "web_open"        => tm.web_open(get_str("url")),
        "web_search"      => tm.web_search(get_str("query")),
        "timer_set"       => tm.timer_set(get_u64("seconds"), get_str("message")),
        "reminder_create" => {
            tm.reminder_create(get_str("time"), get_str("message"))
                .unwrap_or_else(|e| e)
        }
        _ => format!("Unknown tool: '{}'", tool_name),
    }
}

// ─── Voice Commands ───────────────────────────────────────────────────────────

use voice::tts::TtsEngine;

#[tauri::command]
async fn speak(text: String) -> Result<(), String> {
    let model_dir = "../models/piper/en/en_US/lessac/medium";
    let tts = TtsEngine::new(model_dir).map_err(|e| e.to_string())?;
    let audio = tts.speak(&text).map_err(|e| e.to_string())?;
    tts.play(audio).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn start_listening(app_handle: tauri::AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    state.voice_manager.start(app_handle);
    Ok(())
}

#[tauri::command]
fn stop_listening(state: State<'_, AppState>) -> Result<(), String> {
    state.voice_manager.stop();
    Ok(())
}

// ─── Direct Tool Commands (legacy/direct access from frontend) ────────────────

#[tauri::command]
fn file_search(dir: String, query: String, state: State<'_, AppState>) -> String {
    state.tool_manager.file_search(&dir, &query)
}

#[tauri::command]
fn file_read(path: String, state: State<'_, AppState>) -> String {
    state.tool_manager.file_read(&path)
}

#[tauri::command]
fn file_write(path: String, content: String, state: State<'_, AppState>) -> String {
    state.tool_manager.file_write(&path, &content)
}

#[tauri::command]
fn file_move(source: String, dest: String, state: State<'_, AppState>) -> String {
    state.tool_manager.file_move(&source, &dest)
}

#[tauri::command]
fn file_delete(path: String, state: State<'_, AppState>) -> String {
    state.tool_manager.file_delete(&path)
}

#[tauri::command]
fn list_directory(path: String, state: State<'_, AppState>) -> String {
    state.tool_manager.list_directory(&path)
}

#[tauri::command]
fn folder_organize(path: String, method: String, state: State<'_, AppState>) -> String {
    state.tool_manager.folder_organize(&path, &method)
}

#[tauri::command]
fn app_launch(app_name: String, state: State<'_, AppState>) -> String {
    state.tool_manager.app_launch(&app_name)
}

#[tauri::command]
fn app_list(state: State<'_, AppState>) -> String {
    state.tool_manager.app_list()
}

#[tauri::command]
fn app_close(app_name: String, state: State<'_, AppState>) -> String {
    state.tool_manager.app_close(&app_name)
}

#[tauri::command]
fn window_focus(title: String, state: State<'_, AppState>) -> String {
    state.tool_manager.window_focus(&title)
}

#[tauri::command]
fn clipboard_read(state: State<'_, AppState>) -> String {
    state.tool_manager.clipboard_read()
}

#[tauri::command]
fn clipboard_write(text: String, state: State<'_, AppState>) -> String {
    state.tool_manager.clipboard_write(&text)
}

#[tauri::command]
fn screenshot(path: Option<String>, state: State<'_, AppState>) -> String {
    state.tool_manager.screenshot(path.as_deref())
}

#[tauri::command]
fn web_open(url: String, state: State<'_, AppState>) -> String {
    state.tool_manager.web_open(&url)
}

#[tauri::command]
fn web_search(query: String, state: State<'_, AppState>) -> String {
    state.tool_manager.web_search(&query)
}

#[tauri::command]
fn timer_set(seconds: u64, message: String, state: State<'_, AppState>) -> String {
    state.tool_manager.timer_set(seconds, &message)
}

#[tauri::command]
fn reminder_create(time: String, message: String, state: State<'_, AppState>) -> Result<String, String> {
    state.tool_manager.reminder_create(&time, &message)
}

#[tauri::command]
fn init_memory(db_path: String, state: State<'_, AppState>) -> Result<String, String> {
    state.tool_manager.init_memory(&db_path)
}

#[tauri::command]
fn get_conversation_history(limit: usize, state: State<'_, AppState>) -> Result<String, String> {
    state.tool_manager.get_conversation_history(limit)
}

#[tauri::command]
fn get_system_status(state: State<'_, AppState>) -> String {
    state.tool_manager.get_system_status()
}

#[tauri::command]
fn get_current_time(state: State<'_, AppState>) -> String {
    state.tool_manager.get_current_time()
}

// ─── Tauri Entry Point ────────────────────────────────────────────────────────

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(AppState {
            model_small: Mutex::new(None),
            model_large: Mutex::new(None),
            voice_manager: Arc::new(VoiceManager::new()),
            tool_manager: Arc::new(ToolManager::new()),
        })
        .invoke_handler(tauri::generate_handler![
            // Model management
            load_model,
            load_large_model,
            unload_large_model,
            get_model_status,
            is_large_model_idle,
            // Core AI
            generate_text,
            agent_run,
            // Voice
            start_listening,
            stop_listening,
            speak,
            // File tools
            file_search,
            file_read,
            file_write,
            file_move,
            file_delete,
            list_directory,
            folder_organize,
            // App tools
            app_launch,
            app_list,
            app_close,
            window_focus,
            // System tools
            clipboard_read,
            clipboard_write,
            screenshot,
            // Web tools
            web_open,
            web_search,
            // Productivity
            timer_set,
            reminder_create,
            // Memory
            init_memory,
            get_conversation_history,
            get_system_status,
            get_current_time,
        ])
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
