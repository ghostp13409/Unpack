import { Loader2 } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { Button } from "../components/Button";
import { DropZone, SUPPORTED } from "../components/DropZone";
import { Sheet } from "../components/Sheet";
import { useDragDrop } from "../hooks/useDragDrop";
import * as api from "../lib/api";
import type { Capture } from "../lib/types";
import { notReadyMessage, providerReady } from "../store/provider";
import { useStore } from "../store/useStore";
import { ReviewPanel } from "./ReviewPanel";

export function CaptureSheet() {
  const open = useStore((s) => s.captureOpen);
  const close = useStore((s) => s.closeCapture);
  const createQuick = useStore((s) => s.createQuick);
  const selectTask = useStore((s) => s.selectTask);
  const ready = useStore((s) => providerReady(s.settings));
  const blockedMessage = useStore((s) => notReadyMessage(s.settings));
  const setView = useStore((s) => s.setView);

  const [text, setText] = useState("");
  const [files, setFiles] = useState<string[]>([]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [capture, setCapture] = useState<Capture | null>(null);
  const textRef = useRef<HTMLTextAreaElement>(null);

  const reset = useCallback(() => {
    setText("");
    setFiles([]);
    setBusy(false);
    setError(null);
    setCapture(null);
  }, []);

  const onClose = useCallback(() => {
    if (busy) return;
    close();
    reset();
  }, [busy, close, reset]);

  const addFiles = useCallback((paths: string[]) => {
    const bad = paths.filter((p) => !SUPPORTED.includes((p.split(".").pop() ?? "").toLowerCase()));
    if (bad.length) setError(`Unsupported file type: ${bad.map((b) => b.split(/[\\/]/).pop()).join(", ")}`);
    const good = paths.filter((p) => !bad.includes(p));
    setFiles((f) => [...f, ...good.filter((g) => !f.includes(g))]);
  }, []);

  const isOver = useDragDrop(addFiles, open && !capture);

  useEffect(() => {
    if (open) setTimeout(() => textRef.current?.focus(), 30);
  }, [open]);

  const process = async () => {
    if (!text.trim() && files.length === 0) return;
    if (!ready) {
      setError(blockedMessage);
      return;
    }
    setBusy(true);
    setError(null);
    try {
      const cap = await api.startCapture(text.trim() || null, files);
      setCapture(cap);
    } catch (e) {
      setError(api.errorMessage(e));
    } finally {
      setBusy(false);
    }
  };

  const quickAdd = async () => {
    const t = await createQuick(text);
    if (t) {
      selectTask(t.id);
      setView("tasks");
      onClose();
    }
  };

  const onKey = (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) {
      e.preventDefault();
      void process();
    } else if (e.key === "Enter" && !e.shiftKey && files.length === 0 && text.trim() && !text.includes("\n")) {
      e.preventDefault();
      void quickAdd();
    }
  };

  const canProcess = !busy && (text.trim().length > 0 || files.length > 0);

  return (
    <Sheet
      open={open}
      title={capture ? "Review" : "Capture"}
      onClose={onClose}
      footer={
        capture ? undefined : (
          <>
            <span className="hint">Enter adds a quick task &middot; Ctrl+Enter processes with AI</span>
            <Button variant="ghost" onClick={onClose} disabled={busy}>
              Cancel
            </Button>
            <Button variant="primary" onClick={process} disabled={!canProcess}>
              {busy ? <Loader2 size={14} className="spin" /> : null}
              {busy ? "Reading" : "Process"}
            </Button>
          </>
        )
      }
    >
      {capture ? (
        <ReviewPanel capture={capture} onDone={onClose} />
      ) : (
        <>
          <textarea
            ref={textRef}
            className="textarea capture-text"
            placeholder="Type a task, or paste notes, a Teams message, an email..."
            value={text}
            onChange={(e) => setText(e.target.value)}
            onKeyDown={onKey}
            disabled={busy}
          />
          <DropZone files={files} onAdd={addFiles} onRemove={(p) => setFiles((f) => f.filter((x) => x !== p))} isOver={isOver} />
          {busy && (
            <div className="notice">
              <Loader2 size={14} className="spin" />
              {files.length > 0 ? `Reading ${files.length === 1 ? "1 file" : `${files.length} files`} and finding tasks...` : "Finding tasks..."}
            </div>
          )}
          {error && <div className="error">{error}</div>}
          {!ready && !error && (
            <div className="notice">
              {blockedMessage}{" "}
              <button className="browse" onClick={() => { onClose(); setView("settings"); }}>
                Open Settings
              </button>
            </div>
          )}
        </>
      )}
    </Sheet>
  );
}
