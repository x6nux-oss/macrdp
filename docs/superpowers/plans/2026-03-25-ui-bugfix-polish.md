# macrdp UI Bug 修复与美化 实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 修复 UI 配置系统、端口冲突处理、组件风格统一（shadcn/ui）、指标扩展、权限优化。

**Architecture:** Tauri v2 后端新增 `UiConfig` 模块处理独立配置文件读写；macrdp-core 扩展 `Metrics` 结构体并添加端口自动选择；前端引入 shadcn/ui 组件库 + 浅色/深色双主题，逐页重写。

**Tech Stack:** Rust (Tauri v2, macrdp-core), React 19, TypeScript, Tailwind CSS v4, shadcn/ui, Lucide React

**Spec:** `docs/superpowers/specs/2026-03-25-ui-bugfix-polish-design.md`

---

## File Structure

### 新建文件
- `macrdp-ui/src-tauri/src/ui_config.rs` — UiConfig 结构体、TOML 读写、From 转换
- `macrdp-ui/src/contexts/ThemeContext.tsx` — 主题 React Context
- `macrdp-ui/src/components/ThemeToggle.tsx` — 右下角主题切换按钮
- `macrdp-ui/src/components/PermissionBanner.tsx` — 全局权限警告横幅
- `macrdp-ui/src/components/MetricsStrip.tsx` — 仪表盘指标长条
- `macrdp-ui/src/components/ServerInfoTags.tsx` — 服务器信息标签栏
- `macrdp-ui/src/components/ui/` — shadcn/ui 生成的组件目录
- `macrdp-ui/src/lib/utils.ts` — shadcn cn() 辅助函数
- `macrdp-ui/components.json` — shadcn/ui 配置

### 修改文件
- `crates/macrdp-core/src/callbacks.rs` — Metrics 扩展（+5 字段 + serde rename）
- `crates/macrdp-core/src/server.rs` — metrics 采集新字段 + 端口自动选择
- `macrdp-ui/src-tauri/src/lib.rs` — 注册 ui_config 模块、启动时请求权限
- `macrdp-ui/src-tauri/src/commands.rs` — 重写 get_config/set_config
- `macrdp-ui/src-tauri/src/state.rs` — AppState 增加 UiConfig
- `macrdp-ui/src-tauri/src/event_bridge.rs` — 适配新 Metrics 字段（自动兼容，无需改代码）
- `macrdp-ui/src-tauri/Cargo.toml` — 可能需要加 toml 依赖
- `macrdp-ui/package.json` — shadcn/ui 依赖
- `macrdp-ui/src/styles/globals.css` — 双主题 CSS 变量
- `macrdp-ui/src/lib/types.ts` — Metrics/UiConfig TypeScript 类型
- `macrdp-ui/src/lib/ipc.ts` — 更新 api 接口
- `macrdp-ui/src/App.tsx` — ThemeProvider 包裹 + PermissionBanner
- `macrdp-ui/src/components/Sidebar.tsx` — Lucide 图标 + 权限红点
- `macrdp-ui/src/pages/Dashboard.tsx` — MetricsStrip + ServerInfoTags
- `macrdp-ui/src/pages/Settings.tsx` — shadcn 控件 + 真正持久化
- `macrdp-ui/src/pages/Permissions.tsx` — 轮询频率调整
- `macrdp-ui/src/pages/Logs.tsx` — shadcn 组件替换
- `macrdp-ui/src/pages/Statistics.tsx` — shadcn 组件替换
- `macrdp-ui/src/pages/About.tsx` — shadcn 组件替换
- `macrdp-ui/src/pages/Popover.tsx` — 适配新 Metrics 字段 + Lucide 图标

---

## Task 1: UiConfig 后端模块

**Files:**
- Create: `macrdp-ui/src-tauri/src/ui_config.rs`
- Modify: `macrdp-ui/src-tauri/src/lib.rs`
- Modify: `macrdp-ui/src-tauri/src/state.rs`
- Modify: `macrdp-ui/src-tauri/src/commands.rs`
- Modify: `macrdp-ui/src-tauri/Cargo.toml`

- [ ] **Step 1: 检查 Cargo.toml 依赖**

Run: `grep -E "^(toml|dirs)" macrdp-ui/src-tauri/Cargo.toml`
`dirs` 已存在（v6），无需添加。如果 `toml` 不存在，添加 `toml = "0.8"` 到 `[dependencies]`。

- [ ] **Step 2: 创建 ui_config.rs — UiConfig 结构体和默认值**

Create `macrdp-ui/src-tauri/src/ui_config.rs`:

