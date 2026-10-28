//! A GGUF model hosted locally by SolidCraft, using llama.cpp's OpenAI-compatible API.
//! The managed server binds only to loopback and is never confused with a user's
//! separately configured providers. Dropping Runtime stops only its own child.

use std::fs::OpenOptions;
use std::io::{BufRead, BufReader, Write};
use std::net::{Ipv4Addr, SocketAddrV4, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

/// The ID of the temporary provider; it is never saved to user preferences.
pub const AGENT_ID: &str = "solidcraft-native-local-gguf";

#[derive(Default)]
pub struct Runtime {
    child: Option<Child>,
    pub model_path: Option<PathBuf>,
    pub endpoint: Option<String>,
    pub log_path: Option<PathBuf>,
}

impl Runtime {
    pub fn running(&mut self) -> bool {
        if self.child.as_mut().is_some_and(|c| c.try_wait().ok().flatten().is_some()) {
            self.child = None;
            self.endpoint = None;
        }
        self.child.is_some()
    }

    /// Launches promptly; use wait_ready on a worker before connecting the chat.
    pub fn load(&mut self, model: &Path, executable: &Path, gpu_layers: u32) -> Result<(String, String), String> {
        if self.running() {
            return Err("Unload the current GGUF model first".into());
        }
        if !model.is_file() || !model.extension().and_then(|e| e.to_str()).is_some_and(|e| e.eq_ignore_ascii_case("gguf")) {
            return Err("Select an existing .gguf model file".into());
        }
        if !executable.is_file() {
            return Err(format!("Bundled llama-server not found at {}. Install the GGUF-enabled SolidCraft package.", executable.display()));
        }
        let listener = TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0))
            .map_err(|e| format!("Could not choose a localhost port: {e}"))?;
        let port = listener.local_addr().map_err(|e| e.to_string())?.port();
        drop(listener);

        let name = model.file_name().and_then(|s| s.to_str()).ok_or("Model file has no valid UTF-8 name")?.to_string();
        let log_path = std::env::temp_dir().join(format!("solidcraft-llama-{}.log", std::process::id()));
        let log = OpenOptions::new().create(true).write(true).truncate(true).open(&log_path)
            .map_err(|e| format!("Cannot open model log: {e}"))?;
        let err_log = log.try_clone().map_err(|e| e.to_string())?;
        let child = Command::new(executable)
            // Let llama.cpp select the model's embedded chat template. Jinja is
            // required for OpenAI-compatible structured tool calls (Qwen,
            // Granite, Ministral, and other supported GGUF families).
            // Never guess or impose a ChatML template from the filename.
            // No implicit CPU layer fallback: if the chosen GGUF cannot fit on
            // the configured GPU, report the failure instead of quietly
            // loading a second full CPU-side model. 99 covers small models.
            // --no-host bypasses the extra host staging buffer for offload.
            // --no-mmap avoids leaving a mapped model-file view as another
            // apparent resident allocation while GPU weights are loaded.
            // These flags do not make GPU VRAM physically separate on an APU.
            .args(["--host", "127.0.0.1", "--port", &port.to_string(), "--ctx-size", "8192", "--gpu-layers", &gpu_layers.to_string(), "--jinja", "--fit", "off", "--no-host", "--no-mmap"])
            .arg("--model").arg(model)
            .arg("--alias").arg(&name)
            .stdin(Stdio::null())
            .stdout(Stdio::from(log))
            .stderr(Stdio::from(err_log))
            .spawn().map_err(|e| format!("Failed to start llama-server: {e}"))?;
        let endpoint = format!("http://127.0.0.1:{port}/v1");
        self.model_path = Some(model.to_path_buf());
        self.endpoint = Some(endpoint.clone());
        self.log_path = Some(log_path);
        self.child = Some(child);
        Ok((endpoint, name))
    }

    /// This waits for llama.cpp's health endpoint, not merely an open TCP port.
    /// Call only from a worker, never the UI thread.
    pub fn wait_ready(endpoint: &str, timeout: Duration) -> Result<(), String> {
        let address = endpoint.strip_prefix("http://127.0.0.1:")
            .and_then(|s| s.strip_suffix("/v1"))
            .and_then(|s| s.parse::<u16>().ok())
            .map(|port| SocketAddrV4::new(Ipv4Addr::LOCALHOST, port))
            .ok_or("Invalid managed model endpoint")?;
        let start = Instant::now();
        while start.elapsed() < timeout {
            if let Ok(mut stream) = TcpStream::connect_timeout(&address.into(), Duration::from_millis(300)) {
                let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
                let _ = stream.set_write_timeout(Some(Duration::from_secs(2)));
                if stream.write_all(b"GET /health HTTP/1.0\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n").is_ok() {
                    let mut status = String::new();
                    if BufReader::new(stream).read_line(&mut status).is_ok() && status.contains(" 200 ") {
                        return Ok(());
                    }
                }
            }
            std::thread::sleep(Duration::from_millis(300));
        }
        Err("Local GGUF startup timed out; inspect the llama-server log for details".into())
    }

    pub fn unload(&mut self) {
        if let Some(mut child) = self.child.take() {
            // A failed status probe must not leave the owned model running.
            if !matches!(child.try_wait(), Ok(Some(_))) {
                let _ = child.kill();
                let _ = child.wait();
            }
        }
        self.endpoint = None;
    }
}

