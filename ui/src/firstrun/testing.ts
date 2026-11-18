// Test helpers of the setup wizard's tests: sample plugins as RB-091-04's `available` answers
// them, and a core that answers requests and pushes events.

import { IpcClient } from '../ipc/client';
import type { CoreToShell, ShellToCore } from '../ipc/protocol';
import type { Transport } from '../ipc/transport';
import type { AvailablePlugin } from './plugins';

export const SAMPLE_PLUGINS: AvailablePlugin[] = [
  {
    id: 'seo',
    name: 'SEO',
    description: 'Titles, meta tags and headings of every page.',
    category: 'checks',
    sizeBytes: 1_500_000,
    recommended: true,
    origin: 'bundle',
    grants: ['views', 'http'],
    installed: false,
    version: '0.9.0',
  },
  {
    id: 'axe',
    name: 'Accessibility',
    description: 'axe-core rules for accessibility.',
    category: 'checks',
    sizeBytes: 2_000_000,
    recommended: true,
    origin: 'repository',
    grants: ['views'],
    installed: false,
    version: '0.9.0',
  },
  {
    id: 'vue-devtools',
    name: 'Vue DevTools',
    description: 'The Vue panel.',
    category: 'devtools',
    sizeBytes: 3_000_000,
    recommended: false,
    origin: 'bundle',
    grants: ['cdp'],
    installed: false,
    version: '0.9.0',
  },
  {
    id: 'video',
    name: 'Video',
    description: 'Records the views.',
    category: 'capture',
    sizeBytes: 500_000,
    recommended: true,
    origin: 'bundle',
    grants: ['out-write'],
    installed: true,
    version: '0.9.0',
  },
];

type Answer = CoreToShell | ((message: ShellToCore) => CoreToShell | Promise<CoreToShell>);

/** A core that records requests; `answer` answers each one (`ack` when it gives nothing), and
 * `emit` pushes an event. */
export function fakeCore(answer: (message: ShellToCore) => Answer | undefined = () => undefined) {
  const listeners = new Set<(message: CoreToShell) => void>();
  const sent: ShellToCore[] = [];
  const transport: Transport = {
    async send(message) {
      sent.push(message);
      const reply = answer(message);
      if (reply === undefined) return { type: 'ack' };
      return typeof reply === 'function' ? reply(message) : reply;
    },
    subscribe(listener) {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
  };
  const emit = (message: CoreToShell): void => listeners.forEach((listener) => listener(message));
  return { client: new IpcClient(transport), sent, emit };
}
