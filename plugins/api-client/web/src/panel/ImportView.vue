<script setup lang="ts">
// The API client's import (RB-096-02): files picked (a Bruno collection as its folder) or a text
// pasted (a curl command, an .http request, a collection's JSON), the format told by the files
// or chosen, the folder below .rbrowser/api and whether the environments go into
// http-client.env.json (the setting `api-client.importEnvironments` gives the default). Preview
// asks the core's `import` op with `preview: true` and shows what would be written and the
// report; Import writes it. Nothing leaves the panel but to the core; no secret comes back. It is
// the side column's Import tab (RB-096-11), no window over the editor.
import { computed, ref } from 'vue';
import { usePanel } from '../../../../web-common/panel-ui/src/mount';
import SettingsText from './SettingsText.vue';
import {
  ACCEPT,
  FORMATS,
  type FormatId,
  type ImportAnswer,
  type Note,
  filesOf,
  firstOf,
  importOp,
  notesByKind,
  withinLimit,
} from './importing';

const props = defineProps<{ environments: boolean }>();
const emit = defineEmits<{ imported: [answer: ImportAnswer] }>();
const panel = usePanel();
const t = panel.t;

const files = ref<File[]>([]);
const pasted = ref('');
const format = ref<FormatId | 'auto'>('auto');
const folder = ref('');
const environments = ref(props.environments);
const answer = ref<ImportAnswer | null>(null);
const problem = ref<string | null>(null);
const busy = ref(false);
const folderInput = ref<HTMLInputElement | null>(null);

const formatItems = computed(() => [
  { label: t('panel.import.formats.auto'), value: 'auto' },
  ...FORMATS.map((id) => ({ label: t(`panel.import.formats.${id}`), value: id })),
]);
const ready = computed(() => files.value.length > 0 || pasted.value.trim().length > 0);
const notes = computed(() => (answer.value ? notesByKind(answer.value.report) : { reported: [], dropped: [] }));
const shown = computed(() => firstOf(answer.value?.requests ?? [], 30));

function noteText(note: Note): string {
  return t(`panel.import.notes.${note.code}`, { at: note.at, detail: note.detail });
}

/** The files of a picked folder join the list (a raw input: Nuxt UI has no folder picker). */
function pickedFolder(event: Event): void {
  const input = event.target as HTMLInputElement;
  files.value = [...files.value, ...Array.from(input.files ?? [])];
  input.value = '';
}

async function ask(preview: boolean): Promise<void> {
  if (busy.value || !ready.value) return;
  if (!withinLimit(files.value, pasted.value)) {
    problem.value = t('panel.import.tooLarge');
    return;
  }
  busy.value = true;
  problem.value = null;
  try {
    const read = await filesOf(files.value, pasted.value);
    const op = importOp(read, { format: format.value, folder: folder.value, environments: environments.value, preview });
    const result = (await panel.call({ tool: 'panel' }, { op })) as ImportAnswer;
    answer.value = result;
    if (!preview) emit('imported', result);
  } catch (error) {
    answer.value = null;
    problem.value = panel.error(error);
  } finally {
    busy.value = false;
  }
}

</script>

