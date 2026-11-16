// Focus mode's quick switcher on the keyboard (RB-010-12): left and right arrow step through the
// breakpoints while the toolbar has the focus. They never reach a page (the toolbar is its own
// CEF view) and leave the address bar and the selects alone. The core checks
// `focusMode.arrowKeys` (`source: 'arrowKey'`).

import { onBeforeUnmount, onMounted, type Ref } from 'vue';
import type { LayoutMode } from '../ipc/protocol';
import type { ToolbarAction } from '../toolbar/actions';

export interface ArrowKey {
  key: string;
  altKey: boolean;
  ctrlKey: boolean;
  metaKey: boolean;
  shiftKey: boolean;
  target: EventTarget | null;
}

type FocusStep = 'layout.focus_next' | 'layout.focus_prev';

function isEditable(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) {
    return false;
  }
  return (
    target.isContentEditable ||
    target.getAttribute('contenteditable') === 'true' ||
    ['INPUT', 'TEXTAREA', 'SELECT'].includes(target.tagName)
  );
}

/** The command an arrow key runs in `mode`, or `null` when the key is not focus mode's. */
export function arrowCommand(event: ArrowKey, mode: LayoutMode | undefined): FocusStep | null {
  if (mode !== 'focus' || event.altKey || event.ctrlKey || event.metaKey || event.shiftKey) {
    return null;
  }
  if (isEditable(event.target)) {
    return null;
  }
  if (event.key === 'ArrowRight') {
    return 'layout.focus_next';
  }
  return event.key === 'ArrowLeft' ? 'layout.focus_prev' : null;
}

/** Listens on the window for the arrow keys while the component lives. */
export function useFocusArrowKeys(
  mode: Readonly<Ref<LayoutMode | undefined>>,
  run: (action: ToolbarAction) => unknown,
  target: Pick<Window, 'addEventListener' | 'removeEventListener'> = window,
): void {
  const onKeyDown = (event: KeyboardEvent): void => {
    const command = arrowCommand(event, mode.value);
    if (command !== null) {
      event.preventDefault();
      void run({ command, source: 'arrowKey' });
    }
  };
  onMounted(() => target.addEventListener('keydown', onKeyDown));
  onBeforeUnmount(() => target.removeEventListener('keydown', onKeyDown));
}
