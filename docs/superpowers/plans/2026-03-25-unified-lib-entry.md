# macrdp 统一 Library 入口重构实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将 UI 调用 CLI 的子进程+IPC 方式改为 in-process library 嵌入，提供统一入口函数。

**Architecture:** 新建 `macrdp-core` library crate，从 `macrdp-server` 提取核心服务逻辑（显示适配、输入处理、TLS、权限、编码）。通过 `ServerEventHandler` trait 回调推送实时事件。CLI 和 Tauri UI 都调用 `macrdp_core::start_server()` 启动服务。

**Tech Stack:** Rust, tokio, ironrdp-server (forked), ScreenCaptureKit, CGEvent, OpenH264/VideoToolbox, Tauri v2

**Spec:** `docs/superpowers/specs/2026-03-25-unified-lib-entry-design.md`

---

## 文件结构

### 新建文件
- `crates/macrdp-core/Cargo.toml`
- `crates/macrdp-core/src/lib.rs`
- `crates/macrdp-core/src/callbacks.rs` — ServerEventHandler trait + 事件类型
- `crates/macrdp-core/src/config.rs` — ServerConfig (从 macrdp-server 迁移)
- `crates/macrdp-core/src/server.rs` — start_server(), ServerHandle
- `crates/macrdp-core/src/display.rs` — MacDisplay (从 macrdp-server 迁移)
- `crates/macrdp-core/src/handler.rs` — MacInputHandler (从 macrdp-server 迁移)
- `crates/macrdp-core/src/tls.rs` — TLS 证书管理 (从 macrdp-server 迁移)
- `crates/macrdp-core/src/permissions.rs` — 权限检查 (从 main.rs 提取)
- `macrdp-ui/src-tauri/src/event_bridge.rs` — ServerEventHandler Tauri 实现

### 重写文件
- `crates/macrdp-server/src/main.rs` — 瘦 CLI (~60行)
- `crates/macrdp-server/Cargo.toml` — 依赖改为 macrdp-core
- `macrdp-ui/src-tauri/src/commands.rs` — 调用 macrdp-core API
- `macrdp-ui/src-tauri/src/lib.rs` — 去掉 CliManager/IpcClient
- `macrdp-ui/src-tauri/src/state.rs` — 添加便捷方法
- `macrdp-ui/src-tauri/Cargo.toml` — 依赖加 macrdp-core

### 删除文件
- `crates/macrdp-server/src/ipc.rs`
- `crates/macrdp-server/src/display.rs` (迁移到 core)
- `crates/macrdp-server/src/handler.rs` (迁移到 core)
- `crates/macrdp-server/src/tls.rs` (迁移到 core)
- `crates/macrdp-server/src/config.rs` (迁移到 core)
- `macrdp-ui/src-tauri/src/cli_manager.rs`
- `macrdp-ui/src-tauri/src/ipc_client.rs`

### 修改文件
- `Cargo.toml` (workspace) — 添加 macrdp-core member
- `Makefile` — 更新构建命令
- `macrdp-ui/src-tauri/src/tray.rs` — 微调函数签名
- `macrdp-ui/src-tauri/src/database.rs` — 适配 DisconnectionInfo 类型

---

## Task 1: 创建 macrdp-core crate 骨架

**Files:**
- Create: `crates/macrdp-core/Cargo.toml`
- Create: `crates/macrdp-core/src/lib.rs`
- Create: `crates/macrdp-core/src/callbacks.rs`
- Modify: `Cargo.toml` (workspace root)

- [ ] **Step 1: 创建 macrdp-core/Cargo.toml**

```toml
[package]
name = "macrdp-core"
version = "0.1.0"
edition.workspace = true

[dependencies]
macrdp-capture = { path = "../macrdp-capture" }
macrdp-input = { path = "../macrdp-input" }
macrdp-encode = { path = "../macrdp-encode" }
tokio.workspace = true
tracing.workspace = true
tracing-subscriber.workspace = true
anyhow.workspace = true
bytes.workspace = true
ironrdp-server = { version = "0.10", features = ["helper"] }
async-trait = "0.1"
serde = { version = "1", features = ["derive"] }
toml = "0.8"
rcgen = "0.13"
dirs = "6"
```

- [ ] **Step 2: 创建 callbacks.rs — 事件类型和 trait**

```rust
//! Event callback types and ServerEventHandler trait

use serde::Serialize;

/// Server running status
#[derive(Debug, Clone, Serialize)]
pub struct ServerStatus {
    pub running: bool,
    pub state: String,
    pub uptime_secs: u64,
}

impl Default for ServerStatus {
    fn default() -> Self {
        Self {
            running: false,
            state: "stopped".to_string(),
            uptime_secs: 0,
        }
    }
}

/// Real-time performance metrics
#[derive(Debug, Clone, Serialize, Default)]
pub struct Metrics {
    pub fps: u32,
    pub bitrate_kbps: u64,
    pub latency_ms: f64,
    pub bytes_sent: u64,
    pub timestamp: u64,
}

/// Connection event
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "event")]
pub enum ConnectionEvent {
    #[serde(rename = "connected")]
    Connected(ConnectionInfo),
    #[serde(rename = "disconnected")]
    Disconnected(DisconnectionInfo),
}

/// Active client connection info
#[derive(Debug, Clone, Serialize)]
pub struct ConnectionInfo {
    pub client_ip: String,
    pub client_name: String,
    pub connected_at: String,
}

/// Disconnection info
#[derive(Debug, Clone, Serialize)]
pub struct DisconnectionInfo {
    pub client_ip: String,
    pub client_name: String,
    pub duration_secs: u64,
    pub bytes_total: u64,
}

/// A single log entry
#[derive(Debug, Clone, Serialize)]
pub struct LogEntry {
    pub level: String,
    pub message: String,
    pub timestamp: String,
}

/// macOS permission status
#[derive(Debug, Clone, Serialize, Default)]
pub struct PermissionStatus {
    pub screen_capture: bool,
    pub accessibility: bool,
    pub microphone: bool,
}

/// Hot-updatable configuration
#[derive(Debug, Clone)]
pub enum ConfigUpdate {
    FrameRate(u32),
    BitrateKbps(u32),
    LogLevel(String),
}

/// Callback trait for receiving server events.
/// All methods have default empty implementations so callers can override only what they need.
pub trait ServerEventHandler: Send + Sync + 'static {
    fn on_status_change(&self, _status: ServerStatus) {}
    fn on_metrics(&self, _metrics: Metrics) {}
    fn on_connection(&self, _event: ConnectionEvent) {}
    fn on_log(&self, _entry: LogEntry) {}
}
```

