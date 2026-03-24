import { useState } from "react";
import { Settings, LogOut, FileText } from "lucide-react";
import { useServerStatus } from "../hooks/useServerStatus";
import { useMetrics } from "../hooks/useMetrics";
import { useConnections } from "../hooks/useConnections";
import { api } from "../lib/ipc";
import { formatBytes, formatDuration } from "../lib/format";

const statusDotColor: Record<string, string> = {
  running: "bg-macos-green",
  starting: "bg-macos-blue",
  stopped: "bg-macos-secondary",
  error: "bg-macos-red",
};

function Popover() {
  const status = useServerStatus();
  const { metrics, stale } = useMetrics();
  const connections = useConnections();
  const [toggling, setToggling] = useState(false);

  const metricsAvailable = metrics && !stale;

  const handleToggle = async () => {
    setToggling(true);
    try {
      if (status.running) {
        await api.stopServer();
      } else {
        await api.startServer();
      }
    } catch (err) {
      console.error("Failed to toggle server:", err);
    } finally {
      setToggling(false);
    }
  };

  const handleQuit = () => {
    if (window.confirm("确定退出 macrdp？")) {
      api.quitApp();
    }
  };

  const handleShowMain = () => {
    api.showMainWindow();
  };

  const handleShowLogs = () => {
    api.showMainWindow();
  };

  const displayedConnections = connections.slice(0, 3);
  const extraCount = connections.length - 3;

  return (
    <div className="min-h-screen bg-[rgba(245,245,247,0.95)] backdrop-blur-xl rounded-xl overflow-hidden shadow-2xl">
      <div className="flex flex-col p-3.5 gap-3">
        {/* Header row */}
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2">
            <div
              className={`h-2 w-2 rounded-full ${statusDotColor[status.state] ?? "bg-macos-secondary"}`}
            />
            <span className="text-sm font-semibold text-macos-text">
              macrdp
            </span>
            {status.running && status.uptime_secs > 0 && (
              <span className="text-xs text-macos-secondary">
                {formatDuration(status.uptime_secs)}
              </span>
            )}
          </div>
          <div className="flex items-center gap-1">
            <button
              type="button"
              onClick={handleShowMain}
              className="rounded-md p-1.5 text-macos-secondary hover:bg-black/[0.06] transition-colors"
              title="设置"
            >
              <Settings size={15} />
            </button>
            <button
              type="button"
              onClick={handleQuit}
              className="rounded-md p-1.5 text-macos-red bg-macos-red/10 hover:bg-macos-red/20 transition-colors"
              title="退出"
            >
              <LogOut size={15} />
            </button>
          </div>
        </div>

        {/* Metrics row */}
        <div className="grid grid-cols-4 gap-1.5">
          <div className="flex flex-col items-center justify-center rounded-md bg-black/[0.04] py-2 px-1">
            <span className="text-sm font-semibold text-macos-text">
              {metricsAvailable ? metrics.fps : "--"}
            </span>
            <span className="text-[10px] text-macos-secondary">FPS</span>
          </div>
          <div className="flex flex-col items-center justify-center rounded-md bg-black/[0.04] py-2 px-1">
            <span className="text-sm font-semibold text-macos-text">
              {metricsAvailable
                ? (metrics.bitrate_kbps / 1000).toFixed(1)
                : "--"}
            </span>
            <span className="text-[10px] text-macos-secondary">Mbps</span>
          </div>
          <div className="flex flex-col items-center justify-center rounded-md bg-black/[0.04] py-2 px-1">
            <span className="text-sm font-semibold text-macos-text">
              {metricsAvailable ? metrics.latency_ms : "--"}
            </span>
            <span className="text-[10px] text-macos-secondary">ms</span>
          </div>
          <div className="flex flex-col items-center justify-center rounded-md bg-black/[0.04] py-2 px-1">
            <span className="text-sm font-semibold text-macos-text">
              {metricsAvailable ? formatBytes(metrics.bytes_sent) : "--"}
            </span>
            <span className="text-[10px] text-macos-secondary">流量</span>
          </div>
        </div>

        {/* Divider */}
        <div className="h-px bg-black/10" />

        {/* Connection list */}
        <div>
          <div className="mb-1.5 text-xs text-macos-secondary">
            连接 ({connections.length})
          </div>
          {connections.length === 0 ? (
            <div className="text-center text-xs text-macos-secondary py-2">
              暂无连接
            </div>
          ) : (
            <div className="space-y-1">
              {displayedConnections.map((conn) => {
                const connectedAt = new Date(conn.connected_at);
                const durationSecs = Math.floor(
                  (Date.now() - connectedAt.getTime()) / 1000,
                );
                return (
                  <div
                    key={`${conn.client_ip}-${conn.connected_at}`}
                    className="flex items-center justify-between text-xs px-2 py-1.5 rounded-md bg-black/[0.03]"
                  >
                    <div className="flex items-center gap-1.5 min-w-0">
                      <div className="h-1.5 w-1.5 shrink-0 rounded-full bg-macos-green" />
                      <span className="truncate text-macos-text font-medium">
                        {conn.client_name || conn.client_ip}
                      </span>
                      <span className="shrink-0 text-macos-secondary">
                        {formatDuration(durationSecs)}
                      </span>
                    </div>
                    <span className="shrink-0 ml-2 text-macos-secondary">
                      {formatBytes(conn.bytes_total)}
                    </span>
                  </div>
                );
              })}
              {extraCount > 0 && (
                <button
                  type="button"
                  onClick={handleShowMain}
                  className="w-full text-center text-xs text-macos-blue hover:underline py-0.5"
                >
                  +{extraCount} 更多
                </button>
              )}
            </div>
          )}
        </div>

        {/* Divider */}
        <div className="h-px bg-black/10" />

        {/* Action buttons row */}
        <div className="grid grid-cols-3 gap-2">
          <button
            type="button"
            disabled={toggling || status.state === "starting"}
            onClick={handleToggle}
            className={`rounded-md py-1.5 text-xs font-medium text-white transition-opacity disabled:opacity-50 ${
              status.running
                ? "bg-macos-red hover:opacity-80"
                : "bg-macos-green hover:opacity-80"
            }`}
          >
            {status.running ? "停止" : "启动"}
          </button>
          <button
            type="button"
            onClick={handleShowMain}
            className="rounded-md bg-macos-blue py-1.5 text-xs font-medium text-white hover:opacity-80 transition-opacity"
          >
            主窗口
          </button>
          <button
            type="button"
            onClick={handleShowLogs}
            className="flex items-center justify-center gap-1 rounded-md bg-black/[0.06] py-1.5 text-xs font-medium text-macos-text hover:bg-black/[0.1] transition-colors"
          >
            <FileText size={12} />
            日志
          </button>
        </div>
      </div>
    </div>
  );
}

export default Popover;
