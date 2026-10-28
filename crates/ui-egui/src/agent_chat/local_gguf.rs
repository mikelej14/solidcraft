//! Local GGUF server owned by SolidCraft, exposing the existing OpenAI-compatible API.
//! No external provider configuration is modified.

use std::net::{Ipv4Addr, SocketAddrV4, TcpListener};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

#[derive(Default)]
pub struct Runtime {
    child: Option<Child>,
    pub model_path: Option<PathBuf>,
    pub endpoint: Option<String>,
    pub error: Option<String>,
}

impl Runtime {
    pub fn running(&mut self) -> bool {
        if self.child.as_mut().is_some_and(|c| c.try_wait().ok().flatten().is_some()) {
            self.child = None;
            self.endpoint = None;
        }
        self.child.is_some()
    }

    pub fn load(&mut self, model: &Path, executable: &Path) -> Result<(String, String), String> {
        if self.running() { return Err("Unload the current GGUF first".into()); }
        if !model.is_file() || !model.extension().is_some_and(|e| e.eq_ignore_ascii_case("gguf")) {
            return Err("Select an existing GGUF model file".into());
        }
        if !executable.is_file() { return Err(format!("llama-server executable missing: {}", executable.display())); }
        let listener = TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0))
            .map_err(|e| format!("Could not select a local port: {e}"))?;
        let port = listener.local_addr().map_err(|e| e.to_string())?.port();
        drop(listener);
        let child = Command::new(executable)
            .args(["--host", "127.0.0.1", "--port", &port.to_string(), "--ctx-size", "8192", "--gpu-layers", "99"])
            .arg("--model").arg(model)
            .stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null())
            .spawn().map_err(|e| format!("Failed to start llama-server: {e}"))?;
        let endpoint = format!("http://127.0.0.1:{port}/v1");
        let name = model.file_name().unwrap_or_default().to_string_lossy().into_owned();
        self.model_path = Some(model.to_path_buf());
        self.endpoint = Some(endpoint.clone());
        self.error = None;
        self.child = Some(child);
        Ok((endpoint, name))
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

pub fn bundled_server() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?;
    let filename = if cfg!(windows) { "llama-server.exe" } else { "llama-server" };
    Some(dir.join(filename))
}
