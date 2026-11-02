import { Copy, Minus, Square, X } from "lucide-react";
import { useEffect, useState } from "react";
import { inTauri } from "../lib/api";

async function win() {
  const { getCurrentWindow } = await import("@tauri-apps/api/window");
  return getCurrentWindow();
}

/** The app's own window chrome: brand on the left, a drag region, and window controls. */
export function TitleBar() {
  const [maximized, setMaximized] = useState(false);

  useEffect(() => {
    if (!inTauri) return;
    let unlisten: (() => void) | undefined;
    void win().then(async (w) => {
      setMaximized(await w.isMaximized());
      unlisten = await w.onResized(async () => setMaximized(await w.isMaximized()));
    });
    return () => unlisten?.();
  }, []);

  const act = (fn: (w: Awaited<ReturnType<typeof win>>) => Promise<void>) => () => {
    if (!inTauri) return;
    void win().then(fn);
  };

  return (
    <header className="titlebar" data-tauri-drag-region>
      <span className="tb-brand" data-tauri-drag-region>
        <img src="/icon-32.png" alt="" width={16} height={16} draggable={false} />
        Unpack
      </span>
      <span className="tb-spacer" data-tauri-drag-region />
      <div className="tb-controls">
        <button className="tb-btn" onClick={act((w) => w.minimize())} aria-label="Minimize" title="Minimize">
          <Minus size={14} />
        </button>
        <button className="tb-btn" onClick={act((w) => w.toggleMaximize())} aria-label={maximized ? "Restore" : "Maximize"} title={maximized ? "Restore" : "Maximize"}>
          {maximized ? <Copy size={12} /> : <Square size={12} />}
        </button>
        <button className="tb-btn tb-close" onClick={act((w) => w.close())} aria-label="Close" title="Close">
          <X size={15} />
        </button>
      </div>
    </header>
  );
}
