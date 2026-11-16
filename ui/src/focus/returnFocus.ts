// The focus back to where it was when a layer opened (RB-090-23, WCAG 2.4.3): a bar over the
// toolbar strip, an inline confirmation. `remember` when the layer opens, `restore` when it is
// closed on purpose (Escape, Cancel, done), `forget` when it closes because the focus left it -
// then the focus is where the reader put it. Nothing happens when the opener is gone, disabled
// or was the body (the command came from a page view or a shortcut there).

export interface ReturnFocus {
  /** Notes the element with the focus now; while one is noted, the first one stays. */
  remember: () => void;
  /** Focuses the noted element if it still stands and can take the focus. */
  restore: () => void;
  /** Drops the noted element. */
  forget: () => void;
}

export function returnFocus(doc: Document = document): ReturnFocus {
  let opener: HTMLElement | null = null;
  return {
    remember() {
      if (opener?.isConnected) {
        return;
      }
      const active = doc.activeElement;
      opener = active instanceof HTMLElement && active !== doc.body ? active : null;
    },
    restore() {
      const target = opener;
      opener = null;
      if (target !== null && target.isConnected && !target.matches(':disabled')) {
        target.focus();
      }
    },
    forget() {
      opener = null;
    },
  };
}
