// Interaction to Next Paint (RB-060-10), as web.dev defines it: an interaction's latency is its
// longest `event` entry; INP is the longest interaction, skipping one per fifty interactions (an
// approximation of the 98th percentile). Only the ten longest interactions are kept.

/** How many of the longest interactions are kept. */
const KEPT = 10;

/** One `event` or `first-input` entry, its target already a CSS path. */
export interface InteractionEntry {
  interactionId: number;
  duration: number;
  target?: string;
}

/** INP with the target of that interaction. */
export interface InpValue {
  value: number;
  target?: string;
  interactions: number;
}

interface Interaction {
  id: number;
  duration: number;
  target?: string;
}

/** The interactions of one measuring segment. */
export class InpTracker {
  private longest: Interaction[] = [];
  private seen = new Set<number>();

  /** Adds one entry; true when the kept interactions changed. */
  add(entry: InteractionEntry): boolean {
    if (!(entry.interactionId > 0) || !Number.isFinite(entry.duration)) return false;
    this.seen.add(entry.interactionId);
    const known = this.longest.find((item) => item.id === entry.interactionId);
    if (known) {
      if (entry.duration <= known.duration) return false;
      known.duration = entry.duration;
      if (entry.target !== undefined) known.target = entry.target;
    } else {
      const last = this.longest.at(-1);
      if (this.longest.length >= KEPT && last && entry.duration <= last.duration) return true;
      const item: Interaction = { id: entry.interactionId, duration: entry.duration };
      if (entry.target !== undefined) item.target = entry.target;
      this.longest.push(item);
    }
    this.longest.sort((a, b) => b.duration - a.duration);
    this.longest.length = Math.min(this.longest.length, KEPT);
    return true;
  }

  /** INP so far; `count` is the page's own interaction count when the engine has one. */
  value(count?: number): InpValue | null {
    if (this.longest.length === 0) return null;
    const interactions = Math.max(count ?? 0, this.seen.size);
    const index = Math.min(this.longest.length - 1, Math.floor(interactions / 50));
    const pick = this.longest[index];
    if (!pick) return null;
    const value: InpValue = { value: Math.round(pick.duration), interactions };
    if (pick.target !== undefined) value.target = pick.target;
    return value;
  }

  /** Starts a new segment. */
  reset(): void {
    this.longest = [];
    this.seen = new Set();
  }
}
