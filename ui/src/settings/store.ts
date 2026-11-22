// The settings window's state: the schema (asked for once), every key's value and origin, the
// reasons a write was refused per key. Writes go to the core (`setSetting`), which checks and
// saves them; the values are read again after every write and taken from `settingsChanged`
// (another window, a file change; read again when it carries none, RB-090-36). The values of the `x-options` sources (RB-080-72: plugins, rules,
// breakpoints) come apart from the schema and are read again on `settingsChanged` and
// `settingSourcesChanged`, so a pick list follows the project while the window is open.
// The last refusal is kept apart for the notice at the top of the window (RB-094-25): the
// reasons under a field may lie below a long list, out of sight.

import { ref, shallowRef } from 'vue';
import type { IpcClient } from '../ipc/client';
import type {
  CoreToShell,
  JsonValue,
  PathKind,
  Scope,
  SettingIssue,
  SettingValue,
} from '../ipc/protocol';
import { isObject } from '../../../plugins/web-common/src/is-object';
import { changedValues } from '../state/settingsChanged';
import { type DynamicData, parseSchema, type SettingDef, type SettingsSchema } from './schema';

/** The reason shown when the core did not answer a write. */
export const WRITE_FAILED_KEY = 'settings.rejected.failed';

export type SettingsStore = ReturnType<typeof createSettingsStore>;

/** A write the core refused: the key and its reasons. */
export interface Refusal {
  key: string;
  issues: SettingIssue[];
}

export function createSettingsStore(client: IpcClient) {
  const schema = shallowRef<SettingsSchema | null>(null);
  const values = shallowRef<ReadonlyMap<string, SettingValue>>(new Map());
  const issues = ref<Record<string, SettingIssue[]>>({});
  const loadError = ref<string | null>(null);
  /** The last write refused, for the notice at the top; `null` once the key is saved or the
   * notice closed. */
  const refusal = shallowRef<Refusal | null>(null);
  /** Per source its values; a source not listed has none known (free entry). */
  const sources = shallowRef<Readonly<Record<string, DynamicData>>>({});
  const stops: (() => void)[] = [];

  const take = (list: SettingValue[]): void => {
    values.value = new Map(list.map((value) => [value.key, value]));
  };

  async function refreshSources(): Promise<void> {
    const answer = await client.expect({ type: 'getSettingSources' }, 'settingSources');
    const read: Record<string, DynamicData> = {};
    for (const [source, data] of Object.entries(answer.sources)) {
      if (isObject(data) && Array.isArray(data.options)) {
        read[source] = data as unknown as DynamicData;
      }
    }
    sources.value = read;
  }

  async function refresh(): Promise<void> {
    const answer = await client.expect({ type: 'getSettings' }, 'settings');
    take(answer.values);
  }

  /** Reads the schema again with the values: the plugins in effect changed, and their keys with
   * them (RB-094-26). A failure keeps the schema shown. */
  async function reloadSchema(): Promise<void> {
    const answer = await client.expect({ type: 'getSettings', includeSchema: true }, 'settings');
    schema.value = parseSchema(answer.schema);
    take(answer.values);
  }

  async function start(): Promise<void> {
    stops.push(
      client.on((message: CoreToShell) => {
        if (message.type === 'settingsChanged') {
          // The new values come with the event (RB-090-36); without them they are asked for.
          const changed = changedValues(message);
          if (changed) {
            values.value = new Map([...values.value, ...changed]);
          } else {
            void refresh().catch(() => undefined);
          }
          void refreshSources().catch(() => undefined);
        } else if (message.type === 'settingSourcesChanged') {
          void refreshSources().catch(() => undefined);
        }
      }),
    );
    try {
      const answer = await client.expect({ type: 'getSettings', includeSchema: true }, 'settings');
      schema.value = parseSchema(answer.schema);
      take(answer.values);
      loadError.value = null;
      void refreshSources().catch(() => undefined);
    } catch (error) {
      loadError.value = error instanceof Error ? error.message : String(error);
    }
  }

  function stop(): void {
    stops.splice(0).forEach((fn) => fn());
  }

  function setIssues(key: string, list: SettingIssue[] | null): void {
    const next = { ...issues.value };
    if (list === null) {
      delete next[key];
      if (refusal.value?.key === key) refusal.value = null;
    } else {
      next[key] = list;
      refusal.value = { key, issues: list };
    }
    issues.value = next;
  }

  /** Closes the notice at the top; the reasons stay at the field. */
  function dismissRefusal(): void {
    refusal.value = null;
  }

  /** Writes `value` (`null`: reset) to `scope`; `false` with the reasons at the key when refused. */
  async function set(key: string, value: JsonValue, scope: Scope): Promise<boolean> {
    let answer: CoreToShell;
    try {
      answer = await client.request({ type: 'setSetting', key, value, scope });
    } catch {
      setIssues(key, [{ path: '', messageKey: WRITE_FAILED_KEY }]);
      return false;
    }
    if (answer.type === 'settingRejected') {
      setIssues(key, answer.issues);
      return false;
    }
    setIssues(key, null);
    await refresh().catch(() => undefined);
    return true;
  }

  /** Removes the value of `def` from the scope it comes from; a default stays as it is. */
  async function reset(def: SettingDef): Promise<boolean> {
    const current = values.value.get(def.key);
    if (current === undefined || current.origin === 'default') {
      return true;
    }
    return set(def.key, null, current.origin);
  }

  /** Resets every key of a section; `true` when all went through. */
  async function resetAll(defs: SettingDef[]): Promise<boolean> {
    let ok = true;
    for (const def of defs) {
      ok = (await reset(def)) && ok;
    }
    return ok;
  }

  /** Lets the person choose a path in the native dialog; `null` when cancelled. The choice
   * comes as the event `pickedPath` after the `ack` (or, should the core answer at once, as the
   * answer). */
  async function pickPath(kind: PathKind, titleKey?: string): Promise<string | null> {
    let stopListening: () => void = () => undefined;
    const picked = new Promise<string | null>((resolve) => {
      stopListening = client.on((message) => {
        if (message.type === 'pickedPath') {
          resolve(message.path ?? null);
        }
      });
    });
    try {
      const answer = await client.request({
        type: 'pickPath',
        kind,
        ...(titleKey === undefined ? {} : { titleKey }),
      });
      if (answer.type === 'pickedPath') {
        return answer.path ?? null;
      }
      return await picked;
    } catch {
      return null;
    } finally {
      stopListening();
    }
  }

  /** `def` with the values of the source its `x-options` names, unless the core sent its own. */
  function withSource(def: SettingDef): SettingDef {
    const source = def.schema['x-options'];
    if (def.dynamic !== undefined || typeof source !== 'string') {
      return def;
    }
    const data = sources.value[source];
    return data === undefined ? def : { ...def, dynamic: data };
  }

  return {
    schema,
    values,
    sources,
    issues,
    refusal,
    dismissRefusal,
    loadError,
    start,
    stop,
    refresh,
    refreshSources,
    reloadSchema,
    withSource,
    set,
    reset,
    resetAll,
    pickPath,
  };
}
