import type { Connection } from "../lib/types";
import { formatBytes } from "../lib/format";

interface ConnectionItemProps {
  connection: Connection;
}

function ConnectionItem({ connection }: ConnectionItemProps) {
  const connectedAt = new Date(connection.connected_at);
  const durationSecs = Math.floor(
    (Date.now() - connectedAt.getTime()) / 1000,
  );
  const durationMin = Math.floor(durationSecs / 60);
  const durationStr =
    durationMin > 0 ? `${durationMin}m ${durationSecs % 60}s` : `${durationSecs}s`;

  return (
    <div className="flex items-center justify-between rounded-lg bg-black/[0.04] px-4 py-3">
      <div className="flex items-center gap-3">
        <div className="h-2 w-2 rounded-full bg-macos-green" />
        <div>
          <span className="text-sm font-medium text-macos-text">
            {connection.client_name || "Unknown"}
          </span>
          <span className="ml-2 text-xs text-macos-secondary">
            {connection.client_ip}
          </span>
        </div>
      </div>
      <div className="flex items-center gap-4 text-xs text-macos-secondary">
        <span>{durationStr}</span>
        <span>{formatBytes(connection.bytes_total)}</span>
      </div>
    </div>
  );
}

export default ConnectionItem;
