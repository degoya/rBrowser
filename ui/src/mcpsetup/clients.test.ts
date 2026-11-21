import { describe, expect, it } from 'vitest';
import type { McpClientPlan } from '../ipc/protocol';
import { canWrite, checkLine, defaultChoice, MCP_CLIENTS, writeCount } from './clients';

const cursor: McpClientPlan = {
  client: 'cursor',
  project: { path: '.cursor/mcp.json', status: 'create', entry: '{}', kept: 0 },
  userFile: { path: 'C:\\Users\\me\\.cursor\\mcp.json', status: 'add', entry: '{}', kept: 2 },
};

describe('the MCP setup clients', () => {
  it('lists the six clients the owner named, Claude Code first', () => {
    expect(MCP_CLIENTS).toEqual(['claudeCode', 'vscode', 'claudeDesktop', 'cursor', 'codex', 'pi']);
  });

  it('writes a file only when the write changes it', () => {
    expect(canWrite(cursor.project)).toBe(true);
    expect(canWrite({ ...cursor.userFile!, status: 'replace' })).toBe(true);
    expect(canWrite({ ...cursor.userFile!, status: 'unchanged' })).toBe(false);
    expect(canWrite({ ...cursor.userFile!, status: 'unreadable' })).toBe(false);
    expect(canWrite(undefined)).toBe(false);
  });

  it('ticks the project file and never the user file by itself', () => {
    expect(defaultChoice(cursor)).toEqual({ project: true, user: false });
    expect(defaultChoice({ client: 'claudeDesktop', projectReason: 'x' })).toEqual({ project: false, user: false });
    expect(defaultChoice(undefined)).toEqual({ project: false, user: false });
  });

  it('counts the files a click writes', () => {
    expect(writeCount(cursor, { project: true, user: true })).toBe(2);
    expect(writeCount(cursor, { project: false, user: true })).toBe(1);
    expect(writeCount({ ...cursor, project: { ...cursor.project!, status: 'unchanged' } }, { project: true, user: false })).toBe(0);
    expect(writeCount(undefined, { project: true, user: true })).toBe(0);
  });

  it('says whether a client reached rBrowser, without a token', () => {
    expect(checkLine({ op: 'check', requestId: 1, enabled: false, running: false, calls: 0 })).toEqual({ key: 'mcp.setup.check.off' });
    expect(checkLine({ op: 'check', requestId: 1, enabled: true, running: false, calls: 0 })).toEqual({ key: 'mcp.setup.check.starting' });
    expect(checkLine({ op: 'check', requestId: 1, enabled: true, running: true, port: 7420, calls: 0 })).toEqual({
      key: 'mcp.setup.check.waiting',
      args: { port: 7420 },
    });
    expect(checkLine({ op: 'check', requestId: 1, enabled: true, running: true, port: 7420, lastCallSecs: 12, calls: 3 })).toEqual({
      key: 'mcp.setup.check.connected',
      args: { seconds: 12, calls: 3 },
    });
  });
});
