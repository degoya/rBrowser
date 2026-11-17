// The hello-panel panel's build (RB-080-87): Vue and Nuxt UI into ../panel/dist, see
// plugins/web-common/panel-ui/vite.panel.ts.
import { dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { panelConfig } from '../../web-common/panel-ui/vite.panel.ts';

export const PANEL = { id: 'hello-panel', root: dirname(fileURLToPath(import.meta.url)) };

export default panelConfig(PANEL);
