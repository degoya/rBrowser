// A panel's share of the panels window's message limit (RB-090-44): the setting
// `plugins.panelMessageMaxBytes`, which the host sends with every panel's settings (4 MiB by
// default). The window refuses a call or an answer over it (`plugins.error.too_large`, its text
// names the setting), so a panel that asks its tool for an answer of a given size (`budget`)
// sizes it by this and keeps room for the envelope around the answer.

/** The setting that holds the panels window's largest message, as JSON text in bytes. */
export const MESSAGE_LIMIT_KEY = 'plugins.panelMessageMaxBytes';
/** The limit when the settings name none (an older host): the page scripts' 64 KiB. */
export const FALLBACK_MESSAGE_BYTES = 64 * 1024;
/** What a budget leaves of the limit for the envelope: at 64 KiB a budget is 60 000 bytes. */
export const ENVELOPE_BYTES = FALLBACK_MESSAGE_BYTES - 60_000;
/** The largest budget a tool takes: a plugin's component answers at most 4 MiB of JSON. */
export const MAX_BUDGET = 4 * 1024 * 1024;

/** The message limit `settings` name, in bytes; [`FALLBACK_MESSAGE_BYTES`] without one. */
export function messageLimit(settings: Record<string, unknown>): number {
  const value = settings[MESSAGE_LIMIT_KEY];
  return typeof value === 'number' && Number.isSafeInteger(value) ? Math.max(value, FALLBACK_MESSAGE_BYTES) : FALLBACK_MESSAGE_BYTES;
}

/** The bytes of JSON one answer may take under the limit `settings` name, at most `cap`. */
export function answerBudget(settings: Record<string, unknown>, cap = MAX_BUDGET): number {
  return Math.min(messageLimit(settings) - ENVELOPE_BYTES, cap);
}
