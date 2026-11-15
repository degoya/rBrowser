// Lighthouse leaves a rejected promise nobody waits for behind when its browser is ended under
// a running measurement - a cancelled run (`checkForQuiet`'s `Runtime.evaluate`, seen with a real
// Chromium; RB-080-67, review 1). Lighthouse creates that promise inside, out of reach, so the
// runner opens a short window when it cancels a measurement, and only a rejection thrown from
// Lighthouse's own code within it is tolerated (rejections.ts); every other one still ends the
// sidecar, as Node would.

/** How long after a cancel Lighthouse's own rejections are tolerated. */
export const DANGLING_WINDOW_MS = 30_000;

const LIGHTHOUSE_FRAME = /[\\/]node_modules[\\/]lighthouse[\\/]/;

export class Dangling {
  readonly #now: () => number;
  readonly #windowMs: number;
  #until = Number.NEGATIVE_INFINITY;

  constructor(now: () => number = Date.now, windowMs = DANGLING_WINDOW_MS) {
    this.#now = now;
    this.#windowMs = windowMs;
  }

  /** A measurement was cancelled under Lighthouse. */
  open(): void {
    this.#until = this.#now() + this.#windowMs;
  }

  /** Whether `reason` is Lighthouse's own rejection of a run cancelled a moment ago. */
  tolerates(reason: unknown): boolean {
    return this.#now() < this.#until && reason instanceof Error && LIGHTHOUSE_FRAME.test(reason.stack ?? '');
  }
}

/** The sidecar's one window. */
export const lighthouseDangling = new Dangling();
