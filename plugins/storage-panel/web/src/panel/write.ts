// Writing an entry (RB-070-01, on Nuxt UI since RB-080-84): `set` or `delete` through the
// command `panel` in one view or all, and the outcome in one sentence - how many views took it,
// then each distinct reason a view refused, in the plugin's words.
import type { Panel } from '../../../../web-common/panel-ui/src/panel';
import { type CookieForm, cookieArgs } from './model';

/** What the editor holds of an entry. */
export interface EntryForm extends CookieForm {
  name: string;
  /** The origin or the cookie's domain. */
  scope: string;
  value: string;
}

/** The arguments of a write of `form` in `target`. */
export function writeArgs(kind: string, op: 'set' | 'delete', target: string | number, form: EntryForm): Record<string, unknown> {
  const args: Record<string, unknown> = { op, kind, target, name: form.name };
  if (op === 'set') args.value = form.value;
  const scope = form.scope.trim();
  if (kind === 'cookies') args.cookie = cookieArgs({ ...form, domain: scope });
  else if (scope) args.origin = scope;
  return args;
}

/** Writes and answers the sentence for the status of the editor. */
export async function writeEntry(panel: Panel, kind: string, op: 'set' | 'delete', target: string | number, form: EntryForm): Promise<string> {
  try {
    const answer = (await panel.call({ command: 'panel' }, writeArgs(kind, op, target, form))) as { results?: { ok: boolean; error?: string }[] } | null;
    const results = answer?.results ?? [];
    const failed = results.filter((one) => !one.ok).map((one) => panel.error(one.error ?? ''));
    return [panel.t('panel.results', { ok: results.length - failed.length, total: results.length }), ...new Set(failed)].join(' ');
  } catch (error) {
    return panel.t('panel.error', { text: panel.error(error) });
  }
}
