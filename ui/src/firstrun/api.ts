// What the setup wizard asks of the core (RB-091-05), in one place: its own `setup` messages
// (a preview, write or basics carries a number of its own; it is acknowledged, and the `setup`
// event with that number is its answer - an answer to another request never is), the folder
// dialog, the device presets, the theme command, and RB-091-04's plugin requests over the
// `pluginPanel` channel (`available`, `installChosen` with its `pluginInstallProgress` events,
// `removeChosen`, `applyRecommended`) - agreed with RB-091-04 in the wave; a rename is this file
// only.

import type { IpcClient } from '../ipc/client';
import type { DevicePreset, SetupAnswer, SetupLayout, SetupRequest } from '../ipc/protocol';
import type { PanelRequest } from '../panels/protocol';
import { THEME_SET_COMMAND } from '../toolbar/themeToggle';
import type { ThemeSetting } from '../theme/theme';
import { type Available, type InstallOutcome, type InstallProgress, parseAvailable, parseOutcome, parseProgress } from './plugins';

type Answer<Op extends SetupAnswer['op']> = Extract<SetupAnswer, { op: Op }>;
type Failed = Answer<'failed'>;
/** A preview as the project step asks for it; the number is this file's. */
type PreviewRequest = Omit<Extract<SetupRequest, { op: 'preview' }>, 'requestId'>;
/** A step that answers by event, without its number yet. */
type Step =
  | PreviewRequest
  | Omit<Extract<SetupRequest, { op: 'write' }>, 'requestId'>
  | Omit<Extract<SetupRequest, { op: 'basics' }>, 'requestId'>;

/** RB-091-04's requests of the plugin step; its own `PanelRequest` union names them after the
 * wave's merge, until then they are cast here once. */
type PluginStepRequest =
  | { op: 'available' }
  | { op: 'installChosen'; ids: string[] }
  | { op: 'removeChosen'; ids: string[] }
  | { op: 'applyRecommended' };

export interface SetupApi {
  info(): Promise<Answer<'info'>>;
  preview(request: PreviewRequest): Promise<Answer<'proposal'> | Failed>;
  write(proposalId: number): Promise<Answer<'written'> | Failed>;
  basics(layout?: SetupLayout, sync?: boolean): Promise<Answer<'applied'> | Failed>;
  close(): Promise<void>;
  /** The native folder dialog (a WSL folder as `\\wsl.localhost\...`); `null` when cancelled. */
  pickFolder(): Promise<string | null>;
  devices(): Promise<DevicePreset[]>;
  setTheme(theme: ThemeSetting): Promise<void>;
  runCommand(id: string, args?: Record<string, string>): Promise<void>;
  available(): Promise<Available>;
  installChosen(ids: string[], onProgress: (progress: InstallProgress) => void): Promise<InstallOutcome>;
  /** Takes installed plugins out of the choice (they stay out at later starts). */
  removeChosen(ids: string[]): Promise<InstallOutcome>;
  /** The wizard's plugin step was skipped while the first choice is pending: the core installs
   * its recommended set (`{ value: { chosen, restart } }`). */
  applyRecommended(): Promise<void>;
}

export function createSetupApi(client: IpcClient): SetupApi {
  let nextId = 0;

  /** Sends `step` with a new number and resolves with the `setup` event that carries it. */
  function untilEvent<Op extends SetupAnswer['op']>(step: Step): Promise<Answer<Op> | Failed> {
    nextId += 1;
    const requestId = nextId;
    const request = { ...step, requestId } as SetupRequest;
    return new Promise((resolve, reject) => {
      const stop = client.on((message) => {
        if (message.type !== 'setup') return;
        const answer = message.answer;
        if ('requestId' in answer && answer.requestId === requestId) {
          stop();
          resolve(answer as Answer<Op> | Failed);
        }
      });
      client.request({ type: 'setup', request }).catch((error: unknown) => {
        stop();
        reject(error instanceof Error ? error : new Error(String(error)));
      });
    });
  }

  async function plugins(request: PluginStepRequest): Promise<unknown> {
    const answer = await client.expect({ type: 'pluginPanel', request: request as unknown as PanelRequest }, 'pluginPanel');
    const message = answer.message as unknown;
    return typeof message === 'object' && message !== null && 'value' in message ? message.value : undefined;
  }

  return {
    async info() {
      const answer = await client.expect({ type: 'setup', request: { op: 'info' } }, 'setup');
      if (answer.answer.op !== 'info') throw new Error(`expected info, the core answered ${answer.answer.op}`);
      return answer.answer;
    },
    preview: (request) => untilEvent<'proposal'>(request),
    write: (proposalId) => untilEvent<'written'>({ op: 'write', proposalId }),
    basics: (layout, sync) =>
      untilEvent<'applied'>({
        op: 'basics',
        ...(layout === undefined ? {} : { layout }),
        ...(sync === undefined ? {} : { sync }),
      }),
    async close() {
      await client.request({ type: 'setup', request: { op: 'close' } });
    },
    async pickFolder() {
      let stop: () => void = () => undefined;
      const picked = new Promise<string | null>((resolve) => {
        stop = client.on((message) => {
          if (message.type === 'pickedPath') resolve(message.path ?? null);
        });
      });
      try {
        const answer = await client.request({ type: 'pickPath', kind: 'folder', titleKey: 'firstrun.project.pick_title' });
        return answer.type === 'pickedPath' ? (answer.path ?? null) : await picked;
      } catch {
        return null;
      } finally {
        stop();
      }
    },
    async devices() {
      return (await client.expect({ type: 'getDevices' }, 'devices')).items;
    },
    async setTheme(theme) {
      await client.request({ type: 'runCommand', id: THEME_SET_COMMAND, args: { theme } });
    },
    async runCommand(id, args) {
      await client.request({ type: 'runCommand', id, ...(args === undefined ? {} : { args }) });
    },
    async available() {
      const parsed = parseAvailable(await plugins({ op: 'available' }));
      if (parsed === null) throw new Error('firstrun.plugins.failed');
      return parsed;
    },
    async installChosen(ids, onProgress) {
      const stop = client.on((message) => {
        if (message.type !== 'pluginPanel') return;
        const progress = parseProgress(message.message);
        if (progress !== null) onProgress(progress);
      });
      try {
        const outcome = parseOutcome(await plugins({ op: 'installChosen', ids }));
        if (outcome === null) throw new Error('firstrun.plugins.failed');
        return outcome;
      } finally {
        stop();
      }
    },
    async removeChosen(ids) {
      const outcome = parseOutcome(await plugins({ op: 'removeChosen', ids }));
      if (outcome === null) throw new Error('firstrun.plugins.failed');
      return outcome;
    },
    async applyRecommended() {
      await plugins({ op: 'applyRecommended' });
    },
  };
}