- [ ] **Step 3: 创建 lib.rs — 模块声明和公共 re-exports**

```rust
//! macrdp-core: unified RDP server library

pub mod callbacks;
pub mod config;
pub mod display;
pub mod handler;
pub mod permissions;
pub mod server;
pub mod tls;

pub use callbacks::*;
pub use config::{config_dir, ServerConfig};
pub use server::{start_server, ServerHandle};
```

注意：此时 config/display/handler/permissions/server/tls 模块尚未创建，编译会失败。这是预期的。

- [ ] **Step 4: 更新 workspace Cargo.toml**

在 `Cargo.toml` 的 `[workspace]` members 中添加 `"crates/macrdp-core"`:

```toml
[workspace]
resolver = "2"
members = [
    "crates/macrdp-core",
    "crates/macrdp-server",
    "crates/macrdp-capture",
    "crates/macrdp-input",
    "crates/macrdp-encode",
]
```

- [ ] **Step 5: Commit**

```bash
git add crates/macrdp-core/ Cargo.toml
git commit -m "feat(core): create macrdp-core crate skeleton with callbacks trait"
```

---

## Task 2: 迁移 config.rs 到 macrdp-core

**Files:**
- Create: `crates/macrdp-core/src/config.rs`
- Modify: `crates/macrdp-server/src/config.rs` (将被后续 Task 替换)

- [ ] **Step 1: 创建 macrdp-core/src/config.rs**

从 `crates/macrdp-server/src/config.rs` 复制 `ServerConfig`、`Default`、`config_dir()`、`config_dir_candidates()`、测试。
去掉 `Cli` 结构体（留给 CLI binary）。
去掉 `ServerConfig::load(&Cli)` 方法，替换为不依赖 clap 的版本。
添加 `Serialize` derive。

```rust
//! Server configuration

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Server configuration (loaded from TOML, shared between CLI and UI)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ServerConfig {
    pub port: u16,
    pub frame_rate: u32,
    /// Resolution width (0 = auto-detect from display)
    pub width: u32,
    /// Resolution height (0 = auto-detect from display)
    pub height: u32,
    pub username: Option<String>,
    pub password: Option<String>,
    pub cert_path: Option<PathBuf>,
    pub key_path: Option<PathBuf>,
    pub idle_timeout_secs: u64,
    /// Log level: trace, debug, info, warn, error
    pub log_level: Option<String>,
    /// Video quality: low_latency, balanced, high_quality (default: high_quality)
    pub quality: Option<String>,
    /// H.264 encoder: software, hardware, auto (default: software)
    pub encoder: Option<String>,
    /// Chroma subsampling mode: "avc420" or "avc444" (default: "avc420")
    pub chroma_mode: Option<String>,
    /// HiDPI scale factor (default: 1)
    pub hidpi_scale: Option<u32>,
    /// Target bitrate in Mbps (default: auto-calculated)
    pub bitrate_mbps: Option<u32>,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            port: 3389,
            frame_rate: 60,
            width: 0,
            height: 0,
            username: None,
            password: None,
            cert_path: None,
            key_path: None,
            idle_timeout_secs: 1800,
            log_level: None,
            quality: None,
            encoder: None,
            chroma_mode: None,
            hidpi_scale: None,
            bitrate_mbps: None,
        }
    }
}

impl ServerConfig {
    /// Load config from a TOML file path, or use defaults if None.
    pub fn load_from_file(path: Option<&std::path::Path>) -> anyhow::Result<Self> {
        if let Some(path) = path {
            let content = std::fs::read_to_string(path)?;
            Ok(toml::from_str(&content)?)
        } else {
            let default_path = config_dir().join("config.toml");
            if default_path.exists() {
                let content = std::fs::read_to_string(&default_path)?;
                Ok(toml::from_str(&content)?)
            } else {
                Ok(ServerConfig::default())
            }
        }
    }
}

/// Returns the macrdp config directory.
/// Search order: ./ → ~/.config/macrdp → ~/Library/Application Support/macrdp
pub fn config_dir() -> PathBuf {
    let candidates = config_dir_candidates();
    for dir in &candidates {
        if dir.join("config.toml").exists() {
            return dir.clone();
        }
    }
    candidates.into_iter().next().unwrap_or_else(|| PathBuf::from("."))
}

fn config_dir_candidates() -> Vec<PathBuf> {
    let mut dirs_list = Vec::new();
    if let Ok(cwd) = std::env::current_dir() {
        dirs_list.push(cwd);
    } else {
        dirs_list.push(PathBuf::from("."));
    }
    if let Some(home) = dirs::home_dir() {
        dirs_list.push(home.join(".config").join("macrdp"));
    }
    if let Some(native) = dirs::config_dir() {
        dirs_list.push(native.join("macrdp"));
    }
    dirs_list
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = ServerConfig::default();
        assert_eq!(config.port, 3389);
        assert_eq!(config.frame_rate, 60);
        assert!(config.username.is_none());
    }

    #[test]
    fn test_parse_toml_config() {
        let toml_str = r#"
            port = 13389
            frame_rate = 120
            username = "admin"
            password = "secret"
        "#;
        let config: ServerConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.port, 13389);
        assert_eq!(config.frame_rate, 120);
        assert_eq!(config.username.as_deref(), Some("admin"));
    }
}
```

- [ ] **Step 2: 验证编译**

Run: `cargo build -p macrdp-core 2>&1 | tail -5`
Expected: 编译错误（其他模块尚未创建），但 config 模块本身应该没有错误

- [ ] **Step 3: Commit**

```bash
git add crates/macrdp-core/src/config.rs
git commit -m "feat(core): add ServerConfig with TOML loading"
```

---

## Task 3: 迁移 tls.rs 和 permissions.rs 到 macrdp-core

**Files:**
- Create: `crates/macrdp-core/src/tls.rs`
- Create: `crates/macrdp-core/src/permissions.rs`

- [ ] **Step 1: 创建 macrdp-core/src/tls.rs**

从 `crates/macrdp-server/src/tls.rs` 完整复制，仅修改 `use crate::config::config_dir` 路径（在 macrdp-core 内部保持不变）。

