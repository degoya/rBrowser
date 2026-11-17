// What the collector reads from one view's page (src/collect.ts) - the JSON the component takes
// through `evaluate` and holds to its limits again (src/page.rs), since the page shapes all of it.

/** Where an element is, in CSS px of the viewport: x, y, width, height. */
export type Box = [number, number, number, number];

/** What every entry carries. */
export interface Located {
  /** A selector that finds the element again (agent/src/selector's CSS path). */
  selector: string;
  /** The element's path from the root, the findings' shared form (src/path.ts). */
  path: string;
  /** Not exposed at all: `display: none`, `visibility: hidden`, `hidden`, inside a closed
   * `<details>` or `inert`. */
  hidden: boolean;
  /** On the screen: not hidden, with a box of some size inside the page's horizontal extent. */
  visible: boolean;
  box: Box;
}

export interface Heading extends Located {
  level: number;
  text: string;
  /** The accessible name, the alts of its images included; '' when it has none. */
  name: string;
  /** The text's full length in characters (`text` is cut). */
  length: number;
  /** It holds an image without an alt attribute (and not marked decorative). */
  image: boolean;
}

export interface Landmark extends Located {
  /** `banner`, `navigation`, `main`, `complementary`, `contentinfo`, `region`, `search`, `form`. */
  role: string;
  tag: string;
  name: string;
}

export interface Link extends Located {
  /** The resolved URL; '' without `href`. */
  href: string;
  text: string;
  internal: boolean;
  /** `target=_blank` without `rel=noopener` or `noreferrer`. */
  blankUnsafe: boolean;
  nofollow: boolean;
}

export interface Image extends Located {
  src: string;
  /** null when the attribute is missing; '' marks the image decorative. */
  alt: string | null;
  /** `presentation` or `none` (decorative), or `aria-hidden`. */
  decorative: boolean;
  width: number;
  height: number;
  naturalWidth: number;
  naturalHeight: number;
  lazy: boolean;
}

export interface Field extends Located {
  tag: string;
  type: string;
  name: string;
  /** The accessible name; '' when it has none. */
  label: string;
  /** Where the name comes from: `label`, `aria-label`, `aria-labelledby`, `title`,
   * `placeholder` or `none`. */
  labelSource: string;
  autocomplete: string;
  required: boolean;
}

export interface Focusable extends Located {
  tag: string;
  role: string;
  name: string;
  tabindex: number;
  /** Fixed or sticky on the screen, itself or an ancestor: no place in the reading order. */
  fixed: boolean;
}

/** Visible content outside every landmark: its outermost element (axe's `region`). */
export interface Outside extends Located {
  text: string;
}

export interface CustomHit extends Located {
  text: string;
}

export interface CustomResult {
  label: string;
  selector: string;
  /** The selector does not parse. */
  invalid: boolean;
  count: number;
  hits: CustomHit[];
}

export interface PageOutline {
  url: string;
  lang: string;
  headings: Heading[];
  landmarks: Landmark[];
  /** Read with the landmarks. */
  outside: Outside[];
  links: Link[];
  images: Image[];
  fields: Field[];
  focus: Focusable[];
  custom: CustomResult[];
  /** The lists cut at MAX_ENTRIES, by name. */
  truncated: string[];
}

/** One custom selector of `outline.customSelectors`. */
export interface CustomSelector {
  label: string;
  selector: string;
}

/** What the component asks for. */
export interface Request {
  sections?: string[];
  custom?: CustomSelector[];
}