```rust
//! Independent UI configuration, stored at ~/.macrdp/config-ui.toml

use std::path::PathBuf;
use serde::{Deserialize, Serialize};

/// UI configuration — all fields have concrete defaults (no Option types).
/// Stored separately from CLI's config.toml at ~/.macrdp/config-ui.toml.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UiConfig {
    // RDP service
    pub port: u16,
    pub frame_rate: u32,
    pub bitrate_mbps: u32,
    pub encoder: String,
    pub chroma_mode: String,
    // Network
    pub bind_address: String,
    pub max_connections: u32,
    pub idle_timeout_secs: u64,
    // Auth
    pub username: String,
    pub password: String,
    // Display
    pub hidpi_scale: u32,
    pub show_cursor: bool,
    // General
    pub log_level: String,
    // UI-only
    pub theme: String,
    pub autostart: bool,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            port: 3389,
            frame_rate: 60,
            bitrate_mbps: 50,
            encoder: "auto".into(),
            chroma_mode: "avc420".into(),
            bind_address: "0.0.0.0".into(),
            max_connections: 3,
            idle_timeout_secs: 1800,
            username: "macrdp".into(),
            password: String::new(),
            hidpi_scale: 2,
            show_cursor: false,
            log_level: "warn".into(),
            theme: "system".into(),
            autostart: false,
        }
    }
}

/// Returns the UI config file path: ~/.macrdp/config-ui.toml
pub fn config_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".macrdp")
        .join("config-ui.toml")
}

impl UiConfig {
    /// Load from ~/.macrdp/config-ui.toml. Creates default if not found.
    pub fn load() -> Result<Self, String> {
        let path = config_path();
        if path.exists() {
            let content = std::fs::read_to_string(&path)
                .map_err(|e| format!("Failed to read config: {e}"))?;
            toml::from_str(&content)
                .map_err(|e| format!("Failed to parse config: {e}"))
        } else {
            let config = Self::default();
            // Create default config file
            let _ = config.save();
            Ok(config)
        }
    }

    /// Save to ~/.macrdp/config-ui.toml
    pub fn save(&self) -> Result<(), String> {
        let path = config_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create config dir: {e}"))?;
        }
        let content = toml::to_string_pretty(self)
            .map_err(|e| format!("Failed to serialize config: {e}"))?;
        std::fs::write(&path, content)
            .map_err(|e| format!("Failed to write config: {e}"))
    }

    /// Update a single field by key. Returns true if the field requires server restart.
    pub fn set_field(&mut self, key: &str, value: serde_json::Value) -> Result<bool, String> {
        let restart_required = match key {
            "port" => {
                self.port = serde_json::from_value(value).map_err(|e| e.to_string())?;
                true
            }
            "frame_rate" => {
                self.frame_rate = serde_json::from_value(value).map_err(|e| e.to_string())?;
                false // hot-updatable
            }
            "bitrate_mbps" => {
                self.bitrate_mbps = serde_json::from_value(value).map_err(|e| e.to_string())?;
                false // hot-updatable
            }
            "encoder" => {
                self.encoder = serde_json::from_value(value).map_err(|e| e.to_string())?;
                true
            }
            "chroma_mode" => {
                self.chroma_mode = serde_json::from_value(value).map_err(|e| e.to_string())?;
                true
            }
            "bind_address" => {
                self.bind_address = serde_json::from_value(value).map_err(|e| e.to_string())?;
                true
            }
            "max_connections" => {
                self.max_connections = serde_json::from_value(value).map_err(|e| e.to_string())?;
                true
            }
            "idle_timeout_secs" => {
                self.idle_timeout_secs = serde_json::from_value(value).map_err(|e| e.to_string())?;
                true
            }
            "username" => {
                self.username = serde_json::from_value(value).map_err(|e| e.to_string())?;
                true
            }
            "password" => {
                self.password = serde_json::from_value(value).map_err(|e| e.to_string())?;
                true
            }
            "hidpi_scale" => {
                self.hidpi_scale = serde_json::from_value(value).map_err(|e| e.to_string())?;
                true
            }
            "show_cursor" => {
                self.show_cursor = serde_json::from_value(value).map_err(|e| e.to_string())?;
                true
            }
            "log_level" => {
                self.log_level = serde_json::from_value(value).map_err(|e| e.to_string())?;
                false // hot-updatable
            }
            "theme" => {
                self.theme = serde_json::from_value(value).map_err(|e| e.to_string())?;
                false
            }
            "autostart" => {
                self.autostart = serde_json::from_value(value).map_err(|e| e.to_string())?;
                false
            }
            _ => return Err(format!("Unknown config key: {key}")),
        };
        Ok(restart_required)
    }

    /// Convert to macrdp_core::ServerConfig for starting the server.
    pub fn to_server_config(&self) -> macrdp_core::ServerConfig {
        macrdp_core::ServerConfig {
            port: self.port,
            frame_rate: self.frame_rate,
            width: 0,  // auto-detect
            height: 0, // auto-detect
            username: Some(self.username.clone()),
            password: if self.password.is_empty() { None } else { Some(self.password.clone()) },
            cert_path: None,
            key_path: None,
            idle_timeout_secs: self.idle_timeout_secs,
            log_level: Some(self.log_level.clone()),
            quality: None, // derived from other settings
            encoder: Some(self.encoder.clone()),
            chroma_mode: Some(self.chroma_mode.clone()),
            hidpi_scale: Some(self.hidpi_scale),
            bitrate_mbps: Some(self.bitrate_mbps),
            show_cursor: Some(self.show_cursor),
        }
    }
}
```

- [ ] **Step 3: 更新 state.rs — AppState 增加 UiConfig**

在 `macrdp-ui/src-tauri/src/state.rs` 中：
- 在 `AppState` 结构体添加 `pub ui_config: Mutex<UiConfig>`
- 在 `AppState::new()` 中调用 `UiConfig::load()` 初始化
- 添加 `use crate::ui_config::UiConfig;`

```rust
pub struct AppState {
    pub status: Mutex<ServerStatus>,
    pub metrics: Mutex<Metrics>,
    pub connections: Mutex<Vec<Connection>>,
    pub logs: Mutex<VecDeque<LogEntry>>,
    pub server_handle: SharedServerHandle,
    pub ui_config: Mutex<UiConfig>,
}

impl AppState {
    pub fn new() -> Self {
        let config = UiConfig::load().unwrap_or_default();
        Self {
            status: Mutex::new(ServerStatus::default()),
            metrics: Mutex::new(Metrics::default()),
            connections: Mutex::new(Vec::new()),
            logs: Mutex::new(VecDeque::new()),
            server_handle: Arc::new(Mutex::new(None)),
            ui_config: Mutex::new(config),
        }
    }
}
```

- [ ] **Step 4: 重写 commands.rs — get_config/set_config/start_server**

修改 `macrdp-ui/src-tauri/src/commands.rs`：

**get_config** — 从 AppState 中的 UiConfig 读取：
```rust
#[tauri::command]
pub async fn get_config(
    state: State<'_, Arc<AppState>>,
) -> Result<serde_json::Value, String> {
    let config = state.ui_config.lock().await.clone();
    serde_json::to_value(&config).map_err(|e| e.to_string())
}
```

