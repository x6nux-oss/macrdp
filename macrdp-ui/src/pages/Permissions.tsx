import { useState, useEffect } from "react";
import { useLocation, useNavigate } from "react-router-dom";
import { api } from "../lib/ipc";
import type { PermissionStatus } from "../lib/types";
import PermissionCard from "../components/PermissionCard";

const permissionDefs = [
  {
    key: "screen_capture" as keyof PermissionStatus,
    name: "屏幕录制",
    description: "用于捕获屏幕内容",
    pane: "screen_capture",
  },
  {
    key: "accessibility" as keyof PermissionStatus,
    name: "辅助功能",
    description: "用于注入键盘和鼠标事件",
    pane: "accessibility",
  },
  {
    key: "microphone" as keyof PermissionStatus,
    name: "麦克风",
    description: "用于音频转发（Phase 3）",
    pane: "microphone",
  },
];

function Permissions() {
  const [perms, setPerms] = useState<PermissionStatus | null>(null);
  const location = useLocation();
  const navigate = useNavigate();
  const firstLaunch = (location.state as { firstLaunch?: boolean })?.firstLaunch ?? false;

  useEffect(() => {
    // Initial fetch
    api.getPermissions().then(setPerms).catch(console.error);

    // Poll every 2 seconds
    const interval = setInterval(() => {
      api.getPermissions().then(setPerms).catch(console.error);
    }, 2000);

    return () => clearInterval(interval);
  }, []);

  const allRequiredGranted =
    perms?.screen_capture === true && perms?.accessibility === true;

  return (
    <div className="space-y-6">
      {firstLaunch && (
        <div className="rounded-lg bg-macos-blue/10 border border-macos-blue/20 px-4 py-3">
          <h2 className="text-sm font-medium text-macos-text">
            首次使用需要授权以下权限
          </h2>
        </div>
      )}

      <h1 className="text-lg font-semibold text-macos-text">
        系统权限
      </h1>

      <div className="space-y-3">
        {permissionDefs.map((def) => (
          <PermissionCard
            key={def.key}
            name={def.name}
            description={def.description}
            granted={perms?.[def.key] ?? false}
            pane={def.pane}
          />
        ))}
      </div>

      {firstLaunch && (
        <div className="flex gap-3">
          <button
            type="button"
            disabled={!allRequiredGranted}
            onClick={() => navigate("/")}
            className="rounded-lg bg-macos-blue px-6 py-2.5 text-sm font-medium text-white transition-opacity hover:opacity-80 disabled:opacity-40"
          >
            继续
          </button>
          <button
            type="button"
            onClick={() => navigate("/")}
            className="rounded-lg bg-black/[0.06] px-6 py-2.5 text-sm font-medium text-macos-text transition-opacity hover:opacity-80"
          >
            跳过
          </button>
        </div>
      )}
    </div>
  );
}

export default Permissions;
