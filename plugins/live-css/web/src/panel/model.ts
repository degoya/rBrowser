// The Live CSS panel's model (RB-094-04): the draft as the component answers it (`set`, src/tools.rs
// `state`), the blocks as the panel edits them, the arguments of a change and the breakpoint
// choices of a block.
import { isObject } from '../../../../web-common/src/is-object';

export const PREFIX = 'plugin.live-css.';
/** How long the panel waits after the last keystroke before the views get the draft. */
export const APPLY_DELAY_MS = 300;

export type Available = 'trusted' | 'untrusted' | 'noProject';
export type Scope = 'all' | 'query' | 'nowhere';

export interface Breakpoint {
  id: string;
  label: string;
  width: number;
}

/** A block as the component answers it: its scope, its media query and the text to copy. */
export interface BlockState {
  css: string;
  breakpoints: string[];
  scope: Scope;
  media: string | null;
  copy: string | null;
}

export interface DraftState {
  available: Available;
  enabled: boolean;
  blocks: BlockState[];
  breakpoints: Breakpoint[];
  file: string;
}

/** A block as the panel edits it; `key` keeps its card across edits. */
export interface EditBlock {
  key: number;
  css: string;
  breakpoints: string[];
}

/** One choice of a block's breakpoints; `missing` for one the grid no longer shows. */
export interface ScopeItem {
  label: string;
  value: string;
  missing: boolean;
}

const strings = (value: unknown): value is string[] => Array.isArray(value) && value.every((item) => typeof item === 'string');
const textOrNull = (value: unknown): value is string | null => value === null || typeof value === 'string';

function readBlock(value: unknown): BlockState | null {
  if (!isObject(value)) return null;
  const { css, breakpoints, scope, media, copy } = value;
  if (typeof css !== 'string' || !strings(breakpoints) || !textOrNull(media) || !textOrNull(copy)) return null;
  if (scope !== 'all' && scope !== 'query' && scope !== 'nowhere') return null;
  return { css, breakpoints, scope, media, copy };
}

function readBreakpoint(value: unknown): Breakpoint | null {
  if (!isObject(value)) return null;
  const { id, label, width } = value;
  return typeof id === 'string' && typeof label === 'string' && typeof width === 'number' ? { id, label, width } : null;
}

/** The component's answer as a draft; null for one that does not read. */
export function readState(value: unknown): DraftState | null {
  if (!isObject(value)) return null;
  const { available, enabled, blocks, breakpoints, file } = value;
  if (available !== 'trusted' && available !== 'untrusted' && available !== 'noProject') return null;
  if (typeof enabled !== 'boolean' || typeof file !== 'string' || !Array.isArray(blocks) || !Array.isArray(breakpoints)) return null;
  const read = blocks.map(readBlock);
  const points = breakpoints.map(readBreakpoint);
  if (read.some((block) => block === null) || points.some((point) => point === null)) return null;
  return { available, enabled, blocks: read as BlockState[], breakpoints: points as Breakpoint[], file };
}

/** The blocks of `state` to edit, each with a new key. */
export function toEdit(state: DraftState, nextKey: () => number): EditBlock[] {
  return state.blocks.map((block) => ({ key: nextKey(), css: block.css, breakpoints: [...block.breakpoints] }));
}

/** The arguments of `set` for the panel's draft. */
export function setArgs(blocks: readonly EditBlock[], enabled: boolean): { blocks: { css: string; breakpoints: string[] }[]; enabled: boolean } {
  return { blocks: blocks.map((block) => ({ css: block.css, breakpoints: [...block.breakpoints] })), enabled };
}

/** Whether the panel's draft is the one `state` holds. */
export function sameDraft(blocks: readonly EditBlock[], enabled: boolean, state: DraftState): boolean {
  if (enabled !== state.enabled || blocks.length !== state.blocks.length) return false;
  return blocks.every((block, index) => {
    const stored = state.blocks[index];
    return stored !== undefined && stored.css === block.css && stored.breakpoints.join('\n') === block.breakpoints.join('\n');
  });
}

/**
 * The breakpoint choices of a block: the grid's, by width, named by `name`; then each chosen id
 * the grid no longer shows, named by `missing`, so it stays visible and can be taken off.
 */
export function scopeItems(
  breakpoints: readonly Breakpoint[],
  selected: readonly string[],
  name: (breakpoint: Breakpoint) => string,
  missing: (id: string) => string,
): ScopeItem[] {
  const known = breakpoints.map((breakpoint) => ({ label: name(breakpoint), value: breakpoint.id, missing: false }));
  const gone = selected
    .filter((id) => !breakpoints.some((breakpoint) => breakpoint.id === id))
    .map((id) => ({ label: missing(id), value: id, missing: true }));
  return [...known, ...gone];
}
