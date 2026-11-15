// The portal as its panel shows it (RB-080-05), mirror of rb-ipc's `PortalState`; kept beside
// `protocol.ts`, which re-exports it. `getPortal` is answered with `{ type: 'portal', ...state }`
// (the open pairing included); `portalChanged` tells the panel to ask again.

/** A paired device. */
export interface PortalDevice {
  id: number;
  /** The user agent, shortened; empty when it sent none. */
  name: string;
  ip: string;
  pairedAtMs: number;
  lastSeenMs: number;
  /** It came back with its cookie at least once; `false` is "pairing not completed". */
  contacted: boolean;
  /** Its agent is connected now. */
  connected: boolean;
  replayed: number;
  failed: number;
  /** Events not sent: a script on the device cannot replay them, or they held a password. */
  skipped: number;
}

/** The open pairing: the URL in the QR code (it holds a one-time token) and its modules. */
export interface PortalPairing {
  url: string;
  size: number;
  /** `size` rows of `0` (light) and `1` (dark). */
  rows: string[];
  expiresAtMs: number;
}

/** The portal's state. */
export interface PortalState {
  /** `portal.enabled`: the portal may run. */
  enabled: boolean;
  running: boolean;
  /** `http://ip:port` while it listens. */
  address?: string;
  devices: PortalDevice[];
  pairing?: PortalPairing;
}
