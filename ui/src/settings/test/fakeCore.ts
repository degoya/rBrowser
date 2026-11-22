// A core for the settings tests: the schema payload built from the real generated schemas
// (`schema/*.v1.json`, walked like `rb_settings::defs_from_schema`), values with origins, and
// the answers of the settings and shortcut messages. Test-only; nothing of the app imports it.

import categorySource from '../../../../crates/rb-settings/src/category.rs?raw';
import configSchema from '../../../../schema/config.v1.json';
import userSchema from '../../../../schema/settings.v1.json';
import { shellMessages } from '../../i18n';
import { IpcClient } from '../../ipc/client';
import type {
  CommandInfo,
  CoreToShell,
  JsonValue,
  Origin,
  ProjectShortcut,
  SettingValue,
  ShellToCore,
} from '../../ipc/protocol';
import { IpcError, type Transport } from '../../ipc/transport';
import type { InstalledMessage } from '../../panels/protocol';
import type { JsonSchema, SettingDef } from '../schema';
import { isObject } from '../../../../plugins/web-common/src/is-object';

type Json = { [key: string]: JsonValue };


function walk(name: string, prop: Json, out: SettingDef[]): void {
  if ('x-category' in prop) {
    out.push({
      key: name,
      category: String(prop['x-category']),
      section: String(prop['x-section']),
      order: Number(prop['x-order']),
      scopeAllowed: prop['x-scope'] as SettingDef['scopeAllowed'],
      localOnly: prop['x-local-only'] === true,
      protectedFromMcp: prop['x-protected'] === true,
      labelKey: `settings.key.${name}.label`,
      descriptionKey: `settings.key.${name}.description`,
      helpKey: `settings.key.${name}.help`,
      schema: prop as JsonSchema,
    });
    return;
  }
  const properties = prop.properties;
  if (isObject(properties)) {
    for (const [child, value] of Object.entries(properties)) {
      if (isObject(value)) {
        walk(`${name}.${child}`, value, out);
      }
    }
  }
}

/** Every definition of the two generated schemas. */
export function realDefs(): SettingDef[] {
  const out: SettingDef[] = [];
  for (const schema of [configSchema, userSchema] as unknown as Json[]) {
    const properties = schema.properties;
    if (isObject(properties)) {
      for (const [name, prop] of Object.entries(properties)) {
        if (name !== '$schema' && name !== 'version' && isObject(prop)) {
          walk(name, prop, out);
        }
      }
    }
  }
  return out;
}

/** The display order of the sections of each category, as the core sends it: the table
 * `SECTIONS` of `rb_settings::category` (the sub-tabs of RB-080-13 follow it). */
function sectionOrder(): Map<string, string[]> {
  const ids = new Map([...categorySource.matchAll(/Category::(\w+) => "([a-z-]+)"/g)].map((m) => [m[1], m[2]]));
  const order = new Map<string, string[]>();
  for (const [, variant, section] of categorySource.matchAll(/s\(Category::(\w+), "([a-z0-9-]+)"\)/g)) {
    const id = ids.get(variant ?? '');
    if (id !== undefined && section !== undefined) {
      order.set(id, [...(order.get(id) ?? []), section]);
    }
  }
  return order;
}

/** The categories in the catalogue's order, each with its sections in the core's order. */
function categories(): { id: string; sections: string[] }[] {
  // The catalogue with the wave parts not merged yet, as the window shows it.
  const settings = (shellMessages() as unknown as Json).settings as Json;
  const sections = settings.section as Json;
  const order = sectionOrder();
  return Object.keys(settings.category as Json).map((id) => {
    const known = order.get(id) ?? [];
    const rank = (section: string): number => (known.includes(section) ? known.indexOf(section) : known.length);
    const listed = Object.keys((sections[id] as Json | undefined) ?? {});
    return { id, sections: listed.sort((a, b) => rank(a) - rank(b)) };
  });
}

export interface FakeCore {
  client: IpcClient;
  sent: ShellToCore[];
  values: Map<string, SettingValue>;
  /** The answer to `getSettingSources`, per source (RB-080-72); change it, then emit. */
  sources: Record<string, JsonValue>;
  commands: CommandInfo[];
  emit(message: CoreToShell): void;
  /** Adds definitions to the schema the core answers from now on - a plugin loaded (RB-094-26). */
  addDefs(more: SettingDef[]): void;
}

