// The load test panel (RB-096-07): built by Vite into ../../../panel/dist (panel.js,
// panel.css) and loaded by ../../../panel/index.html.
import { mountPanel } from '../../../../web-common/panel-ui/src/mount';
import App from './App.vue';
import './panel.css';

mountPanel(App, { prefix: 'plugin.load-test.', name: 'Load test panel' });
