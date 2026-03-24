# macrdp 统一 Library 入口重构设计

> **日期:** 2026-03-25
> **状态:** 已批准
> **范围:** 将 UI 调用 CLI 的方式从子进程 + IPC 改为 in-process library 嵌入

## 1. 背景与动机

当前架构中，Tauri UI 通过 `CliManager` 启动 `macrdp-server` CLI 作为子进程，双方通过 Unix Domain Socket + JSON Lines 协议通信。这带来了：

- **不必要的复杂性:** IPC 协议、消息序列化/反序列化、socket 生命周期管理
- **延迟开销:** 所有状态查询都需要跨进程通信
- **双重错误处理:** 进程崩溃、socket 断连、消息丢失等额外故障模式
- **代码重复:** 两端各自维护状态类型定义和序列化逻辑

## 2. 设计目标

1. 提供统一的 `macrdp-core` library crate，CLI 和 UI 共用同一入口
2. 消除 IPC 层，UI 直接 in-process 调用 library API
3. 通过 Trait 回调机制推送实时事件（指标、连接、日志）
4. 前端代码零改动（Tauri invoke 签名保持不变）

## 3. 架构概览

```
macrdp-capture ─┐
macrdp-input  ──┼→ macrdp-core (lib) ←── macrdp-server (CLI thin binary)
macrdp-encode ──┘        ↑
ironrdp-server-gfx ──────┘              macrdp-ui/src-tauri (Tauri binary)
```

**调用方式对比:**

| | 之前 | 之后 |
|---|---|---|
| UI → 服务 | spawn 子进程 + Unix Socket IPC | 直接函数调用 |
| CLI → 服务 | main() 内联所有逻辑 | 调用 macrdp-core API |
| 事件推送 | IPC JSON Lines 推送 | Trait 回调 |
| 状态查询 | IPC 命令/响应 | ServerHandle 方法 |

## 4. macrdp-core 公共 API

### 4.1 事件回调 Trait

```rust
pub trait ServerEventHandler: Send + Sync + 'static {
    fn on_status_change(&self, status: ServerStatus);
    fn on_metrics(&self, metrics: Metrics);
    fn on_connection(&self, event: ConnectionEvent);
    fn on_log(&self, entry: LogEntry);
}
```

### 4.2 统一入口函数

```rust
pub async fn start_server(
    config: ServerConfig,
    handler: impl ServerEventHandler,
) -> Result<ServerHandle>;
```

`start_server` 内部 spawn tokio 任务运行 RDP 服务循环，立即返回 `ServerHandle`。

### 4.3 控制句柄

```rust
pub struct ServerHandle { /* 内部: Arc 共享状态, JoinHandle, shutdown 信号 */ }

impl ServerHandle {
    pub fn status(&self) -> ServerStatus;
    pub fn active_connections(&self) -> Vec<ConnectionInfo>;
    pub fn metrics(&self) -> Metrics;
    pub async fn update_config(&self, update: ConfigUpdate) -> Result<()>;
    pub async fn stop(self) -> Result<()>;  // 消费 self，防止复用
    pub fn port(&self) -> u16;
}
```

### 4.4 共享类型

```rust
pub struct ServerConfig {
    pub port: u16,
    pub frame_rate: u32,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub username: Option<String>,
    pub password: Option<String>,
    pub cert_path: Option<String>,
    pub key_path: Option<String>,
    pub idle_timeout_secs: Option<u64>,
    pub log_level: String,
    pub quality: String,           // "low_latency" | "balanced" | "high_quality"
    pub encoder: String,           // "software" | "hardware" | "auto"
    pub chroma_mode: String,       // "avc420" | "avc444"
    pub hidpi_scale: Option<f64>,
    pub bitrate_mbps: Option<f64>,
}

pub enum ServerStatus {
    Stopped,
    Starting,
    Running { uptime_secs: u64 },
    Error(String),
}

pub struct Metrics {
    pub fps: u32,
    pub bitrate_kbps: u32,
    pub latency_ms: u32,
    pub bytes_sent: u64,
}

pub enum ConnectionEvent {
    Connected(ConnectionInfo),
    Disconnected(DisconnectionInfo),
}

pub struct ConnectionInfo {
    pub client_ip: String,
    pub client_name: Option<String>,
    pub connected_at: u64,
}

pub struct DisconnectionInfo {
    pub client_ip: String,
    pub client_name: Option<String>,
    pub duration_secs: u64,
    pub bytes_total: u64,
}

pub enum ConfigUpdate {
    FrameRate(u32),
    BitrateKbps(u32),
    LogLevel(String),
}

pub struct LogEntry {
    pub level: String,
    pub message: String,
    pub timestamp: String,
}

pub struct PermissionStatus {
    pub screen_capture: bool,
    pub accessibility: bool,
}
```

### 4.5 工具函数

```rust
pub fn check_permissions() -> PermissionStatus;
pub fn request_permissions();
pub fn detect_display_size() -> (u32, u32);
pub fn init_logging(config: &ServerConfig) -> Result<()>;
```

