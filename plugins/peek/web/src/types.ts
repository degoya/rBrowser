// The data between the page script and the component (plugins/peek/src/inspect.rs).

import type { ElementTarget } from '../../../../agent/src/protocol';

/** Top, right, bottom, left in CSS px. */
export type Sides = [number, number, number, number];

export interface Rect {
  x: number;
  y: number;
  width: number;
  height: number;
}

/** What the page script reads of one element (`inspect` request, `peek_inspect` collector). */
export interface PageData {
  tag: string;
  id: string;
  classes: string[];
  /** The border box in viewport CSS px. */
  rect: Rect;
  dpr: number;
  box: { margin: Sides; border: Sides; padding: Sides };
  color: string;
  /** The element's background and its ancestors', innermost first, up to the first opaque. */
  backgrounds: { color: string; image: boolean }[];
  font: {
    family: string;
    size: number;
    weight: number;
    style: string;
    lineHeight: string;
    letterSpacing: string;
  };
  /** The heuristic role and name (the sync agent's); CDP's win in Chromium. */
  role: string;
  name: string;
  /** The CSS path in the top document, or null inside a shadow root. */
  selector: string | null;
  /** Where the pointer was (viewport CSS px): CDP's locator without a selector. */
  point: { x: number; y: number };
  /**
   * The page's scroll (CSS px): the component adds it to `point`, since `DOM.getNodeForLocation`
   * takes document px (RB-090-54).
   */
  scroll: { x: number; y: number };
  /** The element for the other views (the sync agent's target); null when too large. */
  target: ElementTarget | null;
}

export interface SidesObject {
  top: number;
  right: number;
  bottom: number;
  left: number;
}

/** What the component answers (`inspection`), as `peek_inspect` shows it. */
export interface Inspection {
  tag: string;
  id: string;
  classes: string[];
  size: { width: number; height: number; dpr: number; deviceWidth: number; deviceHeight: number };
  box: {
    margin: SidesObject;
    border: SidesObject;
    padding: SidesObject;
    content: { width: number; height: number };
  };
  colors: { foreground: string | null; background: string | null; backgroundUncertain: boolean };
  contrast: {
    ratio: number;
    level: 'aaa' | 'aa' | 'aa_large' | 'fail';
    largeText: boolean;
    foreground: string;
    background: string;
  } | null;
  font: {
    declared: string;
    rendered: { family: string; postScriptName: string; custom: boolean; glyphs: number }[] | null;
    size: number;
    weight: number;
    style: string;
    lineHeight: string;
    letterSpacing: string;
  };
  role: string;
  name: string;
  accessibility: 'cdp' | 'page';
}

/** The settings the component hands over (`config`). */
export type Modifier = 'Alt' | 'Ctrl' | 'Shift';

export interface Config {
  enabled: boolean;
  modifier: Modifier;
  syncHighlight: boolean;
  showBoxModel: boolean;
}

export const DEFAULT_CONFIG: Config = {
  enabled: true,
  modifier: 'Alt',
  syncHighlight: true,
  showBoxModel: true,
};
