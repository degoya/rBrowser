// The global of `texts.js` (RB-080-59), for the panels that load or bundle it.
interface PanelTextsOf {
  readonly ready: boolean;
  readonly strings: Record<string, string>;
  ask(): void;
  take(next: unknown): boolean;
  missing(key: string): void;
  full(key: string): string | undefined;
  text(key: string, params?: Record<string, unknown>): string;
  error(code: string, params?: Record<string, unknown>): string;
}

// A classic script's global is a `var` of `globalThis`.
declare var PanelTexts: {
  create(prefix: string, name: string): PanelTextsOf;
  format(template: string, params?: Record<string, unknown>): string;
  errorText(strings: Record<string, string>, code: string, params?: Record<string, unknown>): string;
};
