// The measure of RB-093-02: an image's file against what its rendered size needs at the view's
// density (CSS pixels times the device pixel ratio), and the srcset, sizes and LCP problems.
// Pure: the collector hands it what it read, the tests hand it the fixture.
//
// - `oversized`: the file has more than `oversizeFactor` times the pixels needed (the browser
//   loads and decodes what the view never shows).
// - `undersized`: less than `undersizeFactor` times (it looks blurred), a pixel of rounding
//   allowed.
// - `sizes-missing`: a srcset with `w` descriptors and no `sizes`, on the `<img>` or a `<source>`:
//   the browser takes 100vw and picks too large a file.
// - `lcp`: the Largest Contentful Paint image loads lazily (`lazy`) or without
//   `fetchpriority="high"` (`priority`).
//
// Measured is the tighter axis, so an image `object-fit: cover` crops counts by the side it fills.
// A vector, an image not loaded yet, one without room on the page and one under two CSS pixels
// are listed but not measured.

import { hasWidthDescriptors } from './srcset';
import type { ImageFacts, Issue, Measured, Size } from './types';

export interface Options {
  dpr: number;
  oversizeFactor: number;
  undersizeFactor: number;
  checkLcp: boolean;
}

/** The settings' defaults (settings.schema.json). */
export const DEFAULTS: Options = { dpr: 1, oversizeFactor: 1.5, undersizeFactor: 1, checkLcp: true };

/** The smallest rendered side measured, in CSS pixels (a tracking pixel is not). */
export const MIN_RENDERED = 2;

/** `value` with at most two decimals, trailing zeros dropped. */
export function formatNumber(value: number): string {
  return String(Math.round(value * 100) / 100);
}

function sizeText(size: Size): string {
  return `${formatNumber(size.width)} × ${formatNumber(size.height)}`;
}

function measurable(facts: ImageFacts): facts is ImageFacts & { file: Size } {
  return (
    facts.visible &&
    facts.loaded &&
    !facts.vector &&
    facts.file !== null &&
    facts.file.width > 0 &&
    facts.rendered.width >= MIN_RENDERED &&
    facts.rendered.height >= MIN_RENDERED
  );
}

/** The size problems of one measured image. */
function sizeIssues(file: Size, rendered: Size, needed: Size, ratio: number, options: Options): Issue[] {
  const params = (factor: number): Record<string, string> => ({
    dpr: formatNumber(options.dpr),
    rendered: sizeText(rendered),
    needed: sizeText(needed),
    file: sizeText(file),
    ratio: formatNumber(ratio),
    factor: formatNumber(factor),
  });
  if (ratio > options.oversizeFactor) return [{ rule: 'oversized', params: params(options.oversizeFactor) }];
  const short = (have: number, need: number): boolean => need * options.undersizeFactor - have > 1;
  const heightShort = file.height > 0 && short(file.height, needed.height);
  if (short(file.width, needed.width) || heightShort) return [{ rule: 'undersized', params: params(options.undersizeFactor) }];
  return [];
}

function blank(text: string): boolean {
  return text.trim() === '';
}

/** One image at the view's density. */
export function measure(facts: ImageFacts, options: Options): Measured {
  const issues: Issue[] = [];
  const dpr = formatNumber(options.dpr);
  let needed: Size | null = null;
  let ratio: number | null = null;
  if (measurable(facts)) {
    needed = {
      width: Math.max(1, Math.round(facts.rendered.width * options.dpr)),
      height: Math.max(1, Math.round(facts.rendered.height * options.dpr)),
    };
    const across = facts.file.width / needed.width;
    const down = facts.file.height > 0 ? facts.file.height / needed.height : across;
    ratio = Math.round(Math.min(across, down) * 100) / 100;
    issues.push(...sizeIssues(facts.file, facts.rendered, needed, ratio, options));
  }
  if (hasWidthDescriptors(facts.srcset) && blank(facts.sizes)) {
    issues.push({ rule: 'sizes-missing', variant: 'img', params: { dpr } });
  } else if (facts.sourceWithoutSizes) {
    issues.push({ rule: 'sizes-missing', variant: 'source', params: { dpr } });
  }
  if (options.checkLcp && facts.lcp) {
    if (facts.loading.toLowerCase() === 'lazy') {
      issues.push({ rule: 'lcp', variant: 'lazy', params: { dpr } });
    } else if (facts.fetchpriority.toLowerCase() !== 'high') {
      issues.push({ rule: 'lcp', variant: 'priority', params: { dpr } });
    }
  }
  return { ...facts, needed, ratio, issues };
}

/** Every image at the view's density. */
export function analyze(images: ImageFacts[], options: Options): Measured[] {
  return images.map((facts) => measure(facts, options));
}
