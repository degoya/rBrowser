// An inspection as the component answers it, for the tests.

import type { Inspection } from './types';

export function inspection(over: Partial<Inspection> = {}): Inspection {
  return {
    tag: 'button',
    id: 'send',
    classes: ['btn', 'btn-primary'],
    size: { width: 120.5, height: 40, dpr: 2, deviceWidth: 241, deviceHeight: 80 },
    box: {
      margin: { top: 4, right: 8, bottom: 4, left: 8 },
      border: { top: 1, right: 1, bottom: 1, left: 1 },
      padding: { top: 6, right: 12, bottom: 6, left: 12 },
      content: { width: 94.5, height: 26 },
    },
    colors: { foreground: '#777777', background: '#ffffff', backgroundUncertain: false },
    contrast: { ratio: 4.48, level: 'aa_large', largeText: false, foreground: '#777777', background: '#ffffff' },
    font: {
      declared: '"Inter", sans-serif',
      rendered: [{ family: 'Segoe UI', postScriptName: 'SegoeUI', custom: false, glyphs: 4 }],
      size: 16,
      weight: 400,
      style: 'normal',
      lineHeight: '24px',
      letterSpacing: 'normal',
    },
    role: 'button',
    name: 'Send',
    accessibility: 'cdp',
    ...over,
  };
}
