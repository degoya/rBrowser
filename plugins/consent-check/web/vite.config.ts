// The consent check panel's build (RB-094-05): Vue and Nuxt UI into ../panel/dist, see
// plugins/web-common/panel-ui/vite.panel.ts.
import { dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { panelConfig } from '../../web-common/panel-ui/vite.panel.ts';

export const PANEL = { id: 'consent-check', root: dirname(fileURLToPath(import.meta.url)) };

export default panelConfig(PANEL);
