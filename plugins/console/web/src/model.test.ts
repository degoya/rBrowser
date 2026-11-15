import { describe, expect, it } from 'vitest';

import * as M from './panel/model';

describe('failures', () => {
  // On Nuxt UI (RB-080-84) the panel says a code as a sentence (panel-ui's `panel.error`); the
  // page's own words stay as they came.
  it('tells a code from the page\'s own words', () => {
    expect(M.isCode('plugin.console.errors.no_expression')).toBe(true);
    expect(M.isCode('plugins.error.busy reason=no_answer')).toBe(true);
    expect(M.isCode('TypeError: x is not a function')).toBe(false);
    expect(M.isCode('ReferenceError: a.b is not defined')).toBe(false);
  });
});

describe('sources', () => {
  it('shows the file and line, the full URL in the tooltip', () => {
    expect(M.shortSource({ url: 'http://x.test/js/app.js?v=3', line: 12 })).toEqual({
      short: 'app.js:12',
      full: 'http://x.test/js/app.js?v=3:12',
    });
    expect(M.shortSource({ url: 'http://x.test/' })).toEqual({ short: 'x.test', full: 'http://x.test/' });
    expect(M.shortSource(undefined)).toBeNull();
    expect(M.shortSource({ url: '' })).toBeNull();
  });
});

describe("rBrowser's own messages", () => {
  // RB-080-66: a plugin's line names rBrowser and the plugin, never "unknown source".
  it('name rBrowser and the plugin as their source', () => {
    const strings: Record<string, string> = { 'panel.rbrowserSource': 'rBrowser: {plugin}', 'panel.unknownSource': 'unknown source' };
    const t = (key: string, params: Record<string, unknown> = {}): string => (strings[key] ?? key).replace(/\{(\w+)\}/g, (_, name: string) => String(params[name] ?? ''));
    expect(M.sourceLabel(t, { kind: 'rbrowser', source: { url: 'rbrowser://plugin/rulers-guides' } })).toEqual({
      text: 'rBrowser: rulers-guides',
      title: 'rbrowser://plugin/rulers-guides',
    });
    expect(M.sourceLabel(t, { kind: 'console', source: { url: 'http://x.test/a.js', line: 3 } })).toEqual({
      text: 'a.js:3',
      title: 'http://x.test/a.js:3',
    });
    expect(M.sourceLabel(t, { kind: 'console' })).toEqual({ text: 'unknown source', title: '' });
  });
});

describe('filters', () => {
  it('starts with the setting, or every level but debug', () => {
    expect(M.startLevels({})).toEqual(['error', 'warning', 'info', 'log', 'network']);
    expect(M.startLevels({ 'console.levels': ['network', 'error', 'bogus'] })).toEqual(['error', 'network']);
  });

  it('asks grouped for the levels in order, the views and the search', () => {
    expect(M.groupedArgs({ levels: new Set(['log', 'error']), views: new Set(), search: '  ' })).toEqual({
      levels: ['error', 'log'],
      limit: 200,
    });
    expect(M.groupedArgs({ levels: new Set(['error']), views: new Set([4, 2]), search: ' boom ' })).toEqual({
      levels: ['error'],
      views: [2, 4],
      search: 'boom',
      limit: 200,
    });
  });
});

describe('keyboard', () => {
  it('moves through the rows and stops at the ends', () => {
    expect(M.moveFocus(0, 'ArrowDown', 3)).toBe(1);
    expect(M.moveFocus(2, 'ArrowDown', 3)).toBe(2);
    expect(M.moveFocus(0, 'ArrowUp', 3)).toBe(0);
    expect(M.moveFocus(1, 'Home', 3)).toBe(0);
    expect(M.moveFocus(0, 'End', 3)).toBe(2);
    expect(M.moveFocus(0, 'a', 3)).toBeNull();
    expect(M.moveFocus(0, 'ArrowDown', 0)).toBeNull();
  });
});

describe('view colours', () => {
  it('cycles through the palette', () => {
    expect(M.viewColor(0)).toBe(M.PALETTE[0]);
    expect(M.viewColor(M.PALETTE.length)).toBe(M.PALETTE[0]);
    expect(new Set(M.PALETTE).size).toBe(M.PALETTE.length);
  });
});
