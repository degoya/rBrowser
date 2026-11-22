// The TypeScript half of the Rust <-> TS contract test. `contract-fixtures.json` holds one
// message of every type in the exact JSON the Rust side (`crates/rb-ipc`) writes; here the same
// literals are typed against `protocol.ts`, so the type check of `npm run build` fails when the
// TS mirror drifts, and the test fails when the file and the typed copy differ. The Rust half
// parses every entry with `parse_request` / `serde_json::from_value::<CoreToShell>` and writes it
// back unchanged.

import { describe, expect, it } from 'vitest';
import fixtures from './contract-fixtures.json';
import { shellToCore } from './contractShellMessages';
import { coreToShell } from './contractCoreMessages';
import { RECORDING_CORE_TYPES, RECORDING_SHELL_TYPES } from './contractRecordingMessages';
import type { CoreToShell, ShellToCore } from './protocol';

/** Every message type once; a new variant in `protocol.ts` fails the type check here. */
const SHELL_TYPES: Record<ShellToCore['type'], true> = {
  getState: true,
  navigate: true,
  back: true,
  forward: true,
  reload: true,
  setSync: true,
  selectLayoutPreset: true,
  selectEnvironment: true,
  setActiveView: true,
  setBreakpoint: true,
  runCommand: true,
  toolbarRows: true,
  listCommands: true,
  openSettings: true,
  getSettings: true,
  setSetting: true,
  trustDecision: true,
  getAbout: true,
  setBindings: true,
  resetBinding: true,
  checkBinding: true,
  exportBindings: true,
  importBindings: true,
  getEmulation: true,
  getTrust: true,
  revokeTrust: true,
  pickPath: true,
  revealDiagnostics: true,
  getSettingSources: true,
  getDevices: true,
  analyzeBreakpoints: true,
  applyBreakpoints: true,
  runChecks: true,
  highlightFinding: true,
  highlightVital: true,
  dismissPalette: true,
  getFindings: true,
  listFlows: true,
  listProjectShortcuts: true,
  pluginPanel: true,
  getStatusMessages: true,
  reportStatus: true,
  dismissHistory: true,
  getPortal: true,
  extensionDecision: true,
  listWebStoreExtensions: true,
  canvasDrag: true,
  canvasPan: true,
  canvasStep: true,
  canvasSizePick: true,
  canvasEnginePick: true,
  canvasPiece: true,
  canvasMenu: true,
  addViewMenu: true,
  canvasZoom: true,
  canvasPainted: true,
  getChecksProgress: true,
  canvasZoomMenu: true,
  getTextScale: true,
  getVideos: true,
  deleteVideo: true,
  revealVideo: true,
  convertVideo: true,
  nativeMenu: true,
  ...RECORDING_SHELL_TYPES, activityBar: true, collapseActivity: true, // RB-090-67
  getSitemapGallery: true, // RB-090-86
  setup: true, // RB-091-05
  bugReport: true, // RB-093-04
  mcpSetup: true, // RB-091-09
  getBasicAuthStatus: true, // RB-094-29
};
const CORE_TYPES: Record<CoreToShell['type'], true> = {
  ack: true,
  state: true,
  metrics: true,
  pluginProgress: true, // RB-094-10
  settings: true,
  commands: true,
  notification: true,
  trustPrompt: true,
  about: true,
  uiCommand: true,
  canvasLive: true,
  whichKey: true,
  whichKeyHide: true,
  bindingCheck: true,
  bindingsExport: true,
  emulation: true,
  trust: true,
  pickedPath: true,
  diagnosticsSaved: true,
  settingSources: true,
  settingSourcesChanged: true,
  devices: true,
  breakpointProposal: true,
  settingsChanged: true,
  settingRejected: true,
  findings: true,
  flows: true,
  projectShortcuts: true,
  pluginPanel: true,
  statusMessage: true,
  statusMessages: true,
  portal: true,
  portalChanged: true,
  extensionPrompt: true,
  webStoreExtensions: true,
  checksProgress: true,
  textScale: true,
  videos: true,
  videosChanged: true,
  nativeMenuClosed: true,
  ...RECORDING_CORE_TYPES,
  sitemapGallery: true, // RB-090-86
  setup: true, // RB-091-05
  bugReport: true, // RB-093-04
  mcpSetup: true, // RB-091-09
  basicAuthStatus: true, // RB-094-29
};

describe('the IPC contract fixtures', () => {
  it('match the typed shell -> core messages', () => {
    expect(fixtures.shellToCore).toEqual(shellToCore);
  });

  it('match the typed core -> shell messages', () => {
    expect(fixtures.coreToShell).toEqual(coreToShell);
  });

  it('cover every message type exactly once', () => {
    expect(fixtures.shellToCore.map((m) => m.type).sort()).toEqual(Object.keys(SHELL_TYPES).sort());
    expect(fixtures.coreToShell.map((m) => m.type).sort()).toEqual(Object.keys(CORE_TYPES).sort());
  });
});