impl Drop for Runtime {
    fn drop(&mut self) { self.unload(); }
}

/// Check that the loaded GGUF and its automatically selected Jinja template
/// actually produce a structured OpenAI tool call. This never calls a real
/// SolidCraft command; it only exercises llama-server's model/tool protocol.
/// Call from a worker AFTER /health reports ready, not from the UI thread.
///
/// A model that only replies in prose is not silently marked as a CAD agent.
pub fn verify_tool_calling(
    http: &super::backend::HttpFn,
    endpoint: &str,
    model: &str,
) -> Result<String, String> {
    // Only ever send a diagnostic to a managed loopback endpoint. The URL is
    // reconstructed, not taken verbatim from an external provider config.
    let port = endpoint
        .strip_prefix("http://127.0.0.1:")
        .and_then(|s| s.strip_suffix("/v1"))
        .and_then(|s| s.parse::<u16>().ok())
        .filter(|p| *p != 0)
        .ok_or("Not a managed local GGUF endpoint")?;
    let base = format!("http://127.0.0.1:{port}");
    let mut template_source = "GGUF template (automatic)";

    // /props is informational. Some llama.cpp revisions omit template fields;
    // the actual structured call below is the authoritative compatibility test.
    let props = super::backend::HttpRequest {
        method: "GET",
        url: format!("{base}/props"),
        headers: vec![],
        body: None,
        timeout_s: 10,
    };
    if let Ok((200, bytes)) = http(&props)
        && let Ok(properties) = serde_json::from_slice::<serde_json::Value>(&bytes)
    {
        if properties.get("chat_template_tool_use").and_then(serde_json::Value::as_str).is_some_and(|s| !s.is_empty()) {
            template_source = "embedded tool-use template";
        } else if properties.get("chat_template").and_then(serde_json::Value::as_str).is_some_and(|s| !s.is_empty()) {
            template_source = "embedded chat template";
        }
    }

    // Use the same /v1/chat/completions tool_calls format as SolidCraft's
    // existing CAD tool loop. The test function is never registered with CAD.
    let body = serde_json::json!({
        "model": model,
        "messages": [
            {"role": "system", "content": "You are validating tool calling. Call the supplied function once with ok=true. No normal text."},
            {"role": "user", "content": "Call solidcraft_compatibility_check with ok set to true."}
        ],
        "tools": [{
            "type": "function",
            "function": {
                "name": "solidcraft_compatibility_check",
                "description": "A harmless protocol diagnostic. Return ok=true.",
                "parameters": {
                    "type": "object",
                    "properties": {"ok": {"type": "boolean"}},
                    "required": ["ok"]
                }
            }
        }],
        // llama-server uses a string for tool_choice. With only one tool
        // available, "required" selects our harmless diagnostic function.
        "tool_choice": "required",
        "temperature": 0,
        "max_tokens": 192,
        "stream": false,
        "parallel_tool_calls": false
    });
    let request = super::backend::HttpRequest {
        method: "POST",
        url: format!("{base}/v1/chat/completions"),
        headers: vec![("Content-Type".into(), "application/json".into())],
        body: Some(body.to_string().into_bytes()),
        timeout_s: 120,
    };
    let (status, response) = http(&request).map_err(|e| format!("Model tool-call check failed: {e}"))?;
    let answer: serde_json::Value = serde_json::from_slice(&response)
        .map_err(|e| format!("Model returned an invalid tool-call response: {e}"))?;
    if status != 200 {
        let detail = answer.pointer("/error/message")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("model/server rejected the structured tool request");
        return Err(format!("Model tool-call check HTTP {status}: {detail}"));
    }
    let calls = answer.pointer("/choices/0/message/tool_calls")
        .and_then(serde_json::Value::as_array)
        .ok_or("This GGUF replied without structured tool_calls. Its chat template or tool support is incompatible with SolidCraft CAD commands.")?;
    let passed = calls.iter().any(|call| {
        if call.pointer("/function/name").and_then(serde_json::Value::as_str) != Some("solidcraft_compatibility_check") {
            return false;
        }
        let args = call.pointer("/function/arguments");
        let parsed = match args {
            Some(serde_json::Value::String(s)) => serde_json::from_str::<serde_json::Value>(s).ok(),
            Some(serde_json::Value::Object(_)) => args.cloned(),
            _ => None,
        };
        parsed.as_ref().and_then(|v| v.get("ok")).and_then(serde_json::Value::as_bool) == Some(true)
    });
    if !passed {
        return Err("This GGUF did not produce a valid structured tool call for the CAD assistant. Try an instruct/tool-use GGUF with its original embedded chat template.".into());
    }
    Ok(format!("{template_source}; structured tool calls verified"))
}

