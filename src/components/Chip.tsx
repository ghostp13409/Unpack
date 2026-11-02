import { X } from "lucide-react";
import type { ReactNode } from "react";

export function Chip({ children, onRemove, title }: { children: ReactNode; onRemove?: () => void; title?: string }) {
  return (
    <span className={`chip ${onRemove ? "removable" : ""}`} title={title}>
      {children}
      {onRemove && (
        <button className="chip-x" onClick={onRemove} aria-label="Remove">
          <X size={11} />
        </button>
      )}
    </span>
  );
}
