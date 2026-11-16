// The breakpoint analyser's wizard without its view (RB-010-13): it asks the core to analyse
// (`analyzeBreakpoints`), takes the `breakpointProposal` event, keeps the person's choice and
// writes only on `apply` (`applyBreakpoints`). With a config that has breakpoints already, only
// the new ones are listed - the diff - and they are appended unless the person asks to replace.

import { computed, ref, type Ref } from 'vue';
import type { IpcClient } from '../ipc/client';
import { IpcError } from '../ipc/transport';
import type { AnalyzerHint, AnalyzerMode, BreakpointProposal, CoreToShell } from '../ipc/protocol';

export type ProposedView = BreakpointProposal['breakpoints'][number];

export interface WizardRow extends ProposedView {
  selected: boolean;
}

export interface PreviewCell {
  id: string;
  label: string;
  width: number;
  height: number;
  col: number;
  row: number;
}

export interface GridPreview {
  columns: number;
  rows: number;
  cells: PreviewCell[];
}

export type WizardPhase = 'idle' | 'running' | 'ready' | 'applying' | 'applied' | 'failed';

export interface WizardMessage {
  key: string;
  args: Record<string, string>;
}

/** The most columns the plugin's grid has (`plugins/breakpoint-analyzer/src/apply.rs`). */
export const MAX_COLUMNS = 4;

/** The kinds of static-source hints the shell has texts for (`analyzer.wizard.hints.*`). */
export const HINT_KINDS = ['dynamic', 'range', 'presets'] as const;

/** The kind of a static-source hint (RB-030-07), the last segment of the plugin's key; an
 * unknown kind reads as `dynamic`. The view shows `analyzer.wizard.hints.<kind>`. */
export function hintKind(hint: AnalyzerHint): (typeof HINT_KINDS)[number] {
  const kind = hint.key.split('.').pop() ?? '';
  return HINT_KINDS.find((known) => known === kind) ?? 'dynamic';
}

/** The rows a proposal lists: all of them for a new config, only the new ones as a diff. */
export function rowsOf(proposal: BreakpointProposal): WizardRow[] {
  return proposal.breakpoints
    .filter((view) => !proposal.existing || view.isNew)
    .map((view) => ({ ...view, selected: view.isNew }));
}

/** The grid the chosen views make, as the plugin writes it (`apply.rs`): row by row, at most
 * [`MAX_COLUMNS`] wide - or, `appended` to a config that has a grid, all in one new row below
 * it. */
export function gridPreview(views: ProposedView[], { appended = false } = {}): GridPreview {
  const columns = Math.max(1, appended ? views.length : Math.min(views.length, MAX_COLUMNS));
  const cells = views.map((view, index) => ({
    id: view.id,
    label: view.label,
    width: view.width,
    height: view.height,
    col: (index % columns) + 1,
    row: Math.floor(index / columns) + 1,
  }));
  return { columns, rows: Math.max(1, Math.ceil(views.length / columns)), cells };
}

/** Extra URLs typed one per line: trimmed, `http(s)` only, each once. */
export function parseUrls(text: string): string[] {
  const urls = text
    .split(/\s+/)
    .map((line) => line.trim())
    .filter((line) => /^https?:\/\//i.test(line));
  return [...new Set(urls)];
}

/** The i18n key a command failed with (`CommandError::Failed`, shown as `command failed: <key>`);
 * `null` for any other failure, which has no key of its own. */
export function commandFailureKey(message: string): string | null {
  const match = /^command failed: ([\w.-]+)$/.exec(message);
  return match?.[1] ?? null;
}

/** Whether a notification belongs to the analyser (its outcome or failure). */
function isAnalyzerMessage(key: string): boolean {
  return key.startsWith('analyzer.') || key.startsWith('plugins.error.') || key.startsWith('plugin.');
}

export function createWizard(client: IpcClient) {
  const phase = ref<WizardPhase>('idle');
  const proposal = ref<BreakpointProposal | null>(null);
  const rows: Ref<WizardRow[]> = ref([]);
  const replace = ref(false);
  const message = ref<WizardMessage | null>(null);

  const chosen = computed(() => rows.value.filter((row) => row.selected));
  /** Appended below the configured grid unless the person replaces it. */
  const appended = computed(() => proposal.value?.existing === true && !replace.value);
  const preview = computed(() => gridPreview(chosen.value, { appended: appended.value }));

  const stop = client.on((event: CoreToShell) => {
    if (event.type === 'breakpointProposal') {
      const { type: _type, ...next } = event;
      proposal.value = next;
      rows.value = rowsOf(next);
      replace.value = false;
      message.value = null;
      phase.value = 'ready';
    } else if (event.type === 'notification' && isAnalyzerMessage(event.messageKey)) {
      message.value = { key: event.messageKey, args: event.args ?? {} };
      if (phase.value === 'applying' && event.level === 'info') {
        phase.value = 'applied';
      } else if (event.level === 'error') {
        phase.value = 'failed';
      }
    }
  });

  async function request(next: WizardPhase, send: () => Promise<unknown>): Promise<void> {
    phase.value = next;
    message.value = null;
    try {
      await send();
    } catch (error) {
      phase.value = 'failed';
      message.value =
        error instanceof IpcError
          ? { key: 'analyzer.wizard.failed', args: { message: error.message } }
          : { key: 'analyzer.wizard.failed', args: { message: String(error) } };
    }
  }

  /** Asks for a proposal; it arrives as an event. */
  function analyze(urls: string[], mode?: AnalyzerMode): Promise<void> {
    return request('running', () =>
      client.request({ type: 'analyzeBreakpoints', urls, ...(mode ? { mode } : {}) }),
    );
  }

  /** Writes the chosen views; nothing is written before this. */
  function apply(): Promise<void> {
    const current = proposal.value;
    if (current === null || chosen.value.length === 0) {
      return Promise.resolve();
    }
    return request('applying', () =>
      client.request({
        type: 'applyBreakpoints',
        proposalId: current.proposalId,
        selected: chosen.value.map((row) => row.id),
        replace: replace.value,
      }),
    );
  }

  return { phase, proposal, rows, replace, message, chosen, appended, preview, analyze, apply, stop };
}

export type Wizard = ReturnType<typeof createWizard>;
