// RB-080-66: a panel runs in a frame sandboxed without `allow-same-origin` (an opaque origin),
// where reading `localStorage` or `sessionStorage` throws a SecurityError. The Vue DevTools kit
// reads `localStorage` while it loads, so the panel died before it drew. The sandbox stays as it
// is: a storage the frame cannot read is replaced by one in memory for this document, which is
// all the kit keeps there (its timeline switches); nothing outlives the panel.

const NAMES = ['localStorage', 'sessionStorage'] as const;

/** A `Storage` in memory. */
export function memoryStorage(): Storage {
  const items = new Map<string, string>();
  const storage = {
    get length() {
      return items.size;
    },
    key: (index: number) => [...items.keys()][index] ?? null,
    getItem: (key: string) => items.get(String(key)) ?? null,
    setItem: (key: string, value: string) => {
      items.set(String(key), String(value));
    },
    removeItem: (key: string) => {
      items.delete(String(key));
    },
    clear: () => {
      items.clear();
    },
  };
  return storage as Storage;
}

/** Puts a storage in memory where `target`'s own cannot be read. */
export function ensureStorage(target: Window): void {
  for (const name of NAMES) {
    try {
      void target[name];
      continue;
    } catch {
      // The sandbox refuses it: replaced below.
    }
    Object.defineProperty(target, name, { value: memoryStorage(), configurable: true });
  }
}

ensureStorage(window);