/** The API client's `secrets`, a key annotated `x-secret` (RB-094-26). */
export const SECRETS: SettingDef = {
  key: 'api-client.secrets',
  category: 'network',
  section: 'api-client',
  order: 2,
  scopeAllowed: ['local'],
  localOnly: true,
  protectedFromMcp: true,
  labelKey: 'plugin.api-client.settings.secrets.label',
  descriptionKey: 'plugin.api-client.settings.secrets.description',
  helpKey: 'plugin.api-client.settings.secrets.help',
  schema: { type: 'object', default: {}, 'x-secret': true },
  plugin: 'api-client',
};

/** The API client's `benchRepeats`, as the core sends a plugin key once its plugin is loaded. */
export const BENCH_REPEATS: SettingDef = {
  key: 'api-client.benchRepeats',
  category: 'network',
  section: 'api-client',
  order: 5,
  scopeAllowed: ['user', 'project', 'local'],
  localOnly: false,
  protectedFromMcp: false,
  labelKey: 'plugin.api-client.settings.benchRepeats.label',
  descriptionKey: 'plugin.api-client.settings.benchRepeats.description',
  helpKey: 'plugin.api-client.settings.benchRepeats.help',
  schema: { type: 'integer', minimum: 1, maximum: 100, default: 10 },
  plugin: 'api-client',
};

export const SAMPLE_COMMANDS: CommandInfo[] = [
  {
    id: 'sync.toggle',
    titleKey: 'commands.sync.toggle',
    group: 'sync',
    bindings: ['Ctrl+Shift+Space S S'],
    defaultBindings: ['Ctrl+Shift+Space S S'],
    changed: false,
    mcp: { covered: 'sync_set' },
  },
  {
    id: 'reload.all',
    titleKey: 'commands.reload.all',
    group: 'reload',
    bindings: ['Ctrl+Shift+Space R R'],
    defaultBindings: ['Ctrl+Shift+Space R R'],
    changed: false,
    mcp: { covered: 'reload' },
  },
  {
    id: 'view.set_breakpoint.2',
    titleKey: 'commands.view.set_breakpoint.2',
    group: 'breakpoints',
    bindings: ['Ctrl+Shift+2', 'Ctrl+Shift+Space B 2'],
    defaultBindings: ['Ctrl+Shift+2', 'Ctrl+Shift+Space B 2'],
    changed: false,
    mcp: { covered: 'set_breakpoint' },
  },
  // A third-party plugin whose default `Ctrl+W` is reserved (RB-060-11, `droppedDefaults`).
  {
    id: 'plugin.acme.close_all',
    // Its title lives in the plugin's own catalogue; the core sends it with the list (RB-080-82).
    titleKey: 'plugin.acme.commands.close_all',
    group: 'plugins',
    bindings: ['Ctrl+Shift+Space X A'],
    defaultBindings: ['Ctrl+Shift+Space X A'],
    changed: false,
    mcp: { omitted: 'test' },
    droppedDefaults: ['Ctrl+W'],
  } as CommandInfo,
];

/** The texts of the plugin commands, as the core sends them beside the list (RB-080-82). */
export const SAMPLE_PLUGIN_TEXTS: Record<string, string> = {
  'plugin.acme.commands.close_all': 'Acme: Close every tab',
};

export interface FakeCoreOptions {
  project?: boolean;
  origins?: Record<string, Origin>;
  /** The answer to `listProjectShortcuts` (RB-010-16). */
  projectShortcuts?: ProjectShortcut[];
  /** The answer to `pluginPanel { op: 'installed' }` (Settings > Plugins, RB-080-51). */
  installed?: InstalledMessage;
}

