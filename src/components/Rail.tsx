import { History, ListTodo, Plus, Settings, Target } from "lucide-react";
import { providerReady } from "../store/provider";
import { useStore, type View } from "../store/useStore";

const ITEMS: { view: View; label: string; Icon: typeof Target }[] = [
  { view: "focus", label: "Focus", Icon: Target },
  { view: "tasks", label: "Tasks", Icon: ListTodo },
  { view: "activity", label: "Activity", Icon: History },
];

export function Rail() {
  const view = useStore((s) => s.view);
  const setView = useStore((s) => s.setView);
  const openCapture = useStore((s) => s.openCapture);
  const ready = useStore((s) => providerReady(s.settings));

  return (
    <nav className="rail" aria-label="Main">
      {ITEMS.map(({ view: v, label, Icon }) => (
        <button
          key={v}
          className={`rail-btn ${view === v ? "active" : ""}`}
          title={label}
          aria-label={label}
          onClick={() => setView(v)}
        >
          <Icon size={19} strokeWidth={1.75} />
        </button>
      ))}
      <div className="rail-spacer" />
      <button
        className={`rail-btn ${view === "settings" ? "active" : ""}`}
        title="Settings (Ctrl+,)"
        aria-label="Settings"
        onClick={() => setView("settings")}
      >
        <Settings size={19} strokeWidth={1.75} />
        {!ready && <span className="rail-badge" title="The selected provider is not ready" />}
      </button>
      <button className="rail-capture" title="Capture (Ctrl+N)" aria-label="Capture" onClick={openCapture}>
        <Plus size={20} strokeWidth={2} />
      </button>
    </nav>
  );
}
