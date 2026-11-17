// The project's lists the palette and the shortcut editor show (RB-060-11, RB-010-16), mirror of
// `crates/rb-ipc/src/lists.rs`; re-exported by `protocol.ts`.

/** A flow of the project, `.rbrowser/flows/<name>.ts` (RB-060-11). */
export interface FlowSummary {
  name: string;
  description?: string;
}

/** A project breakpoint shortcut (`breakpoints[].shortcut`, RB-010-16). */
export interface ProjectShortcut {
  breakpoint: string;
  binding: string;
  /** Why it is not in force (an i18n key); absent while it is. */
  reasonKey?: string;
  /** The command or breakpoint that holds the combination. */
  other?: string;
}
