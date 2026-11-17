// The part of react-devtools-inline's backend the React DevTools hook uses (RB-080-24); the
// package ships no types. `createBridge(window, wall)` takes a custom wall, so the bridge runs
// over rBrowser's channel instead of `postMessage`.
declare module 'react-devtools-inline/backend' {
  /** A message of the bridge's own protocol. */
  export interface WallMessage {
    event: string;
    payload?: unknown;
  }

  /** The transport a bridge runs over. */
  export interface Wall {
    listen(fn: (message: WallMessage) => void): () => void;
    send(event: string, payload: unknown, transferable?: unknown[]): void;
  }

  /** The backend's end of the bridge. */
  export interface BackendBridge {
    shutdown(): void;
  }

  /** Installs `__REACT_DEVTOOLS_GLOBAL_HOOK__` on `target`; before React loads. */
  export function initialize(target: Window): void;
  /** Starts the backend over `bridge` once the frontend listens. */
  export function activate(target: Window, options?: { bridge?: BackendBridge }): void;
  /** A bridge over `wall` (default: `postMessage` to the parent window). */
  export function createBridge(target: Window, wall?: Wall): BackendBridge;
}
