// The About page's facts (RB-010-18), part of the shell UI <-> core protocol (`protocol.ts`
// re-exports it): versions, engines, API versions and folders.

export interface AboutInfo {
  version: string;
  commit: string;
  cef: string;
  chromium: string;
  firefox?: string;
  webkit?: string;
  node?: string;
  /** The build time, UTC; absent when unknown. */
  built?: string;
  /** Operating system and architecture, `windows x86_64`. */
  platform?: string;
  firefoxPath?: string;
  /** API versions by name: `plugin`, `configSchema`, later `mcp`. */
  apis?: Record<string, string>;
  /** Folders by name: `data`, `local`, `logs`. */
  dirs?: Record<string, string>;
}
