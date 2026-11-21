// The security header panel's build (RB-093-03): Vue and Nuxt UI into ../panel/dist, see
// plugins/web-common/panel-ui/vite.panel.ts.
import { dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { panelConfig } from '../../web-common/panel-ui/vite.panel.ts';

export const PANEL = { id: 'security-headers', root: dirname(fileURLToPath(import.meta.url)) };

export default panelConfig(PANEL);
