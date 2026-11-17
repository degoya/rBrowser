// The panel's model, without DOM: the overlays grouped by kind, what each view reported, and the
// lines the panel lists. App.vue draws it. Texts come through the panel's `t` (panel-ui: catalogue
// messages, plurals included; RB-080-83).

import type { Report, State } from '../types';

export const PREFIX = 'plugin.overlays.';
export const TOGGLES = [
  'overflow',
  'outlines',
  'highlight',
  'stacking',
  'scrollbars',
  'text-spacing',
  'target-size',
  'small-text',
  'focus-tracking',
  'force-state',
  'content-chaos',
  'placeholdifier',
  'css-off',
  'js-off',
  'images-off',
  'tab-order',
] as const;
export type Toggle = (typeof TOGGLES)[number];

/** The overlays by kind, in the panel's order (RB-080-86): each group a section of switches. The
 * selector group also holds the selector field and the forced state, its switches' options. */
export const GROUPS: ReadonlyArray<{ id: string; toggles: readonly Toggle[] }> = [
  { id: 'layout', toggles: ['overflow', 'outlines', 'stacking', 'scrollbars'] },
  { id: 'selector', toggles: ['highlight', 'force-state'] },
  { id: 'accessibility', toggles: ['text-spacing', 'target-size', 'small-text', 'focus-tracking', 'tab-order'] },
  { id: 'content', toggles: ['content-chaos', 'placeholdifier', 'css-off', 'js-off', 'images-off'] },
];

/** The states force-state can force, as the panel offers them. */
export const FORCED_STATES = ['hover', 'focus', 'active', 'focus-visible', 'focus-within', 'visited'] as const;

/** A panel text: `key` below `plugin.overlays.` with its parameters filled in (panel-ui's `t`). */
export type Text = (key: string, params?: Record<string, unknown>) => string;

/** The key of an overlay below `panel.toggle.` (and its command id): `-` written `_`. */
export const toggleKey = (overlay: string): string => overlay.replace(/-/g, '_');

export interface ViewNews {
  matches?: { count: number; index: number; error?: string };
  overflow?: Report;
  /** The hit counts of the checking overlays, by overlay id: the whole count and, past what the
   *  page draws, how many it draws (RB-090-17). */
  hits?: Record<string, { count: number; shown?: number }>;
}

/** What each view told the panel, by view id. */
export class Views {
  readonly byId = new Map<number, ViewNews>();

  /** Takes an `event` of the panels window; answers whether it changed anything. */
  take(view: unknown, message: unknown): boolean {
    if (typeof view !== 'number' || !message || typeof message !== 'object') return false;
    const news = this.byId.get(view) ?? {};
    const typed = message as { type?: unknown; count?: unknown; shown?: unknown; index?: unknown; error?: unknown; report?: unknown; overlay?: unknown };
    if (typed.type === 'matches' && typeof typed.count === 'number') {
      news.matches = {
        count: typed.count,
        index: typeof typed.index === 'number' ? typed.index : 0,
        ...(typeof typed.error === 'string' ? { error: typed.error } : {}),
      };
    } else if (typed.type === 'overflow' && typed.report && typeof typed.report === 'object') {
      news.overflow = typed.report as Report;
    } else if (typed.type === 'hits' && typeof typed.overlay === 'string' && typeof typed.count === 'number') {
      const shown = typeof typed.shown === 'number' && typed.shown < typed.count ? { shown: typed.shown } : {};
      news.hits = { ...news.hits, [typed.overlay]: { count: typed.count, ...shown } };
    } else {
      return false;
    }
    this.byId.set(view, news);
    return true;
  }

  /** One line per view, in view order, for the overlays `shown` says are on. */
  lines(t: Text, shown: (overlay: string) => boolean): string[] {
    return [...this.byId.entries()]
      .sort(([left], [right]) => left - right)
      .map(([view, news]) => {
        const parts: string[] = [];
        if (shown('highlight') && news.matches) {
          parts.push(
            news.matches.error
              ? t('panel.invalid')
              : t('panel.matches', {
                  current: news.matches.count === 0 ? 0 : news.matches.index + 1,
                  count: news.matches.count,
                }),
          );
        }
        if (shown('overflow') && news.overflow) {
          const report = news.overflow;
          parts.push(
            report.hasHorizontalScroll
              ? t('panel.scroll', {
                  px: Math.round(report.scrollWidth - report.clientWidth),
                  causes: report.causes.map((mark) => `${mark.selector} (+${mark.overhang ?? 0}px)`).join(', ') || '-',
                })
              : t('panel.no_scroll'),
          );
        }
        for (const [overlay, hits] of Object.entries(news.hits ?? {})) {
          if (!shown(overlay)) continue;
          const name = t(`panel.toggle.${toggleKey(overlay)}`);
          parts.push(
            hits.shown === undefined
              ? t('panel.hits', { overlay: name, count: hits.count })
              : t('panel.hits_shown', { overlay: name, shown: hits.shown, count: hits.count }),
          );
        }
        return parts.length ? `${t('panel.view', { view })}: ${parts.join('; ')}` : '';
      })
      .filter((line) => line !== '');
  }
}

/** Whether every view shows `toggle` (the panel's switches act on every view). */
export function isOn(state: State | undefined, toggle: string): boolean {
  return state?.all?.[toggle]?.enabled === true;
}

/** The selector the highlight keeps. */
export function selectorOf(state: State | undefined): string {
  const selector = state?.all?.highlight?.options?.selector;
  return typeof selector === 'string' ? selector : '';
}

/** The `overlay_set` arguments for a switch: the selector for highlight and force-state when
 * switched on with one, the forced state for force-state, and the active view only on Shift. */
export function setArgs(toggle: Toggle, enabled: boolean, selector: string, state: string, shift: boolean): Record<string, unknown> {
  const options: Record<string, unknown> = {};
  const trimmed = selector.trim();
  if ((toggle === 'highlight' || toggle === 'force-state') && enabled && trimmed) options.selector = trimmed;
  if (toggle === 'force-state' && enabled) options.states = [state];
  return { overlay: toggle, enabled, options, ...(shift ? { target: 'active' } : {}) };
}
