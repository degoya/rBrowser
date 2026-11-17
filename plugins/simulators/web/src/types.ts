// The shapes the component and the page script share (src/catalog.rs, src/apply.rs).

/** One step of the root element's filter (`catalog::Step`). */
export type Step =
  | { op: 'matrix'; values: number[] }
  | { op: 'blur'; px: number }
  | { op: 'contrast' | 'brightness' | 'saturate'; amount: number };

/** The layer over the view (`catalog::Overlay`). */
export type Overlay =
  | { kind: 'tunnel'; clear: number; fade: number; opacity: number }
  | { kind: 'spot'; size: number; blurPx: number; opacity: number }
  | { kind: 'glare'; opacity: number };

/** How the page script draws a simulation (`catalog::Spec`). */
export interface Spec {
  filter: Step[];
  overlay: Overlay | null;
}

/** The page script's bridge (docs/plugins.md, "Page scripts"). */
export interface RbPlugin {
  readonly id: string;
  readonly view: number;
  send(message: unknown): Promise<unknown>;
  emit(message: unknown): void;
  on(listener: (message: unknown) => void): () => void;
  onStop(listener: () => void): () => void;
}
