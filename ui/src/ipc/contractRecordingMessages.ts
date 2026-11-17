// The Recorder and Flows panels' messages of the contract test's typed literals
// (`contract.test.ts`, RB-090-60), in a file of their own so the test stays under the
// file-length limit.

import type { Recording } from '../recorder/recording';
import type { CoreToShell, RecordingEvent, RecordingRequest, ShellToCore } from './protocol';

const RECORDING: Recording = {
  version: 1,
  name: 'login',
  breakpoints: [{ id: 'mobile', width: 393, height: 852, deviceScaleFactor: 2.75 }],
  startedAtMs: 1763380800000,
  appVersion: '0.9.0',
  steps: [
    { id: 1, action: { type: 'navigate', url: 'http://localhost:5173/' }, view: 'mobile', atMs: 0 },
    {
      id: 2,
      name: 'Wait for the menu',
      action: { type: 'waitFor', target: { selectors: [{ kind: 'css', path: '#menu' }] } },
      view: 'mobile',
      atMs: 400,
      onlyIn: ['mobile'],
    },
  ],
};

export const recordingShellMessages = [
  { type: 'getRecordings' },
  { type: 'getRecording', name: 'login' },
  { type: 'saveRecording', name: 'login', recording: RECORDING },
  { type: 'exportRecording', name: 'login', format: 'both', exportName: 'login-flow', write: true, overwrite: true },
  { type: 'getFlowRun' },
] satisfies ShellToCore[];

export const recordingCoreMessages = [
  {
    type: 'recordings',
    project: true,
    folder: '.rbrowser/out/recordings',
    recordings: [{ name: 'login', steps: 2, modifiedMs: 1763380900000 }],
    recordingNow: 'checkout',
  },
  { type: 'recording', name: 'login', recording: RECORDING },
  {
    type: 'recordingExport',
    written: true,
    folder: '.rbrowser/out/exports/login',
    files: [{ path: '.rbrowser/out/exports/login/login.ts', content: "await page.goto('/');\n" }],
  },
  { type: 'recordingsChanged' },
  {
    type: 'flowRun',
    project: true,
    running: {
      runId: '20261117-101500-a1b2c3',
      flow: 'login',
      views: [{ view: 1, status: 'running' }],
      startedAtMs: 1794910500000,
      queued: ['checkout'],
    },
    last: {
      version: 1,
      runId: '20261117-100000-d4e5f6',
      flow: 'login',
      file: '.rbrowser/flows/login.ts',
      startedAt: '2026-11-17T10:00:00Z',
      durationMs: 2400,
      passed: false,
      views: [
        {
          viewId: 1,
          label: 'Mobile',
          breakpoint: 'mobile',
          engine: 'chromium',
          status: 'failed',
          durationMs: 2300,
          steps: [{ name: 'log in', status: 'failed', durationMs: 2000 }],
          error: { message: 'expect failed', stack: '', step: 'log in', line: 7, column: 3, file: 'login.ts' },
          screenshot: 'mobile-1.png',
        },
      ],
    },
  },
  { type: 'flowRunChanged' },
] satisfies CoreToShell[];

/** Every request type of the panels once; a new one in `recordings.ts` fails the type check. */
export const RECORDING_SHELL_TYPES: Record<RecordingRequest['type'], true> = {
  getRecordings: true,
  getRecording: true,
  saveRecording: true,
  exportRecording: true,
  getFlowRun: true,
};

/** Every answer and event type of the panels once. */
export const RECORDING_CORE_TYPES: Record<RecordingEvent['type'], true> = {
  recordings: true,
  recording: true,
  recordingExport: true,
  recordingsChanged: true,
  flowRun: true,
  flowRunChanged: true,
};
