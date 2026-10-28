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
        if !model.is_file() || !model.extension().is_some_and(|e| e.eq_ignore_ascii_case("gguf")) {
            return Err("Select an existing .gguf model file".into());
        }
        if !executable.is_file() {
            return Err(format!("Bundled llama-server not found at {}. Install the GGUF-enabled SolidCraft package.", executable.display()));
        }
        let listener = TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0))
            .map_err(|e| format!("Could not choose a localhost port: {e}"))?;
        let port = listener.local_addr().map_err(|e| e.to_string())?.port();
        drop(listener);

        let name = model.file_name().ok_or("Model file has no name")?.to_string_lossy().into_owned();
        let log_path = std::env::temp_dir().join(format!("solidcraft-llama-{}.log", std::process::id()));
        let log = OpenOptions::new().create(true).write(true).truncate(true).open(&log_path)
            .map_err(|e| format!("Cannot open model log: {e}"))?;
        let err_log = log.try_clone().map_err(|e| e.to_string())?;
        let child = Command::new(executable)
            .args(["--host", "127.0.0.1", "--port", &port.to_string(), "--ctx-size", "8192", "--gpu-layers", &gpu_layers.to_string()])
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
            if child.try_wait().ok().flatten().is_none() {
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
        let target_dir = user_dir.join("SolidCraft").join("llama-b11966-vulkan");
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
                let filename = item.name().replace('\\', "/");
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
    fn invalid_managed_endpoint_is_rejected() {
        assert!(Runtime::wait_ready("http://example.com/v1", Duration::ZERO).is_err());
    }

    #[test]
    fn unloading_idle_runtime_is_safe() {
        let mut runtime = Runtime::default();
        runtime.unload();
        assert!(!runtime.running());
    }
}
