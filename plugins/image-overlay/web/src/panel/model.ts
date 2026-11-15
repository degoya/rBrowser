// The image-overlay panel's pure logic (RB-070-09, RB-080-74; on Nuxt UI since RB-080-86): the
// state and answers of the tool `set`, the design choice per breakpoint and the nudge.

export interface OverlayState {
  enabled: boolean;
  opacity: number;
  blend: string;
  invert: boolean;
  anchor: string;
  offset: { x: number; y: number };
  split: { on: boolean; position: number };
  designs: Record<string, string>;
}

export interface Problem {
  code: string;
  params?: Record<string, unknown>;
}

export interface Shown {
  path: string;
  width: number;
  height: number;
  scale: number;
}

export interface ViewRow {
  view: number;
  label: string;
  breakpoint: string;
  image: Shown | null;
  problem: Problem | null;
}

export interface Design {
  name: string;
  path: string;
  mime?: string;
  hash?: string;
  width?: number;
  height?: number;
  chunks?: number;
  problem?: Problem;
}

/** The answer of `set` (and what the component posts after a change anywhere). */
export interface SetAnswer {
  state?: OverlayState;
  views?: ViewRow[];
  designs?: string[];
  saved?: boolean;
  reload?: boolean;
}

/** The answer of `designs` without arguments. */
export interface Library {
  designs?: Design[];
  more?: boolean;
  trusted?: boolean;
}

export const BLENDS = ['normal', 'difference', 'multiply', 'screen', 'overlay', 'exclusion'] as const;
export const ANCHORS = ['document', 'viewport'] as const;

/** The choice's value for "Nearest width": a select item's value cannot be empty. */
export const AUTO = '@auto';

/** A design's name below `.rbrowser/designs/`. */
export const nameOf = (path: string): string => path.replace(/^(\.rbrowser\/)?designs\//, '');

export interface DesignChoice {
  value: string;
  label: string;
  missing?: true;
}

/**
 * The choices for one breakpoint: Nearest width, every design of the folder by name and size (an
 * unusable one marked), and a chosen design that is gone, marked missing.
 */
export function designChoices(
  t: (key: string, params?: Record<string, unknown>) => string,
  files: readonly string[],
  library: readonly Design[],
  current: string | undefined,
): DesignChoice[] {
  const known = new Map(library.map((design) => [design.path, design]));
  const paths = files.map((file) => `designs/${file}`);
  const choices: DesignChoice[] = [{ value: AUTO, label: t('panel.design_auto') }];
  for (const path of paths) {
    const design = known.get(path);
    const name = nameOf(path);
    let label = name;
    if (design?.problem) label = t('panel.option_unusable', { name });
    else if (design?.width) label = t('panel.option_design', { name, width: design.width, height: design.height });
    choices.push({ value: path, label });
  }
  if (current && !paths.includes(current)) choices.push({ value: current, label: t('panel.option_missing', { name: nameOf(current) }), missing: true });
  return choices;
}

/** Whether the breakpoint's chosen design is no longer in the folder. */
export const isMissing = (files: readonly string[], current: string | undefined): boolean =>
  Boolean(current && !files.map((file) => `designs/${file}`).includes(current));

const ARROWS: Record<string, [number, number]> = { ArrowLeft: [-1, 0], ArrowRight: [1, 0], ArrowUp: [0, -1], ArrowDown: [0, 1] };

/** The offset an arrow key moves to (1 px, with Shift 10 px), or null for another key. */
export function nudged(offset: { x: number; y: number }, key: string, shift: boolean): { x: number; y: number } | null {
  const step = ARROWS[key];
  if (!step) return null;
  const by = shift ? 10 : 1;
  return { x: offset.x + step[0] * by, y: offset.y + step[1] * by };
}
