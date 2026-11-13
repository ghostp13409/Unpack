/**
 * Synthesized completion sounds (Web Audio, no files). Ported from the approved
 * rewards mockup; the note values are deliberate, keep them as they are.
 */

type AudioCtor = typeof AudioContext;

let ac: AudioContext | null = null;
let master: GainNode | null = null;

function ctx(): AudioContext {
  if (!ac) {
    const w = window as unknown as { AudioContext?: AudioCtor; webkitAudioContext?: AudioCtor };
    const Ctor = w.AudioContext ?? w.webkitAudioContext;
    if (!Ctor) throw new Error("Web Audio is not available");
    ac = new Ctor();
  }
  if (ac.state === "suspended") void ac.resume();
  return ac;
}

/** Master gain with a short feedback echo, built once. */
function out(): GainNode {
  if (master) return master;
  const a = ctx();
  master = a.createGain();
  master.gain.value = 0.5;
  const dly = a.createDelay();
  dly.delayTime.value = 0.11;
  const fb = a.createGain();
  fb.gain.value = 0.22;
  const wet = a.createGain();
  wet.gain.value = 0.25;
  master.connect(a.destination);
  master.connect(dly);
  dly.connect(fb);
  fb.connect(dly);
  dly.connect(wet);
  wet.connect(a.destination);
  return master;
}

function note(freq: number, t0: number, dur: number, vol: number, type: OscillatorType = "sine") {
  const a = ctx();
  const o = a.createOscillator();
  const g = a.createGain();
  o.type = type;
  o.frequency.value = freq;
  g.gain.setValueAtTime(0, t0);
  g.gain.linearRampToValueAtTime(vol, t0 + 0.008);
  g.gain.exponentialRampToValueAtTime(0.0001, t0 + dur);
  o.connect(g);
  g.connect(out());
  o.start(t0);
  o.stop(t0 + dur + 0.05);
}

const semi = (f: number, n: number) => f * Math.pow(2, n / 12);

/** Highest a combo can lift a sound, in semitones, however fast tasks get finished. */
export const MAX_TICK_LIFT = 4;
export const MAX_CHIME_LIFT = 5;

/** Soft tick for a sub-task; each combo step raises it a semitone, up to MAX_TICK_LIFT. */
export function playTick(combo: number) {
  const t = ctx().currentTime;
  note(semi(1568, Math.min(combo, MAX_TICK_LIFT)), t, 0.08, 0.18, "triangle");
}

/** Warm two-note chime for a task; each combo step raises it a whole step, up to MAX_CHIME_LIFT. */
export function playChime(combo: number) {
  const t = ctx().currentTime;
  const s = Math.min(combo * 2, MAX_CHIME_LIFT);
  for (const [f, d] of [
    [784, 0],
    [1175, 0.085],
  ]) {
    note(semi(f, s), t + d, 0.6, 0.22, "sine");
    note(semi(f * 2, s), t + d, 0.25, 0.05, "triangle");
  }
}

/** Longer rising chord with a sparkle on top, for a big task. */
export function playBig() {
  const t = ctx().currentTime;
  [523, 659, 784, 1047, 1319].forEach((f, i) => {
    note(f, t + i * 0.07, 0.9, 0.18, "sine");
    note(f * 2, t + i * 0.07, 0.3, 0.04, "triangle");
  });
  [523, 659, 784, 1047].forEach((f) => note(f, t + 0.42, 1.6, 0.09, "sine"));
  [2093, 2637, 3136].forEach((f, i) => note(f, t + 0.5 + i * 0.06, 0.5, 0.03, "sine"));
}
