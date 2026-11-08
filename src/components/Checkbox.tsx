import { Check } from "lucide-react";
import { useLayoutEffect, useRef, useState } from "react";
import { originOf, type Origin } from "../lib/celebrate";

export function Checkbox({
  checked,
  onChange,
  label,
}: {
  checked: boolean;
  /** Called with the centre of the checkbox, where a celebration starts. */
  onChange: (origin: Origin) => void;
  label?: string;
}) {
  // Pop only when this checkbox flips from unchecked to checked while mounted,
  // never when it mounts already checked (show done, detail panel, regroup).
  const wasChecked = useRef(checked);
  const [popping, setPopping] = useState(false);
  useLayoutEffect(() => {
    if (checked && !wasChecked.current) setPopping(true);
    else if (!checked) setPopping(false);
    wasChecked.current = checked;
  }, [checked]);

  return (
    <button
      role="checkbox"
      aria-checked={checked}
      aria-label={label ?? (checked ? "Mark not done" : "Mark done")}
      className={`row-check ${checked ? "checked" : ""} ${popping ? "popping" : ""}`}
      onClick={(e) => {
        e.stopPropagation();
        onChange(originOf(e.currentTarget));
      }}
      onAnimationEnd={(e) => {
        if (e.animationName === "check-pop") setPopping(false);
      }}
    >
      {checked && <Check size={12} strokeWidth={3} />}
    </button>
  );
}
