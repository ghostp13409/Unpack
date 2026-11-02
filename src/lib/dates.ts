const MONTHS = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];

function parseDate(iso: string): Date {
  const [y, m, d] = iso.split("-").map(Number);
  return new Date(Date.UTC(y, m - 1, d));
}

export function todayISO(): string {
  const d = new Date();
  const y = d.getFullYear();
  const m = String(d.getMonth() + 1).padStart(2, "0");
  const day = String(d.getDate()).padStart(2, "0");
  return `${y}-${m}-${day}`;
}

export function addDays(iso: string, days: number): string {
  const d = parseDate(iso);
  d.setUTCDate(d.getUTCDate() + days);
  return d.toISOString().slice(0, 10);
}

/** Whole days from `a` to `b` (positive when b is later). */
export function daysBetween(a: string, b: string): number {
  return Math.round((parseDate(b).getTime() - parseDate(a).getTime()) / 86_400_000);
}

export function shortDate(iso: string): string {
  const d = parseDate(iso);
  return `${MONTHS[d.getUTCMonth()]} ${d.getUTCDate()}`;
}

export type DueTone = "overdue" | "today" | "soon" | "normal";

export function formatDue(due: string, today: string): { label: string; tone: DueTone } {
  const days = daysBetween(today, due);
  if (days < 0) return { label: `Overdue ${-days}d`, tone: "overdue" };
  if (days === 0) return { label: "Today", tone: "today" };
  if (days === 1) return { label: "Tomorrow", tone: "soon" };
  if (days <= 6) return { label: shortDate(due), tone: "soon" };
  return { label: shortDate(due), tone: "normal" };
}

/** "Nov 1, 5:12 PM" in the viewer's local time. */
export function formatDateTime(isoTimestamp: string): string {
  const d = new Date(isoTimestamp);
  const h = d.getHours();
  const hour12 = h % 12 === 0 ? 12 : h % 12;
  const minutes = String(d.getMinutes()).padStart(2, "0");
  const ampm = h < 12 ? "AM" : "PM";
  return `${MONTHS[d.getMonth()]} ${d.getDate()}, ${hour12}:${minutes} ${ampm}`;
}

export function formatDateOnly(isoTimestamp: string): string {
  const d = new Date(isoTimestamp);
  return `${MONTHS[d.getMonth()]} ${d.getDate()}`;
}
