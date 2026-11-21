import { describe, expect, it, vi } from 'vitest';
import { IpcClient } from '../ipc/client';
import type { CoreToShell, ShellToCore } from '../ipc/protocol';
import { IpcError } from '../ipc/transport';
import { copyReportMarkdown } from './copyReport';

function client(answer: (message: ShellToCore) => CoreToShell | Error) {
  const sent: ShellToCore[] = [];
  const ipc = new IpcClient({
    send: async (message) => {
      sent.push(message);
      const reply = answer(message);
      if (reply instanceof Error) throw reply;
      return reply;
    },
    subscribe: () => () => undefined,
  });
  return { ipc, sent };
}

describe('copying a bug report from the status bar', () => {
  it('asks the core for the report by name and puts its text on the clipboard', async () => {
    const { ipc, sent } = client(() => ({
      type: 'bugReport',
      answer: { kind: 'markdown', report: '2026-11-20_14-03-07', text: '# Bug report\n' },
    }));
    const clipboard = { writeText: vi.fn(async () => undefined) };
    expect(await copyReportMarkdown(ipc, '2026-11-20_14-03-07', clipboard)).toBe(true);
    expect(sent).toEqual([{ type: 'bugReport', request: { op: 'getMarkdown', report: '2026-11-20_14-03-07' } }]);
    expect(clipboard.writeText).toHaveBeenCalledWith('# Bug report\n');
  });

  it('copies nothing and says so when the core no longer knows the report', async () => {
    const { ipc, sent } = client((message) =>
      message.type === 'bugReport' ? new IpcError(404, 'gone') : { type: 'ack' },
    );
    const clipboard = { writeText: vi.fn(async () => undefined) };
    expect(await copyReportMarkdown(ipc, 'r', clipboard)).toBe(false);
    expect(clipboard.writeText).not.toHaveBeenCalled();
    expect(sent.at(-1)).toEqual({ type: 'reportStatus', level: 'error', messageKey: 'report.error.copy_failed' });
  });

  it('says so when the clipboard refuses the text', async () => {
    const { ipc, sent } = client((message) =>
      message.type === 'bugReport'
        ? { type: 'bugReport', answer: { kind: 'markdown', report: 'r', text: 'x' } }
        : { type: 'ack' },
    );
    const clipboard = { writeText: vi.fn(async () => Promise.reject(new Error('Document is not focused'))) };
    expect(await copyReportMarkdown(ipc, 'r', clipboard)).toBe(false);
    expect(sent.at(-1)).toEqual({ type: 'reportStatus', level: 'error', messageKey: 'report.error.copy_failed' });
  });

  it('takes only the text of the report it asked for', async () => {
    const { ipc } = client((message) =>
      message.type === 'bugReport'
        ? { type: 'bugReport', answer: { kind: 'markdown', report: 'other', text: 'x' } }
        : { type: 'ack' },
    );
    const clipboard = { writeText: vi.fn(async () => undefined) };
    expect(await copyReportMarkdown(ipc, 'r', clipboard)).toBe(false);
    expect(clipboard.writeText).not.toHaveBeenCalled();
  });
});
