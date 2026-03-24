import { useState, useEffect } from "react";
import { listen } from "@tauri-apps/api/event";
import { api } from "../lib/ipc";
import type { ServerStatus } from "../lib/types";

export function useServerStatus() {
  const [status, setStatus] = useState<ServerStatus>({
    running: false,
    state: "stopped",
    uptime_secs: 0,
    pid: null,
  });

  useEffect(() => {
    // Get initial status
    api.getServerStatus().then(setStatus).catch(console.error);

    // Listen for updates
    const unlisten = listen<ServerStatus>("server-status", (event) => {
      setStatus(event.payload);
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  return status;
}
