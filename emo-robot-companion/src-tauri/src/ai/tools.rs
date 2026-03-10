use chrono::Local;
use clipboard::ClipboardProvider;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};
use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, Signal, System};
use walkdir::WalkDir;

pub struct ToolManager {
    system: Arc<Mutex<System>>,
    db: Arc<Mutex<Option<rusqlite::Connection>>>,
}

impl ToolManager {
    pub fn new() -> Self {
        Self {
            system: Arc::new(Mutex::new(System::new_all())),
            db: Arc::new(Mutex::new(None)),
        }
    }

    fn allowed_roots() -> Vec<PathBuf> {
        let mut roots = Vec::new();

        if let Ok(home) = std::env::var("HOME") {
            roots.push(PathBuf::from(home));
        }
        if let Ok(user_profile) = std::env::var("USERPROFILE") {
            roots.push(PathBuf::from(user_profile));
        }
        if let Ok(cwd) = std::env::current_dir() {
            roots.push(cwd);
        }

        roots
    }

    fn path_is_allowed(path: &Path) -> bool {
        let normalized = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        Self::allowed_roots()
            .into_iter()
            .any(|root| normalized.starts_with(root))
    }

    fn ensure_allowed_path(path: &Path) -> Result<(), String> {
        if Self::path_is_allowed(path) {
            Ok(())
        } else {
            Err(
                "Access denied: path is outside allowed directories (home or current workspace)."
                    .to_string(),
            )
        }
    }

