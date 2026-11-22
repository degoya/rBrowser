// What a Nuxt UI panel imports (RB-080-83): mount it, reach its link, the host's events
// (RB-080-87), the status line's types, the texts' format, the message limit (RB-090-44),
// the frame components, the one tab row (RB-090-48), the settings link (RB-094-21) and the
// form that the sandbox lets answer (RB-096-13).
export { mountPanel, usePanel, PANEL_KEY } from './mount';
export type { Panel, CallTarget, CallError } from './panel';
export { createPanel, NOT_SENT, plain } from './panel';
export { onPanelEvent } from './events';
export type { StatusLine, Tone } from './status';
export { errorText, format } from './format';
export { answerBudget, messageLimit, MESSAGE_LIMIT_KEY } from './limit';
export { default as PanelFrame } from './components/PanelFrame.vue';
export { default as IconAction } from './components/IconAction.vue';
export { default as PanelTabs } from './components/PanelTabs.vue';
export { default as SettingLink } from './components/SettingLink.vue';
export { default as PanelForm } from './components/PanelForm.vue';
