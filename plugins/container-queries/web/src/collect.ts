// One view's container queries: every element with a size `container-type`, every `@container`
// rule of the readable stylesheets, which container each rule queries (the nearest ancestor of
// the elements its selectors match with the rule's name and an axis the condition reads - as the
// browser picks it), which conditions match now, and each container's thresholds. The page
// script draws from it; the component evaluates it as assets/analyze.js (`containers_analyze`).

import { elementPath } from '../../../web-common/src/element-path';
import { type Axis, type Node, type Units, axesOf, boundsOf, evaluate, parseCondition, parsePrelude } from './query';
import { isResized } from './resize';
import type { ConditionInfo, ContainerInfo, Report } from './types';

/** What a page tells about its elements; the live one is {@link liveEnv}, the tests give theirs. */
export interface Env {
  /** The content box of `element`, in px. */
  size(element: Element): { inline: number; block: number };
  units(element: Element): Units;
  /** Whether a media query applies now. */
  media(query: string): boolean;
}

export interface Options {
  /** An element whose subtree is no part of the page (an overlay host). */
  exclude?: Element | null;
  maxContainers?: number;
  maxConditions?: number;
}

/** The limits of one report: data from an untrusted page, held small. */
export const MAX_CONTAINERS = 300;
export const MAX_CONDITIONS = 300;
const MAX_ELEMENTS = 20_000;
const MAX_MATCHES_PER_RULE = 200;
const MAX_TEXT = 300;

interface Rule {
  info: ConditionInfo;
  node: Node;
  axes: Set<Axis>;
  selectors: string[];
  active: boolean;
}

/** A container with its element, for the page script's drawing. */
export interface Found {
  element: Element;
  info: ContainerInfo;
}

const cut = (text: string): string => (text.length > MAX_TEXT ? text.slice(0, MAX_TEXT) : text);
const round = (value: number): number => Math.round(value * 100) / 100;

/** The element's path from the root, the findings' shared form (web-common/src/element-path.ts),
 *  which is also a selector that finds it again: the same in every view and every plugin. */
export const pathOf = elementPath;

/** The element `path` names, or null (a path the page no longer has, or no selector at all). */
export function findByPath(doc: Document, path: string): Element | null {
  try {
    return doc.querySelector(path);
  } catch {
    return null;
  }
}

/** A short name: the tag with its id, or with its first two classes. */
export function labelOf(element: Element): string {
  if (element.id) return cut(`${element.localName}#${element.id}`);
  const classes = Array.from(element.classList).slice(0, 2);
  return cut([element.localName, ...classes].join('.'));
}

/** The live page: computed sizes and fonts, `matchMedia`. */
export function liveEnv(win: Window): Env {
  const px = (value: string): number => parseFloat(value) || 0;
  return {
    size(element) {
      const style = win.getComputedStyle(element);
      const pad = (a: string, b: string): number => px(style.getPropertyValue(a)) + px(style.getPropertyValue(b));
      const borderBox = style.boxSizing === 'border-box';
      const inline = px(style.width) - (borderBox ? pad('padding-left', 'padding-right') + pad('border-left-width', 'border-right-width') : 0);
      const block = px(style.height) - (borderBox ? pad('padding-top', 'padding-bottom') + pad('border-top-width', 'border-bottom-width') : 0);
      return { inline: Math.max(inline, 0), block: Math.max(block, 0) };
    },
    units(element) {
      const root = win.document.documentElement;
      return {
        em: px(win.getComputedStyle(element).fontSize) || 16,
        rem: px(win.getComputedStyle(root).fontSize) || 16,
        vw: win.innerWidth,
        vh: win.innerHeight,
      };
    },
    media: (query) => win.matchMedia(query).matches,
  };
}

