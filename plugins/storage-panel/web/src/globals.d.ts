// The global the page script (../page/pins.js) runs with: the plugin's `rbPlugin` in a view's
// isolated world. The panel's shapes are in src/panel/model.ts (RB-080-84).

interface RbPluginApi {
  readonly id: string;
  readonly view: number;
  readonly maxMessageBytes: number;
  send(message: unknown): Promise<unknown>;
  emit(message: unknown): void;
  on(listener: (message: unknown) => void): () => void;
  onStop(listener: () => void): () => void;
}

declare var rbPlugin: RbPluginApi;
