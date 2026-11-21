// The free size of the size input (RB-010-16): `500x800`, `500 x 800`, `500×800@2`, `500*800 @ 2.5x`.
// The same rules as the core's `breakpoint_keys::parse_free_size`, which checks again.

/** The command that opens the size input (`uiCommand`). */
export const OPEN_COMMAND = 'view.set_size';

/** The largest width or height a view takes, in CSS px (the config's limit for a breakpoint). */
export const MAX_SIDE = 10000;
/** The smallest device pixel ratio (the config's limit for a breakpoint). */
export const MIN_DPR = 0.25;
/** The largest device pixel ratio. */
export const MAX_DPR = 10;

export interface FreeSize {
  width: number;
  height: number;
  dpr?: number;
}

const SIDE = /^\d+$/;
const DPR = /^\d+(\.\d+)?$/;

function side(text: string): number | null {
  if (!SIDE.test(text)) {
    return null;
  }
  const value = Number(text);
  return value >= 1 && value <= MAX_SIDE ? value : null;
}

/** The size `text` names, or `null` when it names none. */
export function parseSize(text: string): FreeSize | null {
  const compact = text.replace(/\s+/g, '');
  const [size = '', dprText, extra] = compact.split('@');
  if (extra !== undefined) {
    return null;
  }
  const parts = size.split(/[xX*×]/);
  if (parts.length !== 2) {
    return null;
  }
  const width = side(parts[0] ?? '');
  const height = side(parts[1] ?? '');
  if (width === null || height === null) {
    return null;
  }
  if (dprText === undefined) {
    return { width, height };
  }
  const dprPlain = dprText.replace(/[xX]$/, '');
  if (!DPR.test(dprPlain)) {
    return null;
  }
  const dpr = Number(dprPlain);
  return dpr >= MIN_DPR && dpr <= MAX_DPR ? { width, height, dpr } : null;
}