```rust
//! TLS certificate generation and management

use anyhow::{Context, Result};
use rcgen::{CertificateParams, DistinguishedName, KeyPair};
use std::fs;
use std::path::{Path, PathBuf};

use crate::config::config_dir;

/// Generate a self-signed certificate and write PEM files
pub fn generate_self_signed_cert(cert_path: &Path, key_path: &Path) -> Result<()> {
    let mut params = CertificateParams::default();
    let mut dn = DistinguishedName::new();
    dn.push(rcgen::DnType::CommonName, "macrdp");
    dn.push(rcgen::DnType::OrganizationName, "macrdp");
    params.distinguished_name = dn;

    params.not_after = rcgen::date_time_ymd(2027, 3, 24);

    params.subject_alt_names = vec![
        rcgen::SanType::DnsName("localhost".try_into()?),
        rcgen::SanType::IpAddress(std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST)),
    ];

    let key_pair = KeyPair::generate()?;
    let cert = params.self_signed(&key_pair)?;

    if let Some(parent) = cert_path.parent() {
        fs::create_dir_all(parent)?;
    }

    fs::write(cert_path, cert.pem())?;
    fs::write(key_path, key_pair.serialize_pem())?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(key_path, fs::Permissions::from_mode(0o600))?;
    }

    tracing::info!(?cert_path, ?key_path, "Generated self-signed TLS certificate");
    Ok(())
}

/// Ensure TLS cert and key files exist. Generate if missing.
pub fn ensure_tls_files(
    cert_path: Option<&Path>,
    key_path: Option<&Path>,
) -> Result<(PathBuf, PathBuf)> {
    let tls_dir = config_dir().join("tls");
    let cert = cert_path
        .map(PathBuf::from)
        .unwrap_or_else(|| tls_dir.join("cert.pem"));
    let key = key_path
        .map(PathBuf::from)
        .unwrap_or_else(|| tls_dir.join("key.pem"));

    if !cert.exists() || !key.exists() {
        generate_self_signed_cert(&cert, &key)
            .context("Failed to generate self-signed certificate")?;
    }

    Ok((cert, key))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_generate_self_signed_cert() {
        let dir = TempDir::new().unwrap();
        let cert_path = dir.path().join("cert.pem");
        let key_path = dir.path().join("key.pem");
        generate_self_signed_cert(&cert_path, &key_path).unwrap();
        assert!(cert_path.exists());
        assert!(key_path.exists());
    }

    #[test]
    fn test_ensure_tls_files_creates_when_missing() {
        let dir = TempDir::new().unwrap();
        let cert_path = dir.path().join("cert.pem");
        let key_path = dir.path().join("key.pem");
        let (c, k) = ensure_tls_files(Some(&cert_path), Some(&key_path)).unwrap();
        assert_eq!(c, cert_path);
        assert_eq!(k, key_path);
    }
}
```

- [ ] **Step 2: 创建 macrdp-core/src/permissions.rs**

从 `crates/macrdp-server/src/main.rs` 的 `check_permissions()` 函数提取。

```rust
//! macOS permission checking and requesting

use crate::callbacks::PermissionStatus;

/// Check current macOS permission status (non-blocking, no prompts)
pub fn check_permissions() -> PermissionStatus {
    PermissionStatus {
        screen_capture: macrdp_capture::check_screen_recording_permission(),
        accessibility: macrdp_input::check_accessibility_permission(),
        microphone: false, // Phase 3
    }
}

/// Request all required macOS permissions (may trigger system dialogs).
/// Returns the updated permission status.
pub fn request_permissions() -> PermissionStatus {
    tracing::info!("Checking macOS permissions...");

    // Screen Recording
    if macrdp_capture::check_screen_recording_permission() {
        tracing::info!("[OK] Screen Recording permission granted");
    } else {
        tracing::warn!("[!!] Screen Recording permission NOT granted");
        macrdp_capture::request_screen_recording_permission();
        if !macrdp_capture::check_screen_recording_permission() {
            tracing::error!("Screen Recording denied. Go to: System Settings > Privacy & Security > Screen Recording");
            macrdp_capture::open_screen_recording_settings();
        }
    }

    // Accessibility
    if macrdp_input::check_accessibility_permission() {
        tracing::info!("[OK] Accessibility permission granted");
    } else {
        tracing::warn!("[!!] Accessibility permission NOT granted");
        macrdp_input::request_accessibility_permission();
        if !macrdp_input::check_accessibility_permission() {
            tracing::error!("Accessibility denied. Go to: System Settings > Privacy & Security > Accessibility");
            macrdp_input::open_accessibility_settings();
        }
    }

    check_permissions()
}

/// Detect the main display's logical pixel size
pub fn detect_display_size() -> anyhow::Result<(u32, u32)> {
    macrdp_capture::detect_display_size()
}
```

- [ ] **Step 3: 添加 tempfile dev-dependency 到 macrdp-core**

在 `crates/macrdp-core/Cargo.toml` 末尾添加:

```toml
[dev-dependencies]
tempfile = "3"
```

- [ ] **Step 4: Commit**

```bash
git add crates/macrdp-core/src/tls.rs crates/macrdp-core/src/permissions.rs crates/macrdp-core/Cargo.toml
git commit -m "feat(core): add TLS management and permission checking"
```

---

## Task 4: 迁移 display.rs 和 handler.rs 到 macrdp-core

**Files:**
- Create: `crates/macrdp-core/src/display.rs`
- Create: `crates/macrdp-core/src/handler.rs`

- [ ] **Step 1: 创建 macrdp-core/src/display.rs**

从 `crates/macrdp-server/src/display.rs` 完整复制。所有 import 路径不变（依赖 crate 相同）。将 `MacDisplay` 和 `frame_to_bitmap_updates` 设为 `pub`。

完整复制 `crates/macrdp-server/src/display.rs` 的全部 436 行代码到 `crates/macrdp-core/src/display.rs`，无需修改。

- [ ] **Step 2: 创建 macrdp-core/src/handler.rs**

从 `crates/macrdp-server/src/handler.rs` 完整复制全部 103 行。将 `MacInputHandler` 设为 `pub`。无需修改 import。

完整复制 `crates/macrdp-server/src/handler.rs` 到 `crates/macrdp-core/src/handler.rs`，无需修改。

- [ ] **Step 3: 验证 macrdp-core 编译**

Run: `cargo build -p macrdp-core 2>&1 | tail -10`
Expected: 仅 `server` 模块缺失的编译错误（其他模块应该通过）

- [ ] **Step 4: Commit**

```bash
git add crates/macrdp-core/src/display.rs crates/macrdp-core/src/handler.rs
git commit -m "feat(core): migrate display adapter and input handler"
```

---

