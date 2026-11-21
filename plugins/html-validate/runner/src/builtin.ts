// The offline engine: html-validate (MIT, bundled) validates the source the component handed in.
// The component chooses the rules and their severity (`html-validate:standard` and two
// document rules, minus what the person switched off, plus what a trusted project's
// `.rbrowser/htmlvalidate.json` sets); this script never widens them. The source is the served
// document or the serialized DOM of the view (`html-validate.source`, RB-094-06), whichever the
// component hands in. A message without a selector (a stray end tag) gets the one of the element
// at its line (src/locate.ts), so the findings panel can highlight it.

import {
  HtmlValidate,
  type Message,
  type RuleConfig,
  ruleExists,
  version,
} from 'html-validate/browser';
import { withSelectors } from './locate';
import { type Answer, type AnswerMessage, cut } from './shape';

/** What the component asks for (src/script.rs builds it). */
export interface BuiltinOptions {
  /** The served source of the page. */
  source: string;
  /** The rules to run with their severity and options (html-validate's own form, as
   * `dist/rules.json` holds it); every other rule is off. */
  rules: RuleConfig;
  /** The most messages the answer holds. */
  maxMessages: number;
  /** The most characters of a message and of an extract. */
  maxText: number;
}

/** The source line a message is on, as context: the whole line, or a window of `max`
 * characters around `offset` when the line is longer (an ellipsis marking each cut). */
export function extractAt(source: string, offset: number, max: number): string {
  const at = Math.max(0, Math.min(offset, source.length));
  const lineStart = source.lastIndexOf('\n', at - 1) + 1;
  const newline = source.indexOf('\n', at);
  const lineEnd = newline === -1 ? source.length : newline;
  const line = source.slice(lineStart, lineEnd).replace(/\r$/, '');
  if (line.length <= max) return line;
  const start = Math.max(lineStart, at - Math.floor(max / 3));
  const end = Math.min(lineStart + line.length, start + max - 2);
  const head = start > lineStart ? '…' : '';
  const tail = end < lineStart + line.length ? '…' : '';
  return `${head}${source.slice(start, end)}${tail}`;
}

function shaped(message: Message, source: string, options: BuiltinOptions): AnswerMessage {
  return {
    rule: message.ruleId,
    severity: message.severity >= 2 ? 'error' : 'warning',
    message: cut(message.message, options.maxText),
    line: message.line,
    column: message.column,
    selector: message.selector,
    extract: extractAt(source, message.offset, options.maxText),
  };
}

/** The rules html-validate itself knows, so a stale id never fails the whole run. */
function known(rules: RuleConfig): RuleConfig {
  return Object.fromEntries(Object.entries(rules).filter(([id]) => ruleExists(id)));
}

/** Validates `options.source` with the given rules. */
export async function validateBuiltin(options: BuiltinOptions): Promise<Answer> {
  const validator = new HtmlValidate({
    root: true,
    extends: [],
    elements: ['html5'],
    rules: known(options.rules),
  });
  const report = await validator.validateString(options.source, 'page.html');
  const messages = report.results.flatMap((result) => result.messages);
  return {
    engine: 'builtin',
    version,
    messages: withSelectors(
      options.source,
      messages
        .slice(0, options.maxMessages)
        .map((message) => shaped(message, options.source, options)),
    ),
    truncated: messages.length > options.maxMessages,
  };
}