**get_config 和 set_config** 都需要从 `pub fn` 改为 `pub async fn`，因为 `UiConfig` 存储在 `tokio::sync::Mutex` 中需要 `.await`。Tauri invoke_handler 自动支持异步命令，无需修改 `lib.rs` 中的注册代码。

**set_config** — 按字段更新 + 持久化 + 热更新下发 + 返回 restart_required：
```rust
#[derive(serde::Serialize)]
pub struct SetConfigResponse {
    pub restart_required: bool,
}

#[tauri::command]
pub async fn set_config(
    key: String,
    value: serde_json::Value,
    state: State<'_, Arc<AppState>>,
) -> Result<SetConfigResponse, String> {
    let restart_required;
    {
        let mut config = state.ui_config.lock().await;
        restart_required = config.set_field(&key, value.clone())?;
        config.save()?;
    }

    // Hot-update: if server is running and field supports it, dispatch ConfigUpdate
    if !restart_required {
        let guard = state.server_handle.lock().await;
        if let Some(ref handle) = *guard {
            let update = match key.as_str() {
                "frame_rate" => serde_json::from_value(value).ok()
                    .map(macrdp_core::ConfigUpdate::FrameRate),
                "bitrate_mbps" => serde_json::from_value::<u32>(value).ok()
                    .map(|v| macrdp_core::ConfigUpdate::BitrateKbps(v * 1000)),
                "log_level" => serde_json::from_value(value).ok()
                    .map(macrdp_core::ConfigUpdate::LogLevel),
                _ => None,
            };
            if let Some(update) = update {
                handle.update_config(update);
            }
        }
    }

    Ok(SetConfigResponse { restart_required })
}
```

> **注意**: `ServerHandle::update_config()` 方法需要在 macrdp-core 中存在。检查现有代码是否已暴露该接口，如果没有，需要在 Task 2 中添加。`ConfigUpdate` 枚举已在 `callbacks.rs` 中定义（`FrameRate`, `BitrateKbps`, `LogLevel`）。

**start_server** — 从 UiConfig 加载：
```rust
// 替换 let config = macrdp_core::ServerConfig::load_from_file(None)... 为：
let server_config = {
    let ui_config = state.ui_config.lock().await;
    ui_config.to_server_config()
};
let handle = macrdp_core::start_server(server_config, bridge)
    .await
    .map_err(|e| format!("Failed to start server: {}", e))?;
```

- [ ] **Step 5: 注册 ui_config 模块**

在 `macrdp-ui/src-tauri/src/lib.rs` 第 1 行添加：
```rust
mod ui_config;
```

- [ ] **Step 6: 编译验证**

Run: `cd macrdp-ui && cargo build --release -p macrdp-ui 2>&1 | tail -20`
Expected: 编译成功，无错误

- [ ] **Step 7: Commit**

```bash
git add macrdp-ui/src-tauri/src/ui_config.rs macrdp-ui/src-tauri/src/commands.rs macrdp-ui/src-tauri/src/state.rs macrdp-ui/src-tauri/src/lib.rs macrdp-ui/src-tauri/Cargo.toml
git commit -m "feat(ui): add independent UiConfig with persistent storage at ~/.macrdp/"
```

---

## Task 2: Metrics 扩展（macrdp-core）

**Files:**
- Modify: `crates/macrdp-core/src/callbacks.rs:24-31`
- Modify: `crates/macrdp-core/src/server.rs:56-67` (ServerHandle::metrics)
- Modify: `crates/macrdp-core/src/server.rs:226-250` (metrics task)

- [ ] **Step 1: 扩展 Metrics 结构体**

在 `crates/macrdp-core/src/callbacks.rs` 修改 `Metrics`：

```rust
/// Real-time performance metrics
#[derive(Debug, Clone, Serialize, Default)]
pub struct Metrics {
    pub fps: u32,
    pub bitrate_kbps: u64,
    #[serde(rename = "latency_ms")]
    pub rtt_ms: f64,
    pub bytes_sent: u64,
    pub timestamp: u64,
    // New fields
    pub encode_ms: f64,
    pub net_ms: f64,
    pub last_frame_bytes: u32,
    pub network_quality: f32,
    pub pending_acks: u32,
}
```

- [ ] **Step 2: 更新 ServerHandle::metrics()**

在 `crates/macrdp-core/src/server.rs` 修改 `metrics()` 方法：

```rust
pub fn metrics(&self) -> Metrics {
    let gfx = self.gfx_state.lock().unwrap();
    let net_ms = (gfx.rtt_ewma_ms - gfx.last_encode_ms).max(0.0);
    Metrics {
        fps: 0,
        bitrate_kbps: 0,
        rtt_ms: gfx.rtt_ewma_ms,
        bytes_sent: gfx.total_bytes_sent,
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
        encode_ms: gfx.last_encode_ms,
        net_ms,
        last_frame_bytes: gfx.last_frame_bytes,
        network_quality: gfx.network_quality,
        pending_acks: gfx.pending_acks,
    }
}
```

- [ ] **Step 3: 更新 metrics 定时任务**

在 `crates/macrdp-core/src/server.rs` 修改 metrics task（约第 226-250 行）：

```rust
let metrics_task = tokio::spawn(async move {
    let mut interval = tokio::time::interval(std::time::Duration::from_secs(1));
    let mut prev_bytes_sent: u64 = 0;
    loop {
        interval.tick().await;
        let (rtt_ms, bytes_sent, encode_ms, last_frame_bytes, network_quality, pending_acks) = {
            let gfx = metrics_gfx.lock().unwrap();
            (
                gfx.rtt_ewma_ms,
                gfx.total_bytes_sent,
                gfx.last_encode_ms,
                gfx.last_frame_bytes,
                gfx.network_quality,
                gfx.pending_acks,
            )
        };
        let delta_bytes = bytes_sent.saturating_sub(prev_bytes_sent);
        prev_bytes_sent = bytes_sent;
        let bitrate_kbps = delta_bytes * 8 / 1000;
        let net_ms = (rtt_ms - encode_ms).max(0.0);
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        handler_for_metrics.on_metrics(Metrics {
            fps,
            bitrate_kbps,
            rtt_ms,
            bytes_sent,
            timestamp,
            encode_ms,
            net_ms,
            last_frame_bytes,
            network_quality,
            pending_acks,
        });
    }
});
```