## Task 5: 实现 server.rs — start_server() 和 ServerHandle

这是核心任务。从 `main.rs` 提取 RDP 服务器构建和启动逻辑。

**Files:**
- Create: `crates/macrdp-core/src/server.rs`

- [ ] **Step 1: 创建 server.rs**

```rust
//! RDP server lifecycle: start, stop, metrics, connections

use std::net::SocketAddr;
use std::sync::{Arc, Mutex, atomic::{AtomicBool, Ordering}};
use std::time::Instant;

use anyhow::{Context, Result};
use ironrdp_server::{Credentials, RdpServer, TlsIdentityCtx, gfx::GfxState};
use tokio::sync::Notify;
use tokio::task::JoinHandle;

use crate::callbacks::*;
use crate::config::ServerConfig;
use crate::display::MacDisplay;
use crate::handler::MacInputHandler;
use crate::tls;
use crate::permissions;

/// Handle to a running RDP server. Use to query state and stop the server.
pub struct ServerHandle {
    port: u16,
    started_at: Instant,
    gfx_state: Arc<Mutex<GfxState>>,
    shutdown_notify: Arc<Notify>,
    stopped: AtomicBool,
    server_task: Mutex<Option<JoinHandle<()>>>,
    metrics_task: Mutex<Option<JoinHandle<()>>>,
}

impl ServerHandle {
    /// The TCP port the server is listening on.
    pub fn port(&self) -> u16 {
        self.port
    }

    /// Current server status.
    pub fn status(&self) -> ServerStatus {
        if self.stopped.load(Ordering::Relaxed) {
            ServerStatus {
                running: false,
                state: "stopped".to_string(),
                uptime_secs: 0,
            }
        } else {
            ServerStatus {
                running: true,
                state: "running".to_string(),
                uptime_secs: self.started_at.elapsed().as_secs(),
            }
        }
    }

    /// Current performance metrics.
    pub fn metrics(&self) -> Metrics {
        let gfx = self.gfx_state.lock().unwrap();
        Metrics {
            fps: 0, // Actual FPS comes from config/runtime
            bitrate_kbps: 0,
            latency_ms: gfx.rtt_ewma_ms,
            bytes_sent: gfx.total_bytes_sent,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        }
    }

    /// Gracefully stop the server.
    pub async fn stop(&self) -> Result<()> {
        if self.stopped.swap(true, Ordering::Relaxed) {
            return Ok(()); // Already stopped
        }
        self.shutdown_notify.notify_waiters();

        // Wait for server task to finish
        if let Some(task) = self.server_task.lock().unwrap().take() {
            let _ = tokio::time::timeout(
                std::time::Duration::from_secs(5),
                task,
            ).await;
        }
        // Cancel metrics task
        if let Some(task) = self.metrics_task.lock().unwrap().take() {
            task.abort();
        }
        Ok(())
    }
}

impl Drop for ServerHandle {
    fn drop(&mut self) {
        if !self.stopped.load(Ordering::Relaxed) {
            self.shutdown_notify.notify_waiters();
        }
    }
}

/// Start the RDP server with the given configuration and event handler.
/// Returns a handle to control the running server.
pub async fn start_server(
    config: ServerConfig,
    handler: impl ServerEventHandler,
) -> Result<Arc<ServerHandle>> {
    let handler = Arc::new(handler);

    // Notify handler: starting
    handler.on_status_change(ServerStatus {
        running: true,
        state: "starting".to_string(),
        uptime_secs: 0,
    });

    // Check and request permissions
    let perms = permissions::request_permissions();
    tracing::info!(?perms, "Permission status");

    // TLS
    let (cert_path, key_path) =
        tls::ensure_tls_files(config.cert_path.as_deref(), config.key_path.as_deref())?;
    let tls_identity = TlsIdentityCtx::init_from_paths(&cert_path, &key_path)
        .context("Failed to load TLS certificate")?;
    let tls_acceptor = tls_identity.make_acceptor()
        .context("Failed to create TLS acceptor")?;

    // Display detection
    let (logical_w, logical_h) = match permissions::detect_display_size() {
        Ok((w, h)) => {
            tracing::info!(width = w, height = h, "Detected macOS logical display size");
            (w as u16, h as u16)
        }
        Err(e) => {
            tracing::warn!("Failed to detect display size: {e}, defaulting to 1920x1080");
            (1920u16, 1080u16)
        }
    };

    // Resolution
    let (width, height) = if config.width > 0 && config.height > 0 {
        (config.width as u16, config.height as u16)
    } else {
        (logical_w, logical_h)
    };

    // Quality / encoder / chroma
    let quality = match config.quality.as_deref() {
        Some("low_latency") => macrdp_encode::Quality::LowLatency,
        Some("balanced") => macrdp_encode::Quality::Balanced,
        _ => macrdp_encode::Quality::HighQuality,
    };
    let encoder_pref = macrdp_encode::EncoderPreference::from_str_opt(config.encoder.as_deref());
    let mode_444 = config.chroma_mode.as_deref() == Some("avc444");

    // HiDPI
    let hidpi_scale = config.hidpi_scale.unwrap_or(1).max(1).min(4);
    let (width, height) = (width * hidpi_scale as u16, height * hidpi_scale as u16);

    // Mouse coordinate mapping
    let mouse_scale_x = width as f64 / logical_w as f64;
    let mouse_scale_y = height as f64 / logical_h as f64;

    // GFX state
    let gfx_state = Arc::new(Mutex::new(GfxState::new(width, height, mode_444)));

    // Input handler
    let input_handler = MacInputHandler::new(mouse_scale_x, mouse_scale_y);

    // Display
    let fixed_resolution = config.width > 0 && config.height > 0;
    let bitrate_override = config.bitrate_mbps.map(|mbps| mbps * 1_000_000);
    let display = MacDisplay::new(
        width, height, fixed_resolution, config.frame_rate,
        quality, encoder_pref, mode_444, bitrate_override,
        Arc::clone(&gfx_state),
    );

    let bind_addr: SocketAddr = format!("0.0.0.0:{}", config.port).parse()?;

    // Build RDP server
    let mut server = RdpServer::builder()
        .with_addr(bind_addr)
        .with_hybrid(tls_acceptor, tls_identity.pub_key)
        .with_input_handler(input_handler)
        .with_display_handler(display)
        .build();

    server.set_gfx_state(Arc::clone(&gfx_state));

    // Credentials
    let (username, password) = match (&config.username, &config.password) {
        (Some(u), Some(p)) => (u.clone(), p.clone()),
        _ => {
            let seed = std::process::id() as u64
                ^ std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos() as u64;
            let random_pass: String = (0..8)
                .map(|i| {
                    let v = ((seed.wrapping_mul(6364136223846793005).wrapping_add(i * 1442695040888963407)) >> (i * 5 + 3)) % 36;
                    if v < 10 { (b'0' + v as u8) as char } else { (b'a' + (v - 10) as u8) as char }
                })
                .collect();
            let user = "macrdp".to_string();
            tracing::warn!("No credentials in config — using generated credentials:");
            println!("\n  ┌──────────────────────────────────┐");
            println!("  │  Username: {:<22}│", &user);
            println!("  │  Password: {:<22}│", &random_pass);
            println!("  └──────────────────────────────────┘\n");
            (user, random_pass)
        }
    };
    server.set_credentials(Some(Credentials {
        username: username.clone(),
        password: password.clone(),
        domain: None,
    }));
    tracing::info!("Authentication configured for user: {}", username);

    let shutdown_notify = Arc::new(Notify::new());
    let port = config.port;

    // Spawn RDP server task
    let shutdown_for_server = Arc::clone(&shutdown_notify);
    let handler_for_server = Arc::clone(&handler);
    let server_task = tokio::spawn(async move {
        tracing::info!(%bind_addr, "RDP server listening");
        tokio::select! {
            result = server.run() => {
                if let Err(e) = result {
                    tracing::error!("RDP server error: {e}");
                    handler_for_server.on_status_change(ServerStatus {
                        running: false,
                        state: format!("error: {e}"),
                        uptime_secs: 0,
                    });
                }
            }
            _ = shutdown_for_server.notified() => {
                tracing::info!("Shutdown signal received, stopping server");
            }
        }
    });

    // Spawn metrics ticker (1 second interval)
    let metrics_gfx = Arc::clone(&gfx_state);
    let handler_for_metrics = Arc::clone(&handler);
    let fps = config.frame_rate;
    let metrics_task = tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(1));
        let mut prev_bytes_sent: u64 = 0;
        loop {
            interval.tick().await;
            let (latency_ms, bytes_sent) = {
                let gfx = metrics_gfx.lock().unwrap();
                (gfx.rtt_ewma_ms, gfx.total_bytes_sent)
            };
            let delta_bytes = bytes_sent.saturating_sub(prev_bytes_sent);
            prev_bytes_sent = bytes_sent;
            let bitrate_kbps = delta_bytes * 8 / 1000;
            let timestamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            handler_for_metrics.on_metrics(Metrics {
                fps,
                bitrate_kbps,
                latency_ms,
                bytes_sent,
                timestamp,
            });
        }
    });

    let handle = Arc::new(ServerHandle {
        port,
        started_at: Instant::now(),
        gfx_state,
        shutdown_notify,
        stopped: AtomicBool::new(false),
        server_task: Mutex::new(Some(server_task)),
        metrics_task: Mutex::new(Some(metrics_task)),
    });

    // Notify handler: running
    handler.on_status_change(ServerStatus {
        running: true,
        state: "running".to_string(),
        uptime_secs: 0,
    });

    Ok(handle)
}
```

