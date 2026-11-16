// The panel's build: Vue, the Nuxt UI components it uses and the Tailwind classes it names into
// ../panel/dist (panel.js, panel.css, THIRD-PARTY-LICENSES.txt), see
// web-common/panel-ui/vite.panel.ts. Put your plugin's id and licence here.
import { dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { panelConfig } from './web-common/panel-ui/vite.panel.ts';

export const PANEL = { id: 'example-feature', root: dirname(fileURLToPath(import.meta.url)), license: 'MIT', source: 'web' };

export default panelConfig(PANEL);
