// The shortcut editor's conflict dialog (RB-060-11): a recorded binding another command holds
// is offered as "replace" (take it from the other command), "swap" (the other command gets this
// one's first binding in exchange) or "cancel". The result is the `setBindings` steps in an
// order the core accepts one by one: the binding is freed before it is given away.

import type { CommandInfo } from '../../ipc/protocol';

export type ConflictChoice = 'replace' | 'swap' | 'cancel';

export interface BindingStep {
  id: string;
  bindings: string[];
}

/** Whether two bindings collide: equal, or one chord starts the other (rb-commands' rule). */
export function bindingsConflict(a: string, b: string): boolean {
  const left = a.split(' ');
  const right = b.split(' ');
  const shorter = left.length <= right.length ? left : right;
  const longer = shorter === left ? right : left;
  return shorter.every((key, index) => key === longer[index]);
}

/** The steps that give `binding` to `command`, taking it from `other`. */
export function resolveConflict(
  choice: ConflictChoice,
  command: CommandInfo,
  other: CommandInfo,
  binding: string,
): BindingStep[] {
  if (choice === 'cancel') {
    return [];
  }
  const freed = other.bindings.filter((held) => !bindingsConflict(held, binding));
  const [old, ...rest] = command.bindings;
  if (choice === 'swap' && old !== undefined) {
    return [
      { id: other.id, bindings: freed },
      { id: command.id, bindings: [binding, ...rest] },
      { id: other.id, bindings: [...freed, old] },
    ];
  }
  return [
    { id: other.id, bindings: freed },
    { id: command.id, bindings: [...command.bindings, binding] },
  ];
}