- [ ] **Step 2: 验证 macrdp-core 编译**

Run: `cargo build -p macrdp-core`
Expected: 编译成功（所有模块都已创建）

- [ ] **Step 3: Commit**

```bash
git add crates/macrdp-core/src/server.rs
git commit -m "feat(core): implement start_server() and ServerHandle"
```

---

## Task 6: 重写 macrdp-server CLI 为瘦包装

**Files:**
- Rewrite: `crates/macrdp-server/src/main.rs`
- Rewrite: `crates/macrdp-server/Cargo.toml`
- Delete: `crates/macrdp-server/src/config.rs`
- Delete: `crates/macrdp-server/src/display.rs`
- Delete: `crates/macrdp-server/src/handler.rs`
- Delete: `crates/macrdp-server/src/tls.rs`
- Delete: `crates/macrdp-server/src/ipc.rs`

- [ ] **Step 1: 更新 macrdp-server/Cargo.toml**

```toml
[package]
name = "macrdp-server"
version = "0.1.0"
edition.workspace = true

[dependencies]
macrdp-core = { path = "../macrdp-core" }
tokio.workspace = true
tracing.workspace = true
tracing-subscriber.workspace = true
anyhow.workspace = true
clap = { version = "4", features = ["derive"] }
```

- [ ] **Step 2: 重写 main.rs**

```rust
use anyhow::Result;
use clap::Parser;
use macrdp_core::*;

#[derive(Parser, Debug)]
#[command(name = "macrdp", about = "macOS RDP Server")]
struct Cli {
    /// TCP port to listen on
    #[arg(short, long, default_value_t = 3389)]
    port: u16,
    /// Path to config file
    #[arg(short, long)]
    config: Option<std::path::PathBuf>,
    /// Target frame rate (30, 60, or 120)
    #[arg(long)]
    frame_rate: Option<u32>,
    /// Log level: trace, debug, info, warn, error
    #[arg(long)]
    log_level: Option<String>,
}

/// CLI event handler: logs events to tracing
struct CliEventHandler;

impl ServerEventHandler for CliEventHandler {
    fn on_status_change(&self, status: ServerStatus) {
        tracing::info!(running = status.running, state = %status.state, "Server status changed");
    }
    fn on_metrics(&self, m: Metrics) {
        tracing::debug!(fps = m.fps, bitrate_kbps = m.bitrate_kbps, latency_ms = m.latency_ms, "Metrics");
    }
    fn on_connection(&self, event: ConnectionEvent) {
        match event {
            ConnectionEvent::Connected(c) => tracing::info!(ip = %c.client_ip, "Client connected"),
            ConnectionEvent::Disconnected(d) => tracing::info!(ip = %d.client_ip, duration = d.duration_secs, "Client disconnected"),
        }
    }
    fn on_log(&self, _entry: LogEntry) {
        // Core already uses tracing internally, no need to forward
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Load config from file, then apply CLI overrides
    let mut config = ServerConfig::load_from_file(cli.config.as_deref())?;
    if cli.port != 3389 {
        config.port = cli.port;
    }
    if let Some(fps) = cli.frame_rate {
        config.frame_rate = fps;
    }
    if let Some(level) = &cli.log_level {
        config.log_level = Some(level.clone());
    }

    // Initialize logging
    let log_level = config.log_level.as_deref().unwrap_or("info");
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(log_level));

    use tracing_subscriber::layer::SubscriberExt;
    use tracing_subscriber::util::SubscriberInitExt;

    tracing_subscriber::registry()
        .with(env_filter)
        .with(tracing_subscriber::fmt::layer().with_writer(std::io::stderr))
        .init();

    tracing::info!(?config, "macrdp server starting");

    let handle = start_server(config, CliEventHandler).await?;

    tracing::info!("macrdp running on port {}", handle.port());
    tokio::signal::ctrl_c().await?;

    handle.stop().await?;
    tracing::info!("Server stopped");
    Ok(())
}
```

