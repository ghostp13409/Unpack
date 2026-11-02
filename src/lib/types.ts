export type Status = "todo" | "in_progress" | "done" | "dropped";
export type Priority = "urgent" | "high" | "medium" | "low";
export type CaptureStatus = "proposed" | "applied" | "discarded" | "failed";
export type InputKind = "paste" | "file";
export type ChangeKind = "create" | "update" | "complete" | "add_note";

export interface Task {
  id: string;
  title: string;
  description: string;
  status: Status;
  priority: Priority;
  category: string | null;
  due_date: string | null;
  parent_id: string | null;
  pinned: boolean;
  snoozed_until: string | null;
  created_at: string;
  updated_at: string;
  completed_at: string | null;
}

export interface NewTask {
  title: string;
  description?: string;
  priority?: Priority;
  category?: string | null;
  due_date?: string | null;
  parent_id?: string | null;
}

/** Omit a field to leave it alone; set a nullable field to null to clear it. */
export interface TaskPatch {
  title?: string;
  description?: string;
  status?: Status;
  priority?: Priority;
  category?: string | null;
  due_date?: string | null;
  parent_id?: string | null;
  pinned?: boolean;
  snoozed_until?: string | null;
}

export interface Note {
  id: string;
  task_id: string;
  body: string;
  source_capture_id: string | null;
  created_at: string;
}

export interface Ranked {
  task_id: string;
  score: number;
  reasons: string[];
}

export type ResolvedOp =
  | { type: "create"; temp_id: string | null; new: Required<NewTask>; parent_temp: string | null }
  | { type: "update"; task_id: string; patch: TaskPatch }
  | { type: "complete"; task_id: string }
  | { type: "add_note"; task_id: string; body: string };

export interface ProposedOp {
  index: number;
  op: ResolvedOp;
  reason: string;
  evidence: string | null;
}

export interface Proposal {
  summary: string;
  ops: ProposedOp[];
  warnings: string[];
}

export interface Capture {
  id: string;
  created_at: string;
  summary: string | null;
  model: string;
  input_tokens: number;
  output_tokens: number;
  status: CaptureStatus;
  proposal: Proposal | null;
}

export interface CaptureInput {
  id: string;
  capture_id: string;
  kind: InputKind;
  filename: string | null;
  extracted_text: string;
  char_count: number;
}

export interface Change {
  id: string;
  capture_id: string;
  task_id: string | null;
  kind: ChangeKind;
  before: Record<string, unknown>;
  after: Record<string, unknown>;
  reason: string;
  evidence: string | null;
  applied: boolean;
  undone: boolean;
  created_at: string;
}

export interface CaptureSummary {
  capture: Capture;
  inputs: CaptureInput[];
  changes: Change[];
}

export interface SourceRef {
  capture_id: string;
  captured_at: string;
  filenames: string[];
  kind: ChangeKind;
  reason: string;
  evidence: string | null;
  undone: boolean;
}

export interface TaskDetail {
  task: Task;
  notes: Note[];
  children: Task[];
  sources: SourceRef[];
}

export type ProviderName = "claude_code" | "api_key";

export interface Settings {
  model: string;
  effort: string;
  user_name: string;
  manager_name: string;
  theme: "system" | "light" | "dark" | string;
  /** "" until the first readiness probe decides. */
  provider: ProviderName | "" | string;
  claude_code_path: string;
}

/** Last readiness result of the selected provider. */
export interface ProviderStatus {
  provider: ProviderName | string;
  ready: boolean;
  detail: string;
  account: string;
  checked_at: string;
}

export interface SettingsView {
  settings: Settings;
  has_key: boolean;
  key_hint: string | null;
  data_dir: string;
  provider_status: ProviderStatus;
}

export type UndoOutcome = { type: "done" } | { type: "needs_confirm"; changed_fields: string[] };

export const PRIORITIES: Priority[] = ["urgent", "high", "medium", "low"];
export const PRIORITY_LABEL: Record<Priority, string> = {
  urgent: "Urgent",
  high: "High",
  medium: "Medium",
  low: "Low",
};
export const STATUS_LABEL: Record<Status, string> = {
  todo: "To do",
  in_progress: "In progress",
  done: "Done",
  dropped: "Dropped",
};
