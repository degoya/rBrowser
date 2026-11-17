import { describe, expect, it } from 'vitest';
import { IpcClient, measureRoundtrip } from './client';
import { IpcErrorCode, type CoreToShell, type ShellToCore } from './protocol';
import { IpcError, type Transport } from './transport';

function transportAnswering(answer: (m: ShellToCore) => CoreToShell): Transport & {
  emit: (m: CoreToShell) => void;
} {
  const listeners = new Set<(m: CoreToShell) => void>();
  return {
    send: (m) => Promise.resolve(answer(m)),
    subscribe(listener) {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
    emit: (m) => listeners.forEach((l) => l(m)),
  };
}

describe('the IPC client', () => {
  it('returns the answer of the expected type', async () => {
    const client = new IpcClient(transportAnswering(() => ({ type: 'about', version: '0.1.0', commit: 'abc', cef: '1', chromium: '2' })));
    const about = await client.expect({ type: 'getAbout' }, 'about');
    expect(about.version).toBe('0.1.0');
  });

  it('refuses an answer of another type', async () => {
    const client = new IpcClient(transportAnswering(() => ({ type: 'ack' })));
    const error = await client.expect({ type: 'getState' }, 'state').catch((e: unknown) => e);
    expect(error).toBeInstanceOf(IpcError);
    expect((error as IpcError).code).toBe(IpcErrorCode.Internal);
  });

  it('measures every roundtrip', async () => {
    let clock = 0;
    const transport = transportAnswering(() => {
      clock += 4;
      return { type: 'ack' };
    });
    const client = new IpcClient(transport, () => clock);
    await client.request({ type: 'back' });
    await client.request({ type: 'forward' });
    expect(client.stats()).toEqual({ count: 2, meanMs: 4, maxMs: 4 });
  });

  it('measureRoundtrip reports the mean of n getState requests', async () => {
    let clock = 0;
    const sent: string[] = [];
    const transport = transportAnswering((m) => {
      sent.push(m.type);
      clock += 2;
      return { type: 'ack' };
    });
    const result = await measureRoundtrip(new IpcClient(transport, () => clock), 5);
    expect(sent).toEqual(['getState', 'getState', 'getState', 'getState', 'getState']);
    expect(result).toEqual({ count: 5, meanMs: 2, maxMs: 2 });
  });

  it('passes core events to every listener until it stops', () => {
    const transport = transportAnswering(() => ({ type: 'ack' }));
    const client = new IpcClient(transport);
    const seen: string[] = [];
    const stop = client.on((m) => seen.push(m.type));
    transport.emit({ type: 'notification', level: 'info', messageKey: 'shell.toolbar.label' });
    stop();
    transport.emit({ type: 'ack' });
    expect(seen).toEqual(['notification']);
  });
});
