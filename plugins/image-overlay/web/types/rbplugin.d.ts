// The page script's bridge as the runtime gives it (docs/plugins.md, "Page scripts"): a frozen
// global of the plugin's isolated world.
declare const rbPlugin: {
  readonly id: string;
  readonly view: number;
  readonly maxMessageBytes: number;
  /** rBrowser's one host of page overlays `<rbrowser-layers>` (RB-090-51). */
  readonly layers: { add(host: Element): boolean; remove(host: Element): void };
  send(message: unknown): Promise<any>;
  emit(message: unknown): void;
  on(listener: (message: any) => void): () => void;
  onStop(listener: () => void): () => void;
};
