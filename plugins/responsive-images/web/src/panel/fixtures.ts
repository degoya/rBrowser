// The responsive images panel's test fixtures (RB-093-02): the English texts by full key and an
// answer of the tool `check` for the acceptance fixture in two breakpoints and a Firefox view, the
// issues with their severity and message as src/tool.rs writes them.
import en from '../../../locales/en.json';
import { analyze, DEFAULTS } from '../analyze';
import { BREAKPOINTS, fixtureImages, PAGE_URL } from '../fixtures';
import type { Measured } from '../types';
import type { Answer, PanelImage, ViewAnswer } from './model';

function flatten(value: unknown, prefix = ''): Record<string, string> {
  if (typeof value === 'string') return { [prefix]: value };
  return Object.entries(value as Record<string, unknown>).reduce<Record<string, string>>(
    (all, [key, child]) => ({ ...all, ...flatten(child, prefix ? `${prefix}.${key}` : key) }),
    {},
  );
}
export const STRINGS = flatten(en);

function panelImage(image: Measured): PanelImage {
  return {
    ...image,
    issues: image.issues.map((issue) => ({
      rule: issue.rule,
      variant: issue.variant ?? null,
      severity: 'warning' as const,
      message: `plugin.responsive-images.findings.${issue.rule}${issue.variant ? `.${issue.variant}` : ''}`,
      params: issue.params,
    })),
  };
}

function view(id: number, index: number): ViewAnswer {
  const breakpoint = BREAKPOINTS[index] as (typeof BREAKPOINTS)[number];
  return {
    id,
    label: breakpoint.id[0]?.toUpperCase() + breakpoint.id.slice(1),
    breakpoint: breakpoint.id,
    engine: 'chromium',
    width: breakpoint.width,
    height: breakpoint.height,
    dpr: breakpoint.dpr,
    url: PAGE_URL,
    images: analyze(fixtureImages(breakpoint), { ...DEFAULTS, dpr: breakpoint.dpr }).map(panelImage),
    more: 0,
    lcpKnown: true,
  };
}

export const ANSWER: Answer = {
  views: [
    view(1, 0),
    view(2, 2),
    { id: 3, label: 'Firefox', breakpoint: 'firefox', engine: 'firefox', width: 1280, height: 800, dpr: 1, error: { code: 'plugins.error.not_available', params: { engine: 'firefox' } } },
  ],
  summary: { error: 0, warning: 8, info: 0 },
  settings: { oversizeFactor: 1.5, undersizeFactor: 1, checkLcp: true },
  cut: false,
};
