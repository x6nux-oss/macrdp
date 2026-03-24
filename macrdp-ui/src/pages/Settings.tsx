import { useState, useEffect } from "react";
import { api } from "../lib/ipc";
import type { ServerConfig } from "../lib/types";

function Settings() {
  const [config, setConfig] = useState<ServerConfig | null>(null);
  const [restartBanner, setRestartBanner] = useState(false);
  const [autostart, setAutostart] = useState(false);

  useEffect(() => {
    api.getConfig().then(setConfig).catch(console.error);
  }, []);

  const updateConfig = async (key: string, value: unknown) => {
    try {
      const result = await api.setConfig(key, value);
      if (result.restart_required) {
        setRestartBanner(true);
      }
      // Refresh config
      const updated = await api.getConfig();
      setConfig(updated);
    } catch (err) {
      console.error("Failed to update config:", err);
    }
  };

  const handleAutostart = async (enabled: boolean) => {
    try {
      await api.setAutostart(enabled);
      setAutostart(enabled);
    } catch (err) {
      console.error("Failed to set autostart:", err);
    }
  };

  if (!config) {
    return (
      <div className="flex items-center justify-center py-20 text-sm text-macos-secondary">
        加载配置中...
      </div>
    );
  }

  const inputClass =
    "w-full rounded-lg border border-black/10 bg-white px-3 py-2 text-sm text-macos-text outline-none focus:border-macos-blue focus:ring-1 focus:ring-macos-blue/30";
  const selectClass =
    "w-full rounded-lg border border-black/10 bg-white px-3 py-2 text-sm text-macos-text outline-none focus:border-macos-blue focus:ring-1 focus:ring-macos-blue/30";
  const labelClass = "block text-xs font-medium text-macos-secondary mb-1.5";

  return (
    <div className="space-y-6">
      {restartBanner && (
        <div className="rounded-lg bg-macos-yellow/20 border border-macos-yellow/40 px-4 py-3 text-sm text-macos-text">
          需要重启服务生效
        </div>
      )}

      {/* RDP Config */}
      <section>
        <h2 className="mb-3 text-base font-medium text-macos-text">
          RDP 配置
        </h2>
        <div className="grid grid-cols-3 gap-4 rounded-lg bg-black/[0.04] p-4">
          <div>
            <label className={labelClass}>端口</label>
            <input
              type="number"
              className={inputClass}
              value={config.port}
              onChange={(e) =>
                updateConfig("port", parseInt(e.target.value, 10))
              }
            />
          </div>
          <div>
            <label className={labelClass}>帧率</label>
            <select
              className={selectClass}
              value={config.frame_rate}
              onChange={(e) =>
                updateConfig("frame_rate", parseInt(e.target.value, 10))
              }
            >
              <option value={30}>30 FPS</option>
              <option value={60}>60 FPS</option>
              <option value={120}>120 FPS</option>
            </select>
          </div>
          <div>
            <label className={labelClass}>码率 (Mbps)</label>
            <input
              type="number"
              className={inputClass}
              value={config.bitrate_mbps}
              onChange={(e) =>
                updateConfig(
                  "bitrate_mbps",
                  parseFloat(e.target.value),
                )
              }
            />
          </div>
        </div>
      </section>

      {/* TLS */}
      <section>
        <h2 className="mb-3 text-base font-medium text-macos-text">TLS</h2>
        <div className="grid grid-cols-2 gap-4 rounded-lg bg-black/[0.04] p-4">
          <div>
            <label className={labelClass}>证书路径</label>
            <input
              type="text"
              className={inputClass}
              value={config.tls_cert_path}
              placeholder="/path/to/cert.pem"
              onChange={(e) =>
                updateConfig("tls_cert_path", e.target.value)
              }
            />
          </div>
          <div>
            <label className={labelClass}>私钥路径</label>
            <input
              type="text"
              className={inputClass}
              value={config.tls_key_path}
              placeholder="/path/to/key.pem"
              onChange={(e) =>
                updateConfig("tls_key_path", e.target.value)
              }
            />
          </div>
        </div>
      </section>

      {/* Network */}
      <section>
        <h2 className="mb-3 text-base font-medium text-macos-text">
          网络
        </h2>
        <div className="grid grid-cols-2 gap-4 rounded-lg bg-black/[0.04] p-4">
          <div>
            <label className={labelClass}>绑定地址</label>
            <input
              type="text"
              className={inputClass}
              value={config.bind_address}
              placeholder="0.0.0.0"
              onChange={(e) =>
                updateConfig("bind_address", e.target.value)
              }
            />
          </div>
          <div>
            <label className={labelClass}>最大连接数</label>
            <input
              type="number"
              className={inputClass}
              value={config.max_connections}
              onChange={(e) =>
                updateConfig(
                  "max_connections",
                  parseInt(e.target.value, 10),
                )
              }
            />
          </div>
        </div>
      </section>

      {/* General */}
      <section>
        <h2 className="mb-3 text-base font-medium text-macos-text">
          通用
        </h2>
        <div className="flex items-center gap-6 rounded-lg bg-black/[0.04] p-4">
          <div className="flex-1">
            <label className={labelClass}>日志级别</label>
            <select
              className={selectClass}
              value={config.log_level}
              onChange={(e) =>
                updateConfig("log_level", e.target.value)
              }
            >
              <option value="trace">trace</option>
              <option value="debug">debug</option>
              <option value="info">info</option>
              <option value="warn">warn</option>
              <option value="error">error</option>
            </select>
          </div>
          <div className="flex items-center gap-3">
            <label className="text-xs font-medium text-macos-secondary">
              开机自启
            </label>
            <button
              type="button"
              role="switch"
              aria-checked={autostart}
              onClick={() => handleAutostart(!autostart)}
              className={`relative h-6 w-11 rounded-full transition-colors ${
                autostart ? "bg-macos-green" : "bg-black/10"
              }`}
            >
              <span
                className={`absolute top-0.5 left-0.5 h-5 w-5 rounded-full bg-white shadow transition-transform ${
                  autostart ? "translate-x-5" : "translate-x-0"
                }`}
              />
            </button>
          </div>
        </div>
      </section>
    </div>
  );
}

export default Settings;
