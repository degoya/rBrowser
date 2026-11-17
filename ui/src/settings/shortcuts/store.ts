// The shortcut editor's state over the core's registry (RB-010-17's messages): the commands
// with their bindings, the check of a recorded binding (reserved, conflict), set, remove, reset
// (one command, a group, all), the steps of a resolved conflict (RB-060-11), export and import.
// The core saves every change in the user settings.

import { computed, ref, shallowRef } from 'vue';
import type { IpcClient } from '../../ipc/client';
import type { BindingCheck, CommandInfo } from '../../ipc/protocol';
import { IpcError } from '../../ipc/transport';
import type { BindingStep } from './conflict';

export type ShortcutsStore = ReturnType<typeof createShortcutsStore>;

export function createShortcutsStore(client: IpcClient) {
  const commands = shallowRef<CommandInfo[]>([]);
  const leader = ref('');
  const error = ref<string | null>(null);

  const take = (answer: { leader: string; commands: CommandInfo[] }): void => {
    leader.value = answer.leader;
    commands.value = answer.commands;
    error.value = null;
  };

  async function run(task: () => Promise<void>): Promise<boolean> {
    try {
      await task();
      return true;
    } catch (failure) {
      error.value = failure instanceof IpcError ? failure.message : String(failure);
      return false;
    }
  }

  const load = (): Promise<boolean> =>
    run(async () => take(await client.expect({ type: 'listCommands' }, 'commands')));

  /** Whether `binding` may be bound to `id`, and why not. */
  async function check(id: string, binding: string): Promise<BindingCheck> {
    const answer = await client.expect({ type: 'checkBinding', id, binding }, 'bindingCheck');
    const { type: _type, ...result } = answer;
    return result;
  }

  const setBindings = (id: string, bindings: string[]): Promise<boolean> =>
    run(async () => take(await client.expect({ type: 'setBindings', id, bindings }, 'commands')));

  /** Adds `binding` to `id`'s bindings. */
  function add(command: CommandInfo, binding: string): Promise<boolean> {
    if (command.bindings.includes(binding)) {
      return Promise.resolve(true);
    }
    return setBindings(command.id, [...command.bindings, binding]);
  }

  function remove(command: CommandInfo, binding: string): Promise<boolean> {
    return setBindings(
      command.id,
      command.bindings.filter((b) => b !== binding),
    );
  }

  /** The defaults of one command, or of every command without `id`. */
  const reset = (id?: string): Promise<boolean> =>
    run(async () =>
      take(await client.expect({ type: 'resetBinding', ...(id === undefined ? {} : { id }) }, 'commands')),
    );

  async function exportJson(): Promise<string | null> {
    try {
      return (await client.expect({ type: 'exportBindings' }, 'bindingsExport')).json;
    } catch (failure) {
      error.value = failure instanceof Error ? failure.message : String(failure);
      return null;
    }
  }

  const importJson = (json: string): Promise<boolean> =>
    run(async () => take(await client.expect({ type: 'importBindings', json }, 'commands')));

  /** Sends `steps` in order (a resolved conflict); stops at the first the core refuses. */
  const apply = (steps: BindingStep[]): Promise<boolean> =>
    run(async () => {
      for (const step of steps) {
        take(await client.expect({ type: 'setBindings', id: step.id, bindings: step.bindings }, 'commands'));
      }
    });

  /** The defaults of the changed commands among `ids` (a group's "reset"). */
  const resetMany = (ids: string[]): Promise<boolean> =>
    run(async () => {
      for (const id of ids) {
        take(await client.expect({ type: 'resetBinding', id }, 'commands'));
      }
    });

  const changedCount = computed(() => commands.value.filter((command) => command.changed).length);

  return {
    commands,
    leader,
    error,
    changedCount,
    load,
    check,
    add,
    remove,
    reset,
    exportJson,
    importJson,
    apply,
    resetMany,
  };
}
