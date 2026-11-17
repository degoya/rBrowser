// What the page part of the Fonts plugin (RB-070-13) answers the component: one view's fonts as
// `src/page.rs` parses them. Every list is capped here and again in the component.

/** A box in CSS px: x, y, width, height. */
export type Box = [number, number, number, number];

/** Where an element is: the selector the marks and the findings panel use, and the element's
 * full path from the root (the findings' stable id is made of it). */
export interface Located {
  selector: string;
  path: string;
}

/** One entry of an `@font-face` `src` list. */
export interface Src {
  /** The absolute URL, or the font name of a `local()`. */
  url: string;
  /** `woff2`, `woff`, `truetype`, `opentype`, `embedded-opentype`, `svg` or empty. */
  format: string;
  local: boolean;
}

/** One `@font-face` (readable rule, or a `document.fonts` entry of a sheet the page may not
 * read: a cross-origin stylesheet such as Google Fonts' CSS, or a `FontFace` made by script). */
export interface Face {
  family: string;
  weight: string;
  style: string;
  /** `font-display`; `auto` when the rule names none. */
  display: string;
  unicodeRange: string;
  /** `document.fonts`: `unloaded` (never needed), `loading`, `loaded`, `error`; `unknown` when the
   * browser does not tell. */
  status: string;
  src: Src[];
  /** The stylesheet's URL; `inline` for a `<style>`; empty when unknown. */
  sheet: string;
  /** Read from the rule itself (its `src` is known). */
  readable: boolean;
  /** A variable font: a weight range or a `*-variations` format. */
  variable: boolean;
}

/** A font file the page loaded (Resource Timing): its sizes in bytes, 0 when the server does not
 * allow them to be read (a cross-origin answer without `Timing-Allow-Origin`). */
export interface Resource {
  url: string;
  size: number;
  decoded: number;
}

/** An element whose text is set in a usage, for the rendered-font check. */
export interface Sample extends Located {
  text: string;
  /** The distinct characters beyond ASCII of its text. */
  nonAscii: string;
}

/** The text set in one `font-family` stack, weight and style. */
export interface Usage {
  stack: string;
  /** The stack's first family, unquoted. */
  family: string;
  weight: number;
  style: string;
  count: number;
  /** Some of it in the first screen. */
  aboveFold: boolean;
  /** The distinct font sizes, in CSS px. */
  sizes: number[];
  samples: Sample[];
}

/** The font sizes of one text role (`h1`...`h6`, `p`, `li`, `a`, `button`, `label`, `input`,
 * `text`). */
export interface Role {
  role: string;
  count: number;
  min: number;
  max: number;
  first: Located;
}

/** An element a readability rule may report. */
export interface Candidate extends Located {
  tag: string;
  size: number;
  /** Line height divided by font size. */
  lineHeight: number;
  text: string;
}

export interface PageFonts {
  url: string;
  viewport: { width: number; height: number };
  faces: Face[];
  /** Stylesheets the page may not read (cross-origin). */
  sheets: string[];
  /** `<link rel="preload" as="font">` URLs. */
  preloads: string[];
  resources: Resource[];
  usages: Usage[];
  roles: Role[];
  smallText: Candidate[];
  smallInputs: Candidate[];
  tightLines: Candidate[];
  /** The text elements read. */
  elements: number;
  /** A list or the walk was cut. */
  truncated: boolean;
}

/** The thresholds the candidates are chosen by (the component checks them again). */
export interface Request {
  minTextPx?: number;
  minInputPx?: number;
  minLineHeight?: number;
}

/** Which text to mark: a family, optionally one weight and style. */
export interface MarkRequest {
  family?: string;
  weight?: number;
  style?: string;
}
