// What a Nuxt UI panel imports (RB-080-83): mount it, reach its link, the host's events
// (RB-080-87), the status line's types, the texts' format and the frame components.
export { mountPanel, usePanel, PANEL_KEY } from './mount';
export type { Panel, CallTarget, CallError } from './panel';
export { createPanel } from './panel';
export { onPanelEvent } from './events';
export type { StatusLine, Tone } from './status';
export { errorText, format } from './format';
export { default as PanelFrame } from './components/PanelFrame.vue';
export { default as IconAction } from './components/IconAction.vue';
