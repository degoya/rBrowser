import { mkdirSync, mkdtempSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

import { describe, expect, it } from 'vitest';

import { findInstance, parseInstance, sameProject } from './instance.ts';

const token = 'c0'.repeat(32);
const file = (project: string, port = 4100) =>
  JSON.stringify({ version: 1, pid: 77, port, token, project });

describe('parseInstance', () => {
  it('reads the instance file of rb-mcp (version 1, a port, a 64-hex token)', () => {
    expect(parseInstance(file('C:\\site'))).toEqual({ version: 1, pid: 77, port: 4100, project: 'C:\\site' });
  });

  it('keeps no token: the harness never needs it', () => {
    expect(JSON.stringify(parseInstance(file('C:\\site')))).not.toContain(token);
  });

  it('refuses another version, port 0, a bad token or no JSON', () => {
    expect(parseInstance(JSON.stringify({ version: 2, pid: 1, port: 1, token, project: '' }))).toBeUndefined();
    expect(parseInstance(file('x', 0))).toBeUndefined();
    expect(parseInstance(JSON.stringify({ version: 1, pid: 1, port: 1, token: 'short', project: '' }))).toBeUndefined();
    expect(parseInstance('{')).toBeUndefined();
  });
});

describe('sameProject', () => {
  it('ignores case, slash direction, a trailing slash and the two WSL share names', () => {
    expect(sameProject('C:\\Site\\', 'c:/site')).toBe(true);
    expect(sameProject('\\\\wsl$\\Ubuntu\\home\\me\\p', '\\\\wsl.localhost\\Ubuntu\\home\\me\\p')).toBe(true);
    expect(sameProject('C:\\site', 'C:\\site2')).toBe(false);
  });
});

describe('findInstance', () => {
  it('finds the file of the project among others', () => {
    const dir = mkdtempSync(join(tmpdir(), 'e2e-mcp-'));
    mkdirSync(join(dir, 'mcp'));
    writeFileSync(join(dir, 'mcp', 'a.json'), file('C:\\other', 4001));
    writeFileSync(join(dir, 'mcp', 'b.json'), file('C:\\site', 4002));
    writeFileSync(join(dir, 'mcp', 'c.json'), 'not json');
    expect(findInstance(join(dir, 'mcp'), 'c:\\site')?.port).toBe(4002);
    expect(findInstance(join(dir, 'mcp'), 'C:\\none')).toBeUndefined();
    expect(findInstance(join(dir, 'missing'), 'C:\\site')).toBeUndefined();
  });
});
