// Requests of the list taken into the API client (RB-096-08): Open in API client and Save as
// request for one request, Save shown as requests for what the filters show. The panel names the
// request - its newest instance, view and id - and the plugin's tools `api_open` and `api_save`
// have the core do the rest: it reads the headers and the body, makes every secret a variable
// and writes the files. The panel never sees a header's value; the answers name the variables
// and whether each has a value, a body that could not be taken (`bodyMissing`) and values that
// could not be kept (`warning`).
import type { Panel } from '../../../../web-common/panel-ui/src/panel';
import type { StatusLine } from '../../../../web-common/panel-ui/src/status';
import type { ListRow } from './types';

/** The most requests one save names (the plugin's and the core's limit). */
export const MAX_SAVED = 200;

export interface CaptureRef {
  view: number;
  id: string;
}

export interface CapturedVariable {
  name: string;
  /** `kept` into the private values now, `replaced` a different value kept there before, `present` there before, `missing` without a value. */
  value: 'kept' | 'replaced' | 'present' | 'missing';
}

export interface OpenAnswer {
  request: { name: string };
  folder: string;
  variables: CapturedVariable[];
  /** Why the body is not in the request: `tooLong`, `unchecked`, `unread`. */
  bodyMissing?: string | null;
  /** `secrets_not_saved`: the private values could not be written. */
  warning?: string;
}

export interface SaveAnswer {
  saved: { id: string; name: string; bodyMissing?: string | null }[];
  skipped: { name: string | null; reason: string }[];
  variables: CapturedVariable[];
  warning?: string;
}

/** What a save left out: `cutAt` the limit when the shown list was longer, `partial` when the list shows only part of the capture. */
export interface SaveLimits {
  cutAt: number;
  partial: boolean;
}

type Say = (tone: StatusLine['tone'], text: string, setting?: StatusLine['setting']) => void;
type T = (key: string, params?: Record<string, unknown>) => string;

/** The newest instance of `row` any view saw; none without a request id. */
export function newestOf(row: ListRow): CaptureRef | null {
  let best: CaptureRef | null = null;
  let at = -Infinity;
  for (const [view, seen] of Object.entries(row.views)) {
    if (!seen?.id) continue;
    const time = seen.time ?? 0;
    if (best === null || time > at) {
      best = { view: Number(view), id: seen.id };
      at = time;
    }
  }
  return best;
}

/** The requests Save shown sends for `rows` (in the list's order), and whether the limit cut it. */
export function toSave(rows: readonly ListRow[]): { requests: CaptureRef[]; cut: boolean } {
  const all = rows.map(newestOf).filter((one): one is CaptureRef => one !== null);
  return { requests: all.slice(0, MAX_SAVED), cut: all.length > MAX_SAVED };
}

/** The names of the variables still without a value. */
export function missingOf(variables: readonly CapturedVariable[]): string[] {
  return variables.filter((variable) => variable.value === 'missing').map((variable) => variable.name);
}

/** The warnings an answer carries: variables without a value or with a new one, bodies left out, values not kept. */
function warningsOf(variables: readonly CapturedVariable[], bodiesMissing: number, warning: string | undefined, t: T): string[] {
  const parts: string[] = [];
  const missing = missingOf(variables);
  if (missing.length) parts.push(t('api.missing', { count: missing.length, names: missing.join(', ') }));
  const replaced = variables.filter((variable) => variable.value === 'replaced').map((variable) => variable.name);
  if (replaced.length) parts.push(t('api.replaced', { count: replaced.length, names: replaced.join(', ') }));
  if (bodiesMissing > 0) parts.push(t('api.bodyMissing', { count: bodiesMissing }));
  if (warning === 'secrets_not_saved') parts.push(t('api.errors.secrets_not_saved'));
  return parts;
}

/** Where the variables without a value get one: the API client's private values (RB-094-21). */
const SECRETS_SETTING = 'api-client.secrets';

/** `line` linked to the private values while `variables` has one without a value: its text says
 * "Set them in Settings > Network > API client" (RB-094-21). */
function linked(line: StatusLine, variables: readonly CapturedVariable[], t: T): StatusLine {
  return missingOf(variables).length ? { ...line, setting: { key: SECRETS_SETTING, label: t('api.setting_link') } } : line;
}

/** What the status line says of an open. */
export function openedLine(answer: OpenAnswer, t: T): StatusLine {
  const parts = warningsOf(answer.variables ?? [], answer.bodyMissing ? 1 : 0, answer.warning, t);
  const line: StatusLine = { tone: parts.length ? 'warn' : 'ok', text: [t('api.opened', { name: answer.request?.name ?? '' }), ...parts].join(' ') };
  return linked(line, answer.variables ?? [], t);
}

/** What the status line says of a save, with what `limits` left out. */
export function savedLine(answer: SaveAnswer, limits: SaveLimits, t: T): StatusLine {
  const saved = answer.saved ?? [];
  const parts = warningsOf(answer.variables ?? [], saved.filter((one) => one.bodyMissing).length, answer.warning, t);
  // The w31 security review: a request holding a {{ of the page is never saved.
  const templates = (answer.skipped ?? []).filter((one) => one.reason === 'template').length;
  if (templates > 0) parts.push(t('api.template', { count: templates }));
  if (limits.cutAt > 0) parts.push(t('api.tooMany', { most: limits.cutAt }));
  if (limits.partial) parts.push(t('api.partial'));
  const line: StatusLine = { tone: parts.length ? 'warn' : 'ok', text: [t('api.saved', { saved: saved.length, skipped: answer.skipped?.length ?? 0 }), ...parts].join(' ') };
  return linked(line, answer.variables ?? [], t);
}

/** The API client failures this panel says in its own words. */
const OWN_ERRORS = ['capture_gone', 'capture_template', 'no_project', 'not_written', 'secrets_not_saved', 'untrusted'];

/** The panel's own text key for the API client's failure `code`, when it has one. */
export function ownErrorKey(code: string): string | null {
  const name = code.startsWith('plugin.api-client.errors.') ? code.slice('plugin.api-client.errors.'.length) : '';
  return OWN_ERRORS.includes(name) ? `api.errors.${name}` : null;
}

export function createApiHandoff(panel: Panel, say: Say) {
  const t: T = (key, params) => panel.t(key, params);

  function failed(error: unknown): void {
    const key = error instanceof Error ? ownErrorKey(error.message) : null;
    say('error', key ? t(key) : panel.error(error));
  }

  /** Opens request `one` in the API client, unsaved. */
  async function open(one: CaptureRef | null): Promise<void> {
    if (!one) {
      say('info', t('api.noSelection'));
      return;
    }
    try {
      const answer = (await panel.call({ tool: 'api_open' }, one)) as OpenAnswer;
      const line = openedLine(answer, t);
      say(line.tone, line.text, line.setting);
    } catch (error) {
      failed(error);
    }
  }

  /** Saves `requests` as API client requests; `limits` says what the list left out. */
  async function save(requests: CaptureRef[], limits: SaveLimits = { cutAt: 0, partial: false }): Promise<void> {
    if (!requests.length) {
      say('info', t('api.nothingShown'));
      return;
    }
    try {
      const answer = (await panel.call({ tool: 'api_save' }, { requests })) as SaveAnswer;
      const line = savedLine(answer, limits, t);
      say(line.tone, line.text, line.setting);
    } catch (error) {
      failed(error);
    }
  }

  return { open, save };
}
