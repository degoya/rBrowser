<script setup lang="ts">
// The analyser's wizard without a project (RB-090-43): the analysis only reads the pages and
// runs, but its breakpoints are written into a project's config, so the wizard says so at the
// top and offers the recent projects (`recentProjects`) to open one - `project.open_recent`,
// the command behind the palette's entry. Without recent projects it says how to start one. A
// project that does not open is a message of the status bar, as every other.
import { computed, onMounted, ref } from 'vue';
import { useI18n } from 'vue-i18n';
import UAlert from '@nuxt/ui/components/Alert.vue';
import UButton from '@nuxt/ui/components/Button.vue';
import USelect from '@nuxt/ui/components/Select.vue';
import type { IpcClient } from '../ipc/client';
import { commandFailureKey } from './wizard';

const props = defineProps<{ client: IpcClient }>();
const { t, te } = useI18n();

const recent = ref<string[]>([]);
const chosen = ref<string | undefined>(undefined);
const items = computed(() => recent.value.map((path) => ({ label: path, value: path })));

onMounted(async () => {
  try {
    const answer = await props.client.expect({ type: 'getSettings' }, 'settings');
    const value = answer.values.find((setting) => setting.key === 'recentProjects')?.value;
    recent.value = Array.isArray(value) ? value.filter((path): path is string => typeof path === 'string') : [];
    chosen.value = recent.value[0];
  } catch {
    recent.value = [];
  }
});

async function open(): Promise<void> {
  if (chosen.value === undefined) {
    return;
  }
  try {
    await props.client.request({ type: 'runCommand', id: 'project.open_recent', args: { path: chosen.value } });
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    const key = commandFailureKey(message);
    const report =
      key !== null && te(key)
        ? { messageKey: key }
        : { messageKey: 'analyzer.wizard.no_project.failed', args: { message } };
    await props.client.request({ type: 'reportStatus', level: 'error', ...report }).catch(() => undefined);
  }
}
</script>

<template>
  <UAlert
    color="warning"
    variant="subtle"
    orientation="vertical"
    :title="t('analyzer.wizard.no_project.title')"
    :description="t('analyzer.wizard.no_project.text')"
    data-testid="no-project"
  >
    <template #actions>
      <div v-if="items.length > 0" class="flex flex-wrap items-center gap-2">
        <USelect
          v-model="chosen"
          :items="items"
          class="min-w-64 max-w-full"
          :aria-label="t('analyzer.wizard.no_project.recent')"
          data-testid="recent-projects"
        />
        <UButton :label="t('analyzer.wizard.no_project.open')" data-testid="open-recent" @click="open" />
      </div>
      <p v-else class="text-sm text-muted">{{ t('analyzer.wizard.no_project.none') }}</p>
    </template>
  </UAlert>
</template>