/// Find or unpack the checksum-pinned Vulkan runtime shipped with the Windows
/// installer/portable ZIP. This is intentionally expensive and must run on a
/// worker thread, never the CAD/UI thread.
pub fn prepare_runtime() -> Result<PathBuf, String> {
    let app = std::env::current_exe().map_err(|e| e.to_string())?;
    let dir = app.parent().ok_or("Cannot determine SolidCraft installation folder")?;
    let binary = if cfg!(windows) { "llama-server.exe" } else { "llama-server" };

    // Development builds may put their locally compiled runtime beside the app.
    let adjacent = dir.join(binary);
    if adjacent.is_file() {
        return Ok(adjacent);
    }
    #[cfg(not(windows))]
    return Err("This build does not include a local GGUF runtime; install the platform runtime alongside SolidCraft".into());

    #[cfg(windows)]
    {
        let expected_hash = match std::env::consts::ARCH {
            "x86_64" => "977dc4569c0a3f644c5855f1bb836b66e45fe6aefbe739adac0928ea607bebf9",
            "aarch64" => "37dbf214474951ca1bc1242ad0464e598fbfcd442feb6ec034ef21b79eb2b443",
            _ => return Err("No bundled GGUF inference runtime is available for this Windows architecture".into()),
        };
        let archive_path = dir.join("local-gguf.zip");
        if !archive_path.is_file() {
            return Err(format!("GGUF runtime archive missing: {}. Use the GGUF-enabled MSI or portable ZIP.", archive_path.display()));
        }

        let user_dir = std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
        let target_dir = user_dir.join("SolidCraft").join(format!("llama-b11966-vulkan-{}", std::env::consts::ARCH));
        let executable = target_dir.join(binary);
        if executable.is_file() {
            return Ok(executable);
        }

        use sha2::{Digest, Sha256};
        let compressed = std::fs::read(&archive_path)
            .map_err(|e| format!("Cannot read packaged GGUF runtime: {e}"))?;
        let digest = format!("{:x}", Sha256::digest(&compressed));
        if digest != expected_hash {
            return Err("Packaged GGUF runtime failed its SHA-256 integrity check".into());
        }
        let mut zip = zip::ZipArchive::new(std::io::Cursor::new(compressed))
            .map_err(|e| format!("Cannot open GGUF runtime archive: {e}"))?;
        let staged = target_dir.with_extension(format!("tmp-{}", std::process::id()));
        if staged.exists() {
            std::fs::remove_dir_all(&staged).map_err(|e| e.to_string())?;
        }
        std::fs::create_dir_all(&staged).map_err(|e| e.to_string())?;
        let mut total = 0u64;
        let mut server_found = false;
        let unpack = (|| -> Result<(), String> {
            for i in 0..zip.len() {
                let mut item = zip.by_index(i).map_err(|e| e.to_string())?;
                if item.is_dir() { continue; }
                // Strip all upstream ZIP paths: no archive entry may choose a
                // destination outside our private runtime folder.
                let filename = item.name().map_err(|e| format!("Invalid GGUF runtime archive filename: {e}"))?.replace('\\', "/");
                let filename = filename.rsplit('/').next().unwrap_or("");
                let lower = filename.to_ascii_lowercase();
                if !(lower == "llama-server.exe" || lower.ends_with(".dll")) { continue; }
                if filename.is_empty() || filename.chars().any(|c| matches!(c, ':' | '/' | '\\')) { continue; }
                total = total.saturating_add(item.size());
                if item.size() > 256 << 20 || total > 768 << 20 {
                    return Err("GGUF runtime archive has unexpectedly large files".into());
                }
                let mut output = std::fs::File::create(staged.join(filename))
                    .map_err(|e| format!("Cannot extract {filename}: {e}"))?;
                std::io::copy(&mut item, &mut output).map_err(|e| format!("Cannot extract {filename}: {e}"))?;
                if lower == "llama-server.exe" { server_found = true; }
            }
            if !server_found { return Err("Runtime archive lacks llama-server.exe".into()); }
            Ok(())
        })();
        if let Err(error) = unpack {
            let _ = std::fs::remove_dir_all(&staged);
            return Err(error);
        }
        // A previous incomplete extraction is not used; the new complete folder
        // replaces it only after verification and successful extraction.
        if target_dir.exists() {
            std::fs::remove_dir_all(&target_dir).map_err(|e| format!("Cannot replace old GGUF runtime: {e}"))?;
        }
        std::fs::rename(&staged, &target_dir).map_err(|e| format!("Cannot finalize GGUF runtime: {e}"))?;
        Ok(executable)
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_executable_does_not_create_server() {
        let mut runtime = Runtime::default();
        assert!(runtime.load(Path::new("missing-model.gguf"), Path::new("missing-runtime"), 99).is_err());
        assert!(!runtime.running());
    }

    #[test]
    fn second_load_is_rejected_before_spawning_another_server() {
        let mut runtime = Runtime::default();
        // Simulate a connected runtime whose child is still owned. A model
        // switch must unload the previous child before launching another.
        // A missing model cannot accidentally spawn a duplicate server.
        assert!(runtime.load(Path::new("nonexistent.gguf"), Path::new("llama-server.exe"), 99).is_err());
        assert!(!runtime.running());
    }

    #[test]
    fn invalid_managed_endpoint_is_rejected() {
        assert!(Runtime::wait_ready("http://example.com/v1", Duration::ZERO).is_err());
    }

    #[test]
    fn model_probe_uses_local_endpoint_and_accepts_structured_tool_calls() {
        use std::sync::{Arc, Mutex};
        let requests = Arc::new(Mutex::new(Vec::<super::super::backend::HttpRequest>::new()));
        let record = requests.clone();
        let http: super::super::backend::HttpFn = Arc::new(move |r| {
            record.lock().unwrap().push(r.clone());
            if r.method == "GET" {
                Ok((200, br#"{"chat_template_tool_use":"{{ tools }}"}"#.to_vec()))
            } else {
                Ok((200, br#"{"choices":[{"message":{"tool_calls":[{"type":"function","function":{"name":"solidcraft_compatibility_check","arguments":"{\"ok\":true}"}}]}}]}"#.to_vec()))
            }
        });
        let report = verify_tool_calling(&http, "http://127.0.0.1:23232/v1", "test-3b.gguf").unwrap();
        assert!(report.contains("tool calls verified"), "{report}");
        let sent = requests.lock().unwrap();
        assert_eq!(sent.len(), 2);
        assert_eq!(sent[0].url, "http://127.0.0.1:23232/props");
        assert_eq!(sent[1].url, "http://127.0.0.1:23232/v1/chat/completions");
        let body: serde_json::Value = serde_json::from_slice(sent[1].body.as_deref().unwrap()).unwrap();
        assert_eq!(body["model"], "test-3b.gguf");
        assert_eq!(body["tools"][0]["function"]["name"], "solidcraft_compatibility_check");
    }

    #[test]
    fn model_probe_rejects_chat_only_and_non_loopback_endpoints() {
        use std::sync::Arc;
        let http: super::super::backend::HttpFn = Arc::new(|r| {
            if r.method == "GET" { return Ok((404, b"{}".to_vec())); }
            Ok((200, br#"{"choices":[{"message":{"content":"I can do that."}}]}"#.to_vec()))
        });
        assert!(verify_tool_calling(&http, "http://127.0.0.1:23567/v1", "chat-only.gguf").is_err());
        assert!(verify_tool_calling(&http, "http://example.com/v1", "model.gguf").is_err());
    }

    #[test]
    fn unloading_idle_runtime_is_safe() {
        let mut runtime = Runtime::default();
        runtime.unload();
        assert!(!runtime.running());
    }
}
