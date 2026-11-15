// The part of react-devtools-inline's frontend the React panel uses (RB-080-24); the package
// ships no types. `createBridge(window, wall)` takes a custom wall, so the frontend runs over
// rBrowser's channel instead of `postMessage`.
declare module 'react-devtools-inline/frontend' {
  import type { ComponentType } from 'react';

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

  /** The frontend's end of the bridge. */
  export interface FrontendBridge {
    shutdown(): void;
  }

  /** The DevTools store over a bridge. */
  export interface Store {
    readonly roots: readonly number[];
  }

  /** The props the DevTools component takes (the ones the panel sets). */
  export interface DevToolsProps {
    browserTheme?: 'light' | 'dark';
    showTabBar?: boolean;
    warnIfLegacyBackendDetected?: boolean;
    warnIfUnsupportedVersionDetected?: boolean;
    readOnly?: boolean;
  }

  /** A bridge over `wall` (default: `postMessage` to `target`). */
  export function createBridge(target: Window, wall?: Wall): FrontendBridge;
  /** The store over `bridge`. */
  export function createStore(bridge: FrontendBridge): Store;
  /** The DevTools component over `bridge` and `store`. */
  export function initialize(
    target: Window,
    options?: { bridge?: FrontendBridge; store?: Store },
  ): ComponentType<DevToolsProps>;
}
