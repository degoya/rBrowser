// A page state for the page API's tests.

import type { PageState } from '../protocol';

/** A tablet view with the bus on. */
export const STATE: PageState = {
  info: {
    id: 2,
    label: 'Tablet',
    width: 768,
    height: 1024,
    index: 1,
    engine: 'chromium',
    emulation: { dpr: 2, mobile: true, touch: true, orientation: 'portrait' },
  },
  bus: { enabled: true, maxPayloadBytes: 65536, maxPerSecond: 100 },
};
