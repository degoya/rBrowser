// The HTTP debugger's state of the network panel (RB-080-37, on Nuxt UI since RB-080-84): the
// breakpoints with their switch and the requests they hold, the composed requests and their
// results - all from the plugin's `debug` tool -, and the asks: `breakpoint_add`, `_enable`,
// `_remove`, `breakpoints_toggle`, `continue`, `abort`, `compose` and `repeat`. Every outcome
// and every refused form is said in `note`, in words.
import { ref } from 'vue';
import type { Panel } from '../../../../web-common/panel-ui/src/panel';
import { latestRefresh } from '../../../../web-common/panel-ui/src/refresh';
import { breakpointArgs, composeArgs } from './model';
import type { DebugAnswer, ListView } from './types';

export type DebugStore = ReturnType<typeof createDebug>;

export function createDebug(panel: Panel, views: () => ListView[]) {
  const answer = ref<DebugAnswer | null>(null);
  const note = ref('');
  /** The held request open in the editor. */
  const selected = ref<string | null>(null);

  // One read at a time; an action or event during a read reads again right after it, and the
  // overtaken answer is not shown (review of RB-080-84).
  const reader = latestRefresh(
    () => panel.call({ tool: 'debug' }),
    (raw) => {
      const value = raw as DebugAnswer;
      if (value && Array.isArray(value.held) && Array.isArray(value.breakpoints)) answer.value = value;
      const held = answer.value?.held ?? [];
      if (selected.value && !held.some((entry) => entry.id === selected.value)) selected.value = null;
      if (answer.value?.lastError) note.value = panel.error(answer.value.lastError);
      else if (answer.value?.pending) note.value = panel.t('breakpoints.pending', { count: answer.value.pending });
    },
    (error) => {
      note.value = panel.error(error);
    },
  );
  const refresh = (): Promise<void> => reader.refresh();

  /** Runs `tool`, then tells the outcome and refreshes. */
  function act(tool: string, args: unknown): Promise<boolean> {
    return panel.call({ tool }, args).then(
      async () => {
        note.value = panel.t('breakpoints.queued');
        await refresh();
        return true;
      },
      (error: unknown) => {
        note.value = panel.error(error);
        return false;
      },
    );
  }

  /** Adds a breakpoint from the form; true when it was sent. */
  function addBreakpoint(form: { pattern: string; regex: boolean; method: string; stage: string; views: number[] }): Promise<boolean> {
    const outcome = breakpointArgs({ ...form, views: form.views.join(',') });
    if (!outcome.ok) {
      note.value = panel.t(outcome.error);
      return Promise.resolve(false);
    }
    return act('breakpoint_add', outcome.args);
  }

  /** Lets the open held request go on (with `edits`) or aborts it, for its view or all. */
  function resolve(tool: 'continue' | 'abort', all: boolean, edits?: Record<string, unknown>): void {
    const current = (answer.value?.held ?? []).find((entry) => entry.id === selected.value);
    if (!current) return;
    const args: Record<string, unknown> = { id: current.id, allViews: all };
    if (tool === 'continue') args.edits = edits ?? {};
    selected.value = null;
    void act(tool, args);
  }

  function compose(form: { method: string; url: string; headers: string; body: string; view: string; times: string | number }): void {
    const outcome = composeArgs(form);
    if (!outcome.ok) {
      note.value = panel.t(outcome.error);
      return;
    }
    void act('compose', outcome.args);
  }

  /** Repeats captured request `id` of `view`, `times` in a row (the Requests tab's buttons). */
  function repeat(view: number, id: string, times: number): void {
    void act('repeat', { view, requestId: id, times });
  }

  /** A view's label by id. */
  const label = (id: number): string => views().find((view) => view.id === id)?.label ?? String(id);

  return {
    answer,
    note,
    selected,
    views,
    label,
    refresh,
    poll: (): Promise<void> => reader.poll(),
    addBreakpoint,
    resolve,
    compose,
    repeat,
    toggle: () => void act('breakpoints_toggle', {}),
    enable: (id: string, enabled: boolean) => void act('breakpoint_enable', { id, enabled }),
    remove: (id: string) => void act('breakpoint_remove', { id }),
  };
}
