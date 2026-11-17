// The Recorder panel's channel on the shell's IPC (RB-090-60): the stored recordings, one of
// them, the edited one written back, its export (a preview, or written into a new export folder)
// and the commands of the registry it runs (`recorder.start`, ...); the core's
// `recordingsChanged` (a recording saved or stopped) asks for the list again.

import type { IpcClient } from '../ipc/client';
import type { CoreToShell, JsonValue, RecordingExport, RecordingExportFormat, RecordingList } from '../ipc/protocol';
import type { Subscribe } from '../whichkey/whichkey';
import type { Recording } from './recording';

export interface RecorderChannel {
  list(): Promise<RecordingList>;
  get(name: string): Promise<Recording>;
  save(name: string, recording: Recording): Promise<RecordingList>;
  /** `overwrite`: replace a flow of the same name, the person said so (RB-090-66). */
  render(
    name: string,
    format: RecordingExportFormat,
    exportName: string,
    write: boolean,
    overwrite?: boolean,
  ): Promise<RecordingExport>;
  /** Runs the registry's command `id`. */
  run(id: string, args?: JsonValue): Promise<void>;
  /** Calls `listener` when the recordings changed; returns the function that stops. */
  onChanged(listener: () => void): () => void;
}

function listOf(answer: Extract<CoreToShell, { type: 'recordings' }>): RecordingList {
  const { type: _type, ...list } = answer;
  return list;
}

export function recorderChannel(client: IpcClient, subscribe: Subscribe): RecorderChannel {
  return {
    async list() {
      return listOf(await client.expect({ type: 'getRecordings' }, 'recordings'));
    },
    async get(name) {
      return (await client.expect({ type: 'getRecording', name }, 'recording')).recording;
    },
    async save(name, recording) {
      return listOf(await client.expect({ type: 'saveRecording', name, recording }, 'recordings'));
    },
    async render(name, format, exportName, write, overwrite = false) {
      const message = exportName.trim() === '' ? {} : { exportName: exportName.trim() };
      const replace = overwrite ? { overwrite: true } : {};
      const { type: _type, ...files } = await client.expect(
        { type: 'exportRecording', name, format, write, ...message, ...replace },
        'recordingExport',
      );
      return files;
    },
    async run(id, args) {
      await client.request(args === undefined ? { type: 'runCommand', id } : { type: 'runCommand', id, args });
    },
    onChanged(listener) {
      return subscribe((message: CoreToShell) => {
        if (message.type === 'recordingsChanged') {
          listener();
        }
      });
    },
  };
}
