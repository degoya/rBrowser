// The texts and colours of a plugin row's badges (RB-080-51): every i18n key written out, so the
// key lint sees it, and a colour that says the same as the text.

import { type InstalledPlugin, type PluginSourceId, type PluginState, type SignatureState, switchable } from './installed';

type BadgeColor = 'neutral' | 'success' | 'warning' | 'error' | 'info';

export const SOURCE_KEYS: Record<PluginSourceId, string> = {
  bundled: 'settings.plugins.source.bundled',
  user: 'settings.plugins.source.user',
  project: 'settings.plugins.source.project',
};

export const STATE_KEYS: Record<PluginState, string> = {
  active: 'settings.plugins.state.active',
  disabled: 'settings.plugins.state.disabled',
  suspended: 'settings.plugins.state.suspended',
  blocked: 'settings.plugins.state.blocked',
  error: 'settings.plugins.state.error',
};

export const STATE_COLORS: Record<PluginState, BadgeColor> = {
  active: 'success',
  disabled: 'neutral',
  suspended: 'warning',
  blocked: 'warning',
  error: 'error',
};

const SIGNATURE_KEYS: Record<SignatureState, string> = {
  signed: 'settings.plugins.signature.signed',
  unsigned: 'settings.plugins.signature.unsigned',
  invalid: 'settings.plugins.signature.invalid',
  untrusted: 'settings.plugins.signature.untrusted',
  unknown: 'settings.plugins.signature.unknown',
};

export const SIGNATURE_COLORS: Record<SignatureState, BadgeColor> = {
  signed: 'success',
  unsigned: 'warning',
  invalid: 'error',
  untrusted: 'warning',
  unknown: 'neutral',
};

type Translate = (key: string, named?: Record<string, unknown>) => string;

/** "Signed by <key id>" for a verified signature whose key is named, else the state's text. */
export function signatureText(plugin: InstalledPlugin, t: Translate): string {
  const { state, keyId } = plugin.signature;
  if (state === 'signed' && keyId !== undefined) {
    return t('settings.plugins.signature.signed_by', { key: keyId });
  }
  return t(SIGNATURE_KEYS[state]);
}

/** What the next start changes for `plugin`, given the ids in `plugins.disabled`; `null` when
 * the list and the load agree, and for a row without a switch. */
export function restartKey(plugin: InstalledPlugin, disabled: readonly string[]): string | null {
  if (!switchable(plugin)) {
    return null;
  }
  const off = disabled.includes(plugin.id);
  if (plugin.state === 'disabled') {
    return off ? null : 'settings.plugins.restart_enable';
  }
  return off ? 'settings.plugins.restart_disable' : null;
}
