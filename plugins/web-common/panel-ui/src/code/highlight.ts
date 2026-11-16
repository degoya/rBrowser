/// <reference path="../../../panel/highlight.d.ts" />
// The panels' syntax highlighter (RB-080-65) in a Nuxt UI panel (RB-080-84): the one source,
// plugins/web-common/panel/highlight-tokens.js and highlight.js, bundled as the classic scripts
// they are (each defines its global), and its colours for the app theme. highlight.js switches
// its dark colours on the system's scheme only; a panel follows `ui.theme` through the `dark`
// class of the root (../theme.ts), so highlight.css sets them by that class too.
import '../../../panel/highlight-tokens.js';
import '../../../panel/highlight.js';
import './highlight.css';

/** The highlighter, `CodeHighlight` of highlight.js. */
export const highlighter = (): typeof CodeHighlight => CodeHighlight;

/** A token of a text cut by its colour and by a mark of its own (a used part, a match). */
export interface MarkedToken {
  text: string;
  /** The token's kind, `plain` when not coloured. */
  kind: HighlightKind;
  marked: boolean;
}

/**
 * `text` cut into tokens of `lang` (one plain token when highlighting is off, there is no
 * language or the text is too large) and again at the edges of `marks` - ranges of `[start,
 * end)` in the text's own offsets, sorted and not overlapping. Joined, the pieces are the text.
 */
export function markedTokens(text: string, lang: HighlightLang | null, marks: readonly [number, number][]): MarkedToken[] {
  const hl = highlighter();
  const coloured = hl.enabled && lang !== null && !hl.tooLarge(text);
  const tokens: HighlightToken[] = coloured ? hl.tokenize(text, lang) : [{ kind: 'plain', text }];
  const out: MarkedToken[] = [];
  let at = 0;
  let mark = 0;
  for (const token of tokens) {
    let start = at;
    const end = at + token.text.length;
    while (start < end) {
      while (mark < marks.length && (marks[mark]?.[1] ?? 0) <= start) mark += 1;
      const range = marks[mark];
      const inside = range !== undefined && range[0] <= start;
      const stop = Math.min(end, range === undefined ? end : inside ? range[1] : range[0]);
      const piece = text.slice(start, stop);
      const kind = token.kind === 'punct' ? 'plain' : token.kind;
      const last = out[out.length - 1];
      if (last && last.kind === kind && last.marked === inside) last.text += piece;
      else out.push({ text: piece, kind, marked: inside });
      start = stop;
    }
    at = end;
  }
  return out.filter((piece) => piece.text.length > 0);
}
