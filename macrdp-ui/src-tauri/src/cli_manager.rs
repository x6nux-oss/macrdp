use std::path::PathBuf;
use std::sync::Arc;

use tokio::process::{Child, Command};
use tokio::sync::Mutex;

/// 管理 macrdp-server CLI 子进程的生命周期
pub struct CliManager {
    child: Arc<Mutex<Option<Child>>>,
    socket_path: PathBuf,
    cli_binary: String,
}

impl CliManager {
    pub fn new(socket_path: PathBuf) -> Self {
        // 查找 CLI 二进制：同目录下的 macrdp-server（软链接或实际文件），回退到 PATH
        let cli_binary = Self::find_cli_binary();
        Self {
            child: Arc::new(Mutex::new(None)),
            socket_path,
            cli_binary,
        }
    }

    fn find_cli_binary() -> String {
        let target_triple = if cfg!(target_arch = "aarch64") {
            "aarch64-apple-darwin"
        } else {
            "x86_64-apple-darwin"
        };
        let sidecar_name = format!("macrdp-server-{}", target_triple);

        if let Ok(exe) = std::env::current_exe() {
            if let Some(dir) = exe.parent() {
                // 打包后: .app/Contents/MacOS/macrdp-server-{target}
                let candidate = dir.join(&sidecar_name);
                if candidate.exists() {
                    return candidate.to_string_lossy().to_string();
                }
                // 开发模式: src-tauri/target/debug/ 旁边的 binaries/ 目录不在这里
                // Tauri dev 会将 externalBin 复制到 target/debug/ 下
                let candidate = dir.join("macrdp-server");
                if candidate.exists() {
                    return candidate.to_string_lossy().to_string();
                }
            }
        }
        // 回退到 PATH
        "macrdp-server".to_string()
    }

    pub fn socket_path(&self) -> &PathBuf {
        &self.socket_path
    }

    /// Start the macrdp server process.
    pub async fn start(&self) -> Result<(), String> {
        let mut guard = self.child.lock().await;

        // Check if already running
        if let Some(ref mut child) = *guard {
            match child.try_wait() {
                Ok(Some(_status)) => {
                    // Process has exited, clean up
                    *guard = None;
                }
                Ok(None) => {
                    return Err("Server is already running".into());
                }
                Err(e) => {
                    tracing::warn!("Error checking child process: {}", e);
                    *guard = None;
                }
            }
        }

        let child = Command::new(&self.cli_binary)
            .arg("--ipc-socket")
            .arg(&self.socket_path)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .map_err(|e| format!("Failed to start server: {}", e))?;

        *guard = Some(child);
        Ok(())
    }

    /// Stop the macrdp server process.
    pub async fn stop(&self) -> Result<(), String> {
        let mut guard = self.child.lock().await;

        if let Some(ref mut child) = *guard {
            child
                .kill()
                .await
                .map_err(|e| format!("Failed to kill server process: {}", e))?;
            *guard = None;
            Ok(())
        } else {
            Err("Server is not running".into())
        }
    }

    /// Check if the server process is running.
    pub async fn is_running(&self) -> bool {
        let mut guard = self.child.lock().await;

        if let Some(ref mut child) = *guard {
            match child.try_wait() {
                Ok(Some(_status)) => {
                    // Process has exited, clean up
                    *guard = None;
                    false
                }
                Ok(None) => true,
                Err(_) => {
                    *guard = None;
                    false
                }
            }
        } else {
            false
        }
    }

    /// Get the PID of the running server process.
    pub async fn pid(&self) -> Option<u32> {
        let guard = self.child.lock().await;
        guard.as_ref().and_then(|child| child.id())
    }
}
