<script setup lang="ts">
// One line of a todo file (RB-110-13): a task with its checkbox (checked done, a cross for
// cancelled), a project and a Markdown heading as headings - bold, a heading sized by its level
// (RB-110-15) - a note as muted text; the tags as badges. Its actions - edit, move up and down,
// cancel or open again, delete - are icons with their names; editing happens in place (Enter
// saves, Esc keeps the old text; a heading as its whole line, `## ...`) - not for a line too long
// for an answer, which is shown cut - and deleting asks in place first and says how many lines go
// with it.
import { computed, nextTick, ref } from 'vue';
import { usePanel } from '../../../../web-common/panel-ui/src/mount';
import { look, type Item } from './model';

const props = defineProps<{ item: Item; below: number; busy: boolean }>();
const emit = defineEmits<{
  toggle: [];
  cancel: [];
  save: [text: string];
  remove: [];
  move: [up: boolean];
  editing: [on: boolean];
}>();

const { t } = usePanel();

const editing = ref(false);
const draft = ref('');
const confirming = ref(false);
const field = ref<{ inputRef?: HTMLInputElement } | null>(null);

const checked = computed<boolean | 'indeterminate'>(() => {
  if (props.item.state === 'done') return true;
  if (props.item.state === 'cancelled') return 'indeterminate';
  return false;
});
const finished = computed(() => props.item.state === 'done' || props.item.state === 'cancelled');
const shownTags = computed(() => props.item.tags.filter((tag) => tag.name !== 'project'));
/** A cut line ends in an ellipsis: what is shown is not all of it. */
const shownText = computed(() => (props.item.cut ? `${props.item.text}\u2026` : props.item.text));
const shape = computed(() => look(props.item));
const heading = computed(() => shape.value.ariaLevel !== null);
/** The six heading levels' icons, written out here: the icon bundle finds only names in a `.vue`. */
const HEADING_ICONS = ['i-lucide-heading-1', 'i-lucide-heading-2', 'i-lucide-heading-3', 'i-lucide-heading-4', 'i-lucide-heading-5', 'i-lucide-heading-6'];
const icon = computed(() => {
  if (props.item.kind === 'heading') return HEADING_ICONS[(props.item.level ?? 1) - 1] ?? 'i-lucide-heading-6';
  return props.item.kind === 'project' ? 'i-lucide-folder' : 'i-lucide-text';
});

async function edit(): Promise<void> {
  draft.value = props.item.kind === 'project' ? props.item.text : props.item.raw;
  editing.value = true;
  emit('editing', true);
  await nextTick();
  field.value?.inputRef?.focus();
}

function keep(): void {
  editing.value = false;
  emit('editing', false);
}

function save(): void {
  const text = draft.value.trim();
  if (text === '') return;
  editing.value = false;
  emit('editing', false);
  emit('save', text);
}

function remove(): void {
  confirming.value = false;
  emit('remove');
}
</script>

