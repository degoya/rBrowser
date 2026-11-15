// The palette's registry and settings providers (RB-060-11): every command of the command
// registry - core and plugins, fresh at every open, so an enabled plugin's command is there and
// a disabled one's is gone - and every settings key as a deep link into the settings window.

import { PALETTE_COMMAND } from './constants';
import type { PaletteEntry, PaletteProvider } from './types';

export const commandsProvider: PaletteProvider = {
  id: 'commands',
  entries: ({ commands, t }) =>
    commands
      .filter((command) => command.id !== PALETTE_COMMAND)
      .map(
        (command): PaletteEntry => ({
          key: `command:${command.id}`,
          scope: 'command',
          title: t(command.titleKey),
          group: t(`commands.group.${command.group}`),
          description: command.id,
          keywords: [command.id],
          ...(command.bindings[0] === undefined ? {} : { shortcut: command.bindings[0] }),
          commandId: command.id,
          action: { kind: 'send', message: { type: 'runCommand', id: command.id } },
        }),
      ),
};

export const settingsProvider: PaletteProvider = {
  id: 'settings',
  entries: ({ settings, t }) =>
    (settings?.defs ?? []).map(
      (def): PaletteEntry => ({
        key: `setting:${def.key}`,
        scope: 'setting',
        title: t(def.labelKey),
        group: t(`settings.category.${def.category}`),
        description: t(def.descriptionKey),
        keywords: [def.key],
        action: { kind: 'send', message: { type: 'openSettings', key: def.key } },
      }),
    ),
};
