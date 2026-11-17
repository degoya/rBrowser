// The answer of `pluginPanel { op: 'installed' }` for the tests: two bundled plugins that run,
// one switched off, a user-installed package refused for its signature (named by its file, what
// it claims marked as unverified - here the id of the bundled SEO plugin) and an unsigned project
// plugin. Test-only; nothing of the app imports it.

import type { InstalledMessage } from '../../panels/protocol';

export const SAMPLE_INSTALLED: InstalledMessage = {
  loading: false,
  folders: { bundled: 'C:\\Tools\\rbrowser\\plugins', user: 'C:\\Users\\dev\\AppData\\Roaming\\rBrowser\\plugins' },
  missing: {},
  plugins: [
    {
      id: 'seo',
      verified: true,
      name: 'SEO',
      version: '0.1.0',
      description: '<b>Checks</b> a page for the SEO essentials.',
      source: 'bundled',
      signature: { state: 'signed', keyId: 'rbrowser-plugin-v1' },
      state: 'active',
      commands: 3,
      kinds: ['check-plugin', 'feature-plugin'],
    },
    {
      id: 'overlays',
      verified: true,
      name: 'Overlays',
      version: '0.3.0',
      description: 'Grid, flex and overflow overlays.',
      source: 'bundled',
      signature: { state: 'signed', keyId: 'rbrowser-plugin-v1' },
      state: 'active',
      commands: 0,
    },
    {
      id: 'snippets',
      verified: true,
      name: 'Snippets',
      version: '0.1.0',
      description: 'Run your own scripts in the views.',
      source: 'bundled',
      signature: { state: 'signed', keyId: 'rbrowser-plugin-v1' },
      state: 'disabled',
      commands: 2,
    },
    {
      file: 'broken-2.0.0.rbplugin',
      verified: false,
      declared: { name: '<img src=x onerror=alert(1)>', version: '2.0.0', claimsIdOf: 'SEO' },
      source: 'user',
      signature: { state: 'invalid' },
      state: 'error',
      error: {
        code: 'plugins.error.signature_invalid',
        message: 'The plugin package was changed after it was signed and is not loaded.',
      },
      commands: 0,
    },
    {
      id: 'acme',
      verified: true,
      name: 'Acme tools',
      version: '1.2.3',
      description: '',
      source: 'project',
      signature: { state: 'unsigned' },
      state: 'active',
      commands: 1,
    },
  ],
};