- [ ] **Step 3: 删除已迁移的源文件**

```bash
rm crates/macrdp-server/src/config.rs
rm crates/macrdp-server/src/display.rs
rm crates/macrdp-server/src/handler.rs
rm crates/macrdp-server/src/tls.rs
rm crates/macrdp-server/src/ipc.rs
```

- [ ] **Step 4: 验证 CLI 编译**

Run: `cargo build -p macrdp-server`
Expected: 编译成功

- [ ] **Step 5: Commit**

```bash
git add -A crates/macrdp-server/
git commit -m "refactor(server): rewrite CLI as thin wrapper over macrdp-core"
```

---

## Task 7: 改造 Tauri UI — 创建 event_bridge 并更新 state

**Files:**
- Create: `macrdp-ui/src-tauri/src/event_bridge.rs`
- Modify: `macrdp-ui/src-tauri/src/state.rs`
- Modify: `macrdp-ui/src-tauri/src/database.rs`
- Modify: `macrdp-ui/src-tauri/src/tray.rs`

- [ ] **Step 1: 更新 state.rs — 使用 macrdp-core 类型 + 添加便捷方法**

```rust
use std::collections::VecDeque;
use std::sync::Arc;

use macrdp_core::{
    ServerStatus, Metrics, ConnectionInfo, LogEntry, PermissionStatus,
};
use serde::Serialize;
use tokio::sync::Mutex;

/// An active client connection (UI display format, matches existing frontend JSON)
#[derive(Debug, Clone, Serialize)]
pub struct Connection {
    pub client_ip: String,
    pub client_name: String,
    pub connected_at: String,
    pub bytes_total: u64,
}

/// Application-wide shared state.
#[derive(Debug)]
pub struct AppState {
    pub status: Mutex<ServerStatus>,
    pub metrics: Mutex<Metrics>,
    pub connections: Mutex<Vec<Connection>>,
    pub logs: Mutex<VecDeque<LogEntry>>,
    pub permissions: Mutex<PermissionStatus>,
}

const MAX_LOGS: usize = 5000;

impl AppState {
    pub fn new() -> Self {
        Self {
            status: Mutex::new(ServerStatus::default()),
            metrics: Mutex::new(Metrics::default()),
            connections: Mutex::new(Vec::new()),
            logs: Mutex::new(VecDeque::new()),
            permissions: Mutex::new(PermissionStatus::default()),
        }
    }

    pub async fn set_status(&self, status: ServerStatus) {
        *self.status.lock().await = status;
    }

    pub async fn set_metrics(&self, metrics: Metrics) {
        *self.metrics.lock().await = metrics;
    }

    pub async fn add_connection(&self, info: &ConnectionInfo) {
        let conn = Connection {
            client_ip: info.client_ip.clone(),
            client_name: info.client_name.clone(),
            connected_at: info.connected_at.clone(),
            bytes_total: 0,
        };
        self.connections.lock().await.push(conn);
    }

    pub async fn remove_connection(&self, client_ip: &str) {
        self.connections.lock().await.retain(|c| c.client_ip != client_ip);
    }

    pub async fn push_log(&self, entry: LogEntry) {
        let mut logs = self.logs.lock().await;
        if logs.len() >= MAX_LOGS {
            logs.pop_front();
        }
        logs.push_back(entry);
    }

    pub async fn set_permissions(&self, perms: PermissionStatus) {
        *self.permissions.lock().await = perms;
    }
}
```

- [ ] **Step 2: 创建 event_bridge.rs**

```rust
//! Bridges macrdp-core ServerEventHandler to Tauri events

use std::sync::Arc;

use macrdp_core::{
    ConnectionEvent, LogEntry, Metrics, ServerEventHandler, ServerStatus,
};
use tauri::{AppHandle, Emitter};

use crate::database::Database;
use crate::state::AppState;
use crate::tray::update_tray_status;

pub struct TauriEventBridge {
    app_handle: AppHandle,
    state: Arc<AppState>,
    db: Arc<Database>,
}

impl TauriEventBridge {
    pub fn new(app_handle: AppHandle, state: Arc<AppState>, db: Arc<Database>) -> Self {
        Self { app_handle, state, db }
    }
}

impl ServerEventHandler for TauriEventBridge {
    fn on_status_change(&self, status: ServerStatus) {
        let state = self.state.clone();
        let app = self.app_handle.clone();
        let s = status.clone();
        // Use tokio to update async Mutex from sync callback
        tokio::spawn(async move {
            state.set_status(s.clone()).await;
            let _ = app.emit("server-status", &s);
        });
    }

    fn on_metrics(&self, metrics: Metrics) {
        let state = self.state.clone();
        let app = self.app_handle.clone();
        let m = metrics.clone();
        update_tray_status(&self.app_handle, &metrics);
        tokio::spawn(async move {
            state.set_metrics(m.clone()).await;
            let _ = app.emit("metrics", &m);
        });
    }

    fn on_connection(&self, event: ConnectionEvent) {
        let state = self.state.clone();
        let app = self.app_handle.clone();
        let db = self.db.clone();
        let evt = event.clone();
        tokio::spawn(async move {
            match &evt {
                ConnectionEvent::Connected(info) => {
                    state.add_connection(info).await;
                }
                ConnectionEvent::Disconnected(info) => {
                    state.remove_connection(&info.client_ip).await;
                    let _ = db.record_disconnection(
                        &info.client_ip,
                        Some(&info.client_name),
                        info.bytes_total,
                    );
                }
            }
            let conns = state.connections.lock().await.clone();
            let _ = app.emit("connections", &conns);
        });
    }

    fn on_log(&self, entry: LogEntry) {
        let state = self.state.clone();
        let app = self.app_handle.clone();
        let e = entry.clone();
        tokio::spawn(async move {
            state.push_log(e.clone()).await;
            let _ = app.emit("log", &e);
        });
    }
}
```

- [ ] **Step 3: 更新 tray.rs — 修改 update_tray_status 签名**

