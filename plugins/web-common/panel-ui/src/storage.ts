// A panel runs in a frame sandboxed without `allow-same-origin` (an opaque origin), where reading
// `localStorage` or `sessionStorage` throws a SecurityError (RB-080-66). Nuxt UI and VueUse may
// read them (`useStorage`), so a panel replaces a storage it cannot read with one in memory for
// this document before anything else loads; nothing outlives the panel (RB-080-83).

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

/** Puts a storage in memory where `target`'s own cannot be read; the names it replaced. */
export function ensureStorage(target: Window): string[] {
  const replaced: string[] = [];
  for (const name of NAMES) {
    try {
      void target[name];
      continue;
    } catch {
      // The sandbox refuses it: replaced below.
    }
    Object.defineProperty(target, name, { value: memoryStorage(), configurable: true });
    replaced.push(name);
  }
  return replaced;
}