- [ ] **Step 4: 修复编译 — 更新所有 latency_ms 引用**

搜索项目中所有 `latency_ms` 引用（除 serde rename），改为 `rtt_ms`：

Run: `rg "latency_ms" crates/macrdp-core/src/ --files-with-matches`

预计需要修改 `server.rs` 中的直接赋值。由于使用了 `#[serde(rename)]`，序列化输出仍为 `latency_ms`，前端无需改动。

- [ ] **Step 5: 编译验证**

Run: `cargo build -p macrdp-core 2>&1 | tail -10`
Expected: 编译成功

- [ ] **Step 6: 运行现有测试**

Run: `cargo test -p macrdp-core 2>&1 | tail -10`
Expected: 所有测试通过

- [ ] **Step 7: Commit**

```bash
git add crates/macrdp-core/src/callbacks.rs crates/macrdp-core/src/server.rs
git commit -m "feat(core): expand Metrics with encode_ms, net_ms, network_quality, pending_acks"
```

---

## Task 3: 端口冲突检测（macrdp-core）

**Files:**
- Modify: `crates/macrdp-core/src/server.rs:128-270` (start_server 函数)

- [ ] **Step 1: 添加端口探测函数**

在 `crates/macrdp-core/src/server.rs` 中 `start_server` 函数之前添加：

```rust
use std::net::TcpListener;

/// Try to bind to the given address. If the port is in use, try the next 99 ports.
/// Returns the successfully bound TcpListener and the actual port.
fn find_available_port(addr: &str, preferred_port: u16) -> Result<(TcpListener, u16)> {
    for offset in 0..100u16 {
        let port = preferred_port.checked_add(offset)
            .context("Port number overflow")?;
        let bind_addr = format!("{addr}:{port}");
        match TcpListener::bind(&bind_addr) {
            Ok(listener) => {
                if offset > 0 {
                    tracing::warn!(
                        preferred = preferred_port,
                        actual = port,
                        "Preferred port in use, using alternative"
                    );
                }
                return Ok((listener, port));
            }
            Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => {
                tracing::debug!(port, "Port in use, trying next");
                continue;
            }
            Err(e) => {
                return Err(e).context(format!("Failed to bind to {bind_addr}"));
            }
        }
    }
    anyhow::bail!(
        "No available port found in range {}-{}",
        preferred_port,
        preferred_port + 99
    )
}
```

- [ ] **Step 2: 修改 start_server 使用 find_available_port**

在 `start_server()` 中，替换现有的 `bind_addr` 构造（约第 188 行）：

```rust
// 替换:
// let bind_addr: SocketAddr = format!("0.0.0.0:{}", config.port).parse()?;
// let port = config.port;

// 为:
let (listener, port) = find_available_port("0.0.0.0", config.port)?;
let bind_addr = listener.local_addr()?;
// Drop listener so the RDP server can rebind the port.
drop(listener);
```

> **已知偏差（Spec vs 实现）**: Spec 要求"绑定和使用是同一个 listener"以消除 TOCTOU，但 IronRDP 的 `RdpServer::with_addr()` 自己绑定端口，无法传入已有 TcpListener。因此采用 probe-then-release 方案，TOCTOU 窗口为微秒级，在局域网本地使用场景下风险可接受。未来如 ironrdp-server fork 支持传入 listener，可完全消除此问题。

- [ ] **Step 3: 更新 ServerHandle 构造使用实际端口**

确保 `ServerHandle { port, ... }` 使用 `find_available_port` 返回的实际 `port`（而非 `config.port`）。检查当前代码已经用 `port` 局部变量构造，确认无误即可。

- [ ] **Step 4: 编译验证**

Run: `cargo build -p macrdp-core 2>&1 | tail -10`
Expected: 编译成功

- [ ] **Step 5: Commit**

```bash
git add crates/macrdp-core/src/server.rs
git commit -m "feat(core): add port conflict detection with automatic fallback"
```

---

## Task 4: 权限优化（后端）

**Files:**
- Modify: `macrdp-ui/src-tauri/src/lib.rs:34-75` (setup 钩子)

- [ ] **Step 1: 在 setup 钩子中添加启动时权限请求**

在 `macrdp-ui/src-tauri/src/lib.rs` 的 `.setup(|app| { ... })` 中，`state::init_app_state(&handle)?;` 之后添加：

```rust
// Request permissions proactively on startup
std::thread::spawn(|| {
    let perms = macrdp_core::permissions::request_permissions();
    tracing::info!(?perms, "Startup permission request completed");
});
```

使用 `std::thread::spawn` 是因为 `request_permissions()` 可能触发系统对话框（阻塞），不应阻塞 Tauri 启动。

- [ ] **Step 2: 编译验证**

Run: `cd macrdp-ui && cargo build -p macrdp-ui 2>&1 | tail -10`
Expected: 编译成功

- [ ] **Step 3: Commit**

```bash
git add macrdp-ui/src-tauri/src/lib.rs
git commit -m "feat(ui): request permissions proactively on app startup"
```

---

## Task 5: shadcn/ui 初始化

**Files:**
- Create: `macrdp-ui/components.json`
- Create: `macrdp-ui/src/lib/utils.ts`
- Create: `macrdp-ui/src/components/ui/` (多个组件文件)
- Modify: `macrdp-ui/package.json`
- Modify: `macrdp-ui/src/styles/globals.css`
- Modify: `macrdp-ui/tailwind.config.ts`

