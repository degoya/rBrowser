// The container-queries panel's build (RB-094-01): Vue and Nuxt UI into ../panel/dist, see
// plugins/web-common/panel-ui/vite.panel.ts. The page script and the analysis stay esbuild's
// (scripts/bundle.mjs).
import { dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { panelConfig } from '../../web-common/panel-ui/vite.panel.ts';

export const PANEL = { id: 'container-queries', root: dirname(fileURLToPath(import.meta.url)) };

export default panelConfig(PANEL);
