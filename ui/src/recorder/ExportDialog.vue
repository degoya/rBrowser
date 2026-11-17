<script setup lang="ts">
// The Recorder panel's export dialog (RB-090-60, the panel UI of RB-050-02): what to make - a
// flow, a Playwright test or both -, the export's name, a preview of every file rendered by the
// `recorder-export` plugin with syntax highlighting (`highlight.ts`, text in spans, never HTML),
// "Copy" per file and "Save" (RB-090-66): the flow goes into the flows folder
// (`.rbrowser/flows/<name>.ts`, named under the name field; an existing flow is replaced only
// when the person ticks it), the test into a new export folder (`automation.recorderExport.dir`;
// nothing is overwritten there). A save closes the dialog and the panel says where the files
// went (the core's notice too); a refusal keeps it open with the reason. The preview follows
// every change of the format or the name. Keyboard: a dialog of Nuxt UI (focus trapped, Escape
// closes), every control native and named.
import { computed, ref, watch } from 'vue';
import { useI18n } from 'vue-i18n';
import UButton from '@nuxt/ui/components/Button.vue';
import UCheckbox from '@nuxt/ui/components/Checkbox.vue';
import UFormField from '@nuxt/ui/components/FormField.vue';
import UInput from '@nuxt/ui/components/Input.vue';
import UModal from '@nuxt/ui/components/Modal.vue';
import URadioGroup from '@nuxt/ui/components/RadioGroup.vue';
import type { RecordingExportFile, RecordingExportFormat } from '../ipc/protocol';
import { highlight } from './highlight';
import type { RecorderChannel } from './host';
import { RECORDER_ICONS } from './icons';

const props = withDefaults(
  defineProps<{
    channel: RecorderChannel;
    /** The recording to export. */
    name: string;
    clipboard?: Pick<Clipboard, 'writeText'>;
    /** Inline instead of in a portal on the body (the tests). */
    inline?: boolean;
  }>(),
  { clipboard: undefined, inline: false },
);
const open = defineModel<boolean>('open', { required: true });
/** The files were written: where (the export folder, or the flows folder for a flow only). */
const emit = defineEmits<{ saved: [folder: string] }>();
const { t } = useI18n();

const format = ref<RecordingExportFormat>('flow');
const exportName = ref('');
const files = ref<RecordingExportFile[]>([]);
const error = ref<string | null>(null);
/** Where "Save" puts the flow, and whether a flow of that name exists. */
const flowFile = ref<string | null>(null);
const flowExists = ref(false);
const overwrite = ref(false);
const copied = ref<string | null>(null);
const busy = ref(false);

const formats = computed(() => [
  { value: 'flow', label: t('recorder.export.format.flow'), description: t('recorder.export.format.flow_hint') },
  { value: 'spec', label: t('recorder.export.format.spec'), description: t('recorder.export.format.spec_hint') },
  { value: 'both', label: t('recorder.export.format.both') },
]);
/** Why "Save" cannot run now, or null. */
const saveBlocked = computed(() =>
  flowExists.value && !overwrite.value ? t('recorder.export.overwrite_first', { file: flowFile.value ?? '' }) : null,
);
const highlighted = computed(() => files.value.map((file) => ({ ...file, tokens: highlight(file.content) })));

function reason(caught: unknown): string {
  return caught instanceof Error ? caught.message : String(caught);
}

/** Renders in order: an answer older than the newest request is dropped. */
let seq = 0;
async function preview(): Promise<void> {
  const mine = ++seq;
  error.value = null;
  try {
    const answer = await props.channel.render(props.name, format.value, exportName.value, false);
    if (mine === seq) {
      files.value = answer.files;
      flowFile.value = answer.flowFile ?? null;
      flowExists.value = answer.flowExists === true;
      overwrite.value = false;
    }
  } catch (caught) {
    if (mine === seq) {
      files.value = [];
      flowFile.value = null;
      flowExists.value = false;
      error.value = t('recorder.export.failed', { reason: reason(caught) });
    }
  }
}