export function createFakeCore(options: FakeCoreOptions = {}): FakeCore {
  const defs = realDefs();
  const listeners = new Set<(message: CoreToShell) => void>();
  const sent: ShellToCore[] = [];
  const values = new Map<string, SettingValue>(
    defs.map((def) => [
      def.key,
      { key: def.key, value: def.schema.default ?? null, origin: options.origins?.[def.key] ?? 'default' },
    ]),
  );
  const commands = structuredClone(SAMPLE_COMMANDS);
  const sources: Record<string, JsonValue> = {};
  const emit = (message: CoreToShell): void => listeners.forEach((listener) => listener(message));
  const list = (): CoreToShell => ({
    type: 'commands',
    leader: 'Ctrl+Shift+Space',
    commands: structuredClone(commands),
    texts: { ...SAMPLE_PLUGIN_TEXTS },
  });

  const handle = (message: ShellToCore): CoreToShell => {
    switch (message.type) {
      case 'getSettings':
        return {
          type: 'settings',
          values: [...values.values()],
          ...(message.includeSchema
            ? {
                schema: {
                  version: 1,
                  categories: categories(),
                  defs: defs as unknown as JsonValue,
                  project: options.project === false ? null : { root: '/work/site', hasLocal: false },
                } as JsonValue,
              }
            : {}),
        };
      case 'setSetting': {
        const def = defs.find((d) => d.key === message.key);
        if (def === undefined) {
          return { type: 'settingRejected', key: message.key, issues: [{ path: '', messageKey: 'settings.rejected.unknown_key' }] };
        }
        if (def.localOnly && message.scope !== 'local') {
          return { type: 'settingRejected', key: message.key, issues: [{ path: '', messageKey: 'settings.rejected.local_only' }] };
        }
        if (typeof message.value === 'number' && message.value < 0) {
          return { type: 'settingRejected', key: message.key, issues: [{ path: '', messageKey: 'settings.rejected.minimum' }] };
        }
        values.set(message.key, {
          key: message.key,
          value: message.value ?? def.schema.default ?? null,
          origin: message.value === null ? 'default' : message.scope,
        });
        return { type: 'ack' };
      }
      case 'runCommand':
        return { type: 'ack' };
      case 'openSettings':
        return { type: 'ack' };
      case 'revealDiagnostics':
        return { type: 'ack' };
      case 'getSettingSources':
        return { type: 'settingSources', sources: structuredClone(sources) };
      case 'pickPath':
        setTimeout(() => emit({ type: 'pickedPath', path: 'C:\\node\\node.exe' }), 0);
        return { type: 'ack' };
      case 'listCommands':
        return list();
      case 'pluginPanel':
        return { type: 'pluginPanel', message: options.installed ?? { loading: false, plugins: [], folders: {} } };
      case 'listProjectShortcuts':
        return { type: 'projectShortcuts', items: structuredClone(options.projectShortcuts ?? []) };
      case 'checkBinding':
        if (message.binding === 'Ctrl+T') {
          return { type: 'bindingCheck', ok: false, reasonKey: 'shortcuts.reserved.browser_new_tab' };
        }
        {
          const holder = commands.find((c) => c.id !== message.id && c.bindings.includes(message.binding));
          if (holder !== undefined) {
            return { type: 'bindingCheck', ok: false, reasonKey: 'shortcuts.rejected.conflict', conflict: holder.id };
          }
        }
        return { type: 'bindingCheck', ok: true };
      case 'setBindings': {
        const command = commands.find((c) => c.id === message.id);
        if (command !== undefined) {
          command.bindings = message.bindings;
          command.changed = JSON.stringify(message.bindings) !== JSON.stringify(command.defaultBindings);
        }
        return list();
      }
      case 'resetBinding':
        for (const command of commands) {
          if (message.id === undefined || command.id === message.id) {
            command.bindings = [...command.defaultBindings];
            command.changed = false;
          }
        }
        return list();
      case 'exportBindings':
        return {
          type: 'bindingsExport',
          json: JSON.stringify({
            version: 1,
            bindings: Object.fromEntries(commands.filter((c) => c.changed).map((c) => [c.id, c.bindings])),
          }),
        };
      case 'importBindings': {
        const file = JSON.parse(message.json) as { bindings: Record<string, string[]> };
        for (const command of commands) {
          command.bindings = file.bindings[command.id] ?? [...command.defaultBindings];
          command.changed = JSON.stringify(command.bindings) !== JSON.stringify(command.defaultBindings);
        }
        return list();
      }
      default:
        throw new IpcError(501, `the fake core does not do ${message.type}`);
    }
  };

  const transport: Transport = {
    send(message) {
      sent.push(message);
      return Promise.resolve(handle(message));
    },
    subscribe(listener) {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
  };
  const addDefs = (more: SettingDef[]): void => {
    for (const def of more) {
      defs.push(def);
      values.set(def.key, { key: def.key, value: def.schema.default ?? null, origin: 'default' });
    }
  };
  return { client: new IpcClient(transport), sent, values, sources, commands, emit, addDefs };
}
