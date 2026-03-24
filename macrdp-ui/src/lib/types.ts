export interface ServerStatus {
  running: boolean;
  state: "stopped" | "running" | "starting" | "error";
  uptime_secs: number;
  pid: number | null;
}

export interface Metrics {
  fps: number;
  bitrate_kbps: number;
  latency_ms: number;
  bytes_sent: number;
  timestamp: number;
}

export interface Connection {
  client_ip: string;
  client_name: string;
  connected_at: string;
  bytes_total: number;
}

export interface LogEntry {
  level: "trace" | "debug" | "info" | "warn" | "error";
  message: string;
  timestamp: string;
}

export interface PermissionStatus {
  screen_capture: boolean;
  accessibility: boolean;
  microphone: boolean;
}

export interface ConnectionHistory {
  id: number;
  client_ip: string;
  client_name: string;
  connected_at: string;
  disconnected_at: string;
  duration_secs: number;
  bytes_total: number;
}

export interface TrafficStats {
  date: string;
  bytes_sent: number;
  connection_count: number;
}

export interface ServerConfig {
  port: number;
  frame_rate: number;
  width: number;
  height: number;
  bind_address: string;
  max_connections: number;
  log_level: string;
  tls_cert_path: string;
  tls_key_path: string;
  bitrate_mbps: number;
}
