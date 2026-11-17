// The panel's model: sources, requests, the order of the categories, keyboard moves, severities.
// The texts and failure sentences are panel-ui's (format.ts), tested there and in panel.test.ts.
import { describe, expect, it } from 'vitest';
import * as M from './panel/model';

describe('the error report model', () => {
  it('shortens a source to its file, line and column', () => {
    expect(M.shortSource({ url: 'https://a.test/js/app.js?v=2', line: 3, column: 9 })).toEqual({
      short: 'app.js:3:9',
      full: 'https://a.test/js/app.js?v=2:3:9',
    });
    expect(M.shortSource({ url: 'https://a.test/' })).toEqual({ short: 'a.test', full: 'https://a.test/' });
    expect(M.shortSource(undefined)).toBeNull();
  });

  it('writes a request as one line', () => {
    expect(M.requestText({ method: 'GET', status: 404, resourceType: 'Image' })).toBe('GET 404 Image');
    expect(M.requestText({ method: 'GET', status: null })).toBe('GET');
    expect(M.requestText(undefined)).toBe('');
  });

  it('groups by category in the report order and leaves empty ones out', () => {
    const groups = M.byCategory([{ rule: 'deprecation' }, { rule: 'js-exception' }, { rule: 'deprecation' }]);
    expect(groups.map((group) => [group.category, group.entries.length])).toEqual([
      ['js-exception', 1],
      ['deprecation', 2],
    ]);
    expect(M.keyName('mixed-content-blocked')).toBe('mixed_content_blocked');
  });

  it('moves the focus with the arrows, Home and End', () => {
    expect(M.moveFocus(0, 'ArrowDown', 3)).toBe(1);
    expect(M.moveFocus(2, 'ArrowDown', 3)).toBe(2);
    expect(M.moveFocus(1, 'Home', 3)).toBe(0);
    expect(M.moveFocus(0, 'End', 3)).toBe(2);
    expect(M.moveFocus(0, 'x', 3)).toBeNull();
    expect(M.moveFocus(0, 'ArrowDown', 0)).toBeNull();
  });

  it('gives every severity a colour and an icon, info for an unknown one', () => {
    expect(M.severityOf('error')).toEqual({ color: 'error', icon: 'i-lucide-circle-x' });
    expect(M.severityOf('warning').color).toBe('warning');
    expect(M.severityOf('nope')).toEqual(M.SEVERITY.info);
  });

  it('gives every view a colour', () => {
    expect(M.viewColor(0)).toBe(M.PALETTE[0]);
    expect(M.viewColor(M.PALETTE.length)).toBe(M.PALETTE[0]);
    expect(M.viewColor(-1)).toBe(M.PALETTE[M.PALETTE.length - 1]);
  });
});