<template>
  <section id="import" class="flex flex-col gap-3" :aria-label="t('panel.import.title')">
    <p class="text-xs text-muted">{{ t('panel.import.description') }}</p>

    <UFileUpload
      id="import-files"
      v-model="files"
      multiple
      layout="list"
      :accept="ACCEPT"
      :label="t('panel.import.files')"
      :description="t('panel.import.filesHint')"
      class="w-full"
    />
    <div class="flex items-center gap-2">
      <UButton id="import-folder" size="xs" color="neutral" variant="outline" icon="i-lucide-folder-open" :label="t('panel.import.pickFolder')" @click="folderInput?.click()" />
      <!-- A raw input (docs/design.md 5.1): Nuxt UI's file upload cannot pick a folder. -->
      <input ref="folderInput" type="file" class="hidden" multiple webkitdirectory aria-hidden="true" tabindex="-1" @change="pickedFolder" />
    </div>
    <UFormField :label="t('panel.import.paste')">
      <UTextarea id="import-paste" v-model="pasted" :rows="4" autoresize class="w-full font-mono" :placeholder="t('panel.import.pastePlaceholder')" />
    </UFormField>
    <div class="flex flex-wrap items-end gap-2">
      <UFormField :label="t('panel.import.format')" class="min-w-40 flex-1">
        <USelect id="import-format" v-model="format" :items="formatItems" value-key="value" size="sm" class="w-full" />
      </UFormField>
      <UFormField :label="t('panel.import.folder')" :hint="t('panel.import.folderHint')" class="min-w-40 flex-1">
        <UInput id="import-target" v-model="folder" size="sm" icon="i-lucide-folder" class="w-full font-mono" />
      </UFormField>
    </div>
    <UCheckbox id="import-environments" v-model="environments" :label="t('panel.import.environments')" />
    <div class="flex flex-wrap gap-2">
      <UButton id="import-preview" size="sm" color="neutral" variant="outline" icon="i-lucide-eye" :label="t('panel.import.preview')" :loading="busy" :disabled="!ready" @click="ask(true)" />
      <UButton id="import-run" size="sm" icon="i-lucide-import" :label="t('panel.import.run')" :loading="busy" :disabled="!ready" @click="ask(false)" />
    </div>

    <p v-if="problem" id="import-problem" role="alert" class="text-sm text-error">{{ problem }}</p>

    <div v-if="answer" id="import-result" class="flex flex-col gap-2 text-sm" role="status">
      <p id="import-summary" class="font-medium">
        {{
          answer.preview
            ? t('panel.import.summary', { requests: answer.requests.length, environments: answer.environments.length })
            : t('panel.import.done', { count: answer.requests.length - answer.failed.length, folder: answer.folder })
        }}
      </p>
      <p v-if="answer.preview" class="text-xs text-muted">{{ t('panel.import.previewNote') }}</p>
      <ul id="import-requests" class="flex flex-col gap-0.5 font-mono text-xs">
        <li v-for="(request, index) in shown.shown" :key="index" class="truncate">
          <span class="text-muted">{{ request.folder }}/</span>{{ request.name }}
          <span class="text-muted">{{ `${request.method} ${request.url}` }}</span>
        </li>
        <li v-if="shown.more > 0" class="text-muted">{{ t('panel.import.more', { count: shown.more }) }}</li>
      </ul>
      <section v-if="answer.report.secrets.length > 0" id="import-secrets" class="flex flex-col gap-0.5">
        <p class="font-medium">{{ t('panel.import.secrets') }}</p>
        <p v-for="secret in answer.report.secrets" :key="`${secret.at}/${secret.name}`" class="text-xs" data-secret>
          <SettingsText text="panel.import.secretItem" :params="{ at: secret.at, name: secret.name, variable: secret.variable }" setting-key="api-client.secrets" />
        </p>
      </section>
      <section v-if="notes.reported.length > 0" id="import-reported" class="flex flex-col gap-0.5">
        <p class="font-medium">{{ t('panel.import.reported') }}</p>
        <p v-for="(note, index) in notes.reported" :key="index" class="text-xs" data-note>{{ noteText(note) }}</p>
      </section>
      <section v-if="notes.dropped.length > 0" id="import-dropped" class="flex flex-col gap-0.5">
        <p class="font-medium">{{ t('panel.import.dropped') }}</p>
        <p v-for="(note, index) in notes.dropped" :key="index" class="text-xs" data-note>{{ noteText(note) }}</p>
      </section>
      <p v-if="answer.kept.length > 0" id="import-kept" class="text-xs text-muted">{{ t('panel.import.kept', { names: answer.kept.join(', ') }) }}</p>
      <p v-for="failure in answer.failed" :key="failure.name" class="text-xs text-error" data-failed>{{ t('panel.import.failed', { name: failure.name }) }}</p>
    </div>
  </section>
</template>
