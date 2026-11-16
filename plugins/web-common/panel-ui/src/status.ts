// The status line of a panel (RB-080-83, docs/design.md 5.7 and 6): one message with its level
// as an icon and a word-carrying text, never by colour alone; an error is read out at once.

export type Tone = 'info' | 'ok' | 'warn' | 'error' | 'busy';

export interface StatusLine {
  tone: Tone;
  text: string;
}

/** Nuxt UI's colour of a tone. */
export const TONE_COLOR = { info: 'info', ok: 'success', warn: 'warning', error: 'error', busy: 'neutral' } as const;

/** The Lucide icon of a tone (the kit's, RB-080-62). */
export const TONE_ICON: Record<Tone, string> = {
  info: 'i-lucide-info',
  ok: 'i-lucide-circle-check',
  warn: 'i-lucide-triangle-alert',
  error: 'i-lucide-circle-x',
  busy: 'i-lucide-loader-circle',
};

/** How urgently a reader hears the line. */
export function liveness(tone: Tone): 'assertive' | 'polite' {
  return tone === 'error' ? 'assertive' : 'polite';
}
