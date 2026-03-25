# macrdp UI Bug 修复与美化设计规格

**日期**: 2026-03-25
**状态**: 已批准

## 概述

针对 macrdp Tauri 桌面应用的 5 项 UI 改进：独立配置文件、端口冲突检测、shadcn/ui 组件迁移、配置编辑修复、权限系统优化。

## 1. 独立 UI 配置文件

### 问题

UI 和 CLI 共用同一个 `config.toml`，且 `set_config` Tauri 命令是空实现（仅打日志），配置修改无法持久化。

### 方案

- UI 使用独立配置文件 `~/.macrdp/config-ui.toml`
- CLI 继续使用自己的搜索路径（`./config.toml` → `~/.config/macrdp/` → `~/Library/Application Support/macrdp/`），两者完全独立
- 应用启动时读取 `~/.macrdp/config-ui.toml`，不存在则创建默认配置
- `set_config` 命令真正实现持久化写入该文件

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

### 后端改动

- `macrdp-ui/src-tauri/src/commands.rs`:
  - `get_config()` — 从 `~/.macrdp/config-ui.toml` 读取
  - `set_config(key, value)` — 更新内存配置 + 写入文件 + 返回是否需要重启
- 新增 `macrdp-ui/src-tauri/src/ui_config.rs` 模块处理 UI 配置的读写逻辑
- 不修改 `macrdp-core` 的 `ServerConfig::load_from_file()`，仅在 Tauri 侧增加配置路径

## 2. 端口冲突检测与自动选择

### 问题

服务启动时直接绑定 `0.0.0.0:{port}`，无冲突检测，端口被占用时报错不友好。

### 方案

- 启动服务前用 `TcpListener::bind()` 探测配置端口是否可用
- 如果被占用，从 `port+1` 开始递增查找，最多尝试 100 个端口
- 找到可用端口后自动使用，仪表盘指标条显示实际端口
- 所有候选端口都不可用时，返回明确错误信息给前端展示

### 实现位置

- `macrdp-ui/src-tauri/src/commands.rs` 的 `start_server()` 函数中，在调用 `macrdp_core::start_server()` 之前加入端口探测逻辑
- 或在 `macrdp-core/src/server.rs` 的 `start_server()` 中实现，使 CLI 也受益

## 3. shadcn/ui 组件迁移与风格统一

### 问题

当前所有 UI 控件手写 Tailwind 类名，组件风格不一致，缺乏统一的设计系统。

### 方案

引入 shadcn/ui（基于 Radix UI + Tailwind CSS），逐页替换手写控件：

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
- 禁止使用任何 emoji 作为图标
- 侧边栏导航、信息标签、连接头像、警告图标等全部使用 Lucide SVG 图标

### 主题系统

- 浅色/深色双主题，默认跟随系统 `prefers-color-scheme`
- CSS 变量双轨控制：`prefers-color-scheme` 媒体查询 + `data-theme` HTML 属性
- shadcn/ui 通过 CSS 变量自动适配明暗主题
- 右下角单按钮循环切换：system → light → dark
  - 按钮为 36x36 圆角方块，带毛玻璃背景
  - 图标随模式变化：monitor(系统) / sun(浅色) / moon(深色)
  - hover 显示当前模式文字提示
- 主题偏好存入 `~/.macrdp/config-ui.toml` 的 `theme` 字段

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
    // 已有
    pub fps: u32,
    pub bitrate_kbps: u64,
    pub rtt_ms: f64,           // 重命名: latency_ms → rtt_ms
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

### 数据来源映射

| Metrics 字段 | GfxState 来源 |
|-------------|---------------|
| `encode_ms` | `gfx.last_encode_ms` |
| `net_ms` | `gfx.rtt_ewma_ms - gfx.last_encode_ms` |
| `last_frame_bytes` | `gfx.last_frame_bytes` |
| `network_quality` | `gfx.network_quality` |
| `pending_acks` | `gfx.pending_acks` |

### 仪表盘布局

