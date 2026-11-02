import type { SettingsView } from "../lib/types";

export const API_KEY_MESSAGE = "Add your Anthropic API key in Settings first.";

/** True until settings have loaded (so nothing flashes), then the cached probe result. */
export function providerReady(view: SettingsView | null): boolean {
  if (!view) return true;
  return view.provider_status.ready;
}

/** What to tell the user when the selected provider is not ready. */
export function notReadyMessage(view: SettingsView | null): string {
  if (!view) return "";
  if (view.provider_status.provider === "claude_code") return view.provider_status.detail;
  return API_KEY_MESSAGE;
}
