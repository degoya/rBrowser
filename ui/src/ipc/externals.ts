// The external views of the state snapshot (RB-040-01): rb-engine `external::{ExternalStatus,
// WindowRect, ExternalView}`, re-exported by `protocol.ts`.
import type { EngineKind, ViewId } from './protocol';

/** rb-engine `ExternalStatus`: where an external view stands (RB-040-01). */
export type ExternalStatus =
  | 'starting'
  | 'running'
  | 'exited'
  | 'closed'
  | 'restarting'
  | 'failed'
  | 'unavailable';

/** rb-engine `WindowRect`: an external window in physical screen pixels. */
export interface WindowRect {
  x: number;
  y: number;
  width: number;
  height: number;
}

/** rb-engine `ExternalView`: a view on Firefox or WebKit (RB-040-01). */
export interface ExternalView {
  view: ViewId;
  label: string;
  breakpoint: string;
  engine: EngineKind;
  status: ExternalStatus;
  /** Why it is not running, for the user. */
  reason?: string;
  /** The last committed URL: what a restart loads again. */
  url: string;
  window?: WindowRect;
  /** The emulation fields the engine does not apply as asked. */
  unsupported?: string[];
  /** The window sits in its grid cell (`engines.external.embed`, RB-080-39). */
  embedded?: boolean;
}
