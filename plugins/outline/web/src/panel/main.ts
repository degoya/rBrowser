// The Outline panel (RB-070-12), on Nuxt UI since RB-080-85: built by Vite into
// ../../../panel/dist (panel.js, panel.css) and loaded by ../../../panel/index.html.
import { mountPanel } from '../../../../web-common/panel-ui/src/mount';
import App from './App.vue';
import './panel.css';

mountPanel(App, { prefix: 'plugin.outline.', name: 'outline panel' });