async function save(): Promise<void> {
  busy.value = true;
  error.value = null;
  try {
    const answer = await props.channel.render(props.name, format.value, exportName.value, true, overwrite.value);
    open.value = false;
    emit('saved', answer.folder ?? '');
  } catch (caught) {
    error.value = t('recorder.export.failed', { reason: reason(caught) });
  } finally {
    busy.value = false;
  }
}

async function copy(file: RecordingExportFile): Promise<void> {
  try {
    await (props.clipboard ?? navigator.clipboard).writeText(file.content);
    copied.value = file.path;
  } catch {
    error.value = t('recorder.export.copy_failed');
  }
}

watch(
  () => [open.value, props.name] as const,
  ([isOpen]) => {
    if (isOpen) {
      exportName.value = props.name;
      copied.value = null;
      void preview();
    }
  },
  { immediate: true },
);
watch([format, exportName], () => {
  if (open.value) {
    void preview();
  }
});
</script>

<template>
  <UModal
    v-model:open="open"
    :title="t('recorder.export.title', { name })"
    :description="t('recorder.export.description')"
    :portal="!inline"
    :ui="{ content: 'max-w-3xl' }"
    data-testid="export-dialog"
  >
    <template #body>
      <div class="flex flex-col gap-3">
        <URadioGroup
          v-model="format"
          :items="formats"
          :legend="t('recorder.export.format.legend')"
          orientation="horizontal"
          data-testid="export-format"
        />
        <UFormField :label="t('recorder.export.name')" :hint="t('recorder.export.name_hint')">
          <UInput v-model="exportName" class="w-full" data-testid="export-name" />
        </UFormField>
        <p v-if="flowFile !== null" class="text-sm text-muted" data-testid="export-flow-file">
          {{ t('recorder.export.flow_goes', { file: flowFile }) }}
        </p>
        <UCheckbox
          v-if="flowExists"
          v-model="overwrite"
          :label="t('recorder.export.overwrite', { file: flowFile ?? '' })"
          data-testid="export-overwrite"
        />
        <p v-if="error !== null" class="text-sm text-error" role="alert" data-testid="export-error">{{ error }}</p>
        <section
          v-for="file in highlighted"
          :key="file.path"
          class="rounded-md border border-default"
          :data-testid="`export-file-${file.path}`"
        >
          <header class="flex items-center gap-2 border-b border-default px-2 py-1">
            <code class="min-w-0 flex-1 truncate text-xs">{{ file.path }}</code>
            <span v-if="copied === file.path" class="text-xs text-success" role="status">
              {{ t('recorder.export.copied') }}
            </span>
            <UButton
              :icon="RECORDER_ICONS.recorderCopy"
              color="neutral"
              variant="ghost"
              size="xs"
              :label="t('recorder.export.copy')"
              :title="t('recorder.export.copy_file', { file: file.path })"
              data-testid="export-copy"
              @click="copy(file)"
            />
          </header>
          <pre
            class="max-h-72 overflow-auto p-2 text-xs leading-relaxed"
            tabindex="0"
            :aria-label="t('recorder.export.preview', { file: file.path })"
          ><code><span
            v-for="(token, index) in file.tokens"
            :key="index"
            :class="{
              'text-muted italic': token.kind === 'comment',
              'text-success': token.kind === 'string',
              'text-primary font-semibold': token.kind === 'keyword',
              'text-warning': token.kind === 'number',
            }"
            :data-kind="token.kind"
          >{{ token.text }}</span></code></pre>
        </section>
      </div>
    </template>
    <template #footer>
      <div class="flex w-full flex-wrap justify-end gap-2">
        <UButton color="neutral" variant="ghost" :label="t('recorder.export.close')" data-testid="export-close" @click="open = false" />
        <UButton
          :icon="RECORDER_ICONS.recorderSave"
          :label="t('recorder.export.save')"
          :loading="busy"
          :disabled="files.length === 0 || saveBlocked !== null"
          :title="saveBlocked ?? undefined"
          data-testid="export-save"
          @click="save"
        />
      </div>
    </template>
  </UModal>
</template>
