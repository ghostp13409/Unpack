import { Check } from "lucide-react";

export function Checkbox({ checked, onChange, label }: { checked: boolean; onChange: () => void; label?: string }) {
  return (
    <button
      role="checkbox"
      aria-checked={checked}
      aria-label={label ?? (checked ? "Mark not done" : "Mark done")}
      className={`row-check ${checked ? "checked" : ""}`}
      onClick={(e) => {
        e.stopPropagation();
        onChange();
      }}
    >
      {checked && <Check size={12} strokeWidth={3} />}
    </button>
  );
}
