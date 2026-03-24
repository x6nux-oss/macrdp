import { useState, useEffect, useMemo } from "react";
import { api } from "../lib/ipc";
import { formatBytes, formatDuration } from "../lib/format";
import type { ConnectionHistory, TrafficStats } from "../lib/types";

const PAGE_SIZE = 20;

function Statistics() {
  // Connection history
  const [history, setHistory] = useState<ConnectionHistory[]>([]);
  const [page, setPage] = useState(0);
  const [hasMore, setHasMore] = useState(true);

  // Traffic stats
  const [trafficStats, setTrafficStats] = useState<TrafficStats[]>([]);

  useEffect(() => {
    fetchHistory(0);
    api
      .getTrafficStats(30)
      .then(setTrafficStats)
      .catch(console.error);
  }, []);

  const fetchHistory = async (p: number) => {
    try {
      const data = await api.getConnectionHistory(PAGE_SIZE, p * PAGE_SIZE);
      setHistory(data);
      setPage(p);
      setHasMore(data.length === PAGE_SIZE);
    } catch (err) {
      console.error("Failed to fetch connection history:", err);
    }
  };

  const totalTraffic = useMemo(
    () => trafficStats.reduce((sum, d) => sum + d.bytes_sent, 0),
    [trafficStats],
  );

  const totalConnections = useMemo(
    () => trafficStats.reduce((sum, d) => sum + d.connection_count, 0),
    [trafficStats],
  );

  const maxBytes = useMemo(
    () => Math.max(...trafficStats.map((d) => d.bytes_sent), 1),
    [trafficStats],
  );

  const formatDate = (dateStr: string) => {
    try {
      const d = new Date(dateStr);
      return d.toLocaleDateString("zh-CN", {
        month: "2-digit",
        day: "2-digit",
      });
    } catch {
      return dateStr;
    }
  };

  const formatDateTime = (dateStr: string) => {
    try {
      const d = new Date(dateStr);
      return d.toLocaleString("zh-CN", {
        month: "2-digit",
        day: "2-digit",
        hour: "2-digit",
        minute: "2-digit",
        hour12: false,
      });
    } catch {
      return dateStr;
    }
  };

  return (
    <div className="space-y-6">
      {/* Traffic stats section */}
      <section>
        <h2 className="mb-3 text-base font-medium text-macos-text">
          流量统计
        </h2>

        {/* Summary */}
        <div className="mb-4 grid grid-cols-2 gap-4">
          <div className="rounded-lg bg-black/[0.04] p-4">
            <div className="text-xs text-macos-secondary">30 天总流量</div>
            <div className="mt-1 text-lg font-semibold text-macos-text">
              {formatBytes(totalTraffic)}
            </div>
          </div>
          <div className="rounded-lg bg-black/[0.04] p-4">
            <div className="text-xs text-macos-secondary">30 天总连接数</div>
            <div className="mt-1 text-lg font-semibold text-macos-text">
              {totalConnections}
            </div>
          </div>
        </div>

        {/* Bar chart */}
        {trafficStats.length === 0 ? (
          <div className="rounded-lg bg-black/[0.04] py-8 text-center text-sm text-macos-secondary">
            暂无流量数据
          </div>
        ) : (
          <div className="rounded-lg border border-black/10 bg-black/[0.04] p-4">
            <div className="space-y-1.5">
              {trafficStats.map((day) => (
                <div key={day.date} className="flex items-center gap-3">
                  <span className="w-12 flex-shrink-0 text-right font-mono text-xs text-macos-secondary">
                    {formatDate(day.date)}
                  </span>
                  <div className="flex-1">
                    <div
                      className="h-5 rounded bg-macos-blue/60 transition-all"
                      style={{
                        width: `${Math.max((day.bytes_sent / maxBytes) * 100, 1)}%`,
                      }}
                    />
                  </div>
                  <span className="w-20 flex-shrink-0 text-right font-mono text-xs text-macos-secondary">
                    {formatBytes(day.bytes_sent)}
                  </span>
                </div>
              ))}
            </div>
          </div>
        )}
      </section>

      {/* Connection history table */}
      <section>
        <h2 className="mb-3 text-base font-medium text-macos-text">
          连接历史
        </h2>

        {history.length === 0 && page === 0 ? (
          <div className="rounded-lg bg-black/[0.04] py-8 text-center text-sm text-macos-secondary">
            暂无连接记录
          </div>
        ) : (
          <>
            <div className="overflow-hidden rounded-lg border border-black/10">
              <table className="w-full text-sm">
                <thead>
                  <tr className="border-b border-black/10 bg-black/[0.04]">
                    <th className="px-3 py-2 text-left text-xs font-medium text-macos-secondary">
                      客户端
                    </th>
                    <th className="px-3 py-2 text-left text-xs font-medium text-macos-secondary">
                      IP
                    </th>
                    <th className="px-3 py-2 text-left text-xs font-medium text-macos-secondary">
                      连接时间
                    </th>
                    <th className="px-3 py-2 text-left text-xs font-medium text-macos-secondary">
                      断开时间
                    </th>
                    <th className="px-3 py-2 text-right text-xs font-medium text-macos-secondary">
                      时长
                    </th>
                    <th className="px-3 py-2 text-right text-xs font-medium text-macos-secondary">
                      流量
                    </th>
                  </tr>
                </thead>
                <tbody className="divide-y divide-black/5">
                  {history.map((conn) => (
                    <tr
                      key={conn.id}
                      className="bg-white transition-colors hover:bg-black/[0.02]"
                    >
                      <td className="px-3 py-2 text-macos-text">
                        {conn.client_name || "-"}
                      </td>
                      <td className="px-3 py-2 font-mono text-xs text-macos-secondary">
                        {conn.client_ip}
                      </td>
                      <td className="px-3 py-2 text-xs text-macos-secondary">
                        {formatDateTime(conn.connected_at)}
                      </td>
                      <td className="px-3 py-2 text-xs text-macos-secondary">
                        {formatDateTime(conn.disconnected_at)}
                      </td>
                      <td className="px-3 py-2 text-right text-xs text-macos-secondary">
                        {formatDuration(conn.duration_secs)}
                      </td>
                      <td className="px-3 py-2 text-right text-xs text-macos-secondary">
                        {formatBytes(conn.bytes_total)}
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>

            {/* Pagination */}
            <div className="mt-3 flex items-center justify-between">
              <span className="text-xs text-macos-secondary">
                第 {page + 1} 页
              </span>
              <div className="flex gap-2">
                <button
                  type="button"
                  disabled={page === 0}
                  onClick={() => fetchHistory(page - 1)}
                  className="rounded-lg border border-black/10 bg-white px-3 py-1.5 text-xs font-medium text-macos-secondary transition-colors hover:bg-black/[0.04] disabled:opacity-40"
                >
                  上一页
                </button>
                <button
                  type="button"
                  disabled={!hasMore}
                  onClick={() => fetchHistory(page + 1)}
                  className="rounded-lg border border-black/10 bg-white px-3 py-1.5 text-xs font-medium text-macos-secondary transition-colors hover:bg-black/[0.04] disabled:opacity-40"
                >
                  下一页
                </button>
              </div>
            </div>
          </>
        )}
      </section>
    </div>
  );
}

export default Statistics;
