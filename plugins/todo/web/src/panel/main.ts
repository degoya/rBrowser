// The todo panel (RB-110-13), on Nuxt UI: built by Vite into ../../../panel/dist (panel.js,
// panel.css) and loaded by ../../../panel/index.html.
import { mountPanel } from '../../../../web-common/panel-ui/src/mount';
import App from './App.vue';
import './panel.css';

mountPanel(App, { prefix: 'plugin.todo.', name: 'todo panel' });
