// The status bar's page (RB-080-15): mounted like the toolbar (`mountShell`: theme, i18n, Nuxt
// UI) with a client of its own for the message log.
import './assets/main.css';
import { h } from 'vue';
import { createTransport, mountShell } from './bootstrap';
import { IpcClient } from './ipc/client';
import StatusBarApp from './statusbar/StatusBarApp.vue';

const client = new IpcClient(createTransport());
mountShell({ render: () => h(StatusBarApp, { client }) });
