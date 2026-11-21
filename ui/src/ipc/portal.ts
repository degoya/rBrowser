// The portal as its panel shows it (RB-080-05), mirror of rb-ipc's `PortalState`; kept beside
// `protocol.ts`, which re-exports it. `getPortal` is answered with `{ type: 'portal', ...state }`
// (the open pairing included); `portalChanged` tells the panel to ask again. The presentation
// over the internet rides along (RB-096-05).

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

/** Where a presentation over the internet stands (RB-096-05). */
export type PresentationPhase = 'off' | 'downloading' | 'starting' | 'live';

/** The cloudflared a start would download first, for the consent. */
export interface PresentationDownload {
  version: string;
  sizeMb: string;
  license: string;
  from: string;
}

/** The presentation over the internet through a Cloudflare Quick Tunnel (RB-096-05). */
export interface PortalPresentation {
  /** `portal.presentation.enabled`: it may run. */
  enabled: boolean;
  /** `portal.presentation.consented`: the person agreed to the traffic through Cloudflare. */
  consented: boolean;
  phase: PresentationPhase;
  /** `https://<name>.trycloudflare.com` once the tunnel named it (`getPortal` only). */
  address?: string;
  viewers: number;
  maxViewers: number;
  startedAtMs?: number;
  /** The open link (it holds a one-time token) with its QR code (`getPortal` only). */
  link?: PortalPairing;
  /** What a start downloads first: cloudflared is missing. */
  download?: PresentationDownload;
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
  /** The presentation over the internet (RB-096-05). */
  presentation?: PortalPresentation;
}