- [ ] **Step 1: 安装 shadcn/ui**

Run: `cd macrdp-ui && npx shadcn@latest init`

选择：
- Style: Default
- Base color: Zinc
- CSS variables: Yes
- Tailwind CSS v4: Yes (auto-detected)
- Components directory: src/components/ui
- Utils: src/lib/utils.ts

- [ ] **Step 2: 安装所需 shadcn 组件**

Run: `cd macrdp-ui && npx shadcn@latest add button input select switch badge tooltip alert separator card`

- [ ] **Step 3: 更新 globals.css — 双主题 CSS 变量**

重写 `macrdp-ui/src/styles/globals.css`，在 shadcn 生成的基础上添加 macOS 原生色彩变量。保留 shadcn 的 `@layer base` 变量，额外添加：

```css
@theme {
  /* macOS native colors — light */
  --color-macos-bg: #f5f5f7;
  --color-macos-card: rgba(255, 255, 255, 0.72);
  --color-macos-text: #1d1d1f;
  --color-macos-secondary: #86868b;
  --color-macos-muted: #aeaeb2;
  --color-macos-border: rgba(0, 0, 0, 0.08);
  --color-macos-blue: #007aff;
  --color-macos-green: #34c759;
  --color-macos-red: #ff3b30;
  --color-macos-orange: #ff9500;
}

/* Dark mode overrides */
.dark, [data-theme="dark"] {
  --color-macos-bg: #09090b;
  --color-macos-card: rgba(24, 24, 27, 0.85);
  --color-macos-text: #fafafa;
  --color-macos-secondary: #a1a1aa;
  --color-macos-muted: #71717a;
  --color-macos-border: #27272a;
  --color-macos-blue: #0a84ff;
  --color-macos-green: #30d158;
  --color-macos-red: #ff453a;
  --color-macos-orange: #ff9f0a;
}

@media (prefers-color-scheme: dark) {
  :root:not([data-theme="light"]) {
    --color-macos-bg: #09090b;
    --color-macos-card: rgba(24, 24, 27, 0.85);
    --color-macos-text: #fafafa;
    --color-macos-secondary: #a1a1aa;
    --color-macos-muted: #71717a;
    --color-macos-border: #27272a;
    --color-macos-blue: #0a84ff;
    --color-macos-green: #30d158;
    --color-macos-red: #ff453a;
    --color-macos-orange: #ff9f0a;
  }
}
```

- [ ] **Step 4: 验证前端构建**

Run: `cd macrdp-ui && npm run build 2>&1 | tail -10`
Expected: 构建成功

- [ ] **Step 5: Commit**

```bash
git add macrdp-ui/components.json macrdp-ui/src/lib/utils.ts macrdp-ui/src/components/ui/ macrdp-ui/package.json macrdp-ui/src/styles/globals.css macrdp-ui/tailwind.config.ts
git commit -m "feat(ui): initialize shadcn/ui with macOS native color theme"
```

---

## Task 6: 主题系统

**Files:**
- Create: `macrdp-ui/src/contexts/ThemeContext.tsx`
- Create: `macrdp-ui/src/components/ThemeToggle.tsx`
- Modify: `macrdp-ui/src/App.tsx`

- [ ] **Step 1: 创建 ThemeContext**

Create `macrdp-ui/src/contexts/ThemeContext.tsx`:

```tsx
import { createContext, useContext, useEffect, useState, type ReactNode } from "react";
import { api } from "../lib/ipc";

type Theme = "system" | "light" | "dark";

interface ThemeContextValue {
  theme: Theme;
  setTheme: (theme: Theme) => void;
}

const ThemeContext = createContext<ThemeContextValue>({
  theme: "system",
  setTheme: () => {},
});

export function ThemeProvider({ children }: { children: ReactNode }) {
  const [theme, setThemeState] = useState<Theme>("system");

  useEffect(() => {
    // Load theme from config on mount
    api.getConfig().then((config) => {
      const t = (config as Record<string, unknown>).theme as Theme;
      if (t && ["system", "light", "dark"].includes(t)) {
        setThemeState(t);
        applyTheme(t);
      }
    }).catch(() => {});
  }, []);

  const setTheme = (t: Theme) => {
    setThemeState(t);
    applyTheme(t);
    api.setConfig("theme", t).catch(console.error);
  };

  return (
    <ThemeContext.Provider value={{ theme, setTheme }}>
      {children}
    </ThemeContext.Provider>
  );
}

export function useTheme() {
  return useContext(ThemeContext);
}

function applyTheme(theme: Theme) {
  const root = document.documentElement;
  root.removeAttribute("data-theme");
  root.classList.remove("dark");

  if (theme === "dark") {
    root.setAttribute("data-theme", "dark");
    root.classList.add("dark");
  } else if (theme === "light") {
    root.setAttribute("data-theme", "light");
  }
  // "system" — let prefers-color-scheme handle it
}
```

- [ ] **Step 2: 创建 ThemeToggle 组件**

Create `macrdp-ui/src/components/ThemeToggle.tsx`:

```tsx
import { Monitor, Sun, Moon } from "lucide-react";
import { useTheme } from "../contexts/ThemeContext";
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from "./ui/tooltip";

const modes = ["system", "light", "dark"] as const;
const labels = { system: "跟随系统", light: "浅色模式", dark: "深色模式" };
const icons = { system: Monitor, light: Sun, dark: Moon };

export default function ThemeToggle() {
  const { theme, setTheme } = useTheme();

  const cycle = () => {
    const idx = modes.indexOf(theme);
    setTheme(modes[(idx + 1) % modes.length]);
  };

  const Icon = icons[theme];

  return (
    <TooltipProvider>
      <Tooltip>
        <TooltipTrigger asChild>
          <button
            onClick={cycle}
            className="fixed bottom-4 right-4 z-50 flex h-9 w-9 items-center justify-center rounded-[10px] border border-macos-border bg-macos-card backdrop-blur-xl shadow-md transition-transform hover:scale-105 active:scale-95"
          >
            <Icon className="h-[18px] w-[18px] text-macos-secondary" />
          </button>
        </TooltipTrigger>
        <TooltipContent side="left">
          <p>{labels[theme]}</p>
        </TooltipContent>
      </Tooltip>
    </TooltipProvider>
  );
}
```

