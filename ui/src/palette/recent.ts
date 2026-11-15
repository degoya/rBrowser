// The palette's recent entries (RB-060-11): entry keys, newest first, at most
// `palette.recentCount`. Kept in the shell page's `localStorage` (the shell's own origin,
// `rbrowser://ui/`), a per-user convenience; when the storage is blocked the list lives in
// memory until the window closes.

export const RECENT_STORAGE_KEY = 'rb.palette.recent';

type RecentStorage = Pick<Storage, 'getItem' | 'setItem' | 'removeItem'>;

export interface Recent {
  list(): string[];
  push(key: string, max: number): void;
}

function read(storage: RecentStorage | null): string[] {
  try {
    const parsed: unknown = JSON.parse(storage?.getItem(RECENT_STORAGE_KEY) ?? '[]');
    return Array.isArray(parsed) ? parsed.filter((item): item is string => typeof item === 'string') : [];
  } catch {
    return [];
  }
}

/** The window's storage, or `null` where it cannot be reached. */
export function windowStorage(): RecentStorage | null {
  try {
    return window.localStorage;
  } catch {
    return null;
  }
}

export function createRecent(storage: RecentStorage | null = windowStorage()): Recent {
  let keys = read(storage);
  return {
    list: () => [...keys],
    push(key, max) {
      const limit = Math.max(0, Math.floor(max));
      keys = [key, ...keys.filter((k) => k !== key)].slice(0, limit);
      try {
        if (keys.length === 0) {
          storage?.removeItem(RECENT_STORAGE_KEY);
        } else {
          storage?.setItem(RECENT_STORAGE_KEY, JSON.stringify(keys));
        }
      } catch {
        // Blocked storage: the list holds in memory.
      }
    },
  };
}
