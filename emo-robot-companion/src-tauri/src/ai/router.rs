/// Intent-based model router.
/// Decides whether to use Qwen 0.5B (fast, conversational)
/// or Qwen 1.5B (complex reasoning, tool use) for a given request.

use super::prompts::needs_large_model;

/// Which model tier to use for inference.
#[derive(Debug, Clone, PartialEq)]
pub enum ModelTier {
    /// Qwen 2.5-0.5B — fast, low RAM, for greetings and simple queries.
    Small,
    /// Qwen 2.5-1.5B — slower, more capable, for complex tasks and tool use.
    Large,
}

/// Classify a user's input and return which model tier should handle it.
pub fn route(input: &str) -> ModelTier {
    let lower = input.to_lowercase();

    // Greetings and simple chitchat → always Small
    let simple_patterns = [
        "hi", "hello", "hey", "how are you", "good morning", "good night",
        "thanks", "thank you", "bye", "goodbye", "what time", "what's the time",
        "what day", "tell me a joke", "joke", "weather",
    ];

    let is_simple = simple_patterns.iter().any(|p| {
        lower == *p || lower.starts_with(&format!("{} ", p)) || lower == format!("{}!", p)
    });

    // Very short greetings (< 20 chars) are always simple
    if is_simple || (input.len() < 20 && !lower.contains("file") && !lower.contains("open")) {
        return ModelTier::Small;
    }

    // Everything else: use heuristic from prompts module
    if needs_large_model(input) {
        ModelTier::Large
    } else {
        ModelTier::Small
    }
}

/// Determine whether the given input likely requires a tool call.
pub fn requires_tool(input: &str) -> bool {
    let lower = input.to_lowercase();
    let tool_verbs = [
        "open", "launch", "start", "close", "kill", "find", "search", "look for",
        "read", "write", "create file", "make file", "delete", "remove", "move",
        "rename", "copy", "paste", "screenshot", "capture screen", "clipboard",
        "timer", "remind", "reminder", "alarm", "go to", "browse", "website",
        "organize", "sort", "folder", "directory", "list files", "show files",
        "what processes", "running apps", "what's open", "system info",
        "cpu", "ram", "memory", "disk",
    ];

    tool_verbs.iter().any(|verb| lower.contains(verb))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_greeting() {
        assert_eq!(route("hi"), ModelTier::Small);
        assert_eq!(route("hello there"), ModelTier::Small);
        assert_eq!(route("how are you"), ModelTier::Small);
    }

    #[test]
    fn test_complex_task() {
        assert_eq!(route("write a python script to sort files"), ModelTier::Large);
        assert_eq!(route("find all pdf files in my downloads"), ModelTier::Large);
    }

    #[test]
    fn test_tool_detection() {
        assert!(requires_tool("open notepad"));
        assert!(requires_tool("take a screenshot"));
        assert!(requires_tool("find my report.pdf"));
        assert!(!requires_tool("how are you doing today"));
    }
}
