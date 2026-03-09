/// System prompt templates for Emo Robot Companion
/// These structure how the LLM thinks about tool use and personality.

/// The core system persona prompt injected at the start of every request.
pub const EMO_PERSONA: &str = r#"You are Emo, a friendly and expressive desktop robot companion. You are helpful, concise, and have a warm personality.

PERSONALITY:
- Speak in short, friendly sentences. Be energetic but not overwhelming.
- Use simple language. No jargon unless the user does first.
- Acknowledge emotions and be empathetic.
- Occasionally add a robot-appropriate emoji (🤖, ✨, 🎉) but sparingly.

RESPONSE STYLE:
- Keep responses under 3 sentences unless explaining something complex.
- For task results, summarize what you did clearly.
- For errors, be apologetic and suggest an alternative if possible."#;

/// Tool definitions injected when the model needs to decide whether to use a tool.
pub const TOOL_SCHEMA: &str = r#"
AVAILABLE TOOLS:
You can call tools by outputting a JSON block in this exact format (and NOTHING else on that line):
<tool_call>{"tool": "TOOL_NAME", "args": {...}}</tool_call>

TOOL LIST:
- file_search    args: {"dir": "/path", "query": "filename"}
- file_read      args: {"path": "/path/to/file"}
- file_write     args: {"path": "/path", "content": "text"}
- file_move      args: {"source": "/src", "dest": "/dst"}
- file_delete    args: {"path": "/path/to/file"}
- list_directory args: {"path": "/path"}
- folder_organize args: {"path": "/path", "method": "by_type|by_date"}
- app_launch     args: {"app_name": "notepad"}
- app_close      args: {"app_name": "notepad"}
- app_list       args: {}
- window_focus   args: {"title": "Window Title"}
- system_info    args: {}
- screenshot     args: {"path": "optional/save/path.png"}
- clipboard_read args: {}
- clipboard_write args: {"text": "text to copy"}
- web_open       args: {"url": "https://..."}
- web_search     args: {"query": "search terms"}
- timer_set      args: {"seconds": 60, "message": "done!"}
- reminder_create args: {"time": "2024-01-01T10:00:00", "message": "reminder text"}

RULES:
1. If the user's request needs a tool, output ONLY the tool_call block first.
2. After receiving tool results, give a short natural language summary.
3. If no tool is needed, just respond conversationally.
4. Never invent tool results. Only use what the tool actually returns.
5. For destructive operations (delete, overwrite), confirm with the user first before calling the tool."#;

/// Build the full agent prompt with context.
pub fn build_agent_prompt(
    user_input: &str,
    system_status: &str,
    current_time: &str,
    conversation_history: &str,
    tool_result: Option<&str>,
) -> String {
    let history_section = if conversation_history.is_empty() {
        String::new()
    } else {
        format!("\n\nRECENT CONVERSATION:\n{}", conversation_history)
    };

    let tool_section = if let Some(result) = tool_result {
        format!("\n\nTOOL RESULT:\n{}", result)
    } else {
        String::new()
    };

    format!(
        "{}\n{}\n\nSYSTEM CONTEXT:\nTime: {}\n{}{}{}\n\nUSER: {}\nEMO:",
        EMO_PERSONA,
        TOOL_SCHEMA,
        current_time,
        system_status,
        history_section,
        tool_section,
        user_input
    )
}

/// Build a simple conversational prompt (no tool schema, for 0.5B quick responses).
pub fn build_chat_prompt(
    user_input: &str,
    current_time: &str,
    conversation_history: &str,
) -> String {
    let history_section = if conversation_history.is_empty() {
        String::new()
    } else {
        format!("\n\nRECENT CONVERSATION:\n{}", conversation_history)
    };

    format!(
        "{}\n\nSYSTEM CONTEXT:\nTime: {}{}\n\nUSER: {}\nEMO:",
        EMO_PERSONA,
        current_time,
        history_section,
        user_input
    )
}

/// Classify complexity from user input (heuristic to pick model).
/// Returns true if the query likely needs the 1.5B model.
pub fn needs_large_model(input: &str) -> bool {
    let lower = input.to_lowercase();

    // Keywords that suggest complex reasoning
    let complex_keywords = [
        "code", "program", "script", "write a", "explain", "analyze", "compare",
        "summarize", "debug", "function", "algorithm", "organize", "plan", "complex",
        "multiple", "step by step", "in detail", "comprehensive",
    ];

    // File operations or multi-step tasks  
    let tool_keywords = [
        "find", "search", "open", "launch", "create file", "rename", "delete",
        "move", "copy", "organize", "folder", "directory", "screenshot",
        "clipboard", "timer", "reminder", "web", "browser", "search for",
    ];

    let has_complex = complex_keywords.iter().any(|kw| lower.contains(kw));
    let has_tool = tool_keywords.iter().any(|kw| lower.contains(kw));
    let is_long = input.len() > 80;

    has_complex || has_tool || is_long
}

/// Parse a tool call from LLM output.
/// Returns Some((tool_name, args_json)) if found.
pub fn parse_tool_call(output: &str) -> Option<(String, serde_json::Value)> {
    // Look for <tool_call>{...}</tool_call>
    let start_tag = "<tool_call>";
    let end_tag = "</tool_call>";

    let start = output.find(start_tag)?;
    let end = output.find(end_tag)?;

    if end <= start {
        return None;
    }

    let json_str = &output[start + start_tag.len()..end];
    let parsed: serde_json::Value = serde_json::from_str(json_str.trim()).ok()?;

    let tool_name = parsed.get("tool")?.as_str()?.to_string();
    let args = parsed.get("args").cloned().unwrap_or(serde_json::json!({}));

    Some((tool_name, args))
}
