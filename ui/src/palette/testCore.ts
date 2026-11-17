// A stand-in core for the palette tests: the mock core's state and answers, plus the registry's
// commands (`listCommands`), the settings with their schema, the project's flows (`listFlows`)
// and the device presets (`getDevices`). Test-only.

import { IpcClient } from '../ipc/client';
import { createMockTransport } from '../ipc/mockTransport';
import type {
  CommandInfo,
  CoreToShell,
  DevicePreset,
  FlowSummary,
  JsonValue,
  SettingValue,
  ShellToCore,
} from '../ipc/protocol';
import type { Transport } from '../ipc/transport';
import type { SettingDef } from '../settings/schema';

function command(id: string, group: CommandInfo['group'], bindings: string[]): CommandInfo {
  return {
    id,
    titleKey: `commands.${id}`,
    group,
    bindings,
    defaultBindings: bindings,
    changed: false,
    mcp: { omitted: 'test' },
  };
}

export const PALETTE_COMMANDS: CommandInfo[] = [
  command('sync.toggle', 'sync', ['Ctrl+Shift+Space S S']),
  command('reload.all', 'reload', ['Ctrl+Shift+Space R R']),
  command('view.zoom_fit_all', 'view', ['Ctrl+Shift+Space V F']),
  command('app.command_palette', 'view', ['Ctrl+Shift+Space V P']),
];

const SCROLL: SettingDef = {
  key: 'sync.scroll',
  category: 'sync',
  section: 'scroll',
  order: 1,
  scopeAllowed: ['user', 'project'],
  localOnly: false,
  protectedFromMcp: false,
  labelKey: 'settings.key.sync.scroll.label',
  descriptionKey: 'settings.key.sync.scroll.description',
  helpKey: 'settings.key.sync.scroll.help',
  schema: { type: 'boolean' },
};

export const PALETTE_FLOWS: FlowSummary[] = [{ name: 'login', description: 'Log in as the test user' }, { name: 'cart' }];

export const PALETTE_DEVICES: DevicePreset[] = [
  { id: 'iphone-15', kind: 'mobile', viewport: { width: 393, height: 852 }, dpr: 3, touch: true },
];

export interface PaletteCore {
  client: IpcClient;
  transport: Transport;
  sent: ShellToCore[];
  commands: CommandInfo[];
  values: SettingValue[];
  emit(message: CoreToShell): void;
}

/** The mock core with `listCommands`, `getSettings { includeSchema }`, `listFlows` and `getDevices` answered. */
export function createPaletteCore(): PaletteCore {
  const mock = createMockTransport();
  const commands = structuredClone(PALETTE_COMMANDS);
  const values: SettingValue[] = [{ key: 'ui.theme', value: 'light', origin: 'user' }];
  const transport: Transport = {
    send(message) {
      switch (message.type) {
        case 'listCommands':
          mock.sent.push(message);
          return Promise.resolve({ type: 'commands', leader: 'Ctrl+Shift+Space', commands: structuredClone(commands) });
        case 'getSettings':
          mock.sent.push(message);
          return Promise.resolve({
            type: 'settings',
            values: structuredClone(values),
            ...(message.includeSchema
              ? { schema: { version: 1, categories: [], defs: [SCROLL], project: null } as unknown as JsonValue }
              : {}),
          });
        case 'listFlows':
          mock.sent.push(message);
          return Promise.resolve({ type: 'flows', items: structuredClone(PALETTE_FLOWS) });
        case 'getDevices':
          mock.sent.push(message);
          return Promise.resolve({ type: 'devices', items: structuredClone(PALETTE_DEVICES) });
        case 'setBreakpoint':
        case 'runCommand':
          mock.sent.push(message);
          return Promise.resolve({ type: 'ack' });
        default:
          return mock.send(message);
      }
    },
    subscribe: (listener) => mock.subscribe(listener),
  };
  return { client: new IpcClient(transport), transport, sent: mock.sent, commands, values, emit: mock.emit };
}
