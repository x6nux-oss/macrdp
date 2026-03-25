import { useState, useEffect } from "react";
import { NavLink } from "react-router-dom";
import {
  LayoutDashboard,
  Settings,
  Shield,
  FileText,
  Activity,
  Info,
} from "lucide-react";
import { api } from "../lib/ipc";
import type { PermissionStatus } from "../lib/types";

const navItems = [
  { to: "/", icon: LayoutDashboard, label: "仪表盘" },
  { to: "/settings", icon: Settings, label: "设置" },
  { to: "/permissions", icon: Shield, label: "权限" },
  { to: "/logs", icon: FileText, label: "日志" },
  { to: "/statistics", icon: Activity, label: "统计" },
  { to: "/about", icon: Info, label: "关于" },
];

function Sidebar() {
  const [perms, setPerms] = useState<PermissionStatus | null>(null);

  useEffect(() => {
    const check = () => api.getPermissions().then(setPerms).catch(() => {});
    check();
    const interval = setInterval(check, 5000);
    return () => clearInterval(interval);
  }, []);

  const hasPermissionIssue =
    perms !== null && (!perms.screen_capture || !perms.accessibility);

  return (
    <aside className="flex h-full w-52 flex-col border-r border-macos-border bg-macos-card">
      {/* Titlebar drag region */}
      <div
        className="h-12 flex-shrink-0"
        data-tauri-drag-region
        style={{ WebkitAppRegion: "drag" } as React.CSSProperties}
      />

      {/* Navigation */}
      <nav className="flex-1 space-y-1 px-3">
        {navItems.map(({ to, icon: Icon, label }) => (
          <NavLink
            key={to}
            to={to}
            end={to === "/"}
            className={({ isActive }) =>
              `flex items-center gap-3 rounded-lg px-3 py-2 text-sm font-medium transition-colors ${
                isActive
                  ? "bg-macos-blue/10 text-macos-blue"
                  : "text-macos-secondary hover:bg-macos-bg hover:text-macos-text"
              }`
            }
          >
            <span className="relative">
              <Icon size={18} />
              {to === "/permissions" && hasPermissionIssue && (
                <span className="absolute -right-1 -top-1 h-2 w-2 rounded-full bg-red-500" />
              )}
            </span>
            <span>{label}</span>
          </NavLink>
        ))}
      </nav>

      {/* Bottom status */}
      <div className="border-t border-macos-border p-3">
        <div className="flex items-center gap-2">
          <div className="h-2 w-2 rounded-full bg-macos-secondary" />
          <span className="text-xs text-macos-secondary">未运行</span>
        </div>
      </div>
    </aside>
  );
}

export default Sidebar;
