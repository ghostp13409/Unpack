import { useEffect } from "react";
import { useShortcuts } from "./hooks/useShortcuts";
import { Rail } from "./components/Rail";
import { TitleBar } from "./components/TitleBar";
import { useStore } from "./store/useStore";
import { ActivityView } from "./views/ActivityView";
import { CaptureSheet } from "./views/CaptureSheet";
import { FocusView } from "./views/FocusView";
import { SettingsView } from "./views/SettingsView";
import { TaskDetail } from "./views/TaskDetail";
import { TasksView } from "./views/TasksView";

export default function App() {
  const view = useStore((s) => s.view);
  const selectedId = useStore((s) => s.selectedId);
  const loadAll = useStore((s) => s.loadAll);
  const loadSettings = useStore((s) => s.loadSettings);
  const probeProvider = useStore((s) => s.probeProvider);
  const theme = useStore((s) => s.settings?.settings.theme ?? "system");
  const error = useStore((s) => s.error);
  const setError = useStore((s) => s.setError);
  useShortcuts();

  useEffect(() => {
    void loadAll();
    void loadSettings().then(() => probeProvider());
  }, [loadAll, loadSettings, probeProvider]);

  useEffect(() => {
    const root = document.documentElement;
    if (theme === "light" || theme === "dark") root.setAttribute("data-theme", theme);
    else root.removeAttribute("data-theme");
  }, [theme]);

  useEffect(() => {
    if (!error) return;
    const t = setTimeout(() => setError(null), 6000);
    return () => clearTimeout(t);
  }, [error, setError]);

  return (
    <div className="shell">
      <TitleBar />
      <div className={`app ${selectedId ? "has-detail" : ""}`}>
      <Rail />
      <main className="main">
        {view === "focus" && <FocusView />}
        {view === "tasks" && <TasksView />}
        {view === "activity" && <ActivityView />}
        {view === "settings" && <SettingsView />}
        {error && (
          <div className="error" role="alert" style={{ position: "fixed", bottom: 16, left: 72, right: 16, zIndex: 30 }}>
            {error}
          </div>
        )}
      </main>
      {selectedId && <TaskDetail key={selectedId} id={selectedId} />}
      <CaptureSheet />
      </div>
    </div>
  );
}
