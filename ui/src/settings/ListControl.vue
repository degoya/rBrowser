<script setup lang="ts">
// A list of strings (watch paths, URLs): add, remove, move up and down. A list of objects is a
// `CollapsibleList` (RB-080-13). Each item is rendered by the parent through the `item` slot, so this component
// never imports the field renderer (no import cycle). No entry is ever saved empty (RB-080-72):
// "Add" opens a line that is saved once it holds text, and an entry emptied by hand is dropped -
// the free-text list before RB-080-20 saved `""` on "Add" (the owner's `"categories": [""]`).
// The focus stays in the list (RB-090-23): a moved entry keeps it on the arrow that moved it, or on
// the other arrow when that one disabled itself at either end; a removal hands it to the next
// Remove, or to "Add" when the list is empty.
import { computed, nextTick, ref, watch } from 'vue';
import { useI18n } from 'vue-i18n';
import UButton from '@nuxt/ui/components/Button.vue';
import type { JsonValue } from '../ipc/protocol';
import { SETTINGS_ICONS } from './icons';

const props = defineProps<{
  items: JsonValue[];
  label: string;
  disabled?: boolean;
  /** The value a new item starts with. */
  fresh: () => JsonValue;
}>();
const emit = defineEmits<{ update: [items: JsonValue[]] }>();
const { t } = useI18n();
/** The line "Add" opened, not saved until it holds text. */
const draft = ref(false);
const rows = computed<JsonValue[]>(() => (draft.value ? [...props.items, props.fresh()] : props.items));

type Action = 'up' | 'down' | 'remove';
const root = ref<HTMLElement | null>(null);
/** Where the focus goes once the parent has applied a move or a removal. */
let pending: { index: number; action: Action } | null = null;

/** Notes the target only when the focus is in the list: a click elsewhere never pulls it here. */
function follow(index: number, action: Action): void {
  pending = root.value?.contains(document.activeElement) === true ? { index, action } : null;
}

function control(index: number, action: Action): HTMLElement | null {
  return root.value?.querySelector<HTMLElement>(`[data-index="${index}"] [data-action="${action}"]`) ?? null;
}

function usable(element: HTMLElement | null | undefined): element is HTMLElement {
  return element !== null && element !== undefined && !element.matches(':disabled');
}

function focusPending(): void {
  const target = pending;
  pending = null;
  if (target === null) {
    return;
  }
  if (target.action === 'remove') {
    const last = rows.value.length - 1;
    const next =
      last < 0
        ? root.value?.querySelector<HTMLElement>('[data-testid="list-add"]')
        : control(Math.min(target.index, last), 'remove');
    if (usable(next)) {
      next.focus();
    }
    return;
  }
  const own = control(target.index, target.action);
  const other = control(target.index, target.action === 'up' ? 'down' : 'up');
  const next = usable(own) ? own : other;
  if (usable(next)) {
    next.focus();
  }
}

watch(() => props.items, focusPending, { flush: 'post' });

function isEmpty(value: JsonValue): boolean {
  return typeof value === 'string' && value.trim() === '';
}

function replace(index: number, value: JsonValue): void {
  // An edit is no move: a move or removal the parent did not apply must not take the focus later.
  pending = null;
  if (index >= props.items.length) {
    if (!isEmpty(value)) {
      draft.value = false;
      emit('update', [...props.items, value]);
    }
    return;
  }
  emit(
    'update',
    isEmpty(value) ? props.items.filter((_, i) => i !== index) : props.items.map((item, i) => (i === index ? value : item)),
  );
}

function remove(index: number): void {
  follow(index, 'remove');
  if (index >= props.items.length) {
    draft.value = false;
    void nextTick(focusPending);
    return;
  }
  emit(
    'update',
    props.items.filter((_, i) => i !== index),
  );
}

function move(index: number, by: -1 | 1): void {
  const target = index + by;
  if (target < 0 || target >= props.items.length) {
    return;
  }
  const next = [...props.items];
  const [item] = next.splice(index, 1);
  if (item !== undefined) {
    follow(target, by < 0 ? 'up' : 'down');
    next.splice(target, 0, item);
    emit('update', next);
  }
}

function add(): void {
  draft.value = true;
}
</script>

<template>
  <div ref="root" class="flex flex-col gap-2" role="group" :aria-label="label">
    <p v-if="rows.length === 0" class="text-sm text-muted">{{ t('settings.list.empty') }}</p>
    <div
      v-for="(item, index) in rows"
      :key="index"
      class="flex items-start gap-2 rounded-md border border-default p-2"
      :data-index="index"
      data-testid="list-item"
    >
      <div class="min-w-0 flex-1">
        <slot name="item" :item="item" :index="index" :update="(value: JsonValue) => replace(index, value)" />
      </div>
      <div class="flex shrink-0 items-center gap-0.5">
        <UButton
          :icon="SETTINGS_ICONS.up"
          color="neutral"
          variant="ghost"
          size="xs"
          square
          :disabled="disabled || index === 0 || index >= items.length"
          :aria-label="t('settings.list.move_up', { n: index + 1 })"
          :title="t('settings.list.move_up', { n: index + 1 })"
          data-action="up"
          @click="move(index, -1)"
        />
        <UButton
          :icon="SETTINGS_ICONS.down"
          color="neutral"
          variant="ghost"
          size="xs"
          square
          :disabled="disabled || index >= items.length - 1"
          :aria-label="t('settings.list.move_down', { n: index + 1 })"
          :title="t('settings.list.move_down', { n: index + 1 })"
          data-action="down"
          @click="move(index, 1)"
        />
        <UButton
          :icon="SETTINGS_ICONS.remove"
          color="error"
          variant="ghost"
          size="xs"
          square
          :disabled="disabled"
          :aria-label="t('settings.list.remove', { n: index + 1 })"
          :title="t('settings.list.remove', { n: index + 1 })"
          data-action="remove"
          data-testid="list-remove"
          @click="remove(index)"
        />
      </div>
    </div>
    <div>
      <UButton
        :icon="SETTINGS_ICONS.add"
        color="neutral"
        variant="soft"
        size="xs"
        :disabled="disabled || draft"
        :label="t('settings.list.add')"
        data-testid="list-add"
        @click="add"
      />
    </div>
  </div>
</template>
