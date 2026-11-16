<script setup lang="ts">
// Settings > Keyboard Shortcuts (RB-010-17's editor, built here; the comfort of RB-060-11): every
// command by leader group or as one flat list, with its bindings; search by text or by pressing
// the keys, "only changed", reset a command, a group or all, record and remove bindings with the
// conflict dialog (`CommandRow`), export and import with a preview (`ImportPanel`), and the
// project's breakpoint shortcuts with their conflicts (`ProjectShortcuts`, RB-010-16). By group
// the list is one tab per leader group (RB-080-72), the first group open; a search, a pressed
// key or "only changed" lists the hits of every group at once, the tabs hidden. The search is a
// Nuxt UI `UInput` (RB-080-88).
import { computed, onMounted, ref } from 'vue';
import { useI18n } from 'vue-i18n';
import UButton from '@nuxt/ui/components/Button.vue';
import UInput from '@nuxt/ui/components/Input.vue';
import USwitch from '@nuxt/ui/components/Switch.vue';
import type { IpcClient } from '../../ipc/client';
import type { CommandInfo } from '../../ipc/protocol';
import { SETTINGS_ICONS } from '../icons';
import TabStrip from '../TabStrip.vue';
import CommandRow from './CommandRow.vue';
import ImportPanel from './ImportPanel.vue';
import KeySearch from './KeySearch.vue';
import ProjectShortcuts from './ProjectShortcuts.vue';
import { filterCommands, groupCommands } from './filter';
import { takeShortcutQuery } from '../plugins/shortcutQuery';
import { createShortcutsStore } from './store';

const props = defineProps<{ client: IpcClient; showKeys?: boolean }>();
const { t } = useI18n();
const store = createShortcutsStore(props.client);

// Settings > Plugins opens the editor at one plugin's commands (RB-080-51).
const query = ref(takeShortcutQuery());
const pressed = ref<string | null>(null);
const onlyChanged = ref(false);
const flat = ref(false);
const groupTab = ref<string | null>(null);

onMounted(() => void store.load());

const sections = computed(() =>
  groupCommands(
    filterCommands(store.commands.value, { query: query.value, pressed: pressed.value, onlyChanged: onlyChanged.value, t }),
    flat.value,
    t,
  ),
);

const filtering = computed(() => query.value.trim() !== '' || pressed.value !== null || onlyChanged.value);
const groupTabs = computed(() =>
  flat.value || filtering.value || sections.value.length < 2
    ? []
    : sections.value.flatMap((section) =>
        section.group === null ? [] : [{ label: t(`commands.group.${section.group}`), value: section.group }],
      ),
);
const activeGroup = computed<string | number | undefined>({
  get: () => groupTabs.value.find((tab) => tab.value === groupTab.value)?.value ?? groupTabs.value[0]?.value,
  set: (group) => (groupTab.value = String(group)),
});
/** Whether `section` shows: every one without tabs, else the open tab's. The others stay in the
 * page, hidden, so a command row keeps its recorder state across tabs. */
function shown(section: { group: string | null }): boolean {
  return groupTabs.value.length === 0 || section.group === activeGroup.value;
}

function changedIds(commands: CommandInfo[]): string[] {
  return commands.filter((command) => command.changed).map((command) => command.id);
}
</script>

<template>
  <section class="flex flex-col gap-4" :aria-label="t('settings.shortcuts.title')" data-testid="shortcuts-editor">
    <p class="text-sm text-muted">{{ t('settings.shortcuts.intro', { leader: store.leader.value }) }}</p>
    <div class="flex flex-wrap items-center gap-3">
      <UInput
        v-model="query"
        type="search"
        :icon="SETTINGS_ICONS.search"
        :placeholder="t('settings.shortcuts.search')"
        :aria-label="t('settings.shortcuts.search')"
        class="w-64"
        data-testid="shortcut-search"
      />
      <KeySearch v-model="pressed" />
      <label class="flex items-center gap-2 text-sm">
        <USwitch v-model="onlyChanged" :aria-label="t('settings.shortcuts.only_changed')" />
        {{ t('settings.shortcuts.only_changed') }}
      </label>
      <label class="flex items-center gap-2 text-sm" data-testid="flat">
        <USwitch v-model="flat" :aria-label="t('settings.shortcuts.flat')" />
        {{ t('settings.shortcuts.flat') }}
      </label>
      <UButton
        :icon="SETTINGS_ICONS.reset"
        color="neutral"
        variant="soft"
        size="sm"
        :label="t('settings.shortcuts.reset_all')"
        :disabled="store.changedCount.value === 0"
        data-testid="reset-all"
        @click="store.reset()"
      />
    </div>
    <p v-if="store.error.value" class="text-sm text-error" role="alert">{{ store.error.value }}</p>

    <TabStrip v-if="groupTabs.length > 0" v-model="activeGroup" :items="groupTabs" data-testid="group-tabs" />

    <div
      v-for="section in sections"
      v-show="shown(section)"
      :key="section.group ?? 'all'"
      class="flex flex-col gap-1" :data-group="section.group ?? undefined">
      <div v-if="section.group !== null" class="flex items-center gap-2">
        <h3 class="text-sm font-semibold text-highlighted" data-testid="group-title">{{ t(`commands.group.${section.group}`) }}</h3>
        <UButton
          :icon="SETTINGS_ICONS.reset"
          color="neutral"
          variant="ghost"
          size="xs"
          :label="t('settings.shortcuts.reset_group', { group: t(`commands.group.${section.group}`) })"
          :disabled="changedIds(section.commands).length === 0"
          data-testid="reset-group"
          @click="store.resetMany(changedIds(section.commands))"
        />
      </div>
      <CommandRow
        v-for="command in section.commands"
        :key="command.id"
        :command="command"
        :store="store"
        :show-keys="showKeys"
      />
    </div>
    <p v-if="sections.length === 0" class="text-sm text-muted">{{ t('settings.shortcuts.no_matches') }}</p>

    <ProjectShortcuts :client="client" :store="store" />

    <ImportPanel :store="store" />
  </section>
</template>
