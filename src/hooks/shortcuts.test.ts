import { describe, expect, it } from "vitest";
import { nextCursor, resolveShortcut, type KeyLike } from "./shortcuts";

function key(k: string, over: Partial<KeyLike> = {}): KeyLike {
  return { key: k, ctrlKey: false, metaKey: false, altKey: false, shiftKey: false, typing: false, ...over };
}

describe("resolveShortcut", () => {
  it("maps Ctrl+N, Ctrl+K and Ctrl+, to capture, search and settings", () => {
    expect(resolveShortcut(key("n", { ctrlKey: true }))).toEqual({ type: "capture" });
    expect(resolveShortcut(key("k", { ctrlKey: true }))).toEqual({ type: "search" });
    expect(resolveShortcut(key(",", { ctrlKey: true }))).toEqual({ type: "settings" });
  });

  it("maps J/K, Enter, D and E to list actions", () => {
    expect(resolveShortcut(key("j"))).toEqual({ type: "move", delta: 1 });
    expect(resolveShortcut(key("k"))).toEqual({ type: "move", delta: -1 });
    expect(resolveShortcut(key("Enter"))).toEqual({ type: "open" });
    expect(resolveShortcut(key("d"))).toEqual({ type: "toggleDone" });
    expect(resolveShortcut(key("e"))).toEqual({ type: "editTitle" });
  });

  it("maps Escape to escape even while typing", () => {
    expect(resolveShortcut(key("Escape"))).toEqual({ type: "escape" });
    expect(resolveShortcut(key("Escape", { typing: true }))).toEqual({ type: "escape" });
  });

  it("ignores everything else while typing in an input", () => {
    expect(resolveShortcut(key("j", { typing: true }))).toBeNull();
    expect(resolveShortcut(key("d", { typing: true }))).toBeNull();
    expect(resolveShortcut(key("Enter", { typing: true }))).toBeNull();
    expect(resolveShortcut(key("n", { ctrlKey: true, typing: true }))).toBeNull();
  });

  it("ignores unknown keys and modified letters", () => {
    expect(resolveShortcut(key("x"))).toBeNull();
    expect(resolveShortcut(key("j", { ctrlKey: true }))).toBeNull();
    expect(resolveShortcut(key("d", { altKey: true }))).toBeNull();
    expect(resolveShortcut(key("e", { metaKey: true }))).toBeNull();
  });
});

describe("nextCursor", () => {
  const ids = ["a", "b", "c"];

  it("starts at the first row moving down and the last moving up", () => {
    expect(nextCursor(ids, null, 1)).toBe("a");
    expect(nextCursor(ids, null, -1)).toBe("c");
  });

  it("moves within the list and stops at the ends", () => {
    expect(nextCursor(ids, "a", 1)).toBe("b");
    expect(nextCursor(ids, "c", 1)).toBe("c");
    expect(nextCursor(ids, "b", -1)).toBe("a");
    expect(nextCursor(ids, "a", -1)).toBe("a");
  });

  it("restarts from the edge when the cursor is no longer visible", () => {
    expect(nextCursor(ids, "zzz", 1)).toBe("a");
  });

  it("returns null for an empty list", () => {
    expect(nextCursor([], "a", 1)).toBeNull();
  });
});
