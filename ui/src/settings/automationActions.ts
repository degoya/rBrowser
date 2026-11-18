// The commands beside a service's switch (RB-090-63), by the switch's key: in Settings >
// Automation the CDP proxy's endpoint, Playwright script and new token and the MCP server's
// Claude Code line; in Settings > General > Updates the update check and "Install and restart"
// (RB-090-14); in Settings > General > Start the setup wizard under its switch (RB-091-05).
// `AutomationActions.vue` shows them; `SectionCard.vue` places them under the switch.
import { SETTINGS_ICONS } from './icons';

export interface AutomationAction {
  /** The registry's command. */
  id: string;
  icon: string;
}

export const AUTOMATION_ACTIONS: Readonly<Record<string, readonly AutomationAction[]>> = {
  'cdp.enabled': [
    { id: 'cdp.copy_endpoint', icon: SETTINGS_ICONS.endpoint },
    { id: 'cdp.copy_snippet', icon: SETTINGS_ICONS.script },
    { id: 'cdp.rotate_token', icon: SETTINGS_ICONS.refresh },
  ],
  'mcp.enabled': [{ id: 'mcp.copy_config', icon: SETTINGS_ICONS.terminal }],
  'updates.enabled': [
    { id: 'update.check', icon: SETTINGS_ICONS.refresh },
    { id: 'update.install', icon: SETTINGS_ICONS.install },
  ],
  'ui.firstRunWizard': [{ id: 'app.setup_wizard', icon: SETTINGS_ICONS.wizard }],
};

/** The commands under the switch `key`; none for any other key. */
export function automationActions(key: string): readonly AutomationAction[] {
  return AUTOMATION_ACTIONS[key] ?? [];
}
