import { FolderOpen, Loader2, RefreshCw } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { Button } from "../components/Button";
import * as api from "../lib/api";
import type { Settings } from "../lib/types";
import { useStore } from "../store/useStore";

export function SettingsView() {
  const view = useStore((s) => s.settings);
  const loadSettings = useStore((s) => s.loadSettings);
  const probeProvider = useStore((s) => s.probeProvider);
  const setError = useStore((s) => s.setError);

  const [draft, setDraft] = useState<Settings | null>(null);
  const [key, setKey] = useState("");
  const [editingKey, setEditingKey] = useState(false);
  const [testing, setTesting] = useState(false);
  const [probing, setProbing] = useState(false);
  const [showPath, setShowPath] = useState(false);
  const [testResult, setTestResult] = useState<{ ok: boolean; text: string } | null>(null);
  const [autostart, setAutostartOn] = useState(false);
  const applied = useRef<string>("");

  // Reset the draft only when the saved settings really changed, so a background probe
  // (which refreshes the view) never discards what is being typed.
  useEffect(() => {
    if (!view) return;
    const key = JSON.stringify(view.settings);
    if (key !== applied.current) {
      applied.current = key;
      setDraft(view.settings);
    }
  }, [view]);

  // Re-check the provider whenever Settings opens.
  useEffect(() => {
    void probeProvider();
  }, [probeProvider]);

  // The OS owns this one (a Run registry entry), so read it fresh rather than keep a copy in settings.
  useEffect(() => {
    api.autostartEnabled().then(setAutostartOn, (e) => setError(api.errorMessage(e)));
  }, [setError]);

  if (!view || !draft) return <div className="page" />;

  const status = view.provider_status;
  const provider = draft.provider || status.provider || "api_key";

  const save = async (next: Settings) => {
    setDraft(next);
    try {
      await api.saveSettings(next);
      await loadSettings();
    } catch (e) {
      setError(api.errorMessage(e));
    }
  };

  const toggleAutostart = async (on: boolean) => {
    try {
      await api.setAutostart(on);
      setAutostartOn(await api.autostartEnabled());
    } catch (e) {
      setError(api.errorMessage(e));
    }
  };

  const recheck = async () => {
    setProbing(true);
    setTestResult(null);
    await probeProvider();
    setProbing(false);
  };

  const choose = async (next: string) => {
    await save({ ...draft, provider: next });
    await recheck();
  };

  const saveKey = async () => {
    try {
      await api.setApiKey(key);
      setKey("");
      setEditingKey(false);
      setTestResult(null);
      await recheck();
    } catch (e) {
      setError(api.errorMessage(e));
    }
  };

  const test = async () => {
    setTesting(true);
    setTestResult(null);
    try {
      const model = await api.testConnection();
      setTestResult({ ok: true, text: `Connected. The answer came from ${model}.` });
    } catch (e) {
      setTestResult({ ok: false, text: api.errorMessage(e) });
    } finally {
      setTesting(false);
    }
  };

  const showKeyInput = editingKey || !view.has_key;

  return (
    <div className="page">
      <div className="page-inner">
        <header className="page-header">
          <div>
            <h1>Settings</h1>
          </div>
        </header>

        <section className="settings-section" style={{ marginTop: 0 }}>
          <h3>Claude</h3>
          <div className="settings-grid">
            <label>Provider</label>
            <div className="radio-group" role="radiogroup" aria-label="Provider">
              <label className="radio">
                <input type="radio" name="provider" checked={provider === "claude_code"} onChange={() => choose("claude_code")} />
                Claude Code <span className="sub">(uses your Claude subscription)</span>
              </label>
              <label className="radio">
                <input type="radio" name="provider" checked={provider === "api_key"} onChange={() => choose("api_key")} />
                API key <span className="sub">(pay per use)</span>
              </label>
            </div>

            {provider === "claude_code" ? (
              <>
                <label>Status</label>
                <div className="inline">
                  <span className={status.ready ? "ok-text" : "error"} style={status.ready ? undefined : { marginTop: 0 }}>
                    {probing ? "Checking…" : status.ready ? status.account : status.detail || "Not checked yet."}
                  </span>
                  <Button size="sm" onClick={recheck} disabled={probing}>
                    {probing ? <Loader2 size={12} className="spin" /> : <RefreshCw size={12} />} Re-check
                  </Button>
                  <Button size="sm" onClick={test} disabled={testing || !status.ready}>
                    {testing ? <Loader2 size={12} className="spin" /> : null} Test connection
                  </Button>
                </div>
                <div className="hint">
                  Runs the claude command on this PC, signed in with your Claude account.{" "}
                  <button className="browse" onClick={() => setShowPath((v) => !v)}>
                    {showPath ? "Hide path" : "Set the path"}
                  </button>
                </div>
                {showPath && (
                  <>
                    <label>Claude Code path</label>
                    <input
                      className="input"
                      placeholder="Leave empty to search PATH"
                      value={draft.claude_code_path}
                      onChange={(e) => setDraft({ ...draft, claude_code_path: e.target.value })}
                      onBlur={() => save(draft).then(recheck)}
                    />
                  </>
                )}
              </>
            ) : (
              <>
                <label>API key</label>
                {showKeyInput ? (
                  <div className="inline">
                    <input
                      className="input"
                      type="password"
                      placeholder="sk-ant-..."
                      value={key}
                      onChange={(e) => setKey(e.target.value)}
                      onKeyDown={(e) => e.key === "Enter" && saveKey()}
                      autoFocus={editingKey}
                    />
                    <Button variant="primary" onClick={saveKey} disabled={!key.trim()}>
                      Save
                    </Button>
                    {view.has_key && (
                      <Button variant="ghost" onClick={() => setEditingKey(false)}>
                        Cancel
                      </Button>
                    )}
                  </div>
                ) : (
                  <div className="inline">
                    <span className="small">Key saved ({view.key_hint})</span>
                    <Button size="sm" onClick={() => setEditingKey(true)}>
                      Replace
                    </Button>
                    <Button size="sm" onClick={test} disabled={testing}>
                      {testing ? <Loader2 size={12} className="spin" /> : null} Test connection
                    </Button>
                  </div>
                )}
                <div className="hint">Stored in Windows Credential Manager, never in the database.</div>
              </>
            )}
            {testResult && (
              <>
                <span />
                <span className={testResult.ok ? "ok-text" : "error"} style={testResult.ok ? undefined : { marginTop: 0 }}>
                  {testResult.text}
                </span>
              </>
            )}

            <label>Model</label>
            <input className="input" value={draft.model} onChange={(e) => setDraft({ ...draft, model: e.target.value })} onBlur={() => save(draft)} />
            <div className="hint">Default claude-haiku-5-5. Any Claude model id works with either provider.</div>

            <label>Effort</label>
            <select className="select" value={draft.effort} onChange={(e) => save({ ...draft, effort: e.target.value })}>
              <option value="low">Low (fastest)</option>
              <option value="medium">Medium</option>
              <option value="high">High (most careful)</option>
            </select>
          </div>
        </section>

        <section className="settings-section">
          <h3>You</h3>
          <div className="settings-grid">
            <label>Your name</label>
            <input className="input" value={draft.user_name} onChange={(e) => setDraft({ ...draft, user_name: e.target.value })} onBlur={() => save(draft)} />
            <div className="hint">So the model knows whose tasks to extract from transcripts and emails.</div>
            <label>Manager's name</label>
            <input
              className="input"
              placeholder="Optional"
              value={draft.manager_name}
              onChange={(e) => setDraft({ ...draft, manager_name: e.target.value })}
              onBlur={() => save(draft)}
            />
            <div className="hint">Their asks default to high priority.</div>
          </div>
        </section>

        <section className="settings-section">
          <h3>Appearance</h3>
          <div className="settings-grid">
            <label>Theme</label>
            <select className="select" value={draft.theme} onChange={(e) => save({ ...draft, theme: e.target.value })}>
              <option value="system">System</option>
              <option value="light">Light</option>
              <option value="dark">Dark</option>
            </select>
          </div>
        </section>

        <section className="settings-section">
          <h3>Rewards</h3>
          <div className="settings-grid">
            <label htmlFor="setting-sounds">Celebration sounds</label>
            <input
              id="setting-sounds"
              type="checkbox"
              className="check"
              checked={draft.sounds}
              onChange={(e) => save({ ...draft, sounds: e.target.checked })}
            />
            <div className="hint">A short chime when you finish something.</div>
            <label htmlFor="setting-animations">Celebration animations</label>
            <input
              id="setting-animations"
              type="checkbox"
              className="check"
              checked={draft.animations}
              onChange={(e) => save({ ...draft, animations: e.target.checked })}
            />
            <div className="hint">Confetti and balloons. Also off when Windows' reduce-motion setting is on.</div>
          </div>
        </section>

        <section className="settings-section">
          <h3>Startup</h3>
          <div className="settings-grid">
            <label htmlFor="setting-autostart">Start Unpack when Windows starts</label>
            <input
              id="setting-autostart"
              type="checkbox"
              className="check"
              checked={autostart}
              onChange={(e) => toggleAutostart(e.target.checked)}
            />
            <div className="hint">Opens Unpack when you sign in.</div>
          </div>
        </section>

        <section className="settings-section">
          <h3>Data</h3>
          <div className="settings-grid">
            <label>Folder</label>
            <div className="inline">
              <span className="small muted" style={{ flex: 1, wordBreak: "break-all", userSelect: "text" }}>
                {view.data_dir}
              </span>
              <Button size="sm" onClick={() => api.openDataFolder().catch((e) => setError(api.errorMessage(e)))}>
                <FolderOpen size={13} /> Open folder
              </Button>
            </div>
          </div>
        </section>
      </div>
    </div>
  );
}