function preludeOf(rule: CSSRule): { name: string; query: string } {
  const named = rule as CSSRule & { containerName?: unknown; containerQuery?: unknown; conditionText?: unknown };
  if (typeof named.containerName === 'string' && typeof named.containerQuery === 'string') {
    return { name: named.containerName, query: named.containerQuery };
  }
  if (typeof named.conditionText === 'string') return parsePrelude(named.conditionText);
  const head = /^@container\s+([^{]*)\{/i.exec(rule.cssText)?.[1] ?? '';
  return parsePrelude(head);
}

function isContainerRule(rule: CSSRule): boolean {
  return 'containerQuery' in rule || rule.constructor?.name === 'CSSContainerRule' || (typeof (rule as { conditionText?: unknown }).conditionText === 'string' && /^@container\b/i.test(rule.cssText));
}

/** The selectors of the style rules inside a container rule, pseudo-elements taken out. */
function selectorsIn(rules: CSSRuleList, into: string[]): void {
  for (const rule of Array.from(rules)) {
    if (isContainerRule(rule)) continue;
    const selector = (rule as { selectorText?: unknown }).selectorText;
    if (typeof selector === 'string') into.push(selector.replace(/::?(?:before|after|first-line|first-letter)\b|::[\w-]+(?:\([^)]*\))?/gi, ''));
    else if ('cssRules' in rule) selectorsIn((rule as CSSGroupingRule).cssRules, into);
  }
}

function sheetsOf(doc: Document): CSSStyleSheet[] {
  const adopted = (doc as Document & { adoptedStyleSheets?: CSSStyleSheet[] }).adoptedStyleSheets ?? [];
  return [...Array.from(doc.styleSheets), ...adopted];
}

/** Every `@container` rule of `doc`, in stylesheet order; `active` false inside an @media that does not apply. */
function collectRules(doc: Document, env: Env, units: Units): { rules: Rule[]; unreadable: number } {
  const rules: Rule[] = [];
  let unreadable = 0;
  const seen = new Set<CSSStyleSheet>();
  const walkSheet = (sheet: CSSStyleSheet | null, active: boolean): void => {
    if (!sheet || seen.has(sheet)) return;
    seen.add(sheet);
    let list: CSSRuleList;
    try {
      list = sheet.cssRules;
    } catch {
      unreadable += 1;
      return;
    }
    walk(list, active);
  };
  const walk = (list: CSSRuleList, active: boolean): void => {
    for (const rule of Array.from(list)) {
      if ('styleSheet' in rule) {
        walkSheet((rule as CSSImportRule).styleSheet, active);
        continue;
      }
      if (isContainerRule(rule)) {
        const { name, query } = preludeOf(rule);
        const node = parseCondition(query);
        const selectors: string[] = [];
        selectorsIn((rule as CSSGroupingRule).cssRules, selectors);
        const bounds = boundsOf(node, units);
        const supported = evaluate(node, { inline: 1, block: 1, ...units }) !== null && bounds.every((bound) => bound.px !== null);
        rules.push({ info: { name: cut(name), query: cut(query), bounds, supported }, node, axes: axesOf(node), selectors, active });
      }
      if (!('cssRules' in rule)) continue;
      const media = (rule as { media?: MediaList }).media;
      const applies = media && typeof (rule as { conditionText?: unknown }).conditionText === 'string' && !isContainerRule(rule) ? env.media(media.mediaText) : true;
      walk((rule as CSSGroupingRule).cssRules, active && applies);
    }
  };
  for (const sheet of sheetsOf(doc)) walkSheet(sheet, true);
  return { rules, unreadable };
}

function containerType(element: Element): { type: string; names: string[] } | null {
  const view = element.ownerDocument.defaultView;
  if (!view) return null;
  const style = view.getComputedStyle(element);
  const type = /\b(inline-size|size)\b/.exec(style.getPropertyValue('container-type'))?.[1];
  if (!type) return null;
  const name = style.getPropertyValue('container-name').trim();
  return { type, names: name && name !== 'none' ? name.split(/\s+/).map(cut) : [] };
}

/** The containers and conditions of `doc`, with the elements, for drawing. */
export function scan(doc: Document, env: Env, options: Options = {}): { found: Found[]; report: Report } {
  const maxContainers = options.maxContainers ?? MAX_CONTAINERS;
  const maxConditions = options.maxConditions ?? MAX_CONDITIONS;
  let truncated = false;
  const found: Found[] = [];
  const index = new Map<Element, Found>();
  const all = doc.querySelectorAll('*');
  if (all.length > MAX_ELEMENTS) truncated = true;
  for (let at = 0; at < Math.min(all.length, MAX_ELEMENTS); at += 1) {
    const element = all[at] as Element;
    if (element.localName.startsWith('rbrowser-') || options.exclude?.contains(element)) continue;
    const kind = containerType(element);
    if (!kind) continue;
    if (found.length >= maxContainers) {
      truncated = true;
      break;
    }
    const { inline, block } = env.size(element);
    const info: ContainerInfo = {
      path: cut(pathOf(element)),
      selector: labelOf(element),
      names: kind.names,
      type: kind.type,
      inline: round(inline),
      block: round(block),
      conditions: [],
      matching: [],
      thresholds: { inline: [], block: [] },
      resized: isResized(element),
    };
    const entry = { element, info };
    found.push(entry);
    index.set(element, entry);
  }
  const { rules, unreadable } = collectRules(doc, env, env.units(doc.documentElement));
  if (rules.length > maxConditions) truncated = true;
  const kept = rules.slice(0, maxConditions);
  kept.forEach((rule, number) => {
    if (!rule.active) return;
    const queried = new Set<Found>();
    let matched = 0;
    for (const selector of rule.selectors) {
      let elements: Element[];
      try {
        elements = Array.from(doc.querySelectorAll(selector));
      } catch {
        continue;
      }
      for (const element of elements) {
        if (matched++ >= MAX_MATCHES_PER_RULE) break;
        for (let node = element.parentElement; node; node = node.parentElement) {
          const container = index.get(node);
          if (!container) continue;
          if (rule.info.name && !container.info.names.includes(rule.info.name)) continue;
          if (container.info.type === 'inline-size' && rule.axes.has('block')) continue;
          queried.add(container);
          break;
        }
      }
    }
    for (const container of queried) {
      const { info, element } = container;
      const units = env.units(element);
      info.conditions.push(number);
      if (evaluate(rule.node, { inline: info.inline, block: info.block, ...units }) === true) info.matching.push(number);
      for (const bound of boundsOf(rule.node, units)) {
        if (bound.px !== null) info.thresholds[bound.axis].push(round(bound.px));
      }
    }
  });
  for (const { info } of found) {
    for (const axis of ['inline', 'block'] as const) {
      info.thresholds[axis] = [...new Set(info.thresholds[axis])].sort((a, b) => a - b);
    }
  }
  const report: Report = { containers: found.map((entry) => entry.info), conditions: kept.map((rule) => rule.info), unreadableSheets: unreadable, truncated };
  return { found, report };
}

/** The report of `doc` alone (`containers_analyze`). */
export function analyze(doc: Document, env: Env, options: Options = {}): Report {
  return scan(doc, env, options).report;
}
