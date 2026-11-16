// vue-i18n over the shared English catalogue (`messages.ts`). Phase 1 ships `en` only (RB-010-14
// adds the language setting and the lint); every visible string goes through `t()` with a key.

import { createI18n } from 'vue-i18n';
import catalogue from '@locales/en.json';
import { shellMessageCompiler } from './format';
import { mergeCatalogues, type Catalogue } from './messages';
import { attachPluginTexts } from './pluginTexts';

const parts: Record<string, unknown> = import.meta.glob('@locales/parts/*.json', {
  eager: true,
  import: 'default',
});

export function shellMessages(): Catalogue {
  return mergeCatalogues(catalogue, Object.values(parts));
}

export function createShellI18n() {
  const i18n = createI18n({
    legacy: false,
    locale: 'en',
    fallbackLocale: 'en',
    // A missing key shows as itself; the key lint (check-i18n.sh) is the guard (docs/i18n.md).
    missingWarn: false,
    fallbackWarn: false,
    messages: { en: shellMessages() },
    // rBrowser's message format (`format.ts`, docs/i18n.md), not vue-i18n's own syntax.
    messageCompiler: shellMessageCompiler,
  });
  // The plugins' texts the core sends join this catalogue (RB-080-82).
  attachPluginTexts((messages) => i18n.global.mergeLocaleMessage('en', messages));
  return i18n;
}
