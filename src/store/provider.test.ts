import { describe, expect, it } from "vitest";
import type { SettingsView } from "../lib/types";
import { notReadyMessage, providerReady } from "./provider";

function view(provider: string, ready: boolean, detail = ""): SettingsView {
  return {
    settings: { model: "m", effort: "low", user_name: "", manager_name: "", theme: "system", provider, claude_code_path: "" },
    has_key: false,
    key_hint: null,
    data_dir: "",
    provider_status: { provider, ready, detail, account: "", checked_at: "" },
  };
}

describe("providerReady", () => {
  it("ready_when_status_ready", () => {
    expect(providerReady(null)).toBe(true);
    expect(providerReady(view("claude_code", true))).toBe(true);
    expect(providerReady(view("claude_code", false, "x"))).toBe(false);
  });
});

describe("notReadyMessage", () => {
  it("claude_code_detail_is_the_message", () => {
    expect(notReadyMessage(view("claude_code", false, "Claude Code is signed out."))).toBe("Claude Code is signed out.");
  });
  it("api_key_message_is_fixed", () => {
    expect(notReadyMessage(view("api_key", false, "whatever"))).toBe("Add your Anthropic API key in Settings first.");
    expect(notReadyMessage(null)).toBe("");
  });
});
