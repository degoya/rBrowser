// The PWA panel (RB-080-08), on Nuxt UI since RB-080-85: built by Vite into
// ../../../panel/dist (panel.js, panel.css) and loaded by ../../../panel/index.html.
import { mountPanel } from '../../../../web-common/panel-ui/src/mount';
import App from './App.vue';
import './panel.css';

mountPanel(App, { prefix: 'plugin.pwa-check.', name: 'pwa panel' });
