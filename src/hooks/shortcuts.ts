/** Pure keyboard-shortcut logic; `useShortcuts` wires it to the DOM and the store. */

export interface KeyLike {
  key: string;
  ctrlKey: boolean;
  metaKey: boolean;
  altKey: boolean;
  shiftKey: boolean;
  /** True when the event target is an input, textarea, select or contenteditable. */
  typing: boolean;
}

export type ShortcutAction =
  | { type: "capture" }
  | { type: "search" }
  | { type: "settings" }
  | { type: "move"; delta: 1 | -1 }
  | { type: "open" }
  | { type: "toggleDone" }
  | { type: "editTitle" }
  | { type: "escape" };

export function resolveShortcut(e: KeyLike): ShortcutAction | null {
  if (e.key === "Escape") return { type: "escape" };
  if (e.typing) return null;
  const k = e.key.length === 1 ? e.key.toLowerCase() : e.key;
  if (e.ctrlKey && !e.metaKey && !e.altKey) {
    if (k === "n") return { type: "capture" };
    if (k === "k") return { type: "search" };
    if (k === ",") return { type: "settings" };
    return null;
  }
  if (e.ctrlKey || e.metaKey || e.altKey) return null;
  switch (k) {
    case "j":
      return { type: "move", delta: 1 };
    case "k":
      return { type: "move", delta: -1 };
    case "Enter":
      return { type: "open" };
    case "d":
      return { type: "toggleDone" };
    case "e":
      return { type: "editTitle" };
    default:
      return null;
  }
}

/** The row the cursor lands on after moving by `delta` through `ids`; edges clamp. */
export function nextCursor(ids: string[], current: string | null, delta: 1 | -1): string | null {
  if (ids.length === 0) return null;
  const i = current === null ? -1 : ids.indexOf(current);
  if (i === -1) return delta === 1 ? ids[0] : ids[ids.length - 1];
  const j = Math.min(ids.length - 1, Math.max(0, i + delta));
  return ids[j];
}
