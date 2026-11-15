<script setup lang="ts">
// The editor (RB-070-04, on Nuxt UI since RB-080-84): name, code and where a run goes, Run as
// the card's main action, Save and Delete beside it - Delete asks first, inline, with the focus on
// Keep; kept, Delete has it back, deleted, the name field (RB-090-23). The code is
// coloured as it is typed by the one highlighter's editor over the textarea (RB-080-65); Ctrl+Enter
// runs it.
import { computed, nextTick, onMounted, ref } from 'vue';
import { usePanel } from '../../../../web-common/panel-ui/src/mount';
import { highlighter } from '../../../../web-common/panel-ui/src/code/highlight';
import type { SnippetEntry } from './model';

const props = defineProps<{ selected: SnippetEntry | null; canSave: boolean }>();
const name = defineModel<string>('name', { required: true });
const code = defineModel<string>('code', { required: true });
const target = defineModel<'all' | 'active'>('target', { required: true });
const emit = defineEmits<{ run: []; save: []; remove: [] }>();
const { t } = usePanel();
const confirming = ref(false);
const root = ref<HTMLElement | null>(null);

const modified = computed(() => props.selected !== null && props.selected.code !== code.value);
const canDelete = computed(() => props.selected?.source === 'local');
const targets = computed(() => [
  { label: t('panel.allViews'), value: 'all' },
  { label: t('panel.activeView'), value: 'active' },
]);

function onKey(event: KeyboardEvent): void {
  if (event.key === 'Enter' && (event.ctrlKey || event.metaKey)) {
    event.preventDefault();
    emit('run');
  }
}

const focusById = (id: string): void =>
  void nextTick(() => root.value?.querySelector<HTMLElement>(`#${id}`)?.focus());

function ask(): void {
  confirming.value = true;
  focusById('keep');
}

function keep(): void {
  confirming.value = false;
  focusById('delete');
}

function remove(): void {
  confirming.value = false;
  emit('remove');
  // Delete disables itself once the snippet is gone; the name field stays.
  focusById('name');
}

defineExpose({
  focusCode: () => void nextTick(() => root.value?.querySelector<HTMLTextAreaElement>('#code')?.focus()),
  focusName: () => void nextTick(() => root.value?.querySelector<HTMLInputElement>('#name')?.focus()),
});

onMounted(() => {
  const textarea = root.value?.querySelector<HTMLTextAreaElement>('#code');
  // The code in syntax colours as it is typed (RB-080-65).
  if (textarea) highlighter().editor(textarea, 'js', () => t('panel.plainLarge'));
});
</script>

<template>
  <div ref="root">
    <UCard as="section" :ui="{ body: 'flex flex-col gap-3 p-3 sm:p-3', footer: 'flex flex-col gap-2 px-3 py-2 sm:px-3' }">
      <UFormField :label="t('panel.name')" name="name">
        <UInput id="name" v-model="name" maxlength="80" autocomplete="off" spellcheck="false" size="sm" class="w-full" />
      </UFormField>
      <UFormField :label="t('panel.code')" name="code" :help="t('panel.codeHint') + (modified ? ' ' + t('panel.modified') : '')" :ui="{ help: 'text-xs' }">
        <UTextarea
          id="code"
          v-model="code"
          :rows="8"
          spellcheck="false"
          autocomplete="off"
          class="w-full"
          :ui="{ base: 'font-mono text-xs' }"
          @keydown="onKey"
        />
      </UFormField>
      <UFormField :label="t('panel.target')" name="target">
        <URadioGroup id="target" v-model="target" :items="targets" orientation="horizontal" size="sm" />
      </UFormField>
      <template #footer>
        <div class="flex flex-wrap items-center gap-2">
          <UButton id="run" icon="i-lucide-play" :label="t('panel.run')" size="sm" @click="emit('run')" />
          <UButton id="save" icon="i-lucide-save" :label="t('panel.save')" color="neutral" variant="outline" size="sm" :disabled="!canSave" @click="emit('save')" />
          <UButton
            id="delete"
            icon="i-lucide-trash-2"
            :label="t('panel.delete')"
            color="error"
            variant="ghost"
            size="sm"
            :disabled="!canDelete"
            @click="ask"
          />
        </div>
        <UAlert
          v-if="confirming && canDelete"
          id="confirm-delete"
          color="error"
          variant="subtle"
          icon="i-lucide-trash-2"
          :title="t('panel.confirmDelete', { name: selected?.name ?? '' })"
          :ui="{ root: 'px-3 py-2', title: 'text-sm font-normal' }"
        >
          <template #actions>
            <UButton id="confirm" size="xs" color="error" :label="t('panel.delete')" @click="remove" />
            <UButton id="keep" size="xs" color="neutral" variant="outline" :label="t('panel.keep')" @click="keep" />
          </template>
        </UAlert>
      </template>
    </UCard>
  </div>
</template>