- [ ] **Step 3: 包裹 App 根组件**

修改 `macrdp-ui/src/App.tsx`：

- 导入 `ThemeProvider` 和 `ThemeToggle`
- 在 `MainLayout` 中添加 `<ThemeToggle />`
- 在 `App` 组件最外层用 `<ThemeProvider>` 包裹

```tsx
import { ThemeProvider } from "./contexts/ThemeContext";
import ThemeToggle from "./components/ThemeToggle";

function MainLayout() {
  // ... 现有代码 ...
  return (
    <div className="flex h-screen w-screen overflow-hidden">
      <Sidebar />
      <main className="flex-1 overflow-auto bg-macos-bg p-6">
        <Routes>...</Routes>
      </main>
      <ThemeToggle />
    </div>
  );
}

function App() {
  return (
    <ThemeProvider>
      <Routes>
        <Route path="/popover" element={<Popover />} />
        <Route path="/*" element={<MainLayout />} />
      </Routes>
    </ThemeProvider>
  );
}
```

- [ ] **Step 4: 验证前端构建**

Run: `cd macrdp-ui && npm run build 2>&1 | tail -10`
Expected: 构建成功

- [ ] **Step 5: Commit**

```bash
git add macrdp-ui/src/contexts/ThemeContext.tsx macrdp-ui/src/components/ThemeToggle.tsx macrdp-ui/src/App.tsx
git commit -m "feat(ui): add light/dark theme system with auto-switch"
```

---

## Task 7: 前端类型和 IPC 更新

**Files:**
- Modify: `macrdp-ui/src/lib/types.ts`
- Modify: `macrdp-ui/src/lib/ipc.ts`

- [ ] **Step 1: 更新 types.ts**

```typescript
export interface Metrics {
  fps: number;
  bitrate_kbps: number;
  latency_ms: number;  // 保持 JSON 兼容名 (后端 serde rename)
  bytes_sent: number;
  timestamp: number;
  // New fields
  encode_ms: number;
  net_ms: number;
  last_frame_bytes: number;
  network_quality: number;
  pending_acks: number;
}

export interface UiConfig {
  port: number;
  frame_rate: number;
  bitrate_mbps: number;
  encoder: string;
  chroma_mode: string;
  bind_address: string;
  max_connections: number;
  idle_timeout_secs: number;
  username: string;
  password: string;
  hidpi_scale: number;
  show_cursor: boolean;
  log_level: string;
  theme: string;
  autostart: boolean;
}
```

替换原来的 `ServerConfig` 接口为 `UiConfig`。

- [ ] **Step 2: 更新 ipc.ts**

将 `getConfig` 返回类型改为 `UiConfig`：
```typescript
getConfig: () => invoke<UiConfig>("get_config"),
```

- [ ] **Step 3: 验证构建**

Run: `cd macrdp-ui && npm run build 2>&1 | tail -10`
Expected: 可能有类型错误（其他页面引用了旧的 ServerConfig），记录但先继续。

- [ ] **Step 4: Commit**

```bash
git add macrdp-ui/src/lib/types.ts macrdp-ui/src/lib/ipc.ts
git commit -m "feat(ui): update TypeScript types for expanded Metrics and UiConfig"
```

---

## Task 8: 侧边栏 + 权限横幅

**Files:**
- Create: `macrdp-ui/src/components/PermissionBanner.tsx`
- Modify: `macrdp-ui/src/components/Sidebar.tsx`
- Modify: `macrdp-ui/src/App.tsx`

- [ ] **Step 1: 创建 PermissionBanner 组件**

Create `macrdp-ui/src/components/PermissionBanner.tsx`:

```tsx
import { useState, useEffect } from "react";
import { AlertTriangle } from "lucide-react";
import { Alert, AlertDescription } from "./ui/alert";
import { api } from "../lib/ipc";
import type { PermissionStatus } from "../lib/types";

export default function PermissionBanner() {
  const [perms, setPerms] = useState<PermissionStatus | null>(null);

  useEffect(() => {
    const check = () => api.getPermissions().then(setPerms).catch(() => {});
    check();
    const interval = setInterval(check, 5000);
    return () => clearInterval(interval);
  }, []);

  if (!perms || (perms.screen_capture && perms.accessibility)) return null;

  const missing: string[] = [];
  if (!perms.screen_capture) missing.push("屏幕录制");
  if (!perms.accessibility) missing.push("辅助功能");

  return (
    <Alert variant="destructive" className="rounded-none border-x-0 border-t-0">
      <AlertTriangle className="h-4 w-4" />
      <AlertDescription className="flex items-center justify-between">
        <span>缺少「{missing.join("」「")}」权限 — 部分功能将无法使用</span>
        <button
          onClick={() => api.openSystemPreferences(
            !perms.screen_capture ? "screen_capture" : "accessibility"
          )}
          className="ml-4 text-sm font-medium underline-offset-4 hover:underline"
        >
          前往授权
        </button>
      </AlertDescription>
    </Alert>
  );
}
```

- [ ] **Step 2: 重写 Sidebar — Lucide 图标 + 权限红点**

重写 `macrdp-ui/src/components/Sidebar.tsx`：
- 所有 emoji 替换为 Lucide 图标
- 权限导航项增加红点 badge
- 使用 `useState`+`useEffect` 轮询 `getPermissions()` 每 5 秒检查一次
- 复用 shadcn Badge 或自定义红点

