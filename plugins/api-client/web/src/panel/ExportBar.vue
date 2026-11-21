<script setup lang="ts">
// Export (RB-096-03): the requests of a folder, or every one, as a Postman v2.1 collection, one
// .http file or a curl script, written by the core (op `export`) to
// .rbrowser/out/api-client/exports/ - every variable kept, no secret resolved. The side column's
// Export tab (RB-096-11); a collection's context menu opens it with that collection chosen.
import { computed, ref } from 'vue';
import { usePanel } from '../../../../web-common/panel-ui/src/mount';
import { ALL, type ExportAnswer, type Format, FORMATS, foldersOf } from './code';
import type { Listed } from './model';

const props = defineProps<{ requests: Listed[] }>();
/** The folder to export, `ALL` for every request. */
const folder = defineModel<string>('folder', { default: ALL });
const panel = usePanel();
const t = panel.t;

const format = ref<Format>('postman');
const result = ref<ExportAnswer | null>(null);
const failure = ref<string | null>(null);
const busy = ref(false);

const folderItems = computed(() => [
  { label: t('panel.export.all'), value: ALL },
  ...foldersOf(props.requests).map((name) => ({ label: name, value: name })),
]);
const formatItems = computed(() => FORMATS.map((value) => ({ label: t(`panel.export.formats.${value}`), value })));

async function run(): Promise<void> {
  if (busy.value) return;
  busy.value = true;
  failure.value = null;
  result.value = null;
  try {
    result.value = (await panel.call(
      { tool: 'panel' },
      { op: { op: 'export', folder: folder.value === ALL ? null : folder.value, format: format.value } },
    )) as ExportAnswer;
  } catch (error) {
    failure.value = panel.error(error);
  } finally {
    busy.value = false;
  }
}
</script>

<template>
  <section id="export" class="flex flex-col gap-1.5" :aria-label="t('panel.export.title')">
    <p v-if="requests.length === 0" id="export-empty" class="text-xs text-muted">{{ t('panel.export.nothing') }}</p>
    <USelect id="export-folder" v-model="folder" :items="folderItems" value-key="value" size="sm" icon="i-lucide-folder" class="w-full" :aria-label="t('panel.export.folder')" />
    <div class="flex items-center gap-1.5">
      <USelect id="export-format" v-model="format" :items="formatItems" value-key="value" size="sm" class="min-w-0 flex-1" :aria-label="t('panel.export.format')" />
      <UButton id="export-run" :disabled="requests.length === 0" size="sm" color="neutral" variant="outline" icon="i-lucide-download" :loading="busy" :label="t('panel.export.run')" @click="run" />
    </div>
    <p v-if="failure" id="export-failure" role="alert" class="text-xs text-error">{{ failure }}</p>
    <div v-if="result" id="export-result" role="status" class="flex flex-col gap-0.5 text-xs">
      <p class="text-success">{{ t('panel.export.done', { count: result.count, path: result.path }) }}</p>
      <p v-if="result.env.length > 0" class="text-muted">{{ t('panel.code.env', { names: result.env.join(', ') }) }}</p>
      <p v-if="result.skipped.length > 0" class="text-warning">{{ t('panel.export.skipped', { ids: result.skipped.join(', ') }) }}</p>
      <p v-for="note in result.notes" :key="note" class="text-warning">{{ t(`panel.code.notes.${note}`) }}</p>
    </div>
  </section>
</template>
