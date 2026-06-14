use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

const SOCKET_PATH: &str = "/tmp/termai-ai.sock";
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const CONNECT_RETRY_INTERVAL: Duration = Duration::from_millis(100);

/// A suggestion action from the AI engine.
#[derive(Clone, Debug)]
pub struct AiAction {
    pub label: String,
    pub command: String,
    pub risk: String,
}

/// A suggestion from the AI engine.
#[derive(Clone, Debug)]
pub struct AiSuggestion {
    pub title: String,
    pub description: String,
    pub actions: Vec<AiAction>,
    pub created: Instant,
}

/// Messages from the AI background thread.
pub enum AiMessage {
    Suggestion(AiSuggestion),
    NoSuggestion,
    Completion(String),
    NoCompletion,
}

/// IPC client that manages the Go AI engine process and communicates via Unix socket.
pub struct AiClient {
    _child: Option<Child>,
    stream: Option<UnixStream>,
    pub rx: mpsc::Receiver<AiMessage>,
    tx: mpsc::Sender<AiMessage>,
    analyzing: Arc<AtomicBool>,
}

impl AiClient {
    /// Spawn the Go AI engine and connect to it.
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel();

        // Try to find the AI engine binary
        let ai_binary = find_ai_binary();

        let (child, stream) = if let Some(bin) = ai_binary {
            // Spawn the Go AI engine in serve mode
            let child = Command::new(&bin)
                .args(["serve", "--socket", SOCKET_PATH])
                .stdout(Stdio::null())
                .stderr(Stdio::piped())
                .spawn();

            match child {
                Ok(child) => {
                    // Wait for the socket to become available
                    let stream = wait_for_socket(SOCKET_PATH, CONNECT_TIMEOUT);
                    (Some(child), stream)
                }
                Err(e) => {
                    log::warn!("Failed to spawn AI engine: {e}");
                    (None, None)
                }
            }
        } else {
            log::info!("AI engine binary not found, trying to connect to existing socket");
            let stream = UnixStream::connect(SOCKET_PATH).ok();
            (None, stream)
        };

        if let Some(ref s) = stream {
            let _ = s.set_read_timeout(Some(Duration::from_secs(5)));
            let _ = s.set_write_timeout(Some(Duration::from_secs(2)));
        }

