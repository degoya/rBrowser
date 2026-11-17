// The palette's selection on `UCommandPalette`'s highlight (RB-090-46). The palette owns the
// selection (`palette.ts`: Page Up and Page Down, the wrap to the last match, a selection that
// stays on its entry when the rows are rebuilt); Nuxt UI's list (Reka UI's listbox) owns the
// highlight the user sees and the field's `aria-activedescendant`. The list moves its highlight
// on its own - to the first row whenever its rows change, nowhere when the pointer leaves - so
// this puts it back on the selected row, through the one way the list takes a row from outside:
// the pointer moving over it. A row the real pointer moves over becomes the selection, as in
// every Nuxt UI list.

import { nextTick, watch, type Ref } from 'vue';

export interface RowHighlightDeps {
  /** The palette's element; its rows are the list's options, in order. */
  root: Ref<HTMLElement | null>;
  /** The selected row's index. */
  active: Ref<number>;
  /** The rows the list shows; a change re-highlights the selection. */
  rows: Ref<readonly unknown[]>;
  /** Makes the row under the pointer the selection. */
  select: (index: number) => void;
}

export function useRowHighlight(deps: RowHighlightDeps) {
  /** The highlight is being put back: the pointer event is ours, not the user's. */
  let syncing = false;
  /** The row the user's pointer moved over, for the highlight its move causes. */
  let hovered: Element | null = null;
  let forget: ReturnType<typeof setTimeout> | undefined;

  function options(): Element[] {
    return Array.from(deps.root.value?.querySelectorAll('[role="option"]') ?? []);
  }

  /** Puts the list's highlight on the selected row and keeps that row in sight. */
  function sync(): void {
    const row = options()[deps.active.value];
    if (row === undefined) {
      return;
    }
    syncing = true;
    try {
      row.dispatchEvent(new Event('pointermove'));
    } finally {
      syncing = false;
    }
    row.scrollIntoView?.({ block: 'nearest' });
  }

  watch([deps.active, deps.rows], () => void nextTick(sync), { flush: 'post' });

  /** The user's pointer, in the capture phase before the row's own handler moves the highlight. */
  function onPointerMove(event: Event): void {
    if (syncing) {
      return;
    }
    const target = event.target instanceof Element ? event.target : null;
    hovered = target?.closest('[role="option"]') ?? null;
    // Only the highlight this very move causes is the user's.
    clearTimeout(forget);
    forget = setTimeout(() => {
      hovered = null;
    }, 0);
  }

  /** The list moved its highlight: from the pointer it is the selection, else it goes back. */
  function onHighlight(payload: { ref: HTMLElement } | undefined): void {
    if (syncing || payload === undefined) {
      return;
    }
    const index = options().indexOf(payload.ref);
    if (hovered !== null && payload.ref === hovered) {
      hovered = null;
      deps.select(index);
      return;
    }
    if (index !== deps.active.value) {
      void nextTick(sync);
    }
  }

  /** The pointer left the list and took the highlight along: it goes back to the selection. */
  function onLeave(): void {
    void nextTick(sync);
  }

  return { onPointerMove, onHighlight, onLeave, sync };
}
