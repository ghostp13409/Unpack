import { useEffect } from "react";
import { originOf } from "../lib/celebrate";
import { useStore } from "../store/useStore";
import { nextCursor, resolveShortcut } from "./shortcuts";

export function isTypingTarget(el: EventTarget | null): boolean {
  const node = el as HTMLElement | null;
  if (!node || !node.tagName) return false;
  const tag = node.tagName;
  return tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT" || node.isContentEditable === true;
}

/** Task rows currently rendered in the main pane, top to bottom. */
function visibleIds(): string[] {
  return [...document.querySelectorAll<HTMLElement>(".main [data-task-id]")].map((n) => n.dataset.taskId!).filter(Boolean);
}

function focusAfterRender(selector: string, select = false) {
  requestAnimationFrame(() => {
    const el = document.querySelector<HTMLInputElement>(selector);
    el?.focus();
    if (select) el?.select();
  });
}

/**
 * Global keyboard shortcuts: Ctrl+N capture, Ctrl+K search, Ctrl+, settings,
 * J/K move the cursor through the visible rows, Enter open, D done, E edit title,
 * Esc close the detail panel or sheet. Letters are ignored while typing.
 */
export function useShortcuts() {
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const action = resolveShortcut({
        key: e.key,
        ctrlKey: e.ctrlKey,
        metaKey: e.metaKey,
        altKey: e.altKey,
        shiftKey: e.shiftKey,
        typing: isTypingTarget(e.target),
      });
      if (!action) return;
      const s = useStore.getState();

      switch (action.type) {
        case "capture":
          e.preventDefault();
          s.openCapture();
          return;
        case "search":
          e.preventDefault();
          s.setView("tasks");
          focusAfterRender("#task-search", true);
          return;
        case "settings":
          e.preventDefault();
          s.setView("settings");
          return;
        case "escape": {
          if (isTypingTarget(e.target)) {
            (e.target as HTMLElement).blur();
            return;
          }
          if (s.captureOpen) s.closeCapture();
          else if (s.selectedId) s.selectTask(null);
          return;
        }
        case "move": {
          if (s.captureOpen) return;
          const ids = visibleIds();
          const next = nextCursor(ids, s.cursorId ?? s.selectedId, action.delta);
          if (!next) return;
          e.preventDefault();
          s.setCursor(next);
          document.querySelector(`.main [data-task-id="${next}"]`)?.scrollIntoView({ block: "nearest" });
          return;
        }
        case "open": {
          if (s.captureOpen || !s.cursorId) return;
          e.preventDefault();
          s.selectTask(s.cursorId);
          return;
        }
        case "toggleDone": {
          if (s.captureOpen) return;
          const id = s.cursorId && visibleIds().includes(s.cursorId) ? s.cursorId : s.selectedId;
          if (!id) return;
          e.preventDefault();
          const check = document.querySelector(`[data-task-id="${id}"] .row-check`);
          void s.toggleDone(id, check ? originOf(check) : undefined);
          return;
        }
        case "editTitle": {
          if (s.captureOpen) return;
          if (!s.selectedId && s.cursorId) s.selectTask(s.cursorId);
          e.preventDefault();
          focusAfterRender("#task-title", true);
          return;
        }
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);
}
