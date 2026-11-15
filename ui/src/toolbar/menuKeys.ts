// The keyboard of a toolbar menu drawn as a native select (`OverflowMenu.vue`, `ThemeToggle.vue`,
// `NativeSelect.vue`, `BookmarksMenu.vue`):
// an entry runs only when it is chosen. A closed select changes its value on an arrow key or a
// typed letter (Windows), which is browsing, not choosing: such a key opens the popup instead
// (`showPicker`, the key is the user's gesture), and a `change` that a browsing key caused all
// the same runs nothing.

/** The keys that move through a closed select. */
const MOVES = new Set(['ArrowDown', 'ArrowUp', 'ArrowLeft', 'ArrowRight', 'Home', 'End', 'PageDown', 'PageUp']);

/** The keys that open a select everywhere. */
const OPENS = new Set(['Enter', ' ', 'F4']);

/** Opens the popup of `element`; `false` where the browser cannot. */
export function openPicker(element: HTMLSelectElement): boolean {
  try {
    element.showPicker();
    return true;
  } catch {
    return false;
  }
}

export interface MenuKeys {
  /** A press: the popup is about to open. */
  onPointerDown: () => void;
  onKeyDown: (event: KeyboardEvent) => void;
  /** Bound to `keyup` and `blur`: the browsing key is over. */
  release: () => void;
  /** Whether the `change` now was caused by a browsing key, not a choice. */
  browsing: () => boolean;
}

/** The handlers of one menu; `prepare` runs before its popup opens (it may read the entries). */
export function createMenuKeys(prepare: () => void = () => undefined): MenuKeys {
  let browsing = false;
  return {
    onPointerDown() {
      browsing = false;
      prepare();
    },
    onKeyDown(event) {
      if (event.ctrlKey || event.metaKey || event.key === 'Tab' || event.key === 'Escape') {
        return;
      }
      const element = event.currentTarget as HTMLSelectElement;
      const moves = !event.altKey && (MOVES.has(event.key) || (event.key.length === 1 && event.key !== ' '));
      if (!moves && !OPENS.has(event.key) && !(event.altKey && MOVES.has(event.key))) {
        return;
      }
      prepare();
      if (openPicker(element)) {
        // The list is open; the key did its work.
        event.preventDefault();
        browsing = false;
      } else if (moves) {
        // No popup: the key must not walk the closed select, and what it changes chooses nothing.
        event.preventDefault();
        browsing = true;
      }
    },
    release() {
      browsing = false;
    },
    browsing: () => browsing,
  };
}
