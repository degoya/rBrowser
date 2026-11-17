// The payloads of the commands and of workspace trust in the shell <-> core protocol - the
// TypeScript mirror of rb-ipc `payloads.rs` (the command list, the which-key overlay, a
// binding's check; the trust prompt and status). Re-exported by `protocol.ts`; types only.

import type { BlockedItem, Group } from './protocol';
import type { PluginGrantDetails } from './trustGrants';

export interface CommandInfo {
  id: string;
  titleKey: string;
  group: Group;
  /** Text form: `Ctrl+Shift+1`, `Ctrl+Shift+Space L 1`. */
  bindings: string[];
  /** What "reset" restores. */
  defaultBindings: string[];
  /** The user changed the bindings. */
  changed: boolean;
  mcp: McpCoverage;
  /** Default bindings left unbound, reserved or taken (rb-commands `NOTICE_BINDING_DROPPED`). */
  droppedDefaults?: string[];
}

/** rb-commands `McpCoverage`: the MCP tool, or why there is none. */
export type McpCoverage = { covered: string } | { omitted: string };

/** rb-commands `WhichKeyEntry`: a next key after the leader. */
export type WhichKeyEntry =
  | { kind: 'group'; key: string; group: Group; titleKey: string }
  | { kind: 'command'; key: string; id: string; titleKey: string };

/** rb-commands `WhichKey`: what was typed and what each next key does. */
export interface WhichKey {
  typed: string;
  entries: WhichKeyEntry[];
}

export interface BindingCheck {
  ok: boolean;
  /** i18n key of the reason (`shortcuts.reserved.*`, `shortcuts.rejected.*`). */
  reasonKey?: string;
  /** The command holding the binding. */
  conflict?: string;
}

export interface TrustPrompt {
  promptId: number;
  projectRoot: string;
  blocked: BlockedItem[];
}

export interface TrustStatus {
  projectRoot: string;
  trusted: boolean;
  blocked: BlockedItem[];
  /** The capability ids each project plugin package declares, by its blocked item's path. */
  pluginCapabilities?: Record<string, string[]>;
  /** What each project plugin package asks for in detail, by the same path. */
  pluginGrants?: Record<string, PluginGrantDetails>;
}

export interface TrustInfo {
  current: TrustStatus | null;
  projects: { root: string; decidedAt: string }[];
}
