// A panel's texts and error sentences (RB-080-83). The one source is
// plugins/web-common/panel/texts.js (`PanelTexts`, bundled into every Nuxt UI panel): when it
// offers `format` and `errorText` (RB-080-90) they are used; this file holds the same rules for
// a texts.js without them - `{name}` placeholders, the ICU plural
// `{n, plural, =0 {...} one {# x} other {# xs}}` (English: `one` is 1) and the error sentence:
// the text of the code, then the text of its reason; never the bare code.

export type Strings = Record<string, string>;
export type Params = Record<string, unknown>;

interface SharedTexts {
  format?: (template: string, params?: Params) => string;
  errorText?: (strings: Strings, code: string, params?: Params) => string;
}

const shared = (): SharedTexts | undefined => (globalThis as { PanelTexts?: SharedTexts }).PanelTexts;

/** English when the panels window sent no text for a failed call. */
export const CALL_FAILED = 'The plugin could not do that.';

/** The end of the branch or argument that opens at `open` (its `{`), or -1. */
function closing(template: string, open: number): number {
  let depth = 0;
  for (let at = open; at < template.length; at += 1) {
    if (template[at] === '{') depth += 1;
    else if (template[at] === '}') {
      depth -= 1;
      if (depth === 0) return at;
    }
  }
  return -1;
}

/** The branch of `body` (`=0 {..} one {..} other {..}`) for `count`, or undefined. */
function branch(body: string, count: number): string | undefined {
  const branches = new Map<string, string>();
  let at = 0;
  while (at < body.length) {
    const open = body.indexOf('{', at);
    if (open < 0) break;
    const end = closing(body, open);
    if (end < 0) return undefined;
    branches.set(body.slice(at, open).trim(), body.slice(open + 1, end));
    at = end + 1;
  }
  return branches.get(`=${count}`) ?? (count === 1 ? branches.get('one') : undefined) ?? branches.get('other');
}

function fill(template: string, params: Params): string {
  let out = '';
  let at = 0;
  while (at < template.length) {
    const open = template.indexOf('{', at);
    if (open < 0) break;
    const end = closing(template, open);
    if (end < 0) break;
    out += template.slice(at, open);
    const inner = template.slice(open + 1, end);
    const plural = /^\s*(\w+)\s*,\s*plural\s*,([\s\S]*)$/.exec(inner);
    if (plural) {
      const value = params[plural[1] ?? ''];
      const count = Number(value);
      const chosen = branch(plural[2] ?? '', count);
      out += chosen === undefined ? template.slice(open, end + 1) : fill(chosen.replaceAll('#', String(value ?? '')), params);
    } else {
      out += String(params[inner.trim()] ?? '');
    }
    at = end + 1;
  }
  return out + template.slice(at);
}

/** `template` with its placeholders and plurals filled. */
export function format(template: string, params: Params = {}): string {
  const own = shared()?.format;
  return own ? own(template, params) : fill(template, params);
}

/** A failure as older hosts sent it, `code key=value ...`, split into the code and its params. */
export function splitError(error: string): { code: string; params: Params } {
  const [code = ''] = error.split(' ', 1);
  const params: Params = {};
  const marks = [...error.matchAll(/ ([\w-]+)=/g)];
  marks.forEach((mark, index) => {
    const start = (mark.index ?? 0) + mark[0].length;
    params[mark[1] ?? ''] = error.slice(start, marks[index + 1]?.index ?? error.length);
  });
  return { code, params };
}

/** One sentence for a failed call: the text of `code`, then that of its reason; never the code. */
export function errorText(strings: Strings, error: string, params?: Params): string {
  const split = params ? { code: error, params } : splitError(error);
  const own = shared()?.errorText;
  if (own) return own(strings, split.code, split.params);
  const template = strings[split.code];
  const head = template === undefined ? strings['plugins.panel.call_failed'] ?? CALL_FAILED : format(template, split.params);
  const reason = split.params.reason;
  if (typeof reason !== 'string' || reason === '' || template?.includes('{reason}')) return head;
  const said = strings[`plugins.reason.${reason}`];
  if (said !== undefined) return `${head} ${format(said, split.params)}`;
  // A free reason in words is said; a bare token (`no_answer`) is not.
  return /\s/.test(reason) ? `${head} ${reason}` : head;
}