        Self {
            _child: child,
            stream,
            rx,
            tx,
            analyzing: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Send an analysis request asynchronously (non-blocking).
    pub fn analyze(&self, command: &str, output: &str, exit_code: i32) {
        let stream = match self.stream {
            Some(ref s) => match s.try_clone() {
                Ok(s) => s,
                Err(_) => return,
            },
            None => return,
        };

        let request = format!(
            "{{\"type\":\"analyze\",\"command\":{},\"output\":{},\"exit_code\":{}}}",
            json_escape(command),
            json_escape(output),
            exit_code
        );

        let tx = self.tx.clone();
        let analyzing = self.analyzing.clone();
        analyzing.store(true, Ordering::Relaxed);
        thread::spawn(move || {
            let result = send_request(stream, &request);
            analyzing.store(false, Ordering::Relaxed);
            if let Some(msg) = result {
                let _ = tx.send(msg);
            }
        });
    }

    /// Send an autocomplete request asynchronously (non-blocking).
    pub fn autocomplete(&self, partial_cmd: &str, cwd: &str, history: &str) {
        let stream = match self.stream {
            Some(ref s) => match s.try_clone() {
                Ok(s) => s,
                Err(_) => return,
            },
            None => return,
        };

        let request = format!(
            "{{\"type\":\"autocomplete\",\"partial_cmd\":{},\"cwd\":{},\"history\":{}}}",
            json_escape(partial_cmd),
            json_escape(cwd),
            json_escape(history)
        );

        let tx = self.tx.clone();
        thread::spawn(move || {
            if let Some(msg) = send_request(stream, &request) {
                let _ = tx.send(msg);
            }
        });
    }

    /// Poll for a received suggestion (non-blocking).
    pub fn poll(&self) -> Option<AiMessage> {
        self.rx.try_recv().ok()
    }

    /// Whether the IPC socket to the Go AI engine is open.
    pub fn is_connected(&self) -> bool {
        self.stream.is_some()
    }

    /// Whether an analysis request is currently in flight.
    pub fn is_analyzing(&self) -> bool {
        self.analyzing.load(Ordering::Relaxed)
    }
}

impl Drop for AiClient {
    fn drop(&mut self) {
        if let Some(ref mut child) = self._child {
            let _ = child.kill();
        }
        // Clean up socket
        let _ = std::fs::remove_file(SOCKET_PATH);
    }
}

fn find_ai_binary() -> Option<String> {
    // Check next to the current executable
    if let Ok(exe) = std::env::current_exe() {
        let dir = exe.parent()?;
        let ai_path = dir.join("termai-ai");
        if ai_path.exists() {
            return Some(ai_path.to_string_lossy().to_string());
        }
    }

    // Check in ai/ directory (development)
    let dev_paths = [
        "ai/termai-ai",
        "../ai/termai-ai",
    ];
    for p in &dev_paths {
        if std::path::Path::new(p).exists() {
            return Some(p.to_string());
        }
    }

    // Check PATH
    if Command::new("which")
        .arg("termai-ai")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
    {
        return Some("termai-ai".to_string());
    }

    None
}

fn wait_for_socket(path: &str, timeout: Duration) -> Option<UnixStream> {
    let start = Instant::now();
    loop {
        if let Ok(stream) = UnixStream::connect(path) {
            return Some(stream);
        }
        if start.elapsed() > timeout {
            log::warn!("Timeout waiting for AI engine socket");
            return None;
        }
        thread::sleep(CONNECT_RETRY_INTERVAL);
    }
}

fn send_request(mut stream: UnixStream, request: &str) -> Option<AiMessage> {
    // Send request with newline delimiter
    if stream.write_all(request.as_bytes()).is_err() {
        return None;
    }
    if stream.write_all(b"\n").is_err() {
        return None;
    }
    let _ = stream.flush();

    // Read response line
    let mut reader = BufReader::new(&stream);
    let mut line = String::new();
    if reader.read_line(&mut line).is_err() {
        return None;
    }

    parse_response(&line)
}

fn parse_response(json: &str) -> Option<AiMessage> {
    let json = json.trim();
    if json.is_empty() {
        return None;
    }

    let resp_type = extract_json_string(json, "type")?;

    match resp_type.as_str() {
        "no_suggestion" => Some(AiMessage::NoSuggestion),
        "no_completion" => Some(AiMessage::NoCompletion),
        "completion" => {
            let completion = extract_json_string(json, "completion").unwrap_or_default();
            if completion.is_empty() {
                Some(AiMessage::NoCompletion)
            } else {
                Some(AiMessage::Completion(completion))
            }
        }
        "suggestion" => {
            let title = extract_json_string(json, "title").unwrap_or_default();
            let description = extract_json_string(json, "description").unwrap_or_default();
            let actions = extract_actions(json);
            Some(AiMessage::Suggestion(AiSuggestion {
                title,
                description,
                actions,
                created: Instant::now(),
            }))
        }
        _ => None,
    }
}

fn extract_json_string(json: &str, key: &str) -> Option<String> {
    let pattern = format!("\"{}\"", key);
    let idx = json.find(&pattern)?;
    let rest = &json[idx + pattern.len()..];
    // Skip whitespace and colon
    let rest = rest.trim_start();
    let rest = rest.strip_prefix(':')?;
    let rest = rest.trim_start();
    // Read string value
    let rest = rest.strip_prefix('"')?;
    let mut result = String::new();
    let mut chars = rest.chars();
    loop {
        match chars.next()? {
            '"' => return Some(result),
            '\\' => {
                match chars.next()? {
                    '"' => result.push('"'),
                    '\\' => result.push('\\'),
                    'n' => result.push('\n'),
                    't' => result.push('\t'),
                    'r' => result.push('\r'),
                    '/' => result.push('/'),
                    c => {
                        result.push('\\');
                        result.push(c);
                    }
                }
            }
            c => result.push(c),
        }
    }
}

fn extract_actions(json: &str) -> Vec<AiAction> {
    let mut actions = Vec::new();

    // Find "actions" array
    let pattern = "\"actions\"";
    let idx = match json.find(pattern) {
        Some(i) => i,
        None => return actions,
    };

    let rest = &json[idx + pattern.len()..];
    let rest = rest.trim_start();
    let rest = match rest.strip_prefix(':') {
        Some(r) => r.trim_start(),
        None => return actions,
    };

    // Find the array bounds
    let arr_start = match rest.find('[') {
        Some(i) => i,
        None => return actions,
    };
    let rest = &rest[arr_start + 1..];

    // Split by objects — find each { ... }
    let mut depth = 0;
    let mut obj_start = None;
    for (i, ch) in rest.chars().enumerate() {
        match ch {
            '{' => {
                if depth == 0 {
                    obj_start = Some(i);
                }
                depth += 1;
            }
            '}' => {
                depth -= 1;
                if depth == 0 {
                    if let Some(start) = obj_start {
                        let obj = &rest[start..=i];
                        let label = extract_json_string(obj, "label").unwrap_or_default();
                        let command = extract_json_string(obj, "command").unwrap_or_default();
                        let risk = extract_json_string(obj, "risk").unwrap_or_default();
                        actions.push(AiAction { label, command, risk });
                    }
                }
            }
            ']' if depth == 0 => break,
            _ => {}
        }
    }

    actions
}

fn json_escape(s: &str) -> String {
    let mut result = String::with_capacity(s.len() + 2);
    result.push('"');
    for c in s.chars() {
        match c {
            '"' => result.push_str("\\\""),
            '\\' => result.push_str("\\\\"),
            '\n' => result.push_str("\\n"),
            '\r' => result.push_str("\\r"),
            '\t' => result.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                result.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => result.push(c),
        }
    }
    result.push('"');
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_json_escape() {
        assert_eq!(json_escape("hello"), "\"hello\"");
        assert_eq!(json_escape("he\"llo"), "\"he\\\"llo\"");
        assert_eq!(json_escape("line\nbreak"), "\"line\\nbreak\"");
    }

    #[test]
    fn test_extract_json_string() {
        let json = r#"{"type":"suggestion","title":"NVM não carregado"}"#;
        assert_eq!(extract_json_string(json, "type"), Some("suggestion".to_string()));
        assert_eq!(extract_json_string(json, "title"), Some("NVM não carregado".to_string()));
    }

    #[test]
    fn test_parse_response_no_suggestion() {
        let json = r#"{"type":"no_suggestion"}"#;
        let msg = parse_response(json);
        assert!(matches!(msg, Some(AiMessage::NoSuggestion)));
    }

    #[test]
    fn test_parse_response_suggestion() {
        let json = r#"{"type":"suggestion","title":"Test","description":"desc","actions":[{"label":"Do thing","command":"echo hi","risk":"low"}]}"#;
        let msg = parse_response(json);
        match msg {
            Some(AiMessage::Suggestion(s)) => {
                assert_eq!(s.title, "Test");
                assert_eq!(s.description, "desc");
                assert_eq!(s.actions.len(), 1);
                assert_eq!(s.actions[0].label, "Do thing");
                assert_eq!(s.actions[0].command, "echo hi");
                assert_eq!(s.actions[0].risk, "low");
            }
            _ => panic!("Expected Suggestion"),
        }
    }
}
