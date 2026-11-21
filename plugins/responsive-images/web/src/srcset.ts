// The srcset reader (RB-093-02): the candidates of a `srcset` attribute as the HTML standard's
// "parse a srcset attribute" reads them - a URL, then at most one `w` or `x` descriptor - with a
// candidate of a malformed descriptor dropped, as the browser drops it. Sizes descriptors in
// parentheses are not part of srcset; a `h` descriptor (future use) is skipped like the browser.

export interface Candidate {
  url: string;
  /** The `w` descriptor. */
  width?: number;
  /** The `x` descriptor. */
  density?: number;
}

const SPACE = /\s/;

/** One candidate's descriptors, or null when they are malformed. */
function describe(url: string, descriptors: string[]): Candidate | null {
  const candidate: Candidate = { url };
  for (const descriptor of descriptors) {
    const unit = descriptor.slice(-1);
    const value = descriptor.slice(0, -1);
    if (unit === 'w' && /^\d+$/.test(value) && candidate.width === undefined && candidate.density === undefined) {
      const width = Number(value);
      if (width <= 0) return null;
      candidate.width = width;
    } else if (unit === 'x' && /^\d*\.?\d+(?:e[+-]?\d+)?$/i.test(value) && candidate.width === undefined && candidate.density === undefined) {
      const density = Number(value);
      if (!(density > 0)) return null;
      candidate.density = density;
    } else if (unit === 'h' && /^\d+$/.test(value)) {
      continue;
    } else {
      return null;
    }
  }
  return candidate;
}

/** The descriptors after a URL, up to the comma that ends the candidate; returns them and the
 * position after that comma. */
function descriptorsFrom(text: string, start: number): [string[], number] {
  const descriptors: string[] = [];
  let current = '';
  let inParens = false;
  let at = start;
  for (; at < text.length; at += 1) {
    const char = text[at] as string;
    if (inParens) {
      current += char;
      if (char === ')') inParens = false;
    } else if (char === ',') {
      at += 1;
      break;
    } else if (SPACE.test(char)) {
      if (current) descriptors.push(current);
      current = '';
    } else {
      if (char === '(') inParens = true;
      current += char;
    }
  }
  if (current) descriptors.push(current);
  return [descriptors, at];
}

/** The candidates of `srcset`, malformed ones left out. */
export function parseSrcset(srcset: string): Candidate[] {
  const out: Candidate[] = [];
  let at = 0;
  while (at < srcset.length) {
    while (at < srcset.length && (SPACE.test(srcset[at] as string) || srcset[at] === ',')) at += 1;
    if (at >= srcset.length) break;
    let end = at;
    while (end < srcset.length && !SPACE.test(srcset[end] as string)) end += 1;
    let url = srcset.slice(at, end);
    let descriptors: string[] = [];
    if (url.endsWith(',')) {
      url = url.replace(/,+$/, '');
      at = end;
    } else {
      [descriptors, at] = descriptorsFrom(srcset, end);
    }
    if (!url) continue;
    const candidate = describe(url, descriptors);
    if (candidate) out.push(candidate);
  }
  return out;
}

/** Whether a candidate of `srcset` has a `w` descriptor (and so needs `sizes`). */
export function hasWidthDescriptors(srcset: string): boolean {
  return parseSrcset(srcset).some((candidate) => candidate.width !== undefined);
}

function resolve(url: string, base: string): string {
  try {
    return new URL(url, base).href;
  } catch {
    return url;
  }
}

/** The candidate of `srcset` whose URL, resolved against `base`, is `url`; null when none is. */
export function candidateOf(srcset: string, url: string, base: string): Candidate | null {
  return parseSrcset(srcset).find((candidate) => resolve(candidate.url, base) === url) ?? null;
}
