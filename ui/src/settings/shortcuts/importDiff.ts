// The shortcut editor's import preview (RB-060-11): the pasted file is checked against the
// export format of rb-commands (`ShortcutExport`: `{ version: 1, bindings: { <id>: [<binding>] } }`)
// before anything is sent, and the editor shows what would change - an import replaces all of
// the user's bindings, so a command the file omits goes back to its defaults - the commands the
// file names that do not exist, and the bindings two commands would hold. The core checks the
// file again when it is applied and skips what it refuses.

import type { CommandInfo } from '../../ipc/protocol';
import { bindingsConflict } from './conflict';
import { isObject as isRecord } from '../../../../plugins/web-common/src/is-object';

/** `rb_commands::user_bindings::EXPORT_VERSION`. */
export const EXPORT_VERSION = 1;

export type ParsedFile =
  | { ok: true; bindings: Record<string, string[]> }
  | { ok: false; errorKey: string };


function isBindingList(value: unknown): value is string[] {
  return Array.isArray(value) && value.every((item) => typeof item === 'string' && item.trim() !== '');
}

export function parseShortcutFile(json: string): ParsedFile {
  let parsed: unknown;
  try {
    parsed = JSON.parse(json);
  } catch {
    return { ok: false, errorKey: 'settings.shortcuts.import_error.json' };
  }
  if (isRecord(parsed) && parsed.version !== EXPORT_VERSION) {
    return { ok: false, errorKey: 'settings.shortcuts.import_error.version' };
  }
  const bindings = isRecord(parsed) ? parsed.bindings : undefined;
  if (
    !isRecord(bindings) ||
    Object.entries(bindings).some(([id, list]) => id.trim() === '' || !isBindingList(list))
  ) {
    return { ok: false, errorKey: 'settings.shortcuts.import_error.shape' };
  }
  return { ok: true, bindings: bindings as Record<string, string[]> };
}

export interface ImportChange {
  id: string;
  before: string[];
  after: string[];
}

export interface ImportConflict {
  binding: string;
  /** The command earlier in the registry's order. */
  first: string;
  second: string;
}

export interface ImportDiff {
  changes: ImportChange[];
  unknown: string[];
  conflicts: ImportConflict[];
}

const same = (a: string[], b: string[]): boolean => a.length === b.length && a.every((x, i) => x === b[i]);

export function diffImport(commands: readonly CommandInfo[], bindings: Record<string, string[]>): ImportDiff {
  const known = new Set(commands.map((c) => c.id));
  const after = commands.map((command) => ({
    id: command.id,
    before: command.bindings,
    after: bindings[command.id] ?? command.defaultBindings,
  }));
  const conflicts: ImportConflict[] = [];
  after.forEach((first, index) => {
    for (const second of after.slice(index + 1)) {
      for (const binding of second.after) {
        if (first.after.some((held) => bindingsConflict(held, binding))) {
          conflicts.push({ binding, first: first.id, second: second.id });
        }
      }
    }
  });
  return {
    changes: after.filter((row) => !same(row.before, row.after)),
    unknown: Object.keys(bindings).filter((id) => !known.has(id)),
    conflicts,
  };
}
