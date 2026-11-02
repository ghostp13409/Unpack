import type { Priority } from "../lib/types";
import { PRIORITY_LABEL } from "../lib/types";

export function PriorityDot({ priority }: { priority: Priority }) {
  return <span className={`dot ${priority}`} title={PRIORITY_LABEL[priority]} aria-label={PRIORITY_LABEL[priority]} />;
}
