<script setup lang="ts">
// The project's own breakpoint shortcuts in the shortcut editor (RB-010-16): every
// `breakpoints[].shortcut` of `.rbrowser/config.json` with the breakpoint it sets, in force or
// with a conflict line - reserved (the reason), given to two breakpoints, held by a command (its
// title) or a leader chord. Read again whenever a command's bindings changed, since a new
// binding can take a project shortcut's keys.
import { computed, onMounted, ref, watch } from 'vue';
import { useI18n } from 'vue-i18n';
import UKbd from '@nuxt/ui/components/Kbd.vue';
import type { IpcClient } from '../../ipc/client';
import type { ProjectShortcut } from '../../ipc/protocol';
import type { ShortcutsStore } from './store';

const props = defineProps<{ client: IpcClient; store: ShortcutsStore }>();
const { t } = useI18n();

const items = ref<ProjectShortcut[]>([]);

async function load(): Promise<void> {
  try {
    items.value = (await props.client.expect({ type: 'listProjectShortcuts' }, 'projectShortcuts')).items;
  } catch {
    // An older core or no answer: the section stays as it was.
  }
}

onMounted(() => void load());
watch(props.store.commands, () => void load());

/** The holder of a conflict: a command by its title, else the breakpoint's id. */
function holder(other: string): string {
  const command = props.store.commands.value.find((c) => c.id === other);
  return command === undefined ? other : t(command.titleKey);
}

const rows = computed(() =>
  items.value.map((item) => ({
    ...item,
    conflict:
      item.reasonKey === undefined
        ? null
        : item.other === undefined
          ? t('settings.shortcuts.project.conflict', { reason: t(item.reasonKey) })
          : t('settings.shortcuts.project.conflict_with', { reason: t(item.reasonKey), other: holder(item.other) }),
  })),
);
</script>

<template>
  <section v-if="rows.length > 0" class="flex flex-col gap-1" data-testid="project-shortcuts" :aria-label="t('settings.shortcuts.project.title')">
    <h3 class="text-sm font-semibold text-highlighted">{{ t('settings.shortcuts.project.title') }}</h3>
    <p class="text-xs text-muted">{{ t('settings.shortcuts.project.intro') }}</p>
    <div
      v-for="row in rows"
      :key="row.breakpoint"
      class="flex flex-wrap items-center gap-2 rounded-md border border-default px-3 py-2"
      :class="row.conflict === null ? '' : 'border-warning'"
      data-testid="project-shortcut"
      :data-breakpoint="row.breakpoint"
      :data-conflict="row.conflict === null ? undefined : 'true'"
    >
      <div class="min-w-48 flex-1 text-sm text-default">
        {{ t('settings.shortcuts.project.breakpoint', { breakpoint: row.breakpoint }) }}
      </div>
      <UKbd :value="row.binding" size="md" />
      <span v-if="row.conflict === null" class="text-xs text-muted">{{ t('settings.shortcuts.project.in_force') }}</span>
      <p v-else class="basis-full text-xs text-warning" role="alert" data-testid="project-conflict">{{ row.conflict }}</p>
    </div>
  </section>
</template>
