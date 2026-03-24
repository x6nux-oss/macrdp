import { useState } from "react";
import { useServerStatus } from "../hooks/useServerStatus";
import { useMetrics } from "../hooks/useMetrics";
import { useConnections } from "../hooks/useConnections";
import { api } from "../lib/ipc";
import { formatBytes, formatDuration } from "../lib/format";
import MetricCard from "../components/MetricCard";
import ConnectionItem from "../components/ConnectionItem";
import StatusBadge from "../components/StatusBadge";

function Dashboard() {
  const status = useServerStatus();
  const { metrics, stale } = useMetrics();
  const connections = useConnections();
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const handleToggle = async () => {
    setLoading(true);
    setError(null);
    try {
      if (status.running) {
        await api.stopServer();
      } else {
        await api.startServer();
      }
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : String(err);
      console.error("Failed to toggle server:", msg);
      setError(msg);
    } finally {
      setLoading(false);
    }
  };

  const metricsAvailable = metrics && !stale;

  return (
    <div className="space-y-6">
      {/* Top section: toggle + status */}
      <div className="flex items-center gap-5">
        <button
          type="button"
          disabled={loading || status.state === "starting"}
          onClick={handleToggle}
          className={`rounded-lg px-6 py-2.5 text-sm font-medium text-white transition-opacity disabled:opacity-50 ${
            status.running
              ? "bg-macos-red hover:opacity-80"
              : "bg-macos-green hover:opacity-80"
          }`}
        >
          {status.running ? "停止服务" : "启动服务"}
        </button>
        <StatusBadge status={status.state} />
        {status.running && status.uptime_secs > 0 && (
          <span className="text-xs text-macos-secondary">
            运行时间: {formatDuration(status.uptime_secs)}
          </span>
        )}
      </div>

      {/* Error banner */}
      {error && (
        <div className="rounded-lg border border-macos-red/20 bg-macos-red/10 px-4 py-3 text-sm text-macos-red">
          <span className="font-medium">启动失败: </span>{error}
        </div>
      )}

      {/* Metrics grid */}
      <div className="grid grid-cols-4 gap-4">
        <MetricCard
          value={metricsAvailable ? metrics.fps : "--"}
          label="FPS"
        />
        <MetricCard
          value={
            metricsAvailable
              ? (metrics.bitrate_kbps / 1000).toFixed(1)
              : "--"
          }
          label="码率"
          unit="Mbps"
        />
        <MetricCard
          value={metricsAvailable ? metrics.latency_ms : "--"}
          label="延迟"
          unit="ms"
        />
        <MetricCard
          value={
            metricsAvailable ? formatBytes(metrics.bytes_sent) : "--"
          }
          label="流量"
        />
      </div>

      {/* Connections section */}
      <div>
        <div className="mb-3 flex items-center justify-between">
          <h2 className="text-base font-medium text-macos-text">
            当前连接
            <span className="ml-2 text-sm font-normal text-macos-secondary">
              ({connections.length})
            </span>
          </h2>
        </div>
        {connections.length === 0 ? (
          <div className="rounded-lg bg-black/[0.04] py-8 text-center text-sm text-macos-secondary">
            暂无连接
          </div>
        ) : (
          <div className="space-y-2">
            {connections.map((conn) => (
              <ConnectionItem
                key={`${conn.client_ip}-${conn.connected_at}`}
                connection={conn}
              />
            ))}
          </div>
        )}
      </div>
    </div>
  );
}

export default Dashboard;
