import { useState } from "react";
import { api } from "../lib/ipc";

function About() {
  const [updateStatus, setUpdateStatus] = useState<
    "idle" | "checking" | "latest" | "available" | "error"
  >("idle");
  const [updateInfo, setUpdateInfo] = useState<{
    version?: string;
    url?: string;
  }>({});

  const handleCheckUpdate = async () => {
    setUpdateStatus("checking");
    try {
      const result = await api.checkForUpdates();
      if (result.available) {
        setUpdateStatus("available");
        setUpdateInfo({ version: result.version, url: result.url });
      } else {
        setUpdateStatus("latest");
      }
    } catch (err) {
      console.error("Failed to check for updates:", err);
      setUpdateStatus("error");
    }
  };

  return (
    <div className="space-y-6">
      {/* App info card */}
      <div className="rounded-lg border border-black/10 bg-black/[0.04] p-6 text-center">
        <div className="mx-auto mb-4 flex h-16 w-16 items-center justify-center rounded-2xl bg-macos-blue/10">
          <svg
            className="h-8 w-8 text-macos-blue"
            fill="none"
            viewBox="0 0 24 24"
            strokeWidth={1.5}
            stroke="currentColor"
          >
            <path
              strokeLinecap="round"
              strokeLinejoin="round"
              d="M9 17.25v1.007a3 3 0 0 1-.879 2.122L7.5 21h9l-.621-.621A3 3 0 0 1 15 18.257V17.25m6-12V15a2.25 2.25 0 0 1-2.25 2.25H5.25A2.25 2.25 0 0 1 3 15V5.25A2.25 2.25 0 0 1 5.25 3h13.5A2.25 2.25 0 0 1 21 5.25Z"
            />
          </svg>
        </div>
        <h1 className="text-xl font-semibold text-macos-text">macrdp</h1>
        <p className="mt-1 text-sm text-macos-secondary">
          macOS 远程桌面服务端
        </p>
        <p className="mt-2 font-mono text-xs text-macos-secondary">
          版本 1.0.0
        </p>
      </div>

      {/* Check for updates */}
      <section>
        <h2 className="mb-3 text-base font-medium text-macos-text">
          软件更新
        </h2>
        <div className="rounded-lg border border-black/10 bg-black/[0.04] p-4">
          <div className="flex items-center justify-between">
            <div>
              {updateStatus === "idle" && (
                <p className="text-sm text-macos-secondary">
                  点击检查是否有新版本
                </p>
              )}
              {updateStatus === "checking" && (
                <p className="text-sm text-macos-secondary">正在检查...</p>
              )}
              {updateStatus === "latest" && (
                <p className="text-sm text-macos-green">已是最新版本</p>
              )}
              {updateStatus === "available" && (
                <div>
                  <p className="text-sm text-macos-text">
                    发现新版本: {updateInfo.version}
                  </p>
                  {updateInfo.url && (
                    <a
                      href={updateInfo.url}
                      target="_blank"
                      rel="noopener noreferrer"
                      className="mt-1 inline-block text-xs text-macos-blue hover:underline"
                    >
                      前往下载
                    </a>
                  )}
                </div>
              )}
              {updateStatus === "error" && (
                <p className="text-sm text-macos-red">检查更新失败</p>
              )}
            </div>
            <button
              type="button"
              disabled={updateStatus === "checking"}
              onClick={handleCheckUpdate}
              className="rounded-lg bg-macos-blue px-4 py-2 text-sm font-medium text-white transition-opacity hover:opacity-80 disabled:opacity-50"
            >
              检查更新
            </button>
          </div>
        </div>
      </section>

      {/* Links section */}
      <section>
        <h2 className="mb-3 text-base font-medium text-macos-text">
          相关链接
        </h2>
        <div className="rounded-lg border border-black/10 bg-black/[0.04] divide-y divide-black/5">
          <a
            href="https://github.com/aspect-build/macrdp"
            target="_blank"
            rel="noopener noreferrer"
            className="flex items-center justify-between px-4 py-3 transition-colors hover:bg-black/[0.04]"
          >
            <span className="text-sm text-macos-text">GitHub 项目</span>
            <svg
              className="h-4 w-4 text-macos-secondary"
              fill="none"
              viewBox="0 0 24 24"
              strokeWidth={1.5}
              stroke="currentColor"
            >
              <path
                strokeLinecap="round"
                strokeLinejoin="round"
                d="m8.25 4.5 7.5 7.5-7.5 7.5"
              />
            </svg>
          </a>
          <div className="flex items-center justify-between px-4 py-3">
            <span className="text-sm text-macos-text">许可证</span>
            <span className="text-sm text-macos-secondary">MIT</span>
          </div>
          <div className="flex items-center justify-between px-4 py-3">
            <span className="text-sm text-macos-text">协议栈</span>
            <span className="text-sm text-macos-secondary">IronRDP</span>
          </div>
          <div className="flex items-center justify-between px-4 py-3">
            <span className="text-sm text-macos-text">编码器</span>
            <span className="text-sm text-macos-secondary">
              OpenH264 (H.264)
            </span>
          </div>
        </div>
      </section>
    </div>
  );
}

export default About;
