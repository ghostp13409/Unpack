import type { Tier } from "./decide";
import { bigCelebrate, burst } from "./fx";
import { playBig, playChime, playTick } from "./sound";

export type Origin = { x: number; y: number };

/** Centre of an element on screen, the point a celebration starts from. */
export function originOf(el: Element): Origin {
  const r = el.getBoundingClientRect();
  return { x: r.left + r.width / 2, y: r.top + r.height / 2 };
}

/** Fallback origin: the centre of the main pane, or of the window. */
function fallbackOrigin(): Origin {
  const main = document.querySelector(".main");
  return main ? originOf(main) : { x: window.innerWidth / 2, y: window.innerHeight / 2 };
}

function showCombo(at: Origin, n: number) {
  const d = document.createElement("div");
  d.className = "combo-pop";
  d.setAttribute("aria-hidden", "true");
  d.textContent = `×${n}`;
  d.style.left = `${at.x + 14}px`;
  d.style.top = `${at.y - 16}px`;
  document.body.appendChild(d);
  setTimeout(() => d.remove(), 900);
}

/**
 * Play the effect for a completion. `combo` is 0 for the first completion and
 * counts up for each one within the combo window. Effects never throw: a missing
 * audio device or canvas must not break completing a task.
 */
export function celebrate(opts: { tier: Tier; combo: number; origin: Origin | null; sound: boolean; motion: boolean }) {
  const { tier, combo, sound, motion } = opts;
  if (sound) {
    try {
      if (tier === "subtask") playTick(combo);
      else if (tier === "task") playChime(combo);
      else playBig();
    } catch {
      // No audio: stay silent.
    }
  }
  if (!motion) return;
  try {
    const origin = opts.origin ?? fallbackOrigin();
    if (tier === "task") burst(origin, 70, 1);
    else if (tier === "big") bigCelebrate();
    if (combo > 0 && tier !== "subtask") showCombo(origin, combo + 1);
  } catch {
    // No canvas: skip the visuals.
  }
}
