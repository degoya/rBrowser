// What a Nuxt UI panel imports (RB-080-83): mount it, reach its link, the host's events
// (RB-080-87), the status line's types, the texts' format, the message limit (RB-090-44),
// the frame components and the one tab row (RB-090-48).
export { mountPanel, usePanel, PANEL_KEY } from './mount';
export type { Panel, CallTarget, CallError } from './panel';
export { createPanel } from './panel';
export { onPanelEvent } from './events';
export type { StatusLine, Tone } from './status';
export { errorText, format } from './format';
export { answerBudget, messageLimit, MESSAGE_LIMIT_KEY } from './limit';
export { default as PanelFrame } from './components/PanelFrame.vue';
export { default as IconAction } from './components/IconAction.vue';
export { default as PanelTabs } from './components/PanelTabs.vue';
