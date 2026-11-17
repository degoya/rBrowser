// Cumulative Layout Shift by session windows (RB-060-10), as web.dev defines it: shifts less
// than a second apart and within five seconds of the window's first shift form one window, and
// CLS is the largest window's sum. Shifts right after input do not count. The sources are the
// CSS paths of the shifted elements of that largest window, the largest first.

/** The most shift sources a report names. */
export const MAX_SOURCES = 5;
/** The gap that ends a session window, in ms. */
const GAP_MS = 1000;
/** The longest session window, in ms. */
const WINDOW_MS = 5000;

/** One `layout-shift` entry, its sources already CSS paths. */
export interface Shift {
  value: number;
  startTime: number;
  hadRecentInput: boolean;
  sources: string[];
}

/** CLS with its largest sources. */
export interface ClsValue {
  value: number;
  sources: string[];
}

/** The session windows of one measuring segment. */
export class ClsWindows {
  private first = 0;
  private last = 0;
  private sum = 0;
  private open = false;
  private bySource = new Map<string, number>();
  private best: ClsValue = { value: 0, sources: [] };

  /** Adds one shift; true when CLS changed. */
  add(shift: Shift): boolean {
    if (shift.hadRecentInput || !(shift.value > 0)) return false;
    const joins =
      this.open &&
      shift.startTime - this.last < GAP_MS &&
      shift.startTime - this.first < WINDOW_MS;
    if (!joins) {
      this.first = shift.startTime;
      this.sum = 0;
      this.bySource = new Map();
      this.open = true;
    }
    this.last = shift.startTime;
    this.sum += shift.value;
    for (const source of shift.sources) {
      this.bySource.set(source, (this.bySource.get(source) ?? 0) + shift.value);
    }
    if (this.sum <= this.best.value) return false;
    const sources = [...this.bySource.entries()]
      .sort((a, b) => b[1] - a[1])
      .slice(0, MAX_SOURCES)
      .map(([source]) => source);
    this.best = { value: this.sum, sources };
    return true;
  }

  /** CLS so far. */
  value(): ClsValue {
    return { value: this.best.value, sources: [...this.best.sources] };
  }

  /** Starts a new segment. */
  reset(): void {
    this.open = false;
    this.sum = 0;
    this.bySource = new Map();
    this.best = { value: 0, sources: [] };
  }
}