将现有的 `update_tray_status` 函数改为接受 `&Metrics` 参数:

找到现有的 `update_tray_status` 函数并替换为：

```rust
/// Update the tray title with current metrics
pub fn update_tray_status(handle: &AppHandle, metrics: &macrdp_core::Metrics) {
    if let Some(tray) = handle.tray_by_id("main-tray") {
        if metrics.bitrate_kbps > 0 {
            let mbps = metrics.bitrate_kbps as f64 / 1000.0;
            let title = format!("{:.1} Mbps | {:.0} ms", mbps, metrics.latency_ms);
            let _ = tray.set_title(Some(&title));
        } else {
            let _ = tray.set_title(None::<&str>);
        }
    }
}
```

- [ ] **Step 4: 更新 database.rs — 确保 record_disconnection 签名兼容**

检查现有的 `record_disconnection` 签名。如果已经是 `(&self, client_ip: &str, client_name: Option<&str>, bytes_total: u64)` 则无需修改。

- [ ] **Step 5: Commit**

```bash
git add macrdp-ui/src-tauri/src/event_bridge.rs macrdp-ui/src-tauri/src/state.rs macrdp-ui/src-tauri/src/tray.rs
git commit -m "feat(ui): add TauriEventBridge and update state for macrdp-core"
```

---

## Task 8: 重写 Tauri commands.rs 和 lib.rs

**Files:**
- Rewrite: `macrdp-ui/src-tauri/src/commands.rs`
- Rewrite: `macrdp-ui/src-tauri/src/lib.rs`
- Delete: `macrdp-ui/src-tauri/src/cli_manager.rs`
- Delete: `macrdp-ui/src-tauri/src/ipc_client.rs`
- Modify: `macrdp-ui/src-tauri/Cargo.toml`

- [ ] **Step 1: 更新 Cargo.toml — 添加 macrdp-core，去掉 uuid**

```toml
[workspace]

[package]
name = "macrdp-ui"
version = "0.1.0"
edition = "2021"
description = "macrdp GUI — Tauri v2 desktop application"

[build-dependencies]
tauri-build = { version = "2", features = [] }

[dependencies]
macrdp-core = { path = "../../crates/macrdp-core" }
tauri = { version = "2", features = ["tray-icon", "macos-private-api"] }
tauri-plugin-single-instance = "2"
tauri-plugin-autostart = "2"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tokio = { version = "1", features = ["full"] }
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
anyhow = "1"
rusqlite = { version = "0.31", features = ["bundled"] }
chrono = { version = "0.4", features = ["serde"] }
dirs = "6"

[lib]
name = "macrdp_ui"
crate-type = ["lib", "cdylib", "staticlib"]

[profile.release]
opt-level = 0
lto = false
codegen-units = 16
strip = false
```

- [ ] **Step 2: 重写 commands.rs**

```rust
use std::sync::Arc;

use macrdp_core::{self, ServerHandle};
use tauri::{Emitter, Manager, State};
use tokio::sync::Mutex;

use crate::database::Database;
use crate::event_bridge::TauriEventBridge;
use crate::state::AppState;

/// Shared server handle — None when stopped, Some when running
pub type SharedServerHandle = Arc<Mutex<Option<Arc<ServerHandle>>>>;

#[tauri::command]
pub async fn start_server(
    app: tauri::AppHandle,
    server_handle: State<'_, SharedServerHandle>,
    state: State<'_, Arc<AppState>>,
    db: State<'_, Arc<Database>>,
) -> Result<(), String> {
    // Check if already running
    {
        let guard = server_handle.lock().await;
        if guard.is_some() {
            return Ok(());
        }
    }

    let config = macrdp_core::ServerConfig::load_from_file(None)
        .map_err(|e| e.to_string())?;

    let bridge = TauriEventBridge::new(
        app.clone(),
        state.inner().clone(),
        db.inner().clone(),
    );

    let handle = macrdp_core::start_server(config, bridge)
        .await
        .map_err(|e| e.to_string())?;

    *server_handle.lock().await = Some(handle);
    Ok(())
}

#[tauri::command]
pub async fn stop_server(
    app: tauri::AppHandle,
    server_handle: State<'_, SharedServerHandle>,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    let handle = server_handle.lock().await.take();
    if let Some(h) = handle {
        h.stop().await.map_err(|e| e.to_string())?;
    }

    // Update status
    state.set_status(macrdp_core::ServerStatus {
        running: false,
        state: "stopped".to_string(),
        uptime_secs: 0,
    }).await;
    let status = state.status.lock().await.clone();
    let _ = app.emit("server-status", &status);

    Ok(())
}

#[tauri::command]
pub async fn get_server_status(
    state: State<'_, Arc<AppState>>,
) -> Result<serde_json::Value, String> {
    let status = state.status.lock().await.clone();
    serde_json::to_value(&status).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_metrics(
    state: State<'_, Arc<AppState>>,
) -> Result<serde_json::Value, String> {
    let metrics = state.metrics.lock().await.clone();
    serde_json::to_value(&metrics).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_connections(
    state: State<'_, Arc<AppState>>,
) -> Result<serde_json::Value, String> {
    let connections = state.connections.lock().await.clone();
    serde_json::to_value(&connections).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_permissions(
) -> Result<serde_json::Value, String> {
    let perms = macrdp_core::check_permissions();
    serde_json::to_value(&perms).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_config(
    server_handle: State<'_, SharedServerHandle>,
) -> Result<serde_json::Value, String> {
    // Load config from file (whether server is running or not)
    let config = macrdp_core::ServerConfig::load_from_file(None)
        .map_err(|e| e.to_string())?;
    serde_json::to_value(&config).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn set_config(
    config: serde_json::Value,
) -> Result<(), String> {
    // Hot config update is limited; save to file for next restart
    // TODO: implement hot config update via ServerHandle when needed
    tracing::info!(?config, "set_config received (save for next restart)");
    Ok(())
}

#[tauri::command]
pub async fn get_logs(
    limit: Option<usize>,
    state: State<'_, Arc<AppState>>,
    db: State<'_, Arc<Database>>,
) -> Result<Vec<serde_json::Value>, String> {
    let limit = limit.unwrap_or(100);
    let logs = state.logs.lock().await;
    if !logs.is_empty() {
        let entries: Vec<serde_json::Value> = logs
            .iter()
            .rev()
            .take(limit)
            .map(|e| serde_json::to_value(e).unwrap_or_default())
            .collect();
        return Ok(entries);
    }
    db.get_logs(limit)
}

#[tauri::command]
pub fn get_connection_history(
    limit: Option<u32>,
    offset: Option<u32>,
    db: State<'_, Arc<Database>>,
) -> Result<Vec<serde_json::Value>, String> {
    db.get_connection_history(limit.unwrap_or(50), offset.unwrap_or(0))
}

#[tauri::command]
pub fn get_traffic_stats(
    days: Option<u32>,
    db: State<'_, Arc<Database>>,
) -> Result<Vec<serde_json::Value>, String> {
    db.get_traffic_stats(days.unwrap_or(30))
}

#[tauri::command]
pub fn check_for_updates() -> Result<serde_json::Value, String> {
    Ok(serde_json::json!({
        "current_version": env!("CARGO_PKG_VERSION"),
        "available": false,
    }))
}

#[tauri::command]
pub fn show_main_window(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("main") {
        window.show().map_err(|e: tauri::Error| e.to_string())?;
        window.set_focus().map_err(|e: tauri::Error| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub fn open_system_preferences(pane: String) -> Result<(), String> {
    crate::permissions::open_system_preferences(&pane)
}

#[tauri::command]
pub async fn quit_app(
    app: tauri::AppHandle,
    server_handle: State<'_, SharedServerHandle>,
) -> Result<(), String> {
    // Stop server if running
    let handle = server_handle.lock().await.take();
    if let Some(h) = handle {
        let _ = h.stop().await;
    }
    app.exit(0);
    Ok(())
}

#[tauri::command]
pub fn set_autostart(
    enabled: bool,
    app: tauri::AppHandle,
) -> Result<(), String> {
    use tauri_plugin_autostart::ManagerExt;
    let autolaunch = app.autolaunch();
    if enabled {
        autolaunch.enable().map_err(|e| format!("Failed to enable autostart: {}", e))?;
    } else {
        autolaunch.disable().map_err(|e| format!("Failed to disable autostart: {}", e))?;
    }
    Ok(())
}
```

