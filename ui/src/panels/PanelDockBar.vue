<script setup lang="ts">
// The header of the open plugin panels page (RB-080-57, RB-080-61, RB-080-93): docked, a button
// that folds the panels to their icon rail; the Dock menu - left, below, right of the views or a
// window of their own, each with its icon, the current place ticked and the command's shortcut,
// then "Hide the panels"; docked, Close as an icon button (to the rail, or hidden, as
// `panels.closeTo` says). Folded, the page shows the rail instead (`PanelSideBar.vue`).
//
// Every control runs a command of the core (`dock.ts`), the one the shortcuts and MCP run too;
// the selected panel goes along, so the page that opens shows it. The menu is Nuxt UI's
// `UDropdownMenu` (`docs/design.md` 5.1): it opens only from the open panels, at least 120 px, and
// keeps to the page's room (its list scrolls in a low dock). The tooltips are the controls'
// `title`, which the window draws outside the page as well; a control with a visible label keeps
// it as its name and the title as its description (label in name, RB-090-23).
import { computed, onMounted } from 'vue';
import { useI18n } from 'vue-i18n';
import UButton from '@nuxt/ui/components/Button.vue';
import UDropdownMenu, { type DropdownMenuItem } from '@nuxt/ui/components/DropdownMenu.vue';
import type { IpcClient } from '../ipc/client';
import { useShell } from '../state/shell';
import {
  CLOSE_COMMAND,
  DOCK_COMMANDS,
  DOCK_SIDES,
  TOGGLE_COMMAND,
  UNDOCK_COMMAND,
  dockedSide,
  panelArgs,
  type DockPosition,
  type DockState,
} from './dock';
import { DOCK_ICONS } from './icons';
import { chordKeys } from './shortcut';

const props = withDefaults(
  defineProps<{
    client: IpcClient;
    /** Where the panels are, from the settings (`PanelsPage.vue` reads them). */
    state: DockState;
    /** The selected panel, `<plugin>.<panel>`. */
    current?: string | null;
  }>(),
  { current: null },
);
const { t } = useI18n();
const shell = useShell();

const side = computed(() => dockedSide(props.state));
const POSITIONS: readonly DockPosition[] = [...DOCK_SIDES, 'floating'];

function commandOf(position: DockPosition): string {
  return position === 'floating' ? UNDOCK_COMMAND : DOCK_COMMANDS[position];
}

function labelOf(position: DockPosition): string {
  return t(`panels.dockbar.${position === 'floating' ? 'float' : position}`);
}

async function run(id: string, args: Record<string, string> = {}): Promise<void> {
  try {
    await props.client.request({ type: 'runCommand', id, args });
  } catch {
    // The core reports a failed command in the status bar.
  }
}

/** Another place moves the panels with the selected one; the current place changes nothing,
 * the fold stays (review point 6). */
function choose(position: DockPosition): void {
  if (position !== props.state.position) {
    void run(commandOf(position), panelArgs(props.current));
  }
}

/** The menu: the four places, the current one ticked, each with its shortcut; then hiding. */
const items = computed<DropdownMenuItem[][]>(() => [
  POSITIONS.map((position) => ({
    type: 'checkbox' as const,
    label: labelOf(position),
    icon: DOCK_ICONS.position[position],
    checked: position === props.state.position,
    kbds: chordKeys(shell.bindingOf(commandOf(position))),
    onSelect: () => choose(position),
  })),
  [
    {
      label: t('panels.dockbar.hide'),
      icon: DOCK_ICONS.hide,
      kbds: [],
      onSelect: () => void run(CLOSE_COMMAND, { to: 'hidden' }),
    },
  ],
]);
const menuIcon = computed(() => DOCK_ICONS.position[props.state.position]);
const menuLabel = computed(() => t('panels.dockbar.menu', { place: labelOf(props.state.position) }));

onMounted(() => shell.loadCommands());
</script>

<template>
  <div
    role="toolbar"
    :aria-label="t('panels.dockbar.label')"
    class="flex h-9 shrink-0 items-center gap-1 border-b border-default px-1.5 text-xs"
    data-testid="panel-dockbar"
  >
    <UButton
      v-if="side !== null"
      :icon="DOCK_ICONS.collapse[side]"
      color="neutral"
      variant="ghost"
      size="sm"
      square
      :title="t('panels.dockbar.collapse')"
      :aria-label="t('panels.dockbar.collapse')"
      aria-expanded="true"
      data-testid="panel-dock-collapse"
      @click="run(TOGGLE_COMMAND)"
    />
    <UDropdownMenu :items="items" :content="{ align: 'start', collisionPadding: 4 }" :modal="false">
      <UButton
        :icon="menuIcon"
        :label="t('panels.dockbar.dock')"
        color="neutral"
        variant="ghost"
        size="sm"
        :trailing-icon="DOCK_ICONS.menu"
        :title="menuLabel"
        :data-icon="menuIcon"
        data-testid="panel-dock-menu"
      />
    </UDropdownMenu>
    <template v-if="side !== null">
      <span class="flex-1" />
      <UButton
        :icon="DOCK_ICONS.close"
        color="neutral"
        variant="ghost"
        size="sm"
        square
        :title="t('panels.dockbar.close')"
        :aria-label="t('panels.dockbar.close')"
        data-testid="panel-dock-close"
        @click="run(CLOSE_COMMAND)"
      />
    </template>
  </div>
</template>
