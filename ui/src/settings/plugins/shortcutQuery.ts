// The filter the shortcut editor starts with when Settings > Plugins sends the reader to a
// plugin's commands (RB-080-51): set before the Keyboard Shortcuts category opens, taken once by
// the editor as it mounts.

let pending = '';

/** The shortcut editor opens next with `query` in its search field. */
export function openShortcutsWith(query: string): void {
  pending = query;
}

/** The waiting query, once; empty without one. */
export function takeShortcutQuery(): string {
  const query = pending;
  pending = '';
  return query;
}
