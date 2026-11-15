// The snippets panel's logic without the DOM (RB-070-04, on Nuxt UI since RB-080-84): the list's
// filter, the arguments of a run, results of several views joined, keyboard moves, and a failure
// in words - the text of a code, the page's own error as it came.
import { errorText, type Params, type Strings } from '../../../../web-common/panel-ui/src/format';

export interface ViewEntry {
  id: number;
  label: string;
  breakpoint: string;
  engine: string;
}

export type Source = 'user' | 'project' | 'local';

export interface SnippetEntry {
  source: Source;
  name: string;
  description: string;
  world: 'isolated' | 'main';
  target: 'all' | 'active';
  timeoutMs: number;
  file?: string;
  code?: string;
  codeOmitted?: boolean;
  error?: { code: string; params: Record<string, string> };
}

export type Availability = 'loaded' | 'untrusted' | 'disabled' | 'noProject';

export interface ListAnswer {
  snippets: SnippetEntry[];
  project: Availability;
  local: Availability;
  views: ViewEntry[];
  active: number | null;
}

export interface RunResult {
  ok: boolean;
  type: string;
  text: string;
  views: number[];
  value?: unknown;
  stack?: string;
  timedOut?: boolean;
  truncated?: boolean;
}

export interface RunAnswer {
  views: number[];
  results: RunResult[];
  snippet?: { source: string; name: string };
  world?: string;
  timeoutMs?: number;
}

export interface RunRecord {
  id: number;
  title: string;
  results: RunResult[];
}

/** The prefix of every text of the plugin. */
export const PREFIX = 'plugin.snippets.';

/** The sources in the list's order. */
export const SOURCES: readonly Source[] = ['user', 'project', 'local'];

/** The snippets whose name or description holds every word of `query`, in source order. */
export function filter(snippets: SnippetEntry[], query: string): SnippetEntry[] {
  const words = query.toLowerCase().split(/\s+/).filter(Boolean);
  const order = (entry: SnippetEntry): number => SOURCES.indexOf(entry.source);
  return snippets
    .filter((entry) => {
      const hay = (entry.name + ' ' + entry.description).toLowerCase();
      return words.every((word) => hay.includes(word));
    })
    .sort((a, b) => order(a) - order(b));
}

/** The views a run goes to: every view, or the active one. */
export function targetViews(target: 'all' | 'active', views: ViewEntry[], active: number | null): number[] {
  if (target === 'active') return active === null ? [] : [active];
  return views.map((view) => view.id);
}

/** The arguments of `run` in one view: the stored snippet by name when the editor holds it
 * unchanged (so `run_last` repeats it, in every view of the run), else the editor's code. */
export function runArgs(selected: SnippetEntry | null, code: string, view: number, all: number[]): Record<string, unknown> {
  if (selected && selected.code === code) return { name: selected.name, source: selected.source, target: view, remember: all };
  return { code, target: view };
}

/** The answers of one run, view by view, as one list: equal outcomes are one result with their
 * views, in the order the views answered; a view whose call failed is a result too. */
export function join(answers: Array<{ view: number; answer?: RunAnswer; error?: string }>): RunResult[] {
  const results: RunResult[] = [];
  const add = (outcome: RunResult, views: number[]): void => {
    const same = results.find((r) => r.ok === outcome.ok && r.type === outcome.type && r.text === outcome.text);
    if (same) same.views.push(...views);
    else results.push({ ...outcome, views: [...views] });
  };
  for (const { view, answer, error } of answers) {
    if (error !== undefined) add({ ok: false, type: 'failure', text: error, views: [] }, [view]);
    for (const result of answer?.results ?? []) add(result, result.views);
  }
  return results;
}

/** The row a key moves the focus to in a list of `length` rows, or `null` for another key. */
export function moveFocus(index: number, key: string, length: number): number | null {
  if (length === 0) return null;
  switch (key) {
    case 'ArrowDown':
      return Math.min(index + 1, length - 1);
    case 'ArrowUp':
      return Math.max(index - 1, 0);
    case 'Home':
      return 0;
    case 'End':
      return length - 1;
    default:
      return null;
  }
}

/** A code of rBrowser or a plugin (`plugins.error.busy`, `plugin.snippets.errors.budget x=1`). */
const CODE = /^plugins?\.[a-z0-9_.-]+(?: |$)/;

/** A failure as the person reads it: a code in its sentence (never the bare code), anything
 * else - the page's own error - as it came. */
export function failureText(strings: Strings, text: string, params?: Params): string {
  return CODE.test(text) ? errorText(strings, text, params) : text;
}
