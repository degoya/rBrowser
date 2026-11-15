// The bridge the plugin runtime gives every page script (docs/plugins.md, "Page scripts and
// panels"): frozen, in the plugin's own isolated world.
interface RbPluginBridge {
  readonly id: string;
  readonly view: number;
  readonly maxMessageBytes: number;
  send(message: unknown): Promise<unknown>;
  emit(message: unknown): void;
  on(listener: (message: unknown) => void): () => void;
  onStop(listener: () => void): () => void;
}

declare const rbPlugin: RbPluginBridge;

// Vite's raw imports: the committed bundles in the freshness test.
declare module '*?raw' {
  const text: string;
  export default text;
}
