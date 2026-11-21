import { describe, expect, it } from 'vitest';
import { DEV_SERVER_ADDRESSES, proposedViews, reasonKey } from './project';

describe('the project step', () => {
  it('proposes the usual dev server addresses, Vite first', () => {
    expect(DEV_SERVER_ADDRESSES[0]).toBe('http://localhost:5173');
    expect(new Set(DEV_SERVER_ADDRESSES).size).toBe(DEV_SERVER_ADDRESSES.length);
  });

  it('lists the views of a proposal by size or device', () => {
    expect(
      proposedViews({
        breakpoints: [
          { id: 'mobile', label: 'Mobile', width: 393, height: 852 },
          { id: 'iphone-15', device: 'iphone-15' },
          { id: 'odd' },
          'broken',
        ],
      }),
    ).toEqual([
      { id: 'mobile', label: 'Mobile', size: { width: 393, height: 852 } },
      { id: 'iphone-15', label: 'iphone-15', size: { device: 'iphone-15' } },
      { id: 'odd', label: 'odd', size: null },
    ]);
    expect(proposedViews({ version: 1 })).toEqual([]);
  });

  it('names each reason of the analyser, and an unknown one as nothing found', () => {
    expect(reasonKey('unreachable')).toBe('firstrun.project.reason.unreachable');
    expect(reasonKey('plugin_missing')).toBe('firstrun.project.reason.plugin_missing');
    expect(reasonKey('constructor')).toBe('firstrun.project.reason.nothing_found');
    expect(reasonKey(undefined)).toBe('firstrun.project.reason.nothing_found');
  });
});
