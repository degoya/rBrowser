// The page script's bridge as the runtime gives it (docs/plugins.md, "Page scripts"): a frozen
// global of the plugin's isolated world.
declare const rbPlugin: {
  readonly id: string;
  readonly view: number;
  readonly maxMessageBytes: number;
  send(message: unknown): Promise<any>;
  emit(message: unknown): void;
  on(listener: (message: any) => void): () => void;
  onStop(listener: () => void): () => void;
};
