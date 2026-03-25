# macrdp UI Bug 修复与美化设计规格

**日期**: 2026-03-25
**状态**: 已批准

## 概述

针对 macrdp Tauri 桌面应用的 6 项 UI 改进：独立配置文件、端口冲突检测、shadcn/ui 组件迁移、Metrics 扩展与仪表盘重设计、配置编辑修复、权限系统优化。

## 1. 独立 UI 配置文件

### 问题

UI 和 CLI 共用同一个 `config.toml`，且 `set_config` Tauri 命令是空实现（仅打日志），配置修改无法持久化。

### 方案

- UI 使用独立配置文件 `~/.macrdp/config-ui.toml`
- CLI 继续使用自己的搜索路径（`./config.toml` → `~/.config/macrdp/` → `~/Library/Application Support/macrdp/`），两者完全独立
- 应用启动时读取 `~/.macrdp/config-ui.toml`，不存在则创建默认配置
- `set_config` 命令真正实现持久化写入该文件

> **路径选择说明**: 选择 `~/.macrdp/` 而非 `~/Library/Application Support/macrdp/` 是因为 CLI 已使用后者作为候选路径之一，UI 需要一个绝对不会和 CLI 冲突的固定位置。`~/.macrdp/` 短小、好记、跨平台可移植。

### 配置文件内容

包含全部服务器参数 + UI 专属设置：

```toml
# RDP 服务
port = 3389
frame_rate = 60
bitrate_mbps = 50
encoder = "auto"          # "auto" | "hardware" | "software"
chroma_mode = "avc420"    # "avc420" | "avc444"

# 网络
bind_address = "0.0.0.0"
max_connections = 3
idle_timeout_secs = 1800

# 认证
username = "macrdp"
password = ""

# 显示
hidpi_scale = 2
show_cursor = false

# 通用
log_level = "warn"

# UI 专属
theme = "system"          # "system" | "light" | "dark"
autostart = false
```

### UiConfig 结构体

