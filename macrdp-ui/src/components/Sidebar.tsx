import { NavLink } from "react-router-dom";
import {
  LayoutDashboard,
  Settings,
  Shield,
  FileText,
  BarChart3,
  Info,
} from "lucide-react";

const navItems = [
  { to: "/", icon: LayoutDashboard, label: "仪表盘" },
  { to: "/settings", icon: Settings, label: "设置" },
  { to: "/permissions", icon: Shield, label: "权限" },
  { to: "/logs", icon: FileText, label: "日志" },
  { to: "/statistics", icon: BarChart3, label: "统计" },
  { to: "/about", icon: Info, label: "关于" },
];

function Sidebar() {
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
            <Icon size={18} />
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
