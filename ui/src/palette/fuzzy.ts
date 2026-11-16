// The palette's fuzzy match (RB-060-11): the needle's letters in order anywhere in the text, the
// best alignment scored - a letter at a word start (after a space, `.`, `_`, `-`, `/`, `(` or at a
// camelCase hump) and a run of consecutive letters count more, a gap between letters costs.
// One pass per needle letter over the text (O(needle x text)), so a few hundred entries stay
// well under a frame per keystroke.

const WORD_START = 10;
const LETTER = 1;
const CONSECUTIVE = 6;
const GAP = 1;
const LEADING = 0.5;
const LEADING_MAX = 5;
const SEPARATORS = new Set([' ', '.', '_', '-', '/', ':', '(', '>', ',']);

function isUpper(char: string): boolean {
  return char !== char.toLowerCase() && char === char.toUpperCase();
}

/** What a needle letter matched at `index` of `text` is worth by its place. */
function placeBonus(text: string, index: number): number {
  if (index === 0) {
    return WORD_START;
  }
  const before = text.charAt(index - 1);
  const here = text.charAt(index);
  if (SEPARATORS.has(before) || (isUpper(here) && !isUpper(before))) {
    return WORD_START;
  }
  return LETTER;
}

/**
 * The score of `needle` in `text` (higher is better), or `null` when the needle's letters do not
 * all appear in order. Case does not matter; the empty needle scores 0.
 */
export function fuzzyScore(needle: string, text: string): number | null {
  const lowNeedle = needle.toLowerCase();
  const lowText = text.toLowerCase();
  const n = lowNeedle.length;
  const m = lowText.length;
  if (n === 0) {
    return 0;
  }
  if (n > m) {
    return null;
  }
  let previous: number[] = new Array<number>(m).fill(-Infinity);
  for (let i = 0; i < n; i += 1) {
    const current = new Array<number>(m).fill(-Infinity);
    const letter = lowNeedle.charAt(i);
    // The best previous match at least one letter back, less the gap to `j`.
    let gapRun = -Infinity;
    for (let j = 0; j < m; j += 1) {
      if (j >= 2) {
        gapRun = Math.max(gapRun - GAP, (previous[j - 2] ?? -Infinity) - GAP);
      }
      if (lowText.charAt(j) !== letter) {
        continue;
      }
      const bonus = placeBonus(text, j);
      if (i === 0) {
        current[j] = bonus - Math.min(j * LEADING, LEADING_MAX);
      } else {
        const consecutive = j >= 1 ? (previous[j - 1] ?? -Infinity) + CONSECUTIVE : -Infinity;
        const best = Math.max(consecutive, gapRun);
        if (best > -Infinity) {
          current[j] = best + bonus;
        }
      }
    }
    previous = current;
  }
  const score = Math.max(...previous);
  return score === -Infinity ? null : score;
}
