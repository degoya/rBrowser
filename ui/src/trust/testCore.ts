// A stand-in core for the trust tests: one open prompt (id 7) until the project is trusted.

import { IpcClient } from '../ipc/client';
import type {
  BlockedItem,
  CoreToShell,
  PluginGrantDetails,
  ShellToCore,
  TrustInfo,
} from '../ipc/protocol';
import type { Transport } from '../ipc/transport';

export const ROOT = 'C:/work/shop';

/** A project plugin package the fake project holds. */
export interface FakePlugin {
  path: string;
  capabilities: string[];
  grants?: PluginGrantDetails;
}

/** A core that knows only trust: one open prompt, trusted after `trust`. */
export function fakeTrustCore(plugin?: FakePlugin) {
  const listeners = new Set<(message: CoreToShell) => void>();
  const sent: ShellToCore[] = [];
  let trusted = false;
  const projects: TrustInfo['projects'] = [];
  const blocked: BlockedItem[] = [
    { kind: 'flows', path: '.rbrowser/flows/login.ts' },
    { kind: 'hostResolverRules', path: '/network/hostResolverRules' },
  ];
  if (plugin !== undefined) blocked.push({ kind: 'plugins', path: plugin.path });
  const pluginCapabilities =
    plugin === undefined ? undefined : { [plugin.path]: plugin.capabilities };
  const pluginGrants =
    plugin?.grants === undefined ? undefined : { [plugin.path]: plugin.grants };
  const emit = (message: CoreToShell) => listeners.forEach((listener) => listener(message));
  const info = (): CoreToShell => ({
    type: 'trust',
    current: { projectRoot: ROOT, trusted, blocked, pluginCapabilities, pluginGrants },
    projects: [...projects],
  });
  const prompt = (): void => emit({ type: 'trustPrompt', promptId: 7, projectRoot: ROOT, blocked });
  const transport: Transport = {
    async send(message) {
      sent.push(message);
      switch (message.type) {
        case 'getTrust':
          if (!trusted) prompt();
          return info();
        case 'trustDecision':
          if (message.promptId !== 7) throw new Error('unknown prompt');
          if (message.decision === 'trust') {
            trusted = true;
            projects.push({ root: ROOT, decidedAt: '2026-11-10T12:00:00Z' });
          }
          return { type: 'ack' };
        case 'revokeTrust':
          trusted = false;
          projects.splice(0);
          return info();
        case 'runCommand':
          return { type: 'ack' };
        default:
          throw new Error(`not faked: ${message.type}`);
      }
    },
    subscribe(listener) {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
  };
  return { transport, sent, prompt, client: new IpcClient(transport) };
}