## 5. 代码迁移映射

### 5.1 macrdp-server → macrdp-core

| 源文件 | 去向 | 说明 |
|--------|------|------|
| `config.rs` (ServerConfig) | `macrdp-core/src/config.rs` | 去掉 ipc_socket 字段 |
| `config.rs` (Cli struct) | 留在 `macrdp-server/src/main.rs` | CLI 专属 |
| `config.rs` (config_dir) | `macrdp-core/src/config.rs` | 共享配置路径发现 |
| `display.rs` (全部) | `macrdp-core/src/display.rs` | 原样迁移 |
| `handler.rs` (全部) | `macrdp-core/src/handler.rs` | 原样迁移 |
| `tls.rs` (全部) | `macrdp-core/src/tls.rs` | 原样迁移 |
| `ipc.rs` (全部) | **删除** | 不再需要 |
| `main.rs` 权限检查 | `macrdp-core/src/permissions.rs` | 提取为公共函数 |
| `main.rs` 显示检测/HiDPI | `macrdp-core/src/display.rs` | 集成到显示模块 |
| `main.rs` 鼠标坐标映射 | `macrdp-core/src/handler.rs` | 集成到输入模块 |
| `main.rs` GFX/编码器/服务器构建 | `macrdp-core/src/server.rs` | start_server 实现 |
| `main.rs` 凭证生成 | `macrdp-core/src/server.rs` | 集成到启动流程 |
| `main.rs` RDP 主循环 | `macrdp-core/src/server.rs` | 核心循环 |
| `main.rs` tokio runtime + CLI | 留在 `macrdp-server/src/main.rs` | CLI 专属 |

### 5.2 macrdp-core 文件结构

```
crates/macrdp-core/
├── Cargo.toml
└── src/
    ├── lib.rs           # pub mod 声明 + re-export 公共 API
    ├── config.rs        # ServerConfig, ConfigUpdate, config_dir()
    ├── server.rs        # start_server(), ServerHandle, RDP 主循环
    ├── display.rs       # MacDisplay, MacDisplayUpdates
    ├── handler.rs       # MacInputHandler
    ├── tls.rs           # TLS 证书生成/管理
    ├── permissions.rs   # check_permissions(), PermissionStatus
    ├── callbacks.rs     # ServerEventHandler trait + 事件类型定义
    └── logging.rs       # init_logging() 辅助函数
```

### 5.3 macrdp-server 瘦身后

```
crates/macrdp-server/
├── Cargo.toml          # 依赖: macrdp-core, clap, tokio, tracing
└── src/
    └── main.rs         # ~50 行: CLI 解析 → core::start_server() → ctrl_c
```

## 6. Tauri UI 端改造

### 6.1 文件变更

| 文件 | 操作 | 说明 |
|------|------|------|
| `cli_manager.rs` | **删除** | 不再 spawn 子进程 |
| `ipc_client.rs` | **删除** | 不再 IPC 通信 |
| `event_bridge.rs` | **新增** | 实现 ServerEventHandler → Tauri 事件桥接 |
| `commands.rs` | **重写** | 调用 macrdp-core API 替代 IPC 命令 |
| `lib.rs` | **重写** | 去掉 CliManager/IpcClient 初始化 |
| `state.rs` | **简化** | 由回调填充，去掉 IPC 相关逻辑 |
| `tray.rs` | **微调** | 数据来源改为回调驱动 |
| `database.rs` | **保留** | 连接历史/统计持久化不变 |
| `permissions.rs` | **保留** | 打开系统偏好设置功能不变 |

### 6.2 TauriEventBridge 实现

```rust
// macrdp-ui/src-tauri/src/event_bridge.rs
pub struct TauriEventBridge {
    app_handle: AppHandle,
    state: Arc<AppState>,
    db: Arc<Database>,
}

impl ServerEventHandler for TauriEventBridge {
    fn on_status_change(&self, status: ServerStatus) {
        self.state.set_status(status.clone());
        self.app_handle.emit("server-status", &status).ok();
    }

    fn on_metrics(&self, metrics: Metrics) {
        self.state.set_metrics(metrics.clone());
        self.app_handle.emit("metrics", &metrics).ok();
        update_tray_title(&self.app_handle, &metrics);
    }

    fn on_connection(&self, event: ConnectionEvent) {
        match &event {
            ConnectionEvent::Connected(info) => {
                self.state.add_connection(info.clone());
            }
            ConnectionEvent::Disconnected(info) => {
                self.state.remove_connection(&info.client_ip);
                self.db.record_disconnection(info);
            }
        }
        self.app_handle.emit("connections", &event).ok();
    }

    fn on_log(&self, entry: LogEntry) {
        self.state.push_log(entry.clone());
        self.app_handle.emit("log", &entry).ok();
    }
}
```

### 6.3 命令改造示例