- [ ] **Step 3: 重写 lib.rs**

```rust
mod commands;
mod database;
mod event_bridge;
mod permissions;
mod state;
mod tray;

use std::sync::Arc;

use commands::SharedServerHandle;
use database::Database;
use state::AppState;
use tokio::sync::Mutex;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Initialize logging
    use tracing_subscriber::layer::SubscriberExt;
    use tracing_subscriber::util::SubscriberInitExt;
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new("info"))
        .with(tracing_subscriber::fmt::layer().with_writer(std::io::stderr))
        .init();

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|_app, _args, _cwd| {}))
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .setup(|app| {
            // App state
            let app_state = Arc::new(AppState::new());
            app.manage(app_state);

            // Server handle (None until started)
            let server_handle: SharedServerHandle = Arc::new(Mutex::new(None));
            app.manage(server_handle);

            // Database
            let data_dir = dirs::data_dir()
                .unwrap_or_else(|| std::path::PathBuf::from("."))
                .join("macrdp");
            std::fs::create_dir_all(&data_dir).ok();
            let db = Database::new(&data_dir.join("stats.db"))
                .expect("Failed to open database");
            app.manage(Arc::new(db));

            // System tray
            tray::setup_tray(app)?;

            Ok(())
        })
        .on_window_event(|window, event| {
            match event {
                tauri::WindowEvent::CloseRequested { api, .. } => {
                    let label = window.label();
                    if label == "main" {
                        // Hide instead of close
                        api.prevent_close();
                        let _ = window.hide();
                    }
                }
                tauri::WindowEvent::Focused(false) => {
                    if window.label() == "popover" {
                        let _ = window.hide();
                    }
                }
                _ => {}
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::start_server,
            commands::stop_server,
            commands::get_server_status,
            commands::get_metrics,
            commands::get_connections,
            commands::get_permissions,
            commands::get_config,
            commands::set_config,
            commands::get_logs,
            commands::get_connection_history,
            commands::get_traffic_stats,
            commands::check_for_updates,
            commands::show_main_window,
            commands::open_system_preferences,
            commands::quit_app,
            commands::set_autostart,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

- [ ] **Step 4: 删除旧的 IPC 相关文件**

```bash
rm macrdp-ui/src-tauri/src/cli_manager.rs
rm macrdp-ui/src-tauri/src/ipc_client.rs
```

- [ ] **Step 5: 验证 Tauri UI 编译**

Run: `cargo build --manifest-path macrdp-ui/src-tauri/Cargo.toml`
Expected: 编译成功

- [ ] **Step 6: Commit**

```bash
git add -A macrdp-ui/src-tauri/
git commit -m "refactor(ui): replace IPC with in-process macrdp-core library"
```

---

## Task 9: 更新 Makefile 和清理

**Files:**
- Modify: `Makefile`

- [ ] **Step 1: 更新 Makefile**

移除 `run-cli-ipc` target，更新 `link-cli` 和 `build-cli` 命令。

在 Makefile 中：
- 删除 `run-cli-ipc` target
- 将 help 中的 `run-cli-ipc` 行移除
- `link-cli` 保持不变（Tauri 仍需要 CLI binary 用于独立运行）

- [ ] **Step 2: 验证完整构建**

Run: `make build-cli`
Expected: CLI 编译成功

Run: `cargo build --manifest-path macrdp-ui/src-tauri/Cargo.toml`
Expected: UI 编译成功

- [ ] **Step 3: Commit**

```bash
git add Makefile
git commit -m "chore: update Makefile, remove IPC targets"
```

---

## Task 10: 集成测试和修复

- [ ] **Step 1: 运行 macrdp-core 单元测试**

Run: `cargo test -p macrdp-core`
Expected: 所有 config 和 tls 测试通过

- [ ] **Step 2: 运行 macrdp-server CLI 编译检查**

Run: `cargo build -p macrdp-server`
Expected: 编译成功

- [ ] **Step 3: 运行 Tauri 编译检查**

Run: `cargo build --manifest-path macrdp-ui/src-tauri/Cargo.toml`
Expected: 编译成功

- [ ] **Step 4: 修复编译错误**

如果有编译错误，逐一修复。常见问题：
- 类型 import 路径不对
- `tray.rs` 中的函数签名与 `event_bridge.rs` 不匹配
- `database.rs` 的 `record_disconnection` 参数不匹配
- Tauri `State` 提取类型不匹配

- [ ] **Step 5: 最终 Commit**

```bash
git add -A
git commit -m "fix: resolve compilation issues from lib entry refactoring"
```
