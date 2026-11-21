// The consent check's collector (RB-094-05): one expression the component runs through
// `evaluate` in the plugin's isolated world, called with its configuration (data/cmp.json's
// selectors and texts, written into the call's argument by src/cmp.rs). Mode `read` only reads:
// the page's URL, the keys of its localStorage (never a value) and the consent banner shown - the
// first known manager whose banner element exists, whether it is shown and whether a "Reject all"
// button is found. Mode `reject` clicks that button too; it is meant for the check's own probe
// session only, never for a view of the grid.
// SPDX-License-Identifier: GPL-3.0-or-later
((config) => {
  const MAX_KEYS = 200;
  const MAX_KEY_CHARS = 200;

  const shown = (element) => {
    for (let node = element; node && node.nodeType === 1; node = node.parentElement) {
      if (node.hidden) return false;
      const style = getComputedStyle(node);
      if (style.display === 'none' || style.visibility === 'hidden') return false;
    }
    return true;
  };

  const query = (root, selector) => {
    try {
      return root.querySelector(selector);
    } catch (_) {
      return null;
    }
  };

  // The banner and the roots to search in: the element and its open shadow root.
  const roots = (element) => (element.shadowRoot ? [element, element.shadowRoot] : [element]);

  const byText = (banner) => {
    const candidates = roots(banner).flatMap((root) => [...root.querySelectorAll('button, a, [role="button"], input[type="button"]')]);
    for (const text of config.rejectTexts) {
      const found = candidates.find((element) => (element.value || element.textContent || '').trim().toLowerCase() === text && shown(element));
      if (found) return found;
    }
    return null;
  };

  const rejectButton = (cmp, banner) => {
    for (const selector of cmp.reject) {
      for (const root of [...roots(banner), document]) {
        const found = query(root, selector);
        if (found && shown(found)) return found;
      }
    }
    return byText(banner);
  };

  const detect = () => {
    for (const cmp of config.cmps) {
      for (const selector of cmp.banner) {
        const banner = query(document, selector);
        if (banner) {
          const visible = shown(banner);
          return { cmp, banner, visible, button: visible ? rejectButton(cmp, banner) : null };
        }
      }
    }
    return null;
  };

  const storageKeys = () => {
    const keys = [];
    let cut = false;
    try {
      for (let index = 0; index < localStorage.length; index += 1) {
        if (keys.length >= MAX_KEYS) {
          cut = true;
          break;
        }
        const key = localStorage.key(index);
        if (key !== null) keys.push(key.slice(0, MAX_KEY_CHARS));
      }
    } catch (_) {
      // An opaque origin or blocked storage: no keys to name.
    }
    return { keys, cut };
  };

  const found = detect();
  let clicked = false;
  if (config.mode === 'reject' && found && found.button) {
    found.button.click();
    clicked = true;
  }
  const storage = storageKeys();
  return {
    url: location.href,
    storage: storage.keys,
    storageCut: storage.cut,
    banner: {
      cmp: found ? found.cmp.id : null,
      visible: found ? found.visible : false,
      rejectable: Boolean(found && found.button),
    },
    clicked,
  };
})(__CONSENT_CONFIG__)
