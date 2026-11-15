// A panel's read that never drops a request (review of RB-080-84). `refresh()` reads now, or -
// when a read is on its way - once more right after it, with the state of that moment, and the
// answer of the read it overtook is not shown: the newest state wins. `poll()` is the timer's: it
// lets a read on its way stand (nothing changed), so a slow plugin is not asked twice. One read at
// a time, so a plugin's call limit is never exceeded by the panel.

export interface LatestRefresh {
  /** Reads with the current state; the promise settles when the newest read is shown. */
  refresh(): Promise<void>;
  /** Reads unless a read is on its way. */
  poll(): Promise<void>;
  /** Whether a read is on its way. */
  busy(): boolean;
}

/** `load` reads (it takes the current state itself), `apply` shows an answer, `fail` a failure. */
export function latestRefresh<T>(load: () => Promise<T>, apply: (value: T) => void, fail: (error: unknown) => void): LatestRefresh {
  let running: Promise<void> | null = null;
  let again = false;

  async function loop(): Promise<void> {
    do {
      again = false;
      try {
        const value = await load();
        if (!again) apply(value);
      } catch (error) {
        if (!again) fail(error);
      }
    } while (again);
  }

  function refresh(): Promise<void> {
    if (running) {
      again = true;
      return running;
    }
    running = loop().finally(() => {
      running = null;
    });
    return running;
  }

  return {
    refresh,
    poll: () => running ?? refresh(),
    busy: () => running !== null,
  };
}
