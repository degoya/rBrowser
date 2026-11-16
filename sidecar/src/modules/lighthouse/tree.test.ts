import { describe, expect, it } from 'vitest';

import { type TreeDeps, killTree } from './tree.ts';

function deps(platform: NodeJS.Platform, stdout = '', groups: Record<number, number[]> = {}) {
  const calls: string[] = [];
  const value: TreeDeps = {
    platform,
    systemRoot: 'C:\\Windows',
    exec: async (file, args) => {
      calls.push(`${file} ${args.join(' ')}`);
      return { stdout };
    },
    group: (pgid) => groups[pgid] ?? null,
    killGroup: (pgid) => {
      calls.push(`kill -${pgid}`);
    },
  };
  return { calls, value };
}

describe('killTree', () => {
  it('ends the whole tree with taskkill on Windows and counts its lines', async () => {
    const stdout = [
      'SUCCESS: The process with PID 12 (child process of PID 10) has been terminated.',
      'SUCCESS: The process with PID 11 (child process of PID 10) has been terminated.',
      'SUCCESS: The process with PID 10 (child process of PID 4) has been terminated.',
      '',
    ].join('\r\n');
    const { calls, value } = deps('win32', stdout);
    expect(await killTree(10, value, () => true)).toBe(3);
    expect(calls).toEqual(['C:\\Windows\\System32\\taskkill.exe /PID 10 /T /F']);
  });

  it('counts nothing when the tree is gone already', async () => {
    const { value } = deps('win32', '');
    expect(await killTree(10, value, () => true)).toBe(0);
  });

  it('answers null when taskkill does not run', async () => {
    const { value } = deps('win32');
    value.exec = async () => null;
    expect(await killTree(10, value, () => true)).toBeNull();
  });

  it("kills the browser's process group elsewhere and counts its members", async () => {
    const { calls, value } = deps('linux', '', { 20: [20, 21, 22] });
    expect(await killTree(20, value, () => true)).toBe(3);
    expect(calls).toEqual(['kill -20']);
  });

  it('refuses a pid that is no process', async () => {
    const { calls, value } = deps('linux');
    expect(await killTree(0, value, () => true)).toBe(0);
    expect(await killTree(-5, value, () => true)).toBe(0);
    expect(calls).toEqual([]);
  });

  // Review 4: the id of an exited browser may belong to somebody else's process (group) by now.
  it('kills nothing of a browser that has exited, on any platform', async () => {
    for (const platform of ['win32', 'linux'] as const) {
      const { calls, value } = deps(platform, 'SUCCESS', { 20: [20, 21] });
      expect(await killTree(20, value, () => false)).toBe(0);
      expect(calls).toEqual([]);
    }
  });

  it('asks whether the browser lives right before the kill, after counting its group', async () => {
    const { calls, value } = deps('linux', '', { 20: [20, 21] });
    let asked = 0;
    expect(
      await killTree(20, value, () => {
        asked += 1;
        return asked < 2;
      }),
    ).toBe(0);
    expect(calls).toEqual([]);
  });
});
