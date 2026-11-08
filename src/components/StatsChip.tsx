import { Check } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import { computeStats } from "../store/stats";
import { useStore } from "../store/useStore";

const DAYS = ["Mon", "Tue", "Wed", "Thu", "Fri"];
const MIN_BAR = 3;
const MAX_BAR = 38;

function Flame({ size }: { size: number }) {
  return (
    <svg className="flame" width={size} height={size} viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
      <path d="M12 2c1 3.5 5 5.5 5 11a5 5 0 0 1-10 0c0-2.2 1-3.6 2-4.6.2 1.6 1 2.6 2 2.6 0-3 .2-6 1-9z" />
    </svg>
  );
}

/** Quiet streak + today's count in the title bar; hover or focus for the week. */
export function StatsChip() {
  const tasks = useStore((s) => s.tasks);
  const [tick, setTick] = useState(0);
  const [bump, setBump] = useState(false);

  // Roll over at midnight without needing a task change.
  useEffect(() => {
    const id = window.setInterval(() => setTick((n) => n + 1), 60_000);
    return () => window.clearInterval(id);
  }, []);

  // `tick` is a deliberate dependency: it forces a recompute when the day rolls over.
  const stats = useMemo(() => computeStats(tasks), [tasks, tick]);

  const prevToday = useRef(stats.today);
  useEffect(() => {
    if (stats.today <= prevToday.current) {
      prevToday.current = stats.today;
      return;
    }
    prevToday.current = stats.today;
    setBump(true);
    const id = window.setTimeout(() => setBump(false), 500);
    return () => window.clearTimeout(id);
  }, [stats.today]);

  const { today, streak, best, week, todayIndex } = stats;
  const peak = Math.max(1, ...week.days);

  return (
    <div className="stats">
      <button type="button" className={`stats-chip ${bump ? "bump" : ""}`} aria-label={`Stats: ${streak} day streak, ${today} done today`}>
        <span className="seg">
          <Flame size={13} />
          <span className="val">{streak}</span>
        </span>
        <span className="seg">
          <Check size={12} strokeWidth={2.5} aria-hidden="true" />
          <span className="val">{today}</span> today
        </span>
      </button>
      <div className="stats-pop">
        <div className="sp-row">
          <div>
            <div className="sp-big">{today}</div>
            <div className="sp-lbl">done today</div>
          </div>
          <div className="sp-right">
            <div className="sp-big sp-streak">{streak}</div>
            <div className="sp-lbl">day streak</div>
          </div>
        </div>
        <hr />
        <div className="sp-row">
          <span className="sp-lbl">This week</span>
          <span className="sp-lbl">
            {week.total} done · {week.big} big
          </span>
        </div>
        <div className="sp-bars">
          {DAYS.map((d, i) => (
            <div key={d} className={`sp-bar ${i === todayIndex ? "today" : ""}`}>
              <i style={{ height: Math.max(MIN_BAR, Math.round((week.days[i] / peak) * MAX_BAR)) }} />
              {d}
            </div>
          ))}
        </div>
        <hr />
        <div className="sp-lbl">Best streak: {best} {best === 1 ? "workday" : "workdays"}. Weekends never break it.</div>
      </div>
    </div>
  );
}