**服务器信息标签栏** — 静态信息，水平排列：
- 编码协议（H.264 GFX · AVC444）
- 编码器（VideoToolbox）
- 分辨率（3840x2160 @2x）
- TLS 状态（TLS · 自签名证书）

**指标长条** — 一行 9 项，竖线分隔，实时更新：
端口 | 帧率 | 比特率 | RTT | 编码耗时 | 网络延迟 | 帧大小 | 总流量 | 网络质量

每项结构：
- 上方：11px 大写标签
- 中间：22px 加粗数值 + 单位
- 下方：10px 补充说明

网络质量额外显示 5 段条形指示器。

**连接列表** — 每个连接显示：客户端名称、IP、流量、待确认帧数、连接时长。

## 5. 配置页面修复

### 问题

`set_config` 是空实现，配置修改不持久化。部分用户反馈端口等字段"无法编辑"（实际可编辑但不生效）。

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
| | 最大连接数 | Input (number) | 否 |
| | 空闲超时 | Input (number) + 秒 | 否 |
| **认证** | 用户名 | Input (text) | 是 |
| | 密码 | Input (password) | 是 |
| **显示** | HiDPI 缩放 | Select (1x/2x/3x/4x) | 是 |
| | 显示光标 | Switch | 是 |
| **通用** | 日志级别 | Select (trace/debug/info/warn/error)，默认 warn | 否 |
| | 开机自启 | Switch | 否 |

- 修改后调用 `set_config` → 后端更新内存配置 + 写入 `~/.macrdp/config-ui.toml`
- 标记为"需重启"的字段修改后显示提示条「部分配置需重启服务后生效」
- 可热更新的字段（帧率、比特率、日志级别、最大连接数、空闲超时）直接生效

## 6. 权限系统优化

### 问题

权限检查被动，用户不知道缺少权限直到启动失败。

### 方案

**启动时主动请求：**
- 应用启动时立即调用 `request_permissions()` 尝试获取屏幕录制和辅助功能权限
- 触发系统权限对话框

**定时轮询：**
- 每 5 秒检查一次 `check_permissions()`
- 权限状态变化时通过 Tauri 事件推送前端

**全局顶部警告横幅：**
- 缺少任何权限时常驻显示
- 具体列出缺少哪些权限及影响
- 「前往授权」链接打开对应系统偏好设置面板
- 所有权限就绪后自动消失

**侧边栏红点：**
- 权限导航项右侧显示红色圆点 badge
- 与横幅联动，所有权限就绪后消失

## 影响范围

### 后端改动
- `macrdp-core/src/callbacks.rs` — 扩展 `Metrics` 结构体
- `macrdp-core/src/server.rs` — metrics 采集补充新字段，端口探测逻辑
- `macrdp-ui/src-tauri/src/commands.rs` — 重写 `get_config`/`set_config`，配置路径改为 `~/.macrdp/`
- `macrdp-ui/src-tauri/src/ui_config.rs` — 新增模块，处理 UI 配置读写
- `macrdp-ui/src-tauri/src/lib.rs` — 启动时主动请求权限

### 前端改动
- `package.json` — 新增 shadcn/ui 相关依赖
- `src/styles/globals.css` — 重写为浅色/深色双主题 CSS 变量
- `src/components/` — 全部替换为 shadcn/ui 组件 + Lucide 图标
- `src/pages/Dashboard.tsx` — 指标长条布局 + 信息标签栏
- `src/pages/Settings.tsx` — 分组面板 + 真正的配置持久化
- `src/pages/Permissions.tsx` — 权限轮询频率调整
- `src/components/Sidebar.tsx` — Lucide 图标 + 权限红点
- `src/lib/types.ts` — Metrics 类型扩展
- 新增主题切换组件（右下角循环按钮）
- 新增权限警告横幅组件

### 不改动
- `macrdp-core` 的 `ServerConfig::load_from_file()` — CLI 配置路径不变
- `macrdp-server` crate — 无改动
- `macrdp-capture`、`macrdp-input`、`macrdp-encode` — 无改动
- `ironrdp-server-gfx` — 无改动（GfxState 已有所需字段）
