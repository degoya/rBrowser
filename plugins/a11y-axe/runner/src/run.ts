// Runs axe-core in a view and shapes its answer for the plugin's component (src/answer.rs): the
// violations (and, when asked, the incomplete results) with each node's selector, element path,
// impact, HTML and failure summary - cut, so the answer stays far below the 4 MiB `evaluate`
// allows. The component chooses the rules; this script never widens them.

import { elementPath } from './path';

/** What the component asks for (src/script.rs builds it). */
export interface RunOptions {
  /** The axe rule ids to run; nothing runs when empty. */
  rules: string[];
  /** CSS selectors to check only; the whole document when empty. */
  include: string[];
  /** CSS selectors to leave out. */
  exclude: string[];
  /** Report axe's `incomplete` results (needs review) too. */
  incomplete: boolean;
  /** The most nodes one rule reports. */
  maxNodes: number;
  /** The most nodes the whole answer holds. */
  maxTotal: number;
  /** The most characters of a node's HTML and of its failure summary. */
  maxText: number;
}

/** One node of a result, shaped. */
export interface AnswerNode {
  selector: string;
  path: string;
  impact: string | null;
  html: string;
  summary: string;
}

/** One rule's result, shaped. */
export interface AnswerRule {
  id: string;
  impact: string | null;
  help: string;
  description: string;
  helpUrl: string;
  tags: string[];
  nodes: AnswerNode[];
  /** Nodes left out by `maxNodes` or `maxTotal`. */
  omitted: number;
}

/** The answer of one run. */
export interface Answer {
  axeVersion: string;
  url: string;
  violations: AnswerRule[];
  incomplete: AnswerRule[];
  /** Whether any node was left out. */
  truncated: boolean;
}

/** The part of an axe node result the runner reads (`elementRef: true`). */
export interface AxeNode {
  target: unknown[];
  impact?: string | null;
  html?: string;
  failureSummary?: string;
  element?: Element;
}

/** The part of an axe rule result the runner reads. */
export interface AxeRule {
  id: string;
  impact?: string | null;
  help?: string;
  description?: string;
  helpUrl?: string;
  tags?: string[];
  nodes: AxeNode[];
}

/** The part of axe the runner calls. */
export interface AxeLike {
  version: string;
  run(
    context: unknown,
    options: Record<string, unknown>,
  ): Promise<{ url?: string; violations: AxeRule[]; incomplete: AxeRule[] }>;
}

function cut(text: string | undefined, max: number): string {
  const value = text ?? '';
  return value.length > max ? `${value.slice(0, max)}...` : value;
}

/** axe's `target` as one selector: a shadow-DOM step is a list, joined with " >>> ". */
export function selectorOf(target: unknown[]): string {
  const last = target[target.length - 1];
  if (Array.isArray(last)) return last.map(String).join(' >>> ');
  return last === undefined ? '' : String(last);
}

function shape(rules: AxeRule[], options: RunOptions, budget: { left: number }): AnswerRule[] {
  return rules.map((rule) => {
    const room = Math.max(0, Math.min(options.maxNodes, budget.left));
    const kept = rule.nodes.slice(0, room);
    budget.left -= kept.length;
    return {
      id: rule.id,
      impact: rule.impact ?? null,
      help: rule.help ?? '',
      description: rule.description ?? '',
      helpUrl: rule.helpUrl ?? '',
      tags: rule.tags ?? [],
      nodes: kept.map((node) => ({
        selector: selectorOf(node.target),
        path: node.element ? elementPath(node.element) : selectorOf(node.target),
        impact: node.impact ?? null,
        html: cut(node.html, options.maxText),
        summary: cut(node.failureSummary, options.maxText),
      })),
      omitted: rule.nodes.length - kept.length,
    };
  });
}

/** The axe context of `options`: the document, or its include and exclude selectors. */
export function contextOf(options: RunOptions, doc: Document): unknown {
  if (options.include.length === 0 && options.exclude.length === 0) return doc;
  const context: Record<string, string[]> = {};
  if (options.include.length > 0) context.include = options.include;
  if (options.exclude.length > 0) context.exclude = options.exclude;
  return context;
}

/** Runs `axe` on `doc` as `options` say and shapes the answer. */
export async function runAxe(axe: AxeLike, options: RunOptions, doc: Document): Promise<Answer> {
  if (options.rules.length === 0) {
    return { axeVersion: axe.version, url: doc.location?.href ?? '', violations: [], incomplete: [], truncated: false };
  }
  const results = await axe.run(contextOf(options, doc), {
    runOnly: { type: 'rule', values: options.rules },
    resultTypes: options.incomplete ? ['violations', 'incomplete'] : ['violations'],
    elementRef: true,
    iframes: false,
    selectors: true,
    ancestry: false,
    xpath: false,
  });
  const budget = { left: options.maxTotal };
  const violations = shape(results.violations, options, budget);
  const incomplete = options.incomplete ? shape(results.incomplete, options, budget) : [];
  const truncated = [...violations, ...incomplete].some((rule) => rule.omitted > 0);
  return {
    axeVersion: axe.version,
    url: results.url ?? doc.location?.href ?? '',
    violations,
    incomplete,
    truncated,
  };
}
