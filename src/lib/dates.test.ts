import { describe, expect, it } from "vitest";
import { daysBetween, formatDue, formatDateTime } from "./dates";

describe("daysBetween", () => {
  it("counts whole days from a to b", () => {
    expect(daysBetween("2026-11-01", "2026-11-04")).toBe(3);
    expect(daysBetween("2026-11-04", "2026-11-01")).toBe(-3);
    expect(daysBetween("2026-10-30", "2026-11-01")).toBe(2);
  });
});

describe("formatDue", () => {
  const today = "2026-11-01";
  it("overdue", () => {
    expect(formatDue("2026-10-30", today)).toEqual({ label: "Overdue 2d", tone: "overdue" });
  });
  it("today and tomorrow", () => {
    expect(formatDue("2026-11-01", today)).toEqual({ label: "Today", tone: "today" });
    expect(formatDue("2026-11-02", today)).toEqual({ label: "Tomorrow", tone: "soon" });
  });
  it("within the week is soon, later is a short date", () => {
    expect(formatDue("2026-11-05", today)).toEqual({ label: "Nov 5", tone: "soon" });
    expect(formatDue("2026-11-20", today)).toEqual({ label: "Nov 20", tone: "normal" });
  });
});

describe("formatDateTime", () => {
  it("renders an ISO timestamp as a short local string", () => {
    const out = formatDateTime("2026-11-01T17:12:00Z");
    expect(out).toMatch(/Nov 1/);
    expect(out).toMatch(/\d{1,2}:\d{2}/);
  });
});
