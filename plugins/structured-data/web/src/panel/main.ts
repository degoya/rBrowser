// The structured data panel (RB-090-96): built by Vite into ../../../panel/dist (panel.js,
// panel.css) and loaded by ../../../panel/index.html.
import { mountPanel } from '../../../../web-common/panel-ui/src/mount';
import App from './App.vue';
import './panel.css';

mountPanel(App, { prefix: 'plugin.structured-data.', name: 'structured data panel' });