在 `macrdp-ui/src-tauri/src/ui_config.rs` 中定义独立的 `UiConfig` 结构体，所有字段均为非 Option 类型（带默认值），与 `macrdp-core::ServerConfig`（大量 Option 字段）解耦：

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiConfig {
    // 服务器参数
    pub port: u16,                // 默认 3389
    pub frame_rate: u32,          // 默认 60
    pub bitrate_mbps: u32,        // 默认 50
    pub encoder: String,          // 默认 "auto"
    pub chroma_mode: String,      // 默认 "avc420"
    pub bind_address: String,     // 默认 "0.0.0.0"
    pub max_connections: u32,     // 默认 3
    pub idle_timeout_secs: u64,   // 默认 1800
    pub username: String,         // 默认 "macrdp"
    pub password: String,         // 默认 ""
    pub hidpi_scale: u32,         // 默认 2
    pub show_cursor: bool,        // 默认 false
    pub log_level: String,        // 默认 "warn"
    // UI 专属
    pub theme: String,            // 默认 "system"
    pub autostart: bool,          // 默认 false
}
```

启动服务时，`UiConfig` 通过 `impl From<UiConfig> for ServerConfig` 转换为 `ServerConfig` 传给 `macrdp_core::start_server()`。`bind_address` 和 `max_connections` 字段在转换时暂存于 Tauri 侧，由 `commands.rs` 中的 `start_server()` 在构造 `SocketAddr` 时使用。

### 后端 API 签名

```rust
// get_config: 返回完整 UiConfig 的 JSON 表示
#[tauri::command]
pub fn get_config(state: State<'_, Arc<AppState>>) -> Result<UiConfig, String>

// set_config: 按单字段更新，返回是否需要重启
#[tauri::command]
pub fn set_config(
    key: String,
    value: serde_json::Value,
    state: State<'_, Arc<AppState>>,
) -> Result<SetConfigResponse, String>

#[derive(Serialize)]
pub struct SetConfigResponse {
    pub restart_required: bool,
}
```

`set_config` 流程：解析 key/value → 更新内存中的 `UiConfig` → 序列化写入 `~/.macrdp/config-ui.toml` → 如果字段支持热更新且服务运行中，通过 `ServerHandle` 下发 `ConfigUpdate`；否则标记 `restart_required: true`。

## 2. 端口冲突检测与自动选择

### 问题

服务启动时直接绑定 `0.0.0.0:{port}`，无冲突检测，端口被占用时报错不友好。

### 方案

在 `macrdp-core/src/server.rs` 的 `start_server()` 中实现，使 CLI 和 UI 都受益：

- 直接尝试 `TcpListener::bind(addr)`，如果失败（`AddrInUse`），递增端口号重试
- 最多尝试 100 个端口（`port` 到 `port + 99`）
- 成功绑定后将 `TcpListener` 传给后续的服务器启动流程（避免 TOCTOU 竞态：绑定和使用是同一个 listener）
- 所有候选端口都不可用时，返回明确错误信息
- `ServerHandle::port()` 返回实际绑定的端口，UI 仪表盘据此显示

### 对 `start_server` 的修改

需要调整 `macrdp-core` 的 `start_server()` 签名或内部逻辑，使其接受端口范围尝试绑定，并将实际绑定端口存入 `ServerHandle`。

## 3. shadcn/ui 组件迁移与风格统一

### 问题

当前所有 UI 控件手写 Tailwind 类名，组件风格不一致，缺乏统一的设计系统。

### shadcn/ui 初始化

1. 运行 `npx shadcn@latest init` 生成 `components.json`
2. shadcn/ui 已支持 Tailwind CSS v4（自 2025 年末起），初始化时选择 CSS Variables 模式
3. 安装所需组件：`npx shadcn@latest add button input select switch badge tooltip alert separator card`
4. 生成 `src/lib/utils.ts`（包含 `cn()` 辅助函数）
5. Radix UI 依赖由 shadcn 自动管理

### 组件替换清单

| shadcn 组件 | 用途 |
|-------------|------|
| `Button` | 停止服务、保存配置、前往授权等所有按钮 |
| `Input` | 配置页面数字和文本输入 |
| `Select` | 帧率、编码器、日志级别等下拉选择 |
| `Switch` | 开机自启、显示光标等开关 |
| `Badge` | 状态标签（运行中/已停止/错误） |
| `Tooltip` | 主题切换按钮 hover 提示 |
| `Alert` | 权限警告横幅 |
| `Separator` | 分组分隔线 |
| `Card` | 指标卡片、连接卡片、配置分组 |

### 图标规范

- 使用 Lucide React（已安装）作为唯一图标来源
- **禁止使用任何 emoji 作为图标**
- 侧边栏导航、信息标签、连接头像、警告图标等全部使用 Lucide SVG 图标

### 主题系统

- 浅色/深色双主题，默认跟随系统 `prefers-color-scheme`
- CSS 变量双轨控制：`prefers-color-scheme` 媒体查询 + `data-theme` HTML 属性
- shadcn/ui 通过 CSS 变量自动适配明暗主题
- 右下角单按钮循环切换：system → light → dark
  - 按钮为 36x36 圆角方块，带毛玻璃背景
  - 图标随模式变化：Monitor(系统) / Sun(浅色) / Moon(深色)，均为 Lucide 图标
  - hover 显示当前模式文字提示（Tooltip 组件）
- 主题偏好存入 `~/.macrdp/config-ui.toml` 的 `theme` 字段

**主题状态管理**：使用 React Context（`ThemeProvider`）包裹 App 根组件。启动时从 `get_config()` 读取 `theme` 字段初始化。切换时调用 `set_config("theme", value)` 持久化，同时更新 `<html>` 元素的 `data-theme` 属性。

### 色彩系统

保留 macOS 原生风格配色，通过 CSS 变量定义：

**浅色模式：**
- 背景: `#f5f5f7`，卡片: `rgba(255,255,255,0.72)` + `backdrop-filter: blur(20px)`
- 文字: `#1d1d1f`，次要: `#86868b`，弱化: `#aeaeb2`
- 强调色: blue `#007aff`，green `#34c759`，red `#ff3b30`，orange `#ff9500`

**深色模式：**
- 背景: `#09090b`，卡片: `rgba(24,24,27,0.85)` + `backdrop-filter: blur(20px)`
- 文字: `#fafafa`，次要: `#a1a1aa`，弱化: `#71717a`
- 强调色: blue `#0a84ff`，green `#30d158`，red `#ff453a`，orange `#ff9f0a`

## 4. Metrics 扩展与仪表盘重设计

### 问题

`Metrics` 结构体只暴露 5 个字段，`GfxState` 中的丰富数据（编码耗时、网络质量等）未传到前端。仪表盘指标卡片分块显示，不够紧凑。

### Metrics 结构体扩展

```rust
pub struct Metrics {
    // 已有（rtt_ms 通过 serde(rename) 保持 JSON 兼容）
    pub fps: u32,
    pub bitrate_kbps: u64,
    #[serde(rename = "latency_ms")]
    pub rtt_ms: f64,           // 内部重命名，序列化仍为 latency_ms 保持前端兼容
    pub bytes_sent: u64,
    pub timestamp: u64,
    // 新增
    pub encode_ms: f64,        // 编码耗时
    pub net_ms: f64,           // 网络延迟 (rtt - encode)
    pub last_frame_bytes: u32, // 最近帧大小
    pub network_quality: f32,  // 网络质量 (0.0~1.0)
    pub pending_acks: u32,     // 待确认帧数
}
```

> **兼容性说明**: `rtt_ms` 字段使用 `#[serde(rename = "latency_ms")]` 保持 JSON 序列化名不变，避免前端大规模重命名。前端 `types.ts` 中保留 `latency_ms` 字段名，新增字段直接追加。

### 数据来源映射

| Metrics 字段 | GfxState 来源 | 采集位置 |
|-------------|---------------|---------|
| `encode_ms` | `gfx.last_encode_ms` | `server.rs` metrics 任务 |
| `net_ms` | `gfx.rtt_ewma_ms - gfx.last_encode_ms`，clamp >= 0 | `server.rs` metrics 任务 |
| `last_frame_bytes` | `gfx.last_frame_bytes` | `server.rs` metrics 任务 |
| `network_quality` | `gfx.network_quality` | `server.rs` metrics 任务 |
| `pending_acks` | `gfx.pending_acks` | `server.rs` metrics 任务 |

### 仪表盘布局

**服务器信息标签栏** — 静态信息，水平排列（Lucide 图标 + 文字标签）：
- 编码协议（H.264 GFX · AVC444）
- 编码器（VideoToolbox）
- 分辨率（3840x2160 @2x）
- TLS 状态（TLS · 自签名证书）

**指标长条** — 一行 9 项，竖线分隔，单个 Card 组件：

端口(静态) | 帧率 | 比特率 | RTT | 编码耗时 | 网络延迟 | 帧大小 | 总流量 | 网络质量

> **说明**: 端口为静态配置值（仅在端口自动选择时与配置不同），不随 metrics 刷新。其余 8 项实时更新。

每项结构：
- 上方：11px 大写标签
- 中间：22px 加粗数值 + 单位
- 下方：10px 补充说明

网络质量额外显示 5 段条形指示器。

**连接列表** — 每个连接显示：客户端名称、IP、流量、连接时长。待确认帧数（`pending_acks`）显示在连接详情中（注意：当前为全局 GfxState 数据，非逐连接，因为 macrdp 当前架构为单活跃 RDP 会话）。

## 5. 配置页面修复

### 问题

`set_config` 是空实现，配置修改不持久化。用户反馈端口等字段"无法编辑"（实际可编辑但不生效）。

### 方案

所有字段均可编辑，按分组面板展示：

| 分组 | 字段 | 控件类型 | 需重启 |
|------|------|---------|--------|
| **RDP 服务** | 端口 | Input (number) | 是 |
| | 帧率 | Select (30/60/120) | 否 |
| | 比特率 | Input (number) + Mbps | 否 |
| | 编码器 | Select (auto/hardware/software) | 是 |
| | 色度模式 | Select (avc420/avc444) | 是 |
| **网络** | 绑定地址 | Input (text) | 是 |
| | 最大连接数 | Input (number) | 是 |
| | 空闲超时 | Input (number) + 秒 | 是 |
| **认证** | 用户名 | Input (text) | 是 |
| | 密码 | Input (password) | 是 |
| **显示** | HiDPI 缩放 | Select (1x/2x/3x/4x) | 是 |
| | 显示光标 | Switch | 是 |
| **通用** | 日志级别 | Select (trace/debug/info/warn/error)，默认 warn | 否 |
| | 开机自启 | Switch | 否 |

**热更新字段**（服务运行中即时生效）：帧率、比特率、日志级别。通过现有的 `ConfigUpdate` 枚举（`FrameRate`、`BitrateKbps`、`LogLevel`）下发到运行中的服务器。

**需重启字段**：其余字段修改后显示提示条「部分配置需重启服务后生效」。

> **注意**: `max_connections` 和 `idle_timeout_secs` 当前 `ConfigUpdate` 不支持热更新，标记为需重启。未来可扩展 `ConfigUpdate` 枚举支持，但不在本次范围内。

- 修改后调用 `set_config(key, value)` → 后端更新内存中 `UiConfig` + 写入 `~/.macrdp/config-ui.toml` → 返回 `SetConfigResponse { restart_required }`

## 6. 权限系统优化

### 问题

权限检查被动，用户不知道缺少权限直到启动失败。

### 方案

**启动时主动请求：**
- 应用启动时（`lib.rs` 的 `setup()` 钩子中）立即调用 `request_permissions()` 尝试获取屏幕录制和辅助功能权限
- 触发系统权限对话框

**定时轮询：**
- 每 5 秒检查一次 `check_permissions()`（当前为 2 秒，降频减少开销）
- 权限状态变化时通过 Tauri 事件 `"permission-status"` 推送前端

**全局顶部警告横幅：**
- 缺少任何权限时常驻显示（shadcn `Alert` 组件，destructive 变体）
- 具体列出缺少哪些权限及影响（如"缺少辅助功能权限 — 键鼠输入功能将无法使用"）
- 「前往授权」链接打开对应系统偏好设置面板
- 所有权限就绪后自动消失

**侧边栏红点：**
- 权限导航项右侧显示红色圆点 badge（8px）
- 与横幅联动，所有权限就绪后消失

## 影响范围

### macrdp-core 改动
- `callbacks.rs` — 扩展 `Metrics` 结构体（新增 5 字段 + serde rename）
- `server.rs` — metrics 采集补充新字段；端口冲突检测与自动选择逻辑

### macrdp-ui Tauri 后端改动
- `commands.rs` — 重写 `get_config`/`set_config` 签名和实现
- `ui_config.rs` — **新增**，`UiConfig` 结构体定义、文件读写、`From<UiConfig> for ServerConfig` 转换
- `state.rs` — 在 `AppState` 中增加 `UiConfig` 引用
- `event_bridge.rs` — 适配 `Metrics` 新字段的事件发射
- `lib.rs` — 启动时主动请求权限、读取 UI 配置初始化

### 前端改动
- `package.json` — 新增 shadcn/ui 相关依赖（Radix UI 系列）
- `components.json` — **新增**，shadcn/ui 配置
- `src/lib/utils.ts` — **新增**，`cn()` 辅助函数
- `src/styles/globals.css` — 重写为浅色/深色双主题 CSS 变量 + shadcn 基础样式
- `src/components/ui/` — **新增目录**，shadcn/ui 生成的组件文件
- `src/components/Sidebar.tsx` — Lucide 图标 + 权限红点
- `src/components/ThemeToggle.tsx` — **新增**，右下角主题循环切换按钮
- `src/components/PermissionBanner.tsx` — **新增**，全局权限警告横幅
- `src/components/MetricsStrip.tsx` — **新增**，指标长条组件
- `src/contexts/ThemeContext.tsx` — **新增**，主题状态 React Context
- `src/pages/Dashboard.tsx` — 使用 MetricsStrip + 信息标签栏
- `src/pages/Settings.tsx` — 分组面板 + shadcn 控件 + 真正的配置持久化
- `src/pages/Permissions.tsx` — 轮询频率从 2 秒调整为 5 秒
- `src/lib/types.ts` — Metrics 类型新增字段

### 不改动
- `macrdp-core` 的 `ServerConfig` 结构体和 `load_from_file()` — CLI 配置逻辑不变
- `macrdp-server` crate — 无改动
- `macrdp-capture`、`macrdp-input`、`macrdp-encode` — 无改动
- `ironrdp-server-gfx` — 无改动（GfxState 已有所需字段）
