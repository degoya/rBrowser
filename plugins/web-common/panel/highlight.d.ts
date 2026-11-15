// The globals of `highlight-tokens.js` and `highlight.js` (RB-080-65), for the panels that load them.
type HighlightLang = 'json' | 'js' | 'html' | 'xml' | 'css' | 'http' | 'stack';

type HighlightKind =
  | 'plain'
  | 'key'
  | 'string'
  | 'number'
  | 'literal'
  | 'keyword'
  | 'comment'
  | 'punct'
  | 'tag'
  | 'attr'
  | 'fn'
  | 'meta';

interface HighlightToken {
  kind: HighlightKind;
  text: string;
}

/** What `render` did: coloured, switched off, no language, or too large to colour. */
type HighlightOutcome = 'on' | 'off' | 'plain' | 'large';

interface HighlightEditor {
  /** Colours the text again (the textarea's `value` setter and `input` do it themselves). */
  refresh(): void;
}

// A classic script's global is a `var` of `globalThis`.
declare var CodeTokens: {
  readonly LIMIT_BYTES: number;
  readonly LANGS: readonly HighlightLang[];
  tokenize(text: string, lang: HighlightLang): HighlightToken[];
  detect(text: string, hint?: string | null): HighlightLang | null;
  tooLarge(text: string): boolean;
};

declare var CodeHighlight: {
  readonly LIMIT_BYTES: number;
  readonly LANGS: readonly HighlightLang[];
  readonly SETTING: string;
  readonly enabled: boolean;
  tokenize(text: string, lang: HighlightLang): HighlightToken[];
  detect(text: string, hint?: string | null): HighlightLang | null;
  tooLarge(text: string): boolean;
  valueLang(type: string | undefined, text: string): HighlightLang | null;
  configure(settings: Record<string, unknown> | null | undefined): boolean;
  render(target: HTMLElement, text: string, lang: HighlightLang | null, note?: string): HighlightOutcome;
  editor(
    textarea: HTMLTextAreaElement,
    lang: HighlightLang | ((text: string) => HighlightLang | null),
    note?: () => string,
  ): HighlightEditor;
};