```rust
#[tauri::command]
async fn start_server(
    server: State<'_, Mutex<Option<ServerHandle>>>,
    app_state: State<'_, Arc<AppState>>,
    db: State<'_, Arc<Database>>,
    app: AppHandle,
) -> Result<(), String> {
    let config = ServerConfig::load_default()?;
    let bridge = TauriEventBridge::new(app, app_state.inner().clone(), db.inner().clone());
    let handle = macrdp_core::start_server(config, bridge)
        .await
        .map_err(|e| e.to_string())?;
    *server.lock().await = Some(handle);
    Ok(())
}

#[tauri::command]
async fn stop_server(
    server: State<'_, Mutex<Option<ServerHandle>>>,
) -> Result<(), String> {
    if let Some(handle) = server.lock().await.take() {
        handle.stop().await.map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
fn get_metrics(app_state: State<'_, Arc<AppState>>) -> Metrics {
    app_state.get_metrics()
}
```

### 6.4 Tauri 依赖变更

```diff
[dependencies]
+ macrdp-core = { path = "../../crates/macrdp-core" }
- uuid = "1"
```

### 6.5 前端代码

**无需改动。** 前端通过 `tauri.invoke()` 调用命令、通过 `listen()` 监听事件，后端 API 签名和事件名保持一致。

## 7. CLI 端改造

```rust
// crates/macrdp-server/src/main.rs (~50 行)
use macrdp_core::*;
use clap::Parser;

#[derive(Parser)]
struct Cli {
    #[arg(short, long, default_value = "3389")]
    port: u16,
    #[arg(short, long)]
    config: Option<String>,
    #[arg(long)]
    frame_rate: Option<u32>,
    #[arg(long)]
    log_level: Option<String>,
}

struct CliEventHandler;

impl ServerEventHandler for CliEventHandler {
    fn on_status_change(&self, status: ServerStatus) {
        tracing::info!("Status: {:?}", status);
    }
    fn on_metrics(&self, m: Metrics) {
        tracing::debug!("fps={} bitrate={}kbps", m.fps, m.bitrate_kbps);
    }
    fn on_connection(&self, event: ConnectionEvent) {
        match event {
            ConnectionEvent::Connected(c) => tracing::info!("Connected: {}", c.client_ip),
            ConnectionEvent::Disconnected(d) => tracing::info!("Disconnected: {}", d.client_ip),
        }
    }
    fn on_log(&self, _entry: LogEntry) {}
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let config = ServerConfig::load(&cli.config, cli.port, cli.frame_rate, cli.log_level)?;
    init_logging(&config)?;

    let handle = start_server(config, CliEventHandler).await?;
    tracing::info!("macrdp running on port {}", handle.port());

    tokio::signal::ctrl_c().await?;
    handle.stop().await?;
    Ok(())
}
```

## 8. 回调机制实现细节

### 8.1 线程安全

- `ServerEventHandler` 要求 `Send + Sync + 'static`
- 回调通过 `Arc<dyn ServerEventHandler>` 在 tokio 任务间共享
- 回调在独立 tokio 任务中调用，不阻塞 RDP 主循环

### 8.2 频率控制

| 事件 | 触发频率 | 说明 |
|------|---------|------|
| `on_status_change` | 状态变更时 | 低频 |
| `on_metrics` | **每秒 1 次** | 限频，避免高帧率回调风暴 |
| `on_connection` | 连接/断开时 | 低频 |
| `on_log` | 日志产生时 | 通过 tracing Layer 转发 |

### 8.3 日志转发

core 内部注册一个自定义 `tracing::Layer`，将日志事件转发给 `handler.on_log()`。这样 core 内部的所有 `tracing::info!()` 等调用都会自动推送到 handler。

## 9. 工作区 Cargo.toml 变更

```diff
[workspace]
members = [
+   "crates/macrdp-core",
    "crates/macrdp-server",
    "crates/macrdp-capture",
    "crates/macrdp-input",
    "crates/macrdp-encode",
    "crates/ironrdp-server-gfx",
    "crates/ironrdp-acceptor-patched",
]
```

## 10. 影响范围

- **macrdp-capture / macrdp-input / macrdp-encode:** 无改动，仅被 macrdp-core 引用
- **ironrdp-server-gfx:** 无改动
- **前端 React 代码:** 无改动
- **Makefile:** 需更新 build 命令，加入 macrdp-core 编译
- **config.example.toml:** 去掉 ipc_socket 相关（如有）

## 11. 风险与缓解

| 风险 | 缓解措施 |
|------|---------|
| Tauri 进程崩溃导致 RDP 服务也挂 | 服务本身就需要 UI 管理，单进程是合理的 |
| 回调中的 panic 传播到 RDP 主循环 | `std::panic::catch_unwind` 包裹回调调用 |
| 大量日志回调影响性能 | 日志回调可通过 tracing level 过滤 |
| ServerConfig 字段变更兼容性 | config.example.toml 同步更新 |
