// The rulers-guides panel (RB-060-05), on Nuxt UI since RB-080-86: built by Vite into
// ../../../panel/dist (panel.js, panel.css) and loaded by ../../../panel/index.html.
import { mountPanel } from '../../../../web-common/panel-ui/src/mount';
import App from './App.vue';
import './panel.css';

mountPanel(App, { prefix: 'plugin.rulers-guides.', name: 'rulers-guides panel' });
