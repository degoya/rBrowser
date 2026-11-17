// Forced states: the component forces the pseudo-classes over CDP (`CSS.forcePseudoState`,
// Chromium only, src/effects.rs); the page script frames the element it forces them on and
// labels it with the states (`:hover:focus`), in every engine.

import { type Finder, createScanOverlay } from './scan';

const COLOR = '#2f9e44';

export const findForced: Finder = ({ doc, options }) => {
  const selector = typeof options.selector === 'string' ? options.selector : '';
  const states = Array.isArray(options.states) ? options.states.filter((state) => typeof state === 'string') : ['hover'];
  if (!selector) return [];
  try {
    const element = doc.querySelector(selector);
    return element ? [{ element, label: states.map((state) => `:${state}`).join('') }] : [];
  } catch {
    return [];
  }
};

export const createForceState = () => createScanOverlay('force-state', COLOR, findForced);
