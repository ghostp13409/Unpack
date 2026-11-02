import { getCurrentWebview } from "@tauri-apps/api/webview";
import { useEffect, useRef, useState } from "react";
import { inTauri } from "../lib/api";

/** Listens for OS file drops onto the window. Returns whether a drag is hovering. */
export function useDragDrop(onPaths: (paths: string[]) => void, enabled: boolean): boolean {
  const [isOver, setIsOver] = useState(false);
  const handler = useRef(onPaths);
  handler.current = onPaths;

  useEffect(() => {
    if (!enabled || !inTauri) return;
    let unlisten: (() => void) | undefined;
    let cancelled = false;
    getCurrentWebview()
      .onDragDropEvent((event) => {
        const p = event.payload;
        if (p.type === "enter" || p.type === "over") setIsOver(true);
        else if (p.type === "leave") setIsOver(false);
        else if (p.type === "drop") {
          setIsOver(false);
          handler.current(p.paths);
        }
      })
      .then((fn) => {
        if (cancelled) fn();
        else unlisten = fn;
      })
      .catch(() => undefined);
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [enabled]);

  return isOver;
}
