// The project step's known values (RB-091-05): the addresses dev servers usually listen on
// (Vite, Next.js and Create React App, webpack, Angular, Django and others, Astro), proposed for
// the free address field; the views of a proposed config as the step lists them; how long the
// input rests before the preview runs, and the text of why the analyser had no proposal.

import type { SetupReasonCode } from '../ipc/protocol';

export const DEV_SERVER_ADDRESSES: readonly string[] = [
  'http://localhost:5173',
  'http://localhost:3000',
  'http://localhost:8080',
  'http://localhost:4200',
  'http://localhost:8000',
  'http://localhost:4321',
];

export interface ProposedView {
  id: string;
  label: string;
  /** `393 x 852`, or the device preset that sizes it. */
  size: { width: number; height: number } | { device: string } | null;
}

type Json = string | number | boolean | null | Json[] | { [key: string]: Json };

function objectOf(value: Json | undefined): { [key: string]: Json } | null {
  return typeof value === 'object' && value !== null && !Array.isArray(value) ? value : null;
}

/** The breakpoints of `config` as the step lists them. */
export function proposedViews(config: { [key: string]: Json }): ProposedView[] {
  const list = config.breakpoints;
  if (!Array.isArray(list)) return [];
  return list.flatMap((entry): ProposedView[] => {
    const view = objectOf(entry);
    if (view === null || typeof view.id !== 'string') return [];
    const label = typeof view.label === 'string' && view.label !== '' ? view.label : view.id;
    const { width, height, device } = view;
    const size =
      typeof width === 'number' && typeof height === 'number'
        ? { width, height }
        : typeof device === 'string'
          ? { device }
          : null;
    return [{ id: view.id, label, size }];
  });
}

/** How long the input rests before the preview runs by itself (RB-091-08). */
export const PREVIEW_DEBOUNCE_MS = 600;

const REASON_KEYS: Record<SetupReasonCode, string> = {
  no_page: 'firstrun.project.reason.no_page',
  unreachable: 'firstrun.project.reason.unreachable',
  timeout: 'firstrun.project.reason.timeout',
  no_css: 'firstrun.project.reason.no_css',
  plugin_missing: 'firstrun.project.reason.plugin_missing',
  plugin_failed: 'firstrun.project.reason.plugin_failed',
  bad_output: 'firstrun.project.reason.bad_output',
  nothing_found: 'firstrun.project.reason.nothing_found',
  invalid: 'firstrun.project.reason.invalid',
};

/** The text of why the analyser had no proposal; an unknown or missing code says "nothing found". */
export function reasonKey(code: string | undefined): string {
  return code !== undefined && Object.hasOwn(REASON_KEYS, code) ? REASON_KEYS[code as SetupReasonCode] : REASON_KEYS.nothing_found;
}
