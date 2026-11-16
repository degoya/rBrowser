import './assets/main.css';
import { h } from 'vue';
import CellLabelApp from './cells/CellLabelApp.vue';
import { mountShell } from './bootstrap';

const viewId = Number(new URLSearchParams(window.location.search).get('view') ?? '0');
mountShell({ render: () => h(CellLabelApp, { viewId }) });
