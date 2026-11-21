<script setup lang="ts">
// The request's fixed header (RB-096-11), above the editor's tabs so that a tab switch moves
// nothing: the method, the URL and Send; below them the title, the collection a new request
// goes to (a pick list of the folders), Save, Save as new, Delete (asked first), Copy as code
// with its language, and the benchmark of a saved request. Enter in the URL sends. The code and
// the benchmark show their results below the tabs (`runs.ts`).
import { computed, ref, watch } from 'vue';
import { usePanel } from '../../../../web-common/panel-ui/src/mount';
import { type BenchRun, type CodeRun } from './runs';
import { LANGUAGES } from './code';
import { METHODS, type ApiRequest } from './model';

const props = defineProps<{
  busy: boolean;
  id: string | null;
  folders: string[];
  environments: string[];
  environment: string | undefined;
  code: CodeRun;
  bench: BenchRun;
}>();
const emit = defineEmits<{ send: []; save: [asNew: boolean]; remove: [] }>();
const request = defineModel<ApiRequest>({ required: true });
const folder = defineModel<string>('folder', { required: true });
const t = usePanel().t;

/** The URL field's example: a request's form, not a text to translate. */
const URL_EXAMPLE = '{{baseUrl}}/path?query';
/** The top of the collections as a select item: an item's value cannot be empty. */
const TOP = '/';
const confirming = ref(false);
watch(
  () => props.id,
  () => {
    confirming.value = false;
  },
);

const methodItems = computed(() => {
  const known: string[] = [...METHODS];
  return known.includes(request.value.method) ? known : [...known, request.value.method];
});
const folderItems = computed(() => [{ label: t('panel.top'), value: TOP }, ...props.folders.map((name) => ({ label: name, value: name }))]);
const folderChoice = computed({
  get: () => (folder.value === '' ? TOP : folder.value),
  set: (value: string) => {
    folder.value = value === TOP ? '' : value;
  },
});
const languageItems = computed(() => LANGUAGES.map((value) => ({ label: t(`panel.code.languages.${value}`), value })));
/** "No second environment" as a select item. */
const ONE = '/';
const others = computed(() => [
  { label: t('panel.bench.noCompare'), value: ONE },
  ...props.environments.filter((name) => name !== props.environment).map((name) => ({ label: name, value: name })),
]);
const compareChoice = computed({
  get: () => props.bench.compareWith ?? ONE,
  set: (value: string) => {
    props.bench.compareWith = value === ONE ? undefined : value;
  },
});

function patch(next: Partial<ApiRequest>): void {
  request.value = { ...request.value, ...next };
}

function onUrlKey(event: KeyboardEvent): void {
  if (event.key === 'Enter' && (event.ctrlKey || event.metaKey || !event.shiftKey)) {
    event.preventDefault();
    emit('send');
  }
}

function remove(): void {
  confirming.value = false;
  emit('remove');
}
</script>

<template>
  <header id="request-header" class="flex flex-col gap-1.5">
    <div class="flex items-center gap-1.5">
      <USelect
        id="method"
        :model-value="request.method"
        :items="methodItems"
        size="sm"
        class="w-28 font-mono"
        :aria-label="t('panel.method')"
        @update:model-value="(method) => patch({ method: String(method) })"
      />
      <UInput
        id="url"
        :model-value="request.url"
        size="sm"
        class="min-w-0 flex-1 font-mono"
        spellcheck="false"
        autocomplete="off"
        :placeholder="URL_EXAMPLE"
        :aria-label="t('panel.url')"
        @update:model-value="(url) => patch({ url: String(url) })"
        @keydown="onUrlKey"
      />
      <UButton id="send" size="sm" icon="i-lucide-send" :label="t('panel.send')" :loading="busy" :disabled="request.url.trim() === ''" @click="emit('send')" />
    </div>
    <div id="save" class="flex flex-wrap items-center gap-1.5">
      <UInput
        id="name"
        :model-value="request.name"
        size="sm"
        class="min-w-32 flex-1"
        maxlength="200"
        :placeholder="t('panel.name')"
        :aria-label="t('panel.name')"
        @update:model-value="(name) => patch({ name: String(name) })"
      />
      <USelect v-if="!id" id="folder" v-model="folderChoice" :items="folderItems" value-key="value" size="sm" icon="i-lucide-folder" class="w-40 font-mono" :aria-label="t('panel.folder')" />
      <UButton id="save-request" size="sm" color="neutral" variant="outline" icon="i-lucide-save" :label="id ? t('panel.save') : t('panel.saveNew')" @click="emit('save', false)" />
      <UButton id="save-copy" size="sm" color="neutral" variant="ghost" icon="i-lucide-copy-plus" :label="t('panel.saveCopy')" :disabled="!id" @click="emit('save', true)" />
      <template v-if="!confirming">
        <UButton id="delete" size="sm" color="error" variant="ghost" icon="i-lucide-trash" :label="t('panel.delete')" :disabled="!id" @click="confirming = true" />
      </template>
      <span v-else id="confirm-delete" role="alertdialog" class="flex items-center gap-1.5 text-sm" :aria-label="t('panel.confirmDelete', { id: id ?? '' })">
        {{ t('panel.confirmDelete', { id: id ?? '' }) }}
        <UButton id="keep" size="sm" color="neutral" variant="outline" :label="t('panel.keep')" autofocus @click="confirming = false" />
        <UButton id="delete-yes" size="sm" color="error" :label="t('panel.delete')" @click="remove" />
      </span>
    </div>
    <div id="actions" class="flex flex-wrap items-center gap-1.5">
      <USelect id="code-language" v-model="code.language" :items="languageItems" value-key="value" size="sm" class="w-44" :aria-label="t('panel.code.language')" />
      <UButton id="copy-code" size="sm" color="neutral" variant="outline" icon="i-lucide-clipboard-copy" :loading="code.busy" :label="t('panel.code.copy')" @click="code.copy(request, environment)" />
      <USelect
        v-if="others.length > 1"
        id="bench-compare"
        v-model="compareChoice"
        :items="others"
        value-key="value"
        size="sm"
        class="w-44"
        icon="i-lucide-git-compare"
        :disabled="!id"
        :aria-label="t('panel.bench.compareWith')"
      />
      <UButton
        id="bench-run"
        size="sm"
        color="neutral"
        variant="outline"
        icon="i-lucide-gauge"
        :loading="bench.running"
        :disabled="!id"
        :label="t('panel.bench.run')"
        :title="id ? undefined : t('panel.bench.savedOnly')"
        @click="id && bench.start(id, environment)"
      />
    </div>
  </header>
</template>
