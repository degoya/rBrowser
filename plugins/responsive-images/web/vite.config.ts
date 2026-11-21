// The responsive images panel's build (RB-093-02): Vue and Nuxt UI into ../panel/dist, see
// plugins/web-common/panel-ui/vite.panel.ts. The collector stays esbuild's (scripts/bundle.mjs).
import { dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { panelConfig } from '../../web-common/panel-ui/vite.panel.ts';

export const PANEL = { id: 'responsive-images', root: dirname(fileURLToPath(import.meta.url)) };

export default panelConfig(PANEL);
