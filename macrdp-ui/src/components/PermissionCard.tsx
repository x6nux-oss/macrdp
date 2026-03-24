import { api } from "../lib/ipc";

interface PermissionCardProps {
  name: string;
  description: string;
  granted: boolean;
  pane: string;
}

function PermissionCard({
  name,
  description,
  granted,
  pane,
}: PermissionCardProps) {
  return (
    <div className="flex items-center justify-between rounded-lg border border-black/10 bg-black/[0.04] px-5 py-4">
      <div className="flex items-center gap-3">
        <div
          className={`h-2.5 w-2.5 rounded-full ${granted ? "bg-macos-green" : "bg-macos-yellow"}`}
        />
        <div>
          <div className="text-sm font-medium text-macos-text">{name}</div>
          <div className="text-xs text-macos-secondary">{description}</div>
        </div>
      </div>
      <div>
        {granted ? (
          <span className="text-xs font-medium text-macos-green">
            已授权
          </span>
        ) : (
          <button
            type="button"
            className="rounded-md bg-macos-blue px-3 py-1.5 text-xs font-medium text-white transition-opacity hover:opacity-80"
            onClick={() => api.openSystemPreferences(pane)}
          >
            去授权
          </button>
        )}
      </div>
    </div>
  );
}

export default PermissionCard;
