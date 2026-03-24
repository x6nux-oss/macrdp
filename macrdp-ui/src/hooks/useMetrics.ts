import { useState, useEffect, useRef } from "react";
import { listen } from "@tauri-apps/api/event";
import type { Metrics } from "../lib/types";

export function useMetrics() {
  const [metrics, setMetrics] = useState<Metrics | null>(null);
  const [stale, setStale] = useState(false);
  const timerRef = useRef<ReturnType<typeof setTimeout>>(undefined);

  useEffect(() => {
    const unlisten = listen<Metrics>("metrics", (event) => {
      setMetrics(event.payload);
      setStale(false);
      // Reset staleness timer (5 seconds)
      if (timerRef.current) clearTimeout(timerRef.current);
      timerRef.current = setTimeout(() => setStale(true), 5000);
    });

    return () => {
      unlisten.then((fn) => fn());
      if (timerRef.current) clearTimeout(timerRef.current);
    };
  }, []);

  return { metrics, stale };
}
