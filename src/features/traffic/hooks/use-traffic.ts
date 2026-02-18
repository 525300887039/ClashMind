import { useState, useEffect } from "react";
import { listen } from "@tauri-apps/api/event";

interface TrafficData {
  up: number;
  down: number;
}

export function useTraffic() {
  const [traffic, setTraffic] = useState<TrafficData>({ up: 0, down: 0 });

  useEffect(() => {
    const unlisten = listen<TrafficData>("traffic-update", (e) => {
      setTraffic(e.payload);
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  return traffic;
}