    // Initialize SQLite database for memory
    pub fn init_memory(&self, db_path: &str) -> Result<String, String> {
        let conn = rusqlite::Connection::open(db_path)
            .map_err(|e| format!("Failed to open database: {}", e))?;

        // Create tables
        conn.execute(
            "CREATE TABLE IF NOT EXISTS conversations (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                timestamp TEXT NOT NULL,
                role TEXT NOT NULL,
                content TEXT NOT NULL
            )",
            [],
        )
        .map_err(|e| format!("Failed to create conversations table: {}", e))?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS reminders (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                created_at TEXT NOT NULL,
                remind_at TEXT NOT NULL,
                message TEXT NOT NULL,
                completed INTEGER DEFAULT 0
            )",
            [],
        )
        .map_err(|e| format!("Failed to create reminders table: {}", e))?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            )",
            [],
        )
        .map_err(|e| format!("Failed to create settings table: {}", e))?;

        let mut db = self.db.lock().map_err(|_| "Mutex poisoned")?;
        *db = Some(conn);

        Ok("Memory system initialized successfully".to_string())
    }

    // System status
    pub fn get_system_status(&self) -> String {
        let mut sys = self.system.lock().unwrap();
        sys.refresh_all();

        let cpu_usage =
            sys.cpus().iter().map(|cpu| cpu.cpu_usage()).sum::<f32>() / sys.cpus().len() as f32;
        let total_mem = sys.total_memory() / 1024 / 1024;
        let used_mem = sys.used_memory() / 1024 / 1024;

        format!(
            "CPU Usage: {:.1}%\nMemory: {} MB / {} MB used",
            cpu_usage, used_mem, total_mem
        )
    }

    // Current time
    pub fn get_current_time(&self) -> String {
        let now = Local::now();
        now.format("%Y-%m-%d %H:%M:%S").to_string()
    }

    // List directory contents
    pub fn list_directory(&self, path_str: &str) -> String {
        let path = Path::new(path_str);
        if !path.exists() {
            return format!("Error: Path '{}' does not exist.", path_str);
        }
        if let Err(e) = Self::ensure_allowed_path(path) {
            return e;
        }

        let mut entries = Vec::new();
        match fs::read_dir(path) {
            Ok(read_dir) => {
                for entry in read_dir {
                    if let Ok(entry) = entry {
                        let name = entry.file_name();
                        let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
                        let prefix = if is_dir { "[D] " } else { "[F] " };
                        entries.push(format!("{}{}", prefix, name.to_string_lossy()));
                    }
                }
                if entries.is_empty() {
                    "Empty directory".to_string()
                } else {
                    entries.join("\n")
                }
            }
            Err(e) => format!("Error reading directory: {}", e),
        }
    }

    // Search files by name
    pub fn file_search(&self, dir_path: &str, query: &str) -> String {
        let path = Path::new(dir_path);
        if !path.exists() {
            return format!("Error: Directory '{}' does not exist.", dir_path);
        }
        if let Err(e) = Self::ensure_allowed_path(path) {
            return e;
        }

        let mut results = Vec::new();
        let walker = WalkDir::new(path)
            .max_depth(3)
            .into_iter()
            .filter_map(|e| e.ok());

        for entry in walker {
            let file_name = entry.file_name().to_string_lossy();
            if file_name.to_lowercase().contains(&query.to_lowercase()) {
                results.push(entry.path().to_string_lossy().to_string());
            }
            if results.len() >= 20 {
                break;
            }
        }

        if results.is_empty() {
            format!("No files found matching '{}' in {}", query, dir_path)
        } else {
            format!("Found {} matches:\n{}", results.len(), results.join("\n"))
        }
    }

    // Read file contents
    pub fn file_read(&self, path_str: &str) -> String {
        let path = Path::new(path_str);
        if !path.exists() {
            return format!("Error: File '{}' does not exist.", path_str);
        }
        if let Err(e) = Self::ensure_allowed_path(path) {
            return e;
        }

        match fs::read_to_string(path) {
            Ok(content) => {
                if content.len() > 2000 {
                    format!(
                        "{}...\n[truncated, {} total characters]",
                        &content[..2000],
                        content.len()
                    )
                } else {
                    content
                }
            }
            Err(e) => format!("Error reading file: {}", e),
        }
    }

    // Write/create file
    pub fn file_write(&self, path_str: &str, content: &str) -> String {
        let path = Path::new(path_str);
        if let Some(parent) = path.parent() {
            if let Err(e) = Self::ensure_allowed_path(parent) {
                return e;
            }
        }

        if content.len() > 1_000_000 {
            return "Refusing to write files larger than 1MB via automation for safety."
                .to_string();
        }

        match fs::write(path_str, content) {
            Ok(_) => format!("Successfully wrote to '{}'", path_str),
            Err(e) => format!("Error writing file: {}", e),
        }
    }

    // Move/rename file
    pub fn file_move(&self, source: &str, dest: &str) -> String {
        let source_path = Path::new(source);
        let dest_path = Path::new(dest);
        if let Err(e) = Self::ensure_allowed_path(source_path) {
            return e;
        }
        if let Some(parent) = dest_path.parent() {
            if let Err(e) = Self::ensure_allowed_path(parent) {
                return e;
            }
        }

        match fs::rename(source, dest) {
            Ok(_) => format!("Successfully moved '{}' to '{}'", source, dest),
            Err(e) => format!("Error moving file: {}", e),
        }
    }

    // Delete file
    pub fn file_delete(&self, path_str: &str) -> String {
        let path = Path::new(path_str);
        if !path.exists() {
            return format!("Error: File '{}' does not exist.", path_str);
        }
        if let Err(e) = Self::ensure_allowed_path(path) {
            return e;
        }

        match fs::remove_file(path) {
            Ok(_) => format!("Successfully deleted '{}'", path_str),
            Err(e) => format!("Error deleting file: {}", e),
        }
    }

    // Launch application
    pub fn app_launch(&self, app_name: &str) -> String {
        let trimmed = app_name.trim();
        if trimmed.is_empty() {
            return "Refusing to launch an empty command.".to_string();
        }
        if trimmed.contains(';') || trimmed.contains('&') || trimmed.contains('|') {
            return "Refusing potentially unsafe app launch characters.".to_string();
        }

        let result = if cfg!(target_os = "windows") {
            Command::new("cmd")
                .args(["/C", "start", "", trimmed])
                .spawn()
        } else if cfg!(target_os = "macos") {
            Command::new("open").arg(trimmed).spawn()
        } else {
            Command::new(trimmed).spawn()
        };

        match result {
            Ok(_) => format!("Launched '{}'", app_name),
            Err(e) => format!("Error launching '{}': {}", app_name, e),
        }
    }

    // List running processes
    pub fn app_list(&self) -> String {
        let mut sys = self.system.lock().unwrap();
        sys.refresh_processes_specifics(
            ProcessesToUpdate::All,
            true,
            ProcessRefreshKind::everything(),
        );

        let mut apps: Vec<String> = sys
            .processes()
            .values()
            .filter(|p| p.cpu_usage() > 0.0)
            .map(|p| {
                format!(
                    "{} (PID: {}, CPU: {:.1}%)",
                    p.name().to_string_lossy(),
                    p.pid(),
                    p.cpu_usage()
                )
            })
            .collect();

        apps.sort();
        apps.truncate(30);

        if apps.is_empty() {
            "No active processes found".to_string()
        } else {
            format!("Running processes:\n{}", apps.join("\n"))
        }
    }

    // Close application by name
    pub fn app_close(&self, app_name: &str) -> String {
        let mut sys = self.system.lock().unwrap();
        sys.refresh_processes_specifics(
            ProcessesToUpdate::All,
            true,
            ProcessRefreshKind::everything(),
        );

        let mut killed = 0;
        for (_pid, process) in sys.processes() {
            if process
                .name()
                .to_string_lossy()
                .to_lowercase()
                .contains(&app_name.to_lowercase())
            {
                if process.kill_with(Signal::Term).is_some() {
                    killed += 1;
                }
            }
        }

        if killed > 0 {
            format!("Terminated {} process(es) matching '{}'", killed, app_name)
        } else {
            format!("No processes found matching '{}'", app_name)
        }
    }

    // Read clipboard
    pub fn clipboard_read(&self) -> String {
        let mut ctx: clipboard::ClipboardContext = match clipboard::ClipboardProvider::new() {
            Ok(ctx) => ctx,
            Err(e) => return format!("Error accessing clipboard: {}", e),
        };

        match ctx.get_contents() {
            Ok(content) => content,
            Err(e) => format!("Error reading clipboard: {}", e),
        }
    }

    // Write to clipboard
    pub fn clipboard_write(&self, text: &str) -> String {
        let mut ctx: clipboard::ClipboardContext = match clipboard::ClipboardProvider::new() {
            Ok(ctx) => ctx,
            Err(e) => return format!("Error accessing clipboard: {}", e),
        };

        match ctx.set_contents(text.to_string()) {
            Ok(_) => "Copied to clipboard".to_string(),
            Err(e) => format!("Error writing to clipboard: {}", e),
        }
    }

    // Take screenshot
    pub fn screenshot(&self, path: Option<&str>) -> String {
        use screenshots::Screen;

        let screens = match Screen::all() {
            Ok(screens) => screens,
            Err(e) => return format!("Error getting screens: {}", e),
        };

        if screens.is_empty() {
            return "No screens found".to_string();
        }

        match screens[0].capture() {
            Ok(image) => {
                let save_path = path.unwrap_or("screenshot.png");
                let save_path_ref = Path::new(save_path);
                if let Some(parent) = save_path_ref.parent() {
                    if let Err(e) = Self::ensure_allowed_path(parent) {
                        return e;
                    }
                }
                match image.save(save_path) {
                    Ok(_) => format!("Screenshot saved to '{}'", save_path),
                    Err(e) => format!("Error saving screenshot: {}", e),
                }
            }
            Err(e) => format!("Error capturing screenshot: {}", e),
        }
    }

    // Open URL in browser
    pub fn web_open(&self, url: &str) -> String {
        let lower = url.to_lowercase();
        if !(lower.starts_with("https://") || lower.starts_with("http://")) {
            return "Blocked URL: only http:// and https:// links are allowed.".to_string();
        }

        match open::that(url) {
            Ok(_) => format!("Opened '{}' in browser", url),
            Err(e) => format!("Error opening URL: {}", e),
        }
    }

    // Search web (opens search in browser)
    pub fn web_search(&self, query: &str) -> String {
        let encoded = urlencoding::encode(query);
        let search_url = format!("https://www.google.com/search?q={}", encoded);

        match open::that(&search_url) {
            Ok(_) => format!("Searching for '{}'", query),
            Err(e) => format!("Error opening search: {}", e),
        }
    }

    // Set timer
    pub fn timer_set(&self, seconds: u64, message: &str) -> String {
        if seconds == 0 || seconds > 86_400 {
            return "Timer must be between 1 and 86400 seconds (24h).".to_string();
        }

        let msg = message.to_string();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_secs(seconds));
            println!("TIMER: {}", msg);
        });

        format!("Timer set for {} seconds: '{}'", seconds, message)
    }

    // Create reminder
    pub fn reminder_create(&self, time_str: &str, message: &str) -> Result<String, String> {
        let db = self.db.lock().map_err(|_| "Mutex poisoned")?;

        if let Some(ref conn) = *db {
            conn.execute(
                "INSERT INTO reminders (created_at, remind_at, message) VALUES (?1, ?2, ?3)",
                [Local::now().to_rfc3339().as_str(), time_str, message],
            )
            .map_err(|e| format!("Failed to create reminder: {}", e))?;

            Ok(format!("Reminder set for {}: '{}'", time_str, message))
        } else {
            Err("Memory system not initialized".to_string())
        }
    }

    // Store conversation
    pub fn store_conversation(&self, role: &str, content: &str) -> Result<(), String> {
        let db = self.db.lock().map_err(|_| "Mutex poisoned")?;

        if let Some(ref conn) = *db {
            conn.execute(
                "INSERT INTO conversations (timestamp, role, content) VALUES (?1, ?2, ?3)",
                [Local::now().to_rfc3339().as_str(), role, content],
            )
            .map_err(|e| format!("Failed to store conversation: {}", e))?;
        }

        Ok(())
    }

    // Focus a window by title substring (Windows: powershell, Linux/macOS: fallback)
    pub fn window_focus(&self, title: &str) -> String {
        #[cfg(target_os = "windows")]
        {
            let script = format!(
                r#"$wnd = Get-Process | Where-Object {{$_.MainWindowTitle -like '*{}*'}} | Select-Object -First 1; if ($wnd) {{ Add-Type -AssemblyName Microsoft.VisualBasic; [Microsoft.VisualBasic.Interaction]::AppActivate($wnd.Id) }} else {{ Write-Output 'NOT_FOUND' }}"#,
                title
            );
            match Command::new("powershell")
                .args(["-NoProfile", "-Command", &script])
                .output()
            {
                Ok(out) => {
                    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
                    if stdout.contains("NOT_FOUND") {
                        format!("No window found matching '{}'", title)
                    } else {
                        format!("Focused window matching '{}'", title)
                    }
                }
                Err(e) => format!("Error focusing window: {}", e),
            }
        }
        #[cfg(not(target_os = "windows"))]
        {
            // Linux: use wmctrl if available
            match Command::new("wmctrl").args(["-a", title]).status() {
                Ok(s) if s.success() => format!("Focused window matching '{}'", title),
                _ => format!(
                    "Could not focus window '{}' (wmctrl may not be installed)",
                    title
                ),
            }
        }
    }

    // Organize a folder by sorting files into subfolders by type or date
    pub fn folder_organize(&self, path_str: &str, method: &str) -> String {
        let path = Path::new(path_str);
        if !path.exists() || !path.is_dir() {
            return format!("Error: '{}' is not a valid directory.", path_str);
        }

        let entries = match fs::read_dir(path) {
            Ok(e) => e,
            Err(e) => return format!("Error reading directory: {}", e),
        };

        let mut moved = 0usize;
        let mut errors = Vec::new();

        for entry in entries.flatten() {
            let src = entry.path();
            // Skip directories
            if src.is_dir() {
                continue;
            }

            let subfolder = match method {
                "by_type" => {
                    let ext = src
                        .extension()
                        .map(|e| e.to_string_lossy().to_lowercase())
                        .unwrap_or_default();
                    let category = match ext.as_str() {
                        "jpg" | "jpeg" | "png" | "gif" | "bmp" | "svg" | "webp" => "Images",
                        "mp4" | "mkv" | "avi" | "mov" | "wmv" => "Videos",
                        "mp3" | "wav" | "flac" | "ogg" | "aac" => "Audio",
                        "pdf" | "doc" | "docx" | "txt" | "md" | "odt" => "Documents",
                        "zip" | "rar" | "7z" | "tar" | "gz" => "Archives",
                        "rs" | "py" | "js" | "ts" | "html" | "css" | "go" | "c" | "cpp" => "Code",
                        "exe" | "msi" | "deb" | "dmg" => "Installers",
                        _ => "Other",
                    };
                    category.to_string()
                }
                "by_date" => {
                    // Group by year-month of modification time
                    match src.metadata().and_then(|m| m.modified()) {
                        Ok(mtime) => {
                            let datetime: chrono::DateTime<chrono::Local> = mtime.into();
                            datetime.format("%Y-%m").to_string()
                        }
                        Err(_) => "Unknown".to_string(),
                    }
                }
                _ => {
                    return format!(
                        "Unknown organize method '{}'. Use 'by_type' or 'by_date'.",
                        method
                    )
                }
            };

            let dest_dir = path.join(&subfolder);
            if let Err(e) = fs::create_dir_all(&dest_dir) {
                errors.push(format!("Cannot create subdir '{}': {}", subfolder, e));
                continue;
            }

            let file_name = match src.file_name() {
                Some(n) => n,
                None => continue,
            };
            let dest = dest_dir.join(file_name);

            // Avoid overwriting: skip if destination exists
            if dest.exists() {
                continue;
            }

            if let Err(e) = fs::rename(&src, &dest) {
                errors.push(format!(
                    "Failed to move '{}': {}",
                    file_name.to_string_lossy(),
                    e
                ));
            } else {
                moved += 1;
            }
        }

        let mut result = format!(
            "Organized {} file(s) by {} in '{}'.",
            moved, method, path_str
        );
        if !errors.is_empty() {
            result.push_str(&format!(
                " {} error(s): {}",
                errors.len(),
                errors.join("; ")
            ));
        }
        result
    }

    // Get recent conversation history
    pub fn get_conversation_history(&self, limit: usize) -> Result<String, String> {
        let db = self.db.lock().map_err(|_| "Mutex poisoned")?;

        if let Some(ref conn) = *db {
            let mut stmt = conn.prepare(
                "SELECT timestamp, role, content FROM conversations ORDER BY timestamp DESC LIMIT ?"
            ).map_err(|e| format!("Failed to prepare query: {}", e))?;

            let rows = stmt
                .query_map([limit], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                    ))
                })
                .map_err(|e| format!("Failed to query: {}", e))?;

            let mut history = Vec::new();
            for row in rows {
                if let Ok((ts, role, content)) = row {
                    history.push(format!("[{}] {}: {}", ts, role, content));
                }
            }

            history.reverse();
            Ok(history.join("\n"))
        } else {
            Ok("No conversation history available".to_string())
        }
    }
}
