// The canvas board's page (`canvas-board.html`, RB-080-04): the labels and drags behind the
// canvas's views. It may send only the board's own requests (the core's page guard).
import './assets/main.css';
import { h } from 'vue';
import CanvasBoard from './canvas/CanvasBoard.vue';
import { mountShell } from './bootstrap';

mountShell({ render: () => h(CanvasBoard) });
