<script setup lang="ts">
// One rule block of the draft (RB-094-04): its CSS, coloured as it is typed (CodeEditor, the one
// highlighter), and its breakpoints as a checkbox group of the grid's names - none chosen is every
// view, a chosen one the grid no longer shows stays, marked, until it is taken off. The footer
// says where the block applies (every view, or its media query as code) and offers Copy as media
// query and Remove; Remove asks inline first, with the focus on Keep (docs/design.md 4.4).
import { computed, nextTick, ref } from 'vue';
import CodeEditor from '../../../../web-common/panel-ui/src/code/CodeEditor.vue';
import IconAction from '../../../../web-common/panel-ui/src/components/IconAction.vue';
import { usePanel } from '../../../../web-common/panel-ui/src/mount';
import { type BlockState, type Breakpoint, type EditBlock, scopeItems } from './model';

const props = defineProps<{
  block: EditBlock;
  number: number;
  stored: BlockState | undefined;
  breakpoints: Breakpoint[];
  disabled: boolean;
}>();
const emit = defineEmits<{ css: [value: string]; breakpoints: [value: string[]]; copy: []; remove: [] }>();
const { t } = usePanel();
const confirming = ref(false);
const root = ref<HTMLElement | null>(null);

// The parent owns the block: the card reports each change and shows what it is given.
const css = computed({ get: () => props.block.css, set: (value: string) => emit('css', value) });
const chosen = computed({ get: () => props.block.breakpoints, set: (value: string[]) => emit('breakpoints', [...value]) });
const items = computed(() =>
  scopeItems(
    props.breakpoints,
    props.block.breakpoints,
    (breakpoint) => t('panel.breakpoint', { label: breakpoint.label, width: breakpoint.width }),
    (id) => t('panel.missing', { id }),
  ).map(({ label, value }) => ({ label, value })),
);
const id = computed(() => `block-${props.block.key}`);

const focus = (selector: string): void => void nextTick(() => root.value?.querySelector<HTMLElement>(selector)?.focus());

function ask(): void {
  confirming.value = true;
  focus('[data-keep]');
}

function keep(): void {
  confirming.value = false;
  focus('[data-remove]');
}

function remove(): void {
  confirming.value = false;
  emit('remove');
}
</script>

<template>
  <div ref="root" :data-block="number">
    <UCard as="section" :ui="{ header: 'flex items-center gap-2 px-3 py-2 sm:px-3', body: 'flex flex-col gap-3 p-3 sm:p-3', footer: 'flex flex-col gap-2 px-3 py-2 sm:px-3' }">
      <template #header>
        <h2 class="min-w-0 flex-1 truncate text-sm font-semibold text-highlighted">{{ t('panel.block', { n: number }) }}</h2>
        <IconAction
          data-copy
          icon="i-lucide-copy"
          :label="t('panel.copy')"
          :disabled="!stored?.copy"
          @click="emit('copy')"
        />
        <IconAction data-remove icon="i-lucide-trash-2" :label="t('panel.remove', { n: number })" :disabled="disabled" @click="ask" />
      </template>
      <CodeEditor :id="id + '-css'" v-model="css" lang="css" :rows="6" :disabled="disabled" :label="t('panel.css', { n: number })" />
      <UFormField :label="t('panel.scope')" :help="t('panel.scopeHint')" :ui="{ help: 'text-xs' }">
        <UCheckboxGroup
          v-model="chosen"
          data-scope
          :items="items"
          orientation="horizontal"
          size="sm"
          :disabled="disabled"
        />
      </UFormField>
      <template #footer>
        <p data-where class="text-xs text-muted">
          <template v-if="!stored || stored.scope === 'all'">{{ t('panel.allViews') }}</template>
          <code v-else-if="stored.scope === 'query'" class="font-mono">@media {{ stored.media }}</code>
          <span v-else class="text-warning">{{ t('panel.nowhere') }}</span>
        </p>
        <UAlert
          v-if="confirming"
          data-confirm
          color="error"
          variant="subtle"
          icon="i-lucide-trash-2"
          :title="t('panel.confirmRemove', { n: number })"
          :ui="{ root: 'px-3 py-2', title: 'text-sm font-normal' }"
        >
          <template #actions>
            <UButton data-confirm-remove size="xs" color="error" :label="t('panel.removeConfirm')" @click="remove" />
            <UButton data-keep size="xs" color="neutral" variant="outline" :label="t('panel.keep')" @click="keep" />
          </template>
        </UAlert>
      </template>
    </UCard>
  </div>
</template>
