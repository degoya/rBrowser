// The collector's answer (RB-093-02): what one view's page says about each of its images, and
// what it means at the view's pixel density. src/page.rs reads the same shape.

export interface Size {
  width: number;
  height: number;
}

/** The `<source>` of a `<picture>` the browser took the image from. */
export interface ChosenSource {
  media: string;
  type: string;
  srcset: string;
  sizes: string;
}

/** How the file's own pixels were found: decoded again (`probe`), from the srcset candidate's
 * descriptor (`descriptor`), or the intrinsic size as it is (`natural`). */
export type FileFrom = 'probe' | 'descriptor' | 'natural';

/** What the page says about one `<img>`. */
export interface ImageFacts {
  /** A CSS selector of the element, for the mark (the sync agent's `cssPath`). */
  selector: string;
  /** Its node path as Lighthouse writes it (`1,HTML,1,BODY,3,IMG`), so a finding with a shared
   * id merges with Lighthouse's at the same element. */
  path: string;
  src: string;
  currentSrc: string;
  srcset: string;
  sizes: string;
  /** In a `<picture>`. */
  picture: boolean;
  /** The `<source>` the image came from; null for the `<img>`'s own `src`/`srcset`. */
  source: ChosenSource | null;
  /** A `<source>` of the picture has `w` descriptors and no `sizes`. */
  sourceWithoutSizes: boolean;
  loading: string;
  fetchpriority: string;
  /** Rendered size in CSS pixels (the content box). */
  rendered: Size;
  /** `naturalWidth`/`naturalHeight`: the intrinsic size, density-corrected by the browser. */
  natural: Size;
  /** The file's own pixels; null while it is not loaded, or for a vector image. */
  file: Size | null;
  fileFrom: FileFrom | null;
  /** An SVG: any size is sharp. */
  vector: boolean;
  /** Loaded and decoded (`complete` with a natural size). */
  loaded: boolean;
  /** Takes room on the page (not `display: none`, not zero-sized). */
  visible: boolean;
  /** The page's Largest Contentful Paint element. */
  lcp: boolean;
}

export type Rule = 'oversized' | 'undersized' | 'sizes-missing' | 'lcp';

/** One problem of one image; the message is `findings.<rule>` or `findings.<rule>.<variant>`. */
export interface Issue {
  rule: Rule;
  variant?: string;
  params: Record<string, string>;
}

/** One image measured at the view's density. */
export interface Measured extends ImageFacts {
  /** Rendered size times the density, in device pixels, rounded; null when it is not measured. */
  needed: Size | null;
  /** The file's pixels over the needed ones on the tighter axis, two decimals; null when not
   * measured. */
  ratio: number | null;
  issues: Issue[];
}

/** One view's images. */
export interface PageImages {
  url: string;
  /** The density the images were measured at. */
  dpr: number;
  viewport: Size;
  images: Measured[];
  /** How many images the page has beyond those listed. */
  more: number;
  /** Whether the page reported its Largest Contentful Paint element. */
  lcpKnown: boolean;
}

/** What the component asks for (src/page.rs). */
export interface Request {
  /** The view's density; the page's `devicePixelRatio` when missing or not positive. */
  dpr?: number;
  oversizeFactor?: number;
  undersizeFactor?: number;
  checkLcp?: boolean;
}
