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
      {checked && (
        <svg className="tick" viewBox="0 0 24 24" width="12" height="12" aria-hidden="true">
          {/* Drawn short stroke first, then the long one, when it pops. */}
          <path d="M5 12.5l4.5 4.5L19 7.5" pathLength={1} />
        </svg>
      )}
    </button>
  );
}
