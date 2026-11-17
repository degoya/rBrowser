// The context the toolbar's menus are built from (RB-090-63, `menuModel.ts`): the shell's state,
// its settings and the registry's commands, read when a menu opens. The commands are asked for
// once, by the first menu (`loadCommands`).

import { useI18n } from 'vue-i18n';
import type { Shell } from '../state/shell';
import type { MenuContext } from './menuModel';

export function useMenuContext(shell: Shell): () => MenuContext {
  const { t } = useI18n();
  shell.loadCommands();
  return () => ({
    t: (key, args) => (args === undefined ? t(key) : t(key, args)),
    state: shell.state.value,
    settings: shell.settings.value,
    commands: shell.commands.value,
    systemTheme: shell.systemTheme.value,
  });
}