关键图标映射（Lucide）和权限红点实现：

```tsx
import { LayoutDashboard, Settings, Shield, FileText, Activity, Info } from "lucide-react";
import { useState, useEffect } from "react";
import { useLocation, Link } from "react-router-dom";
import { api } from "../lib/ipc";

const navItems = [
  { path: "/", label: "仪表盘", icon: LayoutDashboard },
  { path: "/settings", label: "配置", icon: Settings },
  { path: "/permissions", label: "权限", icon: Shield, showBadge: true },
  { path: "/logs", label: "日志", icon: FileText },
  { path: "/statistics", label: "统计", icon: Activity },
  { path: "/about", label: "关于", icon: Info },
];

// 在组件中：
const [permsMissing, setPermsMissing] = useState(false);
useEffect(() => {
  const check = () => api.getPermissions().then((p) =>
    setPermsMissing(!p.screen_capture || !p.accessibility)
  ).catch(() => {});
  check();
  const interval = setInterval(check, 5000);
  return () => clearInterval(interval);
}, []);

// 导航项渲染中，权限项的红点：
{item.showBadge && permsMissing && (
  <span className="absolute right-2 top-1/2 -translate-y-1/2 w-2 h-2 rounded-full bg-macos-red" />
)}
```

- [ ] **Step 3: 在 App.tsx 中添加 PermissionBanner**

在 `MainLayout` 中 `<main>` 之前添加 `<PermissionBanner />`。

- [ ] **Step 4: 验证前端构建**

Run: `cd macrdp-ui && npm run build 2>&1 | tail -10`
Expected: 构建成功

- [ ] **Step 5: Commit**

```bash
git add macrdp-ui/src/components/PermissionBanner.tsx macrdp-ui/src/components/Sidebar.tsx macrdp-ui/src/App.tsx
git commit -m "feat(ui): add permission banner, Lucide icons in sidebar"
```

---

## Task 9: 仪表盘页面重写

**Files:**
- Create: `macrdp-ui/src/components/MetricsStrip.tsx`
- Create: `macrdp-ui/src/components/ServerInfoTags.tsx`
- Modify: `macrdp-ui/src/pages/Dashboard.tsx`

- [ ] **Step 1: 创建 MetricsStrip 组件**

Create `macrdp-ui/src/components/MetricsStrip.tsx`:

```tsx
import { Card } from "./ui/card";
import type { Metrics } from "../lib/types";

interface MetricsStripProps {
  metrics: Metrics;
  port: number;
}

interface MetricItemProps {
  label: string;
  value: string;
  unit?: string;
  sub?: string;
  children?: React.ReactNode;
}

function MetricItem({ label, value, unit, sub, children }: MetricItemProps) {
  return (
    <div className="flex-1 px-4 py-3.5 relative min-w-0
      [&:not(:last-child)]:after:content-[''] [&:not(:last-child)]:after:absolute
      [&:not(:last-child)]:after:right-0 [&:not(:last-child)]:after:top-3
      [&:not(:last-child)]:after:bottom-3 [&:not(:last-child)]:after:w-px
      [&:not(:last-child)]:after:bg-macos-border">
      <div className="text-[11px] font-medium uppercase tracking-wide text-macos-muted mb-1 whitespace-nowrap">
        {label}
      </div>
      <div className="text-[22px] font-bold tracking-tight leading-none whitespace-nowrap">
        {value}
        {unit && <span className="text-xs font-normal text-macos-secondary ml-0.5">{unit}</span>}
      </div>
      {sub && <div className="text-[10px] text-macos-muted mt-0.5 whitespace-nowrap">{sub}</div>}
      {children}
    </div>
  );
}

function QualityBar({ quality }: { quality: number }) {
  const segments = 5;
  const active = Math.round(quality * segments);
  return (
    <div className="flex gap-0.5 mt-1.5">
      {Array.from({ length: segments }, (_, i) => (
        <div
          key={i}
          className={`w-4 h-1 rounded-sm ${
            i < active ? "bg-macos-green" : "bg-macos-border"
          }`}
        />
      ))}
    </div>
  );
}

function formatBytes(bytes: number): string {
  if (bytes >= 1e9) return `${(bytes / 1e9).toFixed(1)}`;
  if (bytes >= 1e6) return `${(bytes / 1e6).toFixed(1)}`;
  if (bytes >= 1e3) return `${(bytes / 1e3).toFixed(0)}`;
  return `${bytes}`;
}

function bytesUnit(bytes: number): string {
  if (bytes >= 1e9) return "GB";
  if (bytes >= 1e6) return "MB";
  if (bytes >= 1e3) return "KB";
  return "B";
}

const qualityLabel = (q: number) =>
  q >= 0.8 ? "优秀" : q >= 0.6 ? "良好" : q >= 0.4 ? "一般" : q >= 0.2 ? "较差" : "极差";

export default function MetricsStrip({ metrics, port }: MetricsStripProps) {
  return (
    <Card className="flex items-stretch overflow-x-auto bg-macos-card backdrop-blur-xl border-macos-border">
      <MetricItem label="端口" value={String(port)} sub="0.0.0.0" />
      <MetricItem label="帧率" value={String(metrics.fps)} unit="FPS" sub={`目标 ${metrics.fps}`} />
      <MetricItem label="比特率" value={(metrics.bitrate_kbps / 1000).toFixed(1)} unit="Mbps" />
      <MetricItem label="RTT" value={metrics.latency_ms.toFixed(1)} unit="ms" sub="EWMA" />
      <MetricItem label="编码" value={metrics.encode_ms.toFixed(1)} unit="ms" />
      <MetricItem label="网络" value={metrics.net_ms.toFixed(1)} unit="ms" sub="RTT - 编码" />
      <MetricItem label="帧大小" value={String(Math.round(metrics.last_frame_bytes / 1024))} unit="KB" />
      <MetricItem label="总流量" value={formatBytes(metrics.bytes_sent)} unit={bytesUnit(metrics.bytes_sent)} />
      <MetricItem label="网络质量" value={qualityLabel(metrics.network_quality)}>
        <QualityBar quality={metrics.network_quality} />
      </MetricItem>
    </Card>
  );
}
```

