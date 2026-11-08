/**
 * Confetti and balloons on a full-window canvas overlay. Ported from the approved
 * rewards mockup. The canvas is created on first use and removed when idle.
 */

export interface Point {
  x: number;
  y: number;
}

interface Piece {
  x: number;
  y: number;
  vx: number;
  vy: number;
  r: number;
  vr: number;
  w: number;
  h: number;
  c: string;
  life: number;
  max: number;
  round: boolean;
}

interface Balloon {
  x: number;
  y: number;
  vy: number;
  ph: number;
  c: string;
  s: number;
}

const COLORS = ["#4f6bed", "#8a9cf5", "#ef7a2a", "#f2c14e", "#3b9a62", "#e5697b", "#b18cf0"];

let cv: HTMLCanvasElement | null = null;
let g: CanvasRenderingContext2D | null = null;
let parts: Piece[] = [];
let balloons: Balloon[] = [];
let running = false;

function fit() {
  if (!cv || !g) return;
  const d = window.devicePixelRatio || 1;
  cv.width = window.innerWidth * d;
  cv.height = window.innerHeight * d;
  g.setTransform(d, 0, 0, d, 0, 0);
}

/** The overlay context, creating the canvas on first use. Null if 2D canvas is unavailable. */
function surface(): CanvasRenderingContext2D | null {
  if (g) return g;
  const c = document.createElement("canvas");
  c.setAttribute("aria-hidden", "true");
  Object.assign(c.style, {
    position: "fixed",
    inset: "0",
    width: "100vw",
    height: "100vh",
    pointerEvents: "none",
    zIndex: "50",
  });
  const ctx = c.getContext("2d");
  if (!ctx) return null;
  document.body.appendChild(c);
  cv = c;
  g = ctx;
  fit();
  window.addEventListener("resize", fit);
  return g;
}

function teardown() {
  window.removeEventListener("resize", fit);
  cv?.remove();
  cv = null;
  g = null;
}

/** Throw `count` confetti pieces upward from `origin`; `power` scales their speed. */
export function burst(origin: Point, count: number, power: number) {
  if (!surface()) return;
  for (let i = 0; i < count; i++) {
    const a = -Math.PI / 2 + (Math.random() - 0.5) * Math.PI * 1.1;
    const v = (4 + Math.random() * 6) * power;
    parts.push({
      x: origin.x,
      y: origin.y,
      vx: Math.cos(a) * v,
      vy: Math.sin(a) * v,
      r: Math.random() * Math.PI,
      vr: (Math.random() - 0.5) * 0.4,
      w: 5 + Math.random() * 4,
      h: 3 + Math.random() * 3,
      c: COLORS[i % COLORS.length],
      life: 0,
      max: 60 + Math.random() * 30,
      round: Math.random() < 0.3,
    });
  }
  loop();
}

/** Two wide confetti bursts and nine balloons floating up from below the window. */
export function bigCelebrate() {
  if (!surface()) return;
  const W = window.innerWidth;
  const H = window.innerHeight;
  burst({ x: W * 0.3, y: H * 0.55 }, 90, 1.4);
  setTimeout(() => burst({ x: W * 0.7, y: H * 0.5 }, 90, 1.4), 160);
  for (let i = 0; i < 9; i++) {
    balloons.push({
      x: W * (0.1 + Math.random() * 0.8),
      y: H + 40 + Math.random() * 160,
      vy: -(2.2 + Math.random() * 1.6),
      ph: Math.random() * 6,
      c: COLORS[i % COLORS.length],
      s: 0.8 + Math.random() * 0.5,
    });
  }
  loop();
}

function drawBalloon(g2: CanvasRenderingContext2D, b: Balloon) {
  const x = b.x + Math.sin(b.ph) * 10;
  const y = b.y;
  const s = b.s;
  g2.save();
  g2.translate(x, y);
  g2.scale(s, s);
  g2.strokeStyle = "rgba(120,120,120,.5)";
  g2.lineWidth = 1;
  g2.beginPath();
  g2.moveTo(0, 26);
  g2.quadraticCurveTo(6, 44, -2, 62);
  g2.stroke();
  g2.fillStyle = b.c;
  g2.beginPath();
  g2.ellipse(0, 0, 18, 23, 0, 0, Math.PI * 2);
  g2.fill();
  g2.beginPath();
  g2.moveTo(-4, 24);
  g2.lineTo(4, 24);
  g2.lineTo(0, 20);
  g2.fill();
  g2.fillStyle = "rgba(255,255,255,.35)";
  g2.beginPath();
  g2.ellipse(-6, -8, 4, 7, -0.4, 0, Math.PI * 2);
  g2.fill();
  g2.restore();
}

function loop() {
  if (running) return;
  running = true;
  const step = () => {
    const g2 = g;
    if (!g2) {
      running = false;
      return;
    }
    const W = window.innerWidth;
    const H = window.innerHeight;
    g2.clearRect(0, 0, W, H);
    parts = parts.filter((p) => p.life < p.max);
    for (const p of parts) {
      p.life++;
      p.vx *= 0.97;
      p.vy = p.vy * 0.97 + 0.32;
      p.x += p.vx;
      p.y += p.vy;
      p.r += p.vr;
      g2.globalAlpha = Math.max(0, 1 - p.life / p.max);
      g2.fillStyle = p.c;
      g2.save();
      g2.translate(p.x, p.y);
      g2.rotate(p.r);
      if (p.round) {
        g2.beginPath();
        g2.arc(0, 0, p.h / 1.4, 0, Math.PI * 2);
        g2.fill();
      } else {
        g2.fillRect(-p.w / 2, -p.h / 2, p.w, p.h * Math.abs(Math.cos(p.life * 0.2)) + 0.5);
      }
      g2.restore();
    }
    g2.globalAlpha = 1;
    balloons = balloons.filter((b) => b.y > -120);
    for (const b of balloons) {
      b.y += b.vy;
      b.vy *= 1.004;
      b.ph += 0.045;
      drawBalloon(g2, b);
    }
    if (parts.length || balloons.length) {
      requestAnimationFrame(step);
    } else {
      running = false;
      teardown();
    }
  };
  requestAnimationFrame(step);
}
