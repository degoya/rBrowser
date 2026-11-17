// Test helpers of the wizard's tests: a core that records requests and pushes events.

import { IpcClient } from '../ipc/client';
import type { BreakpointProposal, CoreToShell, ShellToCore } from '../ipc/protocol';
import type { Transport } from '../ipc/transport';

/** A core that records requests and pushes events; `answers` answers a request by its type
 * (`ack` for any other). */
export function fakeCore(answers: Partial<Record<ShellToCore['type'], CoreToShell>> = {}) {
  const listeners = new Set<(message: CoreToShell) => void>();
  const sent: ShellToCore[] = [];
  const transport: Transport = {
    async send(message) {
      sent.push(message);
      return answers[message.type] ?? { type: 'ack' };
    },
    subscribe(listener) {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
  };
  const emit = (message: CoreToShell) => listeners.forEach((listener) => listener(message));
  return { client: new IpcClient(transport), sent, emit, listeners };
}

export function proposal(existing: boolean): BreakpointProposal {
  return {
    proposalId: 7,
    existing,
    detected: [{ width: 768, kind: 'min', label: 'md', count: 3, heuristic: false }],
    sources: [{ url: 'http://site.test/app.css', count: 3 }],
    breakpoints: [
      { id: 'mobile', label: 'mobile', width: 375, height: 667, isNew: false },
      { id: 'md', label: 'md', width: 768, height: 1024, isNew: true },
      { id: 'desktop', label: 'desktop', width: 1440, height: 900, isNew: true },
    ],
  };
}
