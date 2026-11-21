import { describe, expect, it } from 'vitest';
import { IpcClient } from '../ipc/client';
import { FLOWS_CONFIRM_KEY, confirmOf, confirmsDelete } from './confirmDelete';

describe('asking before a delete (RB-090-91)', () => {
  it('asks unless the setting is false', () => {
    expect(confirmOf(true)).toBe(true);
    expect(confirmOf(undefined)).toBe(true);
    expect(confirmOf(false)).toBe(false);
  });

  it('reads the setting, and asks when the core answers none', async () => {
    const answering = (value: boolean | null) =>
      new IpcClient({
        send: async () =>
          value === null
            ? { type: 'ack' }
            : { type: 'settings', values: [{ key: FLOWS_CONFIRM_KEY, value, origin: 'user' }] },
        subscribe: () => () => undefined,
      });
    expect(await confirmsDelete(answering(false), FLOWS_CONFIRM_KEY)).toBe(false);
    expect(await confirmsDelete(answering(true), FLOWS_CONFIRM_KEY)).toBe(true);
    expect(await confirmsDelete(answering(null), FLOWS_CONFIRM_KEY)).toBe(true);
  });
});