<template>
  <li
    :data-line="item.line"
    :data-kind="item.kind"
    :data-state="item.state ?? undefined"
    :data-level="item.level ?? undefined"
    :class="['group flex flex-col gap-1', shape.row]"
    :style="{ paddingInlineStart: `${item.depth * 1.25}rem` }"
  >
    <div :class="['flex min-w-0 gap-2', heading ? 'items-center' : 'items-start']">
      <UCheckbox
        v-if="item.kind === 'task'"
        data-check
        size="sm"
        class="pt-0.5"
        :model-value="checked"
        indeterminate-icon="i-lucide-x"
        :disabled="busy || editing"
        :aria-label="t(finished ? 'panel.reopen' : 'panel.check', { text: item.text })"
        @update:model-value="emit('toggle')"
      />
      <UIcon v-else-if="heading" data-icon :name="icon" class="size-4 shrink-0 text-muted" />
      <UIcon v-else data-icon :name="icon" class="mt-0.5 size-4 shrink-0 text-dimmed" />

      <div v-if="editing" class="flex min-w-0 flex-1 items-center gap-1">
        <UInput
          ref="field"
          v-model="draft"
          data-edit
          size="xs"
          class="min-w-0 flex-1"
          maxlength="1000"
          autocomplete="off"
          :aria-label="t('panel.edit')"
          @keydown.enter.prevent="save"
          @keydown.escape.prevent="keep"
        />
        <UButton data-save size="xs" :label="t('panel.save')" :disabled="draft.trim() === ''" @click="save" />
        <UButton data-keep size="xs" color="neutral" variant="ghost" :label="t('panel.cancelEdit')" @click="keep" />
      </div>
      <div v-else class="flex min-w-0 flex-1 flex-wrap items-center gap-1">
        <span
          data-text
          :role="heading ? 'heading' : undefined"
          :aria-level="shape.ariaLevel ?? undefined"
          :class="['min-w-0 break-words', shape.text, finished ? 'text-muted line-through' : '']"
          >{{ shownText }}</span
        >
        <UBadge
          v-for="tag in shownTags"
          :key="tag.name + (tag.value ?? '')"
          data-tag
          size="sm"
          variant="subtle"
          :color="tag.name === 'critical' || tag.name === 'high' ? 'error' : tag.name === 'today' ? 'warning' : 'neutral'"
          :label="tag.value === null ? `@${tag.name}` : `@${tag.name}(${tag.value})`"
        />
      </div>

      <div v-if="!editing" class="flex shrink-0 items-center opacity-60 group-hover:opacity-100 focus-within:opacity-100">
        <UTooltip v-if="item.kind === 'task'" :text="t(item.state === 'cancelled' ? 'panel.reopen' : 'panel.cancelTask', { text: item.text })">
          <UButton
            data-cancel
            size="xs"
            color="neutral"
            variant="ghost"
            :icon="item.state === 'cancelled' ? 'i-lucide-rotate-ccw' : 'i-lucide-ban'"
            :aria-label="t(item.state === 'cancelled' ? 'panel.reopen' : 'panel.cancelTask', { text: item.text })"
            :disabled="busy"
            @click="emit('cancel')"
          />
        </UTooltip>
        <UTooltip :text="t('panel.edit')">
          <UButton data-edit-start size="xs" color="neutral" variant="ghost" icon="i-lucide-pencil" :aria-label="t('panel.edit')" :disabled="busy || item.cut" @click="edit" />
        </UTooltip>
        <UTooltip :text="t('panel.up')">
          <UButton data-up size="xs" color="neutral" variant="ghost" icon="i-lucide-arrow-up" :aria-label="t('panel.up')" :disabled="busy" @click="emit('move', true)" />
        </UTooltip>
        <UTooltip :text="t('panel.down')">
          <UButton data-down size="xs" color="neutral" variant="ghost" icon="i-lucide-arrow-down" :aria-label="t('panel.down')" :disabled="busy" @click="emit('move', false)" />
        </UTooltip>
        <UTooltip :text="t('panel.delete')">
          <UButton data-delete size="xs" color="error" variant="ghost" icon="i-lucide-trash-2" :aria-label="t('panel.delete')" :disabled="busy" @click="confirming = true" />
        </UTooltip>
      </div>
    </div>
    <UAlert
      v-if="confirming"
      data-confirm
      color="error"
      variant="subtle"
      icon="i-lucide-trash-2"
      :title="below > 0 ? t('panel.confirmDeleteBlock', { text: item.text, n: below }) : t('panel.confirmDelete', { text: item.text })"
      :ui="{ root: 'px-3 py-2', title: 'text-sm font-normal' }"
    >
      <template #actions>
        <UButton data-confirm-delete size="xs" color="error" :label="t('panel.delete')" @click="remove" />
        <UButton data-confirm-keep size="xs" color="neutral" variant="outline" :label="t('panel.keep')" @click="confirming = false" />
      </template>
    </UAlert>
  </li>
</template>
