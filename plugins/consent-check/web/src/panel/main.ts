// The consent check panel (RB-094-05): built by Vite into ../../../panel/dist (panel.js,
// panel.css) and loaded by ../../../panel/index.html.
import { mountPanel } from '../../../../web-common/panel-ui/src/mount';
import App from './App.vue';
import './panel.css';

mountPanel(App, { prefix: 'plugin.consent-check.', name: 'consent check panel' });
