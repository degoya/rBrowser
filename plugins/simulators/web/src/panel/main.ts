// The simulators picker (RB-070-08), on Nuxt UI since RB-080-86: built by Vite into
// ../../../panel/dist (panel.js, panel.css) and loaded by ../../../panel/index.html.
import { mountPanel } from '../../../../web-common/panel-ui/src/mount';
import App from './App.vue';
import { PREFIX } from './model';
import './panel.css';

mountPanel(App, { prefix: PREFIX, name: 'simulators panel' });
