import { describe, expect, it } from 'vitest';
import { IpcClient } from '../ipc/client';
import type { CoreToShell, ReportPreview, ShellToCore } from '../ipc/protocol';
import { IpcError } from '../ipc/transport';
import { createPreview } from './preview';

const PREVIEW: ReportPreview = {
  previewId: 4,
  folder: 'C:/work/shop/.rbrowser/out/reports/2026-11-20_14-03-07',
  markdown: '# Bug report\n\n`<img src=x onerror=alert(1)>`\n',
  images: 3,
  annotate: true,
};

/** A client whose core answers each request with `answer(message)`, recording what was sent. */
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

const waiting = (message: ShellToCore): CoreToShell =>
  message.type === 'bugReport' && message.request.op === 'getPreview'
    ? { type: 'bugReport', answer: { kind: 'preview', ...PREVIEW } }
    : { type: 'ack' };

describe('the bug report preview', () => {
  it('loads the waiting report and saves it with its number', async () => {
    const { ipc, sent } = client(waiting);
    const preview = createPreview(ipc);
    expect(preview.state.value).toEqual({ phase: 'loading' });
    await preview.load();
    expect(preview.state.value).toEqual({ phase: 'open', preview: PREVIEW });
    await preview.decide(true);
    expect(sent.at(-1)).toEqual({ type: 'bugReport', request: { op: 'decide', previewId: 4, save: true } });
    expect(preview.state.value).toEqual({ phase: 'decided', saved: true });
  });

  it('cancels with the same number and decides only once', async () => {
    const { ipc, sent } = client(waiting);
    const preview = createPreview(ipc);
    await preview.load();
    await preview.decide(false);
    await preview.decide(true);
    const decisions = sent.filter((message) => message.type === 'bugReport' && message.request.op === 'decide');
    expect(decisions).toEqual([{ type: 'bugReport', request: { op: 'decide', previewId: 4, save: false } }]);
    expect(preview.state.value).toEqual({ phase: 'decided', saved: false });
  });

  it('says when no report waits, and when the core could not be asked', async () => {
    const none = createPreview(client(() => ({ type: 'ack' })).ipc);
    await none.load();
    expect(none.state.value).toEqual({ phase: 'none' });
    const failed = createPreview(client(() => new IpcError(500, 'down')).ipc);
    await failed.load();
    expect(failed.state.value).toEqual({ phase: 'failed' });
  });

  it('shows a failed decision and lets the person try again', async () => {
    let refuse = true;
    const { ipc } = client((message) => {
      if (message.type === 'bugReport' && message.request.op === 'decide' && refuse) {
        return new IpcError(404, 'no bug report preview 4 waits');
      }
      return waiting(message);
    });
    const preview = createPreview(ipc);
    await preview.load();
    await preview.decide(true);
    expect(preview.state.value).toEqual({ phase: 'open', preview: PREVIEW, failed: true });
    refuse = false;
    await preview.decide(true);
    expect(preview.state.value).toEqual({ phase: 'decided', saved: true });
  });
});