- [ ] **Step 2: 创建 ServerInfoTags 组件**

Create `macrdp-ui/src/components/ServerInfoTags.tsx`:

水平排列静态信息标签：编码协议、编码器、分辨率、TLS 状态。使用 Lucide 图标 + shadcn Badge。

组件接收 config 信息作为 props。

- [ ] **Step 3: 重写 Dashboard.tsx**

- 顶部：标题 + 运行状态 + 停止按钮
- ServerInfoTags
- MetricsStrip
- 连接列表（shadcn Card）
- 所有 emoji 替换为 Lucide 图标
- 使用 `listen("metrics", ...)` 接收实时 Metrics 事件

- [ ] **Step 4: 验证前端构建**

Run: `cd macrdp-ui && npm run build 2>&1 | tail -10`
Expected: 构建成功

- [ ] **Step 5: Commit**

```bash
git add macrdp-ui/src/components/MetricsStrip.tsx macrdp-ui/src/components/ServerInfoTags.tsx macrdp-ui/src/pages/Dashboard.tsx
git commit -m "feat(ui): redesign dashboard with metrics strip and info tags"
```

---

## Task 10: 配置页面重写

**Files:**
- Modify: `macrdp-ui/src/pages/Settings.tsx`

- [ ] **Step 1: 重写 Settings.tsx**

- 使用 shadcn `Card`、`Input`、`Select`、`Switch`、`Button`、`Separator` 组件
- 按分组面板：RDP 服务 / 网络 / 认证 / 显示 / 通用
- 从 `api.getConfig()` 加载 `UiConfig`
- 修改时调用 `api.setConfig(key, value)`
- 如果 `restart_required`，显示顶部提示条
- 所有字段可编辑
- 日志级别默认 "warn"
- 不显示配置文件路径

- [ ] **Step 2: 验证前端构建**

Run: `cd macrdp-ui && npm run build 2>&1 | tail -10`
Expected: 构建成功

- [ ] **Step 3: Commit**

```bash
git add macrdp-ui/src/pages/Settings.tsx
git commit -m "feat(ui): rewrite settings page with shadcn/ui and real config persistence"
```

---

## Task 11: 剩余页面 shadcn 迁移

**Files:**
- Modify: `macrdp-ui/src/pages/Permissions.tsx`
- Modify: `macrdp-ui/src/pages/Logs.tsx`
- Modify: `macrdp-ui/src/pages/Statistics.tsx`
- Modify: `macrdp-ui/src/pages/About.tsx`
- Modify: `macrdp-ui/src/pages/Popover.tsx`

- [ ] **Step 1: 更新 Permissions.tsx**

- 轮询频率从 2000ms 改为 5000ms
- PermissionCard 使用 shadcn Card + Button
- 所有 emoji 替换为 Lucide 图标（Shield, Monitor, Mic, CheckCircle, AlertCircle）

- [ ] **Step 2: 更新 Logs.tsx**

- 按钮用 shadcn Button
- 输入框用 shadcn Input
- Select 用 shadcn Select
- 所有 emoji 替换

- [ ] **Step 3: 更新 Statistics.tsx**

- Card 用 shadcn Card
- 表格保持 Tailwind 样式（shadcn Table 可选）
- 按钮用 shadcn Button

- [ ] **Step 4: 更新 About.tsx**

- Card 用 shadcn Card
- Button 用 shadcn Button
- 所有 emoji 替换为 Lucide 图标

- [ ] **Step 5: 更新 Popover.tsx**

- 适配新 Metrics 字段（显示 RTT、编码耗时等）
- 所有 emoji 替换为 Lucide 图标

- [ ] **Step 6: 验证完整前端构建**

Run: `cd macrdp-ui && npm run build 2>&1 | tail -10`
Expected: 构建成功，无 TypeScript 错误

- [ ] **Step 7: Commit**

```bash
git add macrdp-ui/src/pages/
git commit -m "feat(ui): migrate all pages to shadcn/ui components and Lucide icons"
```

---

## Task 12: 集成测试和清理

**Files:**
- 可能需要修改的任何文件

- [ ] **Step 1: 完整构建验证**

Run: `cd macrdp-ui && cargo tauri build --debug 2>&1 | tail -30`
Expected: 应用编译成功

- [ ] **Step 2: 清理旧组件**

检查并删除不再使用的旧组件文件：
- `src/components/MetricCard.tsx` — 已被 MetricsStrip 替代
- `src/components/StatusBadge.tsx` — 如果已被 shadcn Badge 替代
- `src/components/PermissionCard.tsx` — 如果已重构
- `src/components/ConnectionItem.tsx` — 如果已内联到 Dashboard

Run: `rg "MetricCard|StatusBadge|PermissionCard|ConnectionItem" macrdp-ui/src/ --files-with-matches`

仅删除确认无引用的文件。

- [ ] **Step 3: 清理 tailwind.config.ts**

检查 `tailwind.config.ts` 中的旧 macOS 色彩定义，如果已迁移到 globals.css 的 CSS 变量中，移除 tailwind.config.ts 中的重复定义。

- [ ] **Step 4: 最终编译验证**

Run: `cargo build --workspace 2>&1 | tail -10`
Expected: 整个 workspace 编译成功

Run: `cargo test --workspace 2>&1 | tail -10`
Expected: 所有测试通过

Run: `cd macrdp-ui && npm run build 2>&1 | tail -10`
Expected: 前端构建成功

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "chore(ui): clean up old components and consolidate theme config"
```
