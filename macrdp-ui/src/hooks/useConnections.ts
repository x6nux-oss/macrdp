import { useState, useEffect } from "react";
import { listen } from "@tauri-apps/api/event";
import { api } from "../lib/ipc";
import type { Connection } from "../lib/types";

export function useConnections() {
  const [connections, setConnections] = useState<Connection[]>([]);

  useEffect(() => {
    api.getConnections().then(setConnections).catch(console.error);

    const unlisten = listen<Connection[]>("connections", (event) => {
      setConnections(event.payload);
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  return connections;
}
