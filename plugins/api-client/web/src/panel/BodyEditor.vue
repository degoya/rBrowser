<script setup lang="ts">
// The request's body (RB-096-01): its kind chosen from the list; JSON, raw text and GraphQL in
// the code field (JSON checked as it is typed), a form's fields and a multipart body's parts as
// rows, a binary body's file by its path relative to the request's file. `{{variables}}` work in
// every text; the core puts them in when it sends.
import { computed } from 'vue';
import { usePanel } from '../../../../web-common/panel-ui/src/mount';
import CodeEditor from '../../../../web-common/panel-ui/src/code/CodeEditor.vue';
import IconAction from '../../../../web-common/panel-ui/src/components/IconAction.vue';
import KeyValueTable from './KeyValueTable.vue';
import { BODY_KINDS, type Body, type BodyKind, type Entry, type Part, withBodyKind } from './model';

const body = defineModel<Body>({ required: true });
const t = usePanel().t;

const kindItems = computed(() => BODY_KINDS.map((kind) => ({ label: t(`panel.body.kinds.${kind}`), value: kind })));
const kind = computed<BodyKind>({
  get: () => body.value.kind,
  set: (next) => {
    body.value = withBodyKind(body.value, next);
  },
});

/** Whether a JSON body parses; a body with a `{{variable}}` is checked once it is sent. */
const jsonProblem = computed(() => {
  const current = body.value;
  if (current.kind !== 'json' || current.text.trim() === '' || current.text.includes('{{')) return '';
  try {
    JSON.parse(current.text);
    return '';
  } catch (error) {
    return error instanceof Error ? error.message : String(error);
  }
});

function setText(text: string): void {
  const current = body.value;
  if (current.kind === 'json' || current.kind === 'raw') body.value = { ...current, text };
  else if (current.kind === 'graphql') body.value = { ...current, query: text };
}

function setVariables(variables: string): void {
  const current = body.value;
  if (current.kind === 'graphql') body.value = { ...current, variables };
}

function setPath(path: string): void {
  if (body.value.kind === 'binary') body.value = { kind: 'binary', path };
}

function setFields(fields: Entry[]): void {
  if (body.value.kind === 'form') body.value = { kind: 'form', fields };
}

function setPart(index: number, patch: Partial<Part>): void {
  const current = body.value;
  if (current.kind !== 'multipart') return;
  body.value = { kind: 'multipart', parts: current.parts.map((part, at) => (at === index ? { ...part, ...patch } : part)) };
}

function addPart(file: boolean): void {
  const current = body.value;
  if (current.kind !== 'multipart') return;
  const part: Part = file ? { name: '', value: '', file: './' } : { name: '', value: '' };
  body.value = { kind: 'multipart', parts: [...current.parts, part] };
}

function removePart(index: number): void {
  const current = body.value;
  if (current.kind === 'multipart') body.value = { kind: 'multipart', parts: current.parts.filter((_, at) => at !== index) };
}
</script>

<template>
  <div id="body" class="flex flex-col gap-2 pt-2">
    <USelect id="body-kind" v-model="kind" :items="kindItems" value-key="value" size="sm" class="w-48" :aria-label="t('panel.body.kind')" />
    <template v-if="body.kind === 'json' || body.kind === 'raw'">
      <CodeEditor id="body-text" :model-value="body.text" :lang="body.kind === 'json' ? 'json' : null" :rows="10" :label="t('panel.body.text')" @update:model-value="setText" />
      <p v-if="jsonProblem" id="json-problem" class="text-xs text-error">{{ t('panel.body.jsonProblem', { reason: jsonProblem }) }}</p>
    </template>
    <template v-else-if="body.kind === 'graphql'">
      <CodeEditor id="graphql-query" :model-value="body.query" :lang="null" :rows="8" :label="t('panel.body.query')" @update:model-value="setText" />
      <CodeEditor
        id="graphql-variables"
        :model-value="body.variables"
        lang="json"
        :rows="4"
        :label="t('panel.body.variables')"
        @update:model-value="setVariables"
      />
    </template>
    <KeyValueTable v-else-if="body.kind === 'form'" id="form-fields" toggles :model-value="body.fields" @update:model-value="setFields" />
    <div v-else-if="body.kind === 'multipart'" id="parts" class="flex flex-col gap-1.5">
      <div v-for="(part, index) in body.parts" :key="index" data-part class="flex flex-wrap items-center gap-1.5">
        <UInput :model-value="part.name" size="sm" class="w-32 font-mono" :placeholder="t('panel.table.name')" :aria-label="t('panel.table.name')" @update:model-value="(text) => setPart(index, { name: String(text) })" />
        <UInput
          v-if="part.file !== undefined"
          :model-value="part.file"
          size="sm"
          icon="i-lucide-file"
          class="min-w-0 flex-1 font-mono"
          :placeholder="t('panel.body.file')"
          :aria-label="t('panel.body.file')"
          @update:model-value="(text) => setPart(index, { file: String(text) })"
        />
        <UInput v-else :model-value="part.value" size="sm" class="min-w-0 flex-1 font-mono" :placeholder="t('panel.table.value')" :aria-label="t('panel.table.value')" @update:model-value="(text) => setPart(index, { value: String(text) })" />
        <UInput :model-value="part.contentType ?? ''" size="sm" class="w-36 font-mono" :placeholder="t('panel.body.partType')" :aria-label="t('panel.body.partType')" @update:model-value="(text) => setPart(index, { contentType: String(text) || undefined })" />
        <IconAction icon="i-lucide-x" :label="t('panel.table.remove')" @click="removePart(index)" />
      </div>
      <div class="flex gap-1.5">
        <UButton data-add-text size="xs" color="neutral" variant="ghost" icon="i-lucide-plus" :label="t('panel.body.addText')" @click="addPart(false)" />
        <UButton data-add-file size="xs" color="neutral" variant="ghost" icon="i-lucide-file-plus" :label="t('panel.body.addFile')" @click="addPart(true)" />
      </div>
    </div>
    <UInput
      v-else-if="body.kind === 'binary'"
      id="binary-path"
      :model-value="body.path"
      size="sm"
      icon="i-lucide-file"
      class="w-full font-mono"
      :placeholder="t('panel.body.file')"
      :aria-label="t('panel.body.file')"
      @update:model-value="(text) => setPath(String(text))"
    />
    <p v-if="body.kind === 'binary' || body.kind === 'multipart'" class="text-xs text-muted">{{ t('panel.body.fileHint') }}</p>
  </div>
</template>
