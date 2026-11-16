// A project plugin's grants for the trust dialog, part of the shell UI <-> core protocol
// (`protocol.ts` re-exports it); the mirror of `rb_ipc::PluginGrantDetails`.

/** A project plugin's grants in detail, for the trust dialog (review M1 of Charge 6). */
export interface PluginGrantDetails {
  /** Runs scripts in the views: full control of the page, its network included. */
  evaluate: boolean;
  cdpDomains: string[];
  projectGlobs: string[];
  /** `project-write.globs`: the project files it may change; absent without it (RB-110-13). */
  writeGlobs?: string[];
  sidecarMethods: string[];
  /** Requests per second; absent without `http`. */
  httpRatePerSecond?: number;
  /**
   * What the dialog highlights: `evaluate`, `cdp:<Domain>`, `network-rules`, `console-logs`,
   * `sidecar`, `project-write`.
   */
  risky: string[];
}
