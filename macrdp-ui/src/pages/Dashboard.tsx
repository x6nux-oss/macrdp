import { useState, useEffect } from "react";
import { Square, Monitor, Clock } from "lucide-react";
import { useServerStatus } from "../hooks/useServerStatus";
import { useMetrics } from "../hooks/useMetrics";
import { useConnections } from "../hooks/useConnections";
import { api } from "../lib/ipc";
import { formatBytes, formatDuration } from "../lib/format";
import MetricsStrip from "../components/MetricsStrip";
import ServerInfoTags from "../components/ServerInfoTags";
import StatusBadge from "../components/StatusBadge";
import { Button } from "../components/ui/button";
import { Card, CardHeader, CardTitle, CardContent } from "../components/ui/card";
import type { UiConfig } from "../lib/types";

function Dashboard() {
  const status = useServerStatus();
  const { metrics, stale } = useMetrics();
  const connections = useConnections();
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [config, setConfig] = useState<UiConfig | null>(null);

  useEffect(() => {
    api.getConfig().then(setConfig).catch(console.error);
  }, []);

  const handleStop = async () => {
    setLoading(true);
    setError(null);
    try {
      await api.stopServer();
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : String(err);
      console.error("Failed to stop server:", msg);
      setError(msg);
    } finally {
      setLoading(false);
    }
  };

  const handleStart = async () => {
    setLoading(true);
    setError(null);
    try {
      await api.startServer();
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : String(err);
      console.error("Failed to start server:", msg);
      setError(msg);
    } finally {
      setLoading(false);
    }
  };

  const metricsAvailable = metrics && !stale;
  const port = config?.port ?? 3389;

  return (
    <div className="space-y-5">
      {/* Control bar */}
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-4">
          <h1 className="text-lg font-semibold text-macos-text">仪表盘</h1>
          <StatusBadge status={status.state} />
          {status.running && status.uptime_secs > 0 && (
            <div className="flex items-center gap-1.5 text-xs text-macos-secondary">
              <Clock className="h-3 w-3" />
              <span>{formatDuration(status.uptime_secs)}</span>
            </div>
          )}
        </div>
        <div>
          {status.running ? (
            <Button
              variant="destructive"
              size="sm"
              disabled={loading}
              onClick={handleStop}
            >
              <Square className="h-3.5 w-3.5" />
              停止服务
            </Button>
          ) : (
            <Button
              variant="default"
              size="sm"
              disabled={loading || status.state === "starting"}
              onClick={handleStart}
            >
              启动服务
            </Button>
          )}
        </div>
      </div>

      {/* Error banner */}
      {error && (
        <div className="rounded-lg border border-macos-red/20 bg-macos-red/10 px-4 py-3 text-sm text-macos-red">
          <span className="font-medium">操作失败: </span>{error}
        </div>
      )}

      {/* Server info tags */}
      {config && (
        <ServerInfoTags
          encoder={config.encoder}
          chromaMode={config.chroma_mode}
          tls={!!(config.tls_cert_path && config.tls_key_path)}
        />
      )}

      {/* Metrics strip */}
      {metricsAvailable ? (
        <MetricsStrip metrics={metrics} port={port} />
      ) : (
        <Card className="bg-macos-card backdrop-blur-xl border-macos-border">
          <CardContent>
            <div className="py-6 text-center text-sm text-macos-secondary">
              {status.running ? "等待指标数据..." : "服务未运行"}
            </div>
          </CardContent>
        </Card>
      )}

      {/* Connections section */}
      <Card className="bg-macos-card backdrop-blur-xl border-macos-border">
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <Monitor className="h-4 w-4 text-macos-secondary" />
            当前连接
            <span className="text-sm font-normal text-macos-secondary">
              ({connections.length})
            </span>
          </CardTitle>
        </CardHeader>
        <CardContent>
          {connections.length === 0 ? (
            <div className="rounded-lg bg-black/[0.04] dark:bg-white/[0.04] py-8 text-center text-sm text-macos-secondary">
              暂无连接
            </div>
          ) : (
            <div className="space-y-2">
              {connections.map((conn) => (
                <div
                  key={`${conn.client_ip}-${conn.connected_at}`}
                  className="flex items-center justify-between rounded-lg bg-black/[0.04] dark:bg-white/[0.04] px-4 py-3"
                >
                  <div className="flex items-center gap-3">
                    <div className="h-2 w-2 rounded-full bg-macos-green" />
                    <div>
                      <span className="text-sm font-medium text-macos-text">
                        {conn.client_name || "Unknown"}
                      </span>
                      <span className="ml-2 text-xs text-macos-secondary">
                        {conn.client_ip}
                      </span>
                    </div>
                  </div>
                  <div className="flex items-center gap-4 text-xs text-macos-secondary">
                    <span>
                      {formatDuration(
                        Math.floor(
                          (Date.now() - new Date(conn.connected_at).getTime()) / 1000
                        )
                      )}
                    </span>
                    <span>{formatBytes(conn.bytes_total)}</span>
                  </div>
                </div>
              ))}
            </div>
          )}
        </CardContent>
      </Card>
    </div>
  );
}

export default Dashboard;
