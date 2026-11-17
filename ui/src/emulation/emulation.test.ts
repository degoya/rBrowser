import { describe, expect, it } from 'vitest';
import { EMULATION_COMMANDS, nextReducedMotion, resetMessage, setMessage } from './emulation';
import type { Emulation } from '../ipc/protocol';

const plain: Emulation = { width: 393, height: 852, dpr: 3, mobile: true, touch: true, media: {} };

describe('the emulation messages', () => {
  it('target the active view, or every view with Shift', () => {
    expect(setMessage({ colorScheme: 'dark' }, false)).toEqual({
      type: 'runCommand',
      id: 'emulation.set',
      args: { fields: { colorScheme: 'dark' } },
    });
    expect(setMessage({ reducedMotion: true }, true)).toEqual({
      type: 'runCommand',
      id: 'emulation.set',
      args: { fields: { reducedMotion: true }, target: 'all' },
    });
    expect(resetMessage(false)).toEqual({ type: 'runCommand', id: 'emulation.reset_view' });
    expect(resetMessage(true)).toEqual({
      type: 'runCommand',
      id: 'emulation.reset_view',
      args: { target: 'all' },
    });
  });

  it('flip reduced motion from the active view', () => {
    expect(nextReducedMotion(null)).toBe(true);
    expect(nextReducedMotion(plain)).toBe(true);
    expect(nextReducedMotion({ ...plain, reducedMotion: true })).toBe(false);
  });

  it('name the MCP tool of every command', () => {
    expect(Object.values(EMULATION_COMMANDS).map((c) => c.mcp)).toEqual([
      'emulation_set',
      'emulation_set',
    ]);
  });
});
