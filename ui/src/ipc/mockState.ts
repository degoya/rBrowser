// A believable core state for the mock transport: three views of a local dev server. Used by
// `npm run dev` outside CEF and by the tests.

import type { GridCell, StateSnapshot, ViewInfo } from './protocol';

function cell(breakpoint: string, col: number, x: number, width: number, height: number): GridCell {
  return {
    breakpoint,
    col,
    row: 0,
    colSpan: 1,
    rowSpan: 1,
    zoom: 'fit',
    rect: { x, y: 72, width, height },
    label: { x, y: 52, width, height: 20 },
  };
}

function view(
  id: number,
  breakpoint: string,
  label: string,
  size: [number, number, number],
  rect: [number, number, number],
): ViewInfo {
  const [width, height, dpr] = size;
  const [x, cellWidth, cellHeight] = rect;
  return {
    id,
    label,
    breakpoint,
    cellBreakpoint: breakpoint,
    engine: 'chromium',
    width,
    height,
    dpr,
    scale: Math.min(1, cellWidth / width),
    url: 'http://localhost:5173/',
    title: 'Shop',
    loading: false,
    session: 'default',
    cell: cell(breakpoint, id - 1, x, cellWidth, cellHeight),
  };
}

export function sampleState(): StateSnapshot {
  return {
    project: { name: 'shop', root: 'C:/work/shop', trusted: true },
    views: [
      view(1, 'mobile', 'Mobile', [393, 852, 3], [8, 300, 650]),
      view(2, 'tablet', 'Tablet', [820, 1180, 2], [316, 450, 650]),
      view(3, 'desktop', 'Desktop', [1440, 900, 1], [774, 760, 475]),
    ],
    activeView: 1,
    url: 'http://localhost:5173/',
    environment: 'local',
    environments: [
      { id: 'local', url: 'http://localhost:5173/' },
      { id: 'staging', url: 'https://staging.example.com/', color: '#f59e0b' },
    ],
    layout: { preset: 'grid-main', presets: ['grid-main', 'focus'], mode: 'grid', focus: null },
    sync: {
      enabled: true,
      features: {
        navigation: true,
        scroll: true,
        click: true,
        input: true,
        keyboard: true,
        hover: true,
        leaderOnlySubmit: true,
        leaderPerSession: true,
        activateOnFocus: false,
        hoverThrottleMs: 100,
        desyncTimeoutMs: 750,
        contextMenu: false,
        forms: true,
        focus: false,
        navigationSettleMs: 1000,
        navigationIncludeHash: false,
        detectBrowsersync: true,
        scrollMode: 'anchor',
        scrollDebounceMs: 150,
      },
    },
    breakpoints: [
      { id: 'mobile', label: 'Mobile', width: 393, height: 852 },
      { id: 'tablet', label: 'Tablet', width: 820, height: 1180 },
      { id: 'desktop', label: 'Desktop', width: 1440, height: 900 },
    ],
    sessions: [{ id: 'default' }, { id: 'admin', color: '#e5484d' }],
    detected: [],
    devtools: null,
    bookmarks: [
      { id: 'home', label: 'Home', url: 'http://localhost:5173/', path: '/' },
      { id: 'pdp', label: 'Product', group: 'Shop', url: 'http://localhost:5173/products/42', path: '/products/42' },
      { id: 'checkout', label: 'Checkout', group: 'Shop', url: 'http://localhost:5173/checkout', path: '/checkout' },
      { id: 'storybook', label: 'Storybook', group: 'Tools', url: 'http://localhost:6006/' },
    ],
  };
}
