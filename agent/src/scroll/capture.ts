// Scroll capture (RB-020-04): the scroll events of the document and of every inner scroll
// container, coalesced to one event per scroller and animation frame - however many scroll
// events the engine fires - and, once the scrolling has rested for `sync.scrollDebounceMs`, the
// end position of every scroller of the gesture once more, freshly anchored: a follower that
// dropped an event (it was loading) lands on the end position, and late layout shifts in the
// leader are caught. The listeners are passive; the work per frame is a snapshot per scroller.

import type { ScrollData, SyncEvent } from '../protocol';
import { centreTarget } from '../selector';
import { anchorTarget, chooseAnchor, positionOf, type Scroller } from './anchor';

/** What the scroll capture needs. */
export interface ScrollCaptureHost {
  /** Whether scroll is captured right now (active view, no replay marker, `sync.scroll`). */
  enabled(): boolean;
  emit(event: SyncEvent): void;
  /** `sync.scrollDebounceMs`; 0 sends no end position. */
  debounceMs(): number;
  /** Runs `run` before the next frame is painted. */
  frame(run: () => void): void;
  /** The scroller's state; default [`snapshot`]. */
  snapshot?(scroller: Scroller): ScrollData;
}

/** Where `scroller` stands: its position, its anchor, and for an element its target. */
export function snapshot(scroller: Scroller): ScrollData {
  const data: ScrollData = { ...positionOf(scroller) };
  if (scroller instanceof Element) data.container = centreTarget(scroller);
  const anchor = chooseAnchor(scroller);
  if (anchor) data.anchor = anchorTarget(anchor, scroller);
  return data;
}

/** The frame scheduler of `win`: `requestAnimationFrame`, else a 16 ms timer. */
export function frameOf(win: Window): (run: () => void) => void {
  if (typeof win.requestAnimationFrame === 'function') {
    return (run) => {
      win.requestAnimationFrame(() => run());
    };
  }
  return (run) => {
    setTimeout(run, 16);
  };
}

/** Coalesces scroll events per frame and sends the end position after a rest. */
export class ScrollCapture {
  private readonly host: ScrollCaptureHost;
  /** Scrolled since the last frame. */
  private readonly dirty = new Set<Scroller>();
  /** Scrolled since the gesture began. */
  private readonly gesture = new Set<Scroller>();
  private framePending = false;
  private rest: ReturnType<typeof setTimeout> | null = null;
  private disposed = false;

  constructor(host: ScrollCaptureHost) {
    this.host = host;
  }

  /** `scroller` fired a scroll event. */
  onScroll(scroller: Scroller): void {
    if (this.disposed || !this.host.enabled()) return;
    this.dirty.add(scroller);
    this.gesture.add(scroller);
    if (!this.framePending) {
      this.framePending = true;
      this.host.frame(() => this.flush());
    }
    if (this.rest !== null) clearTimeout(this.rest);
    this.rest = null;
    const wait = this.host.debounceMs();
    if (wait > 0) this.rest = setTimeout(() => this.settle(), wait);
  }

  /** Stops: nothing pending is sent. */
  dispose(): void {
    this.disposed = true;
    if (this.rest !== null) clearTimeout(this.rest);
    this.rest = null;
    this.dirty.clear();
    this.gesture.clear();
  }

  private flush(): void {
    this.framePending = false;
    const due = Array.from(this.dirty);
    this.dirty.clear();
    this.send(due);
  }

  private settle(): void {
    this.rest = null;
    const due = Array.from(this.gesture);
    this.gesture.clear();
    this.send(due);
  }

  private send(scrollers: Scroller[]): void {
    if (this.disposed || !this.host.enabled()) return;
    const take = this.host.snapshot ?? snapshot;
    for (const scroller of scrollers) {
      if (scroller instanceof Element && !scroller.isConnected) continue;
      this.host.emit({ type: 'scroll', ...take(scroller) });
    }
  }
}
