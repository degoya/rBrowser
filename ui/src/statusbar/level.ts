// How the status bar and its history show a message's level (RB-080-15): an icon and a word
// (never the colour alone), the colour, and how a screen reader hears it - errors at once
// (`assertive`), the rest when it is free (`polite`). Info and success fade; warnings and
// errors stay until closed.

import type { NotificationLevel } from '../ipc/protocol';
import { STATUS_ICONS } from './icons';

/** Every level, the most urgent first (the history's filter). */
export const LEVELS: readonly NotificationLevel[] = ['error', 'warning', 'info', 'success'];

export interface LevelLook {
  icon: string;
  /** The i18n key of the level's word. */
  wordKey: string;
  /** The Tailwind text colour (Nuxt UI's semantic colours). */
  color: string;
  live: 'assertive' | 'polite';
}

const LOOKS: Record<NotificationLevel, LevelLook> = {
  error: { icon: STATUS_ICONS.statusError, wordKey: 'status.level.error', color: 'text-error', live: 'assertive' },
  warning: {
    icon: STATUS_ICONS.statusWarning,
    wordKey: 'status.level.warning',
    color: 'text-warning',
    live: 'polite',
  },
  info: { icon: STATUS_ICONS.statusInfo, wordKey: 'status.level.info', color: 'text-info', live: 'polite' },
  success: {
    icon: STATUS_ICONS.statusSuccess,
    wordKey: 'status.level.success',
    color: 'text-success',
    live: 'polite',
  },
};

export function levelOf(level: NotificationLevel): LevelLook {
  return LOOKS[level];
}

/** Whether a message of `level` fades after `ui.statusBar.infoTimeoutSec`. */
export function fades(level: NotificationLevel): boolean {
  return level === 'info' || level === 'success';
}

const TIME = new Intl.DateTimeFormat(undefined, {
  hour: '2-digit',
  minute: '2-digit',
  second: '2-digit',
  hour12: false,
});

/** `at` (ms since the Unix epoch) as the local time of day, `09:05:07`. */
export function formatTime(at: number): string {
  return TIME.format(new Date(at));
}

/** One message as a copied line: time, level word and text, tab-separated. */
export function messageLine(at: number, word: string, text: string): string {
  return `${formatTime(at)}\t${word}\t${text}`;
}
