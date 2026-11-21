// The answer the runner hands the plugin's component (src/answer.rs): one shape for both
// engines, html-validate in the view and the Nu HTML Checker at the person's address, so the
// component makes findings the same way from either.

/** A message's severity, as the findings know it. */
export type Severity = 'error' | 'warning' | 'info';

/** One message about the served source. */
export interface AnswerMessage {
  /** The rule id: html-validate's own (`no-dup-id`), or `nu-error`, `nu-warning`, `nu-info`. */
  rule: string;
  severity: Severity;
  /** The engine's English message text, cut to `maxText`. */
  message: string;
  /** 1-based line and column in the served source; 0 when the engine names none. */
  line: number;
  column: number;
  /** A CSS selector of the element in the source's tree, when the engine names one. */
  selector: string | null;
  /** The source text the message is about, cut to `maxText`. */
  extract: string;
}

/** The answer of one validation. */
export interface Answer {
  engine: 'builtin' | 'nu';
  /** html-validate's version; empty for Nu, whose JSON names none. */
  version: string;
  messages: AnswerMessage[];
  /** Whether messages were left out by `maxMessages`. */
  truncated: boolean;
}

/** The answer of a validation that failed, with the reason. */
export interface Failed {
  engine: 'builtin' | 'nu';
  error: string;
  /** The browser refused the request to the Nu checker (a network error, no HTTP answer). */
  blocked?: boolean;
}

/** `text` cut to `max` characters, an ellipsis marking the cut. */
export function cut(text: string, max: number): string {
  return text.length > max ? `${text.slice(0, Math.max(0, max - 1))}…` : text;
}
