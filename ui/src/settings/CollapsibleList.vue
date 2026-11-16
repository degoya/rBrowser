<script setup lang="ts">
// A list of objects - breakpoints, device presets, grid cells, layout presets - as one line per
// entry (RB-080-13): the summary closed, the entry's fields open. Each line is a button
// (Enter/Space opens it, Alt+Up/Down moves it) with move buttons beside it and a drag handle;
// the open entry ends in its action row: duplicate, and delete with a confirmation in place (no
// browser dialog). The entry's fields come from the parent through the `item` slot, so this
// component never imports the field renderer (no import cycle).
import { nextTick, ref, useId, watch } from 'vue';
import { useI18n } from 'vue-i18n';
import UButton from '@nuxt/ui/components/Button.vue';
import UIcon from '@nuxt/ui/components/Icon.vue';
import type { JsonValue } from '../ipc/protocol';
import { SETTINGS_ICONS } from './icons';
import { moveItem } from './listOps';

const props = defineProps<{
  items: JsonValue[];
  label: string;
  disabled?: boolean;
  /** The value a new entry starts with. */
  fresh: () => JsonValue;
  /** The copy of an entry for "Duplicate". */
  copy: (item: JsonValue, items: readonly JsonValue[]) => JsonValue;
  /** The one line of a closed entry. */
  summary: (item: JsonValue, index: number) => string;
}>();
const emit = defineEmits<{ update: [items: JsonValue[]] }>();
const { t } = useI18n();
const uid = useId();

// A stable key per entry, so an open entry stays open (and keeps the focus) when it moves. The
// list changes only once the core has saved it; until then the keys of the change wait.
let counter = 0;
const nextKey = (): number => (counter += 1);
const keys = ref<number[]>(props.items.map(nextKey));
const opened = ref<number[]>([]);
const confirming = ref<number | null>(null);
const dragFrom = ref<number | null>(null);
let pending: { json: string; keys: number[]; focus: number | null } | null = null;

watch(
  () => props.items,
  (items) => {
    const change = pending;
    pending = null;
    if (change !== null && change.json === JSON.stringify(items)) {
      keys.value = change.keys;
      if (change.focus !== null) {
        const focus = change.focus;
        void nextTick(() => document.getElementById(triggerId(focus))?.focus());
      }
    } else if (items.length !== keys.value.length) {
      keys.value = items.map(nextKey);
    }
  },
);

function triggerId(key: number): string {
  return `${uid}-entry-${key}`;
}

function isOpen(index: number): boolean {
  const key = keys.value[index];
  return key !== undefined && opened.value.includes(key);
}

function toggle(index: number): void {
  const key = keys.value[index];
  if (key === undefined) return;
  opened.value = isOpen(index) ? opened.value.filter((k) => k !== key) : [...opened.value, key];
}

function change(items: JsonValue[], nextKeys: number[], focus: number | null = null): void {
  pending = { json: JSON.stringify(items), keys: nextKeys, focus };
  emit('update', items);
}

function move(from: number, to: number, keepFocus = false): void {
  if (props.disabled || to < 0 || to >= props.items.length || from === to) return;
  change(moveItem(props.items, from, to), moveItem(keys.value, from, to), keepFocus ? (keys.value[from] ?? null) : null);
}

function add(): void {
  const key = nextKey();
  opened.value = [...opened.value, key];
  change([...props.items, props.fresh()], [...keys.value, key]);
}

function duplicate(index: number): void {
  const item = props.items[index];
  if (item === undefined) return;
  const key = nextKey();
  opened.value = [...opened.value, key];
  const items = [...props.items];
  items.splice(index + 1, 0, props.copy(item, props.items));
  const nextKeys = [...keys.value];
  nextKeys.splice(index + 1, 0, key);
  change(items, nextKeys);
}

function remove(index: number): void {
  confirming.value = null;
  change(
    props.items.filter((_, i) => i !== index),
    keys.value.filter((_, i) => i !== index),
  );
}

function replace(index: number, value: JsonValue): void {
  change(
    props.items.map((item, i) => (i === index ? value : item)),
    keys.value,
  );
}

function onDragStart(index: number, event: DragEvent): void {
  dragFrom.value = index;
  event.dataTransfer?.setData('text/plain', String(index));
  if (event.dataTransfer) event.dataTransfer.effectAllowed = 'move';
}

function onDrop(index: number): void {
  if (dragFrom.value !== null) move(dragFrom.value, index);
  dragFrom.value = null;
}
</script>

<template>
  <div class="flex flex-col gap-2" role="group" :aria-label="label">
    <p
      v-if="items.length === 0"
      class="rounded-md border border-dashed border-default px-3 py-4 text-center text-sm text-muted"
      data-testid="list-empty"
    >
      {{ t('settings.list.empty') }}
    </p>
    <ul v-else class="flex flex-col gap-1.5">
      <li
        v-for="(item, index) in items"
        :key="keys[index]"
        class="rounded-md bg-default ring ring-default"
        data-testid="list-item"
        :data-open="isOpen(index) ? 'true' : undefined"
        @dragover.prevent
        @drop.prevent="onDrop(index)"
      >
        <div class="flex items-center gap-0.5 pe-1">
          <span
            v-if="!disabled"
            draggable="true"
            class="flex cursor-grab items-center self-stretch ps-1.5 text-dimmed hover:text-muted"
            aria-hidden="true"
            :title="t('settings.list.drag')"
            data-testid="list-drag"
            @dragstart="onDragStart(index, $event)"
            @dragend="dragFrom = null"
          >
            <UIcon :name="SETTINGS_ICONS.grip" class="size-4" />
          </span>
          <!-- Exception to Nuxt UI first (design.md 5.1, RB-080-88): the line's own button, not a
               `UCollapsible` or `UAccordion` trigger - the line is dragged by its handle, moved
               with Alt+Up/Down and carries its move buttons in the same row, which neither
               component's trigger takes. -->
          <button
            :id="triggerId(keys[index] ?? 0)"
            type="button"
            class="flex min-w-0 flex-1 items-center gap-1.5 rounded-md px-1.5 py-1.5 text-start text-sm text-default hover:bg-elevated/50 focus-visible:outline-2 focus-visible:outline-primary"
            :aria-expanded="isOpen(index) ? 'true' : 'false'"
            :aria-controls="`${triggerId(keys[index] ?? 0)}-panel`"
            data-testid="list-toggle"
            @click="toggle(index)"
            @keydown.alt.up.prevent="move(index, index - 1, true)"
            @keydown.alt.down.prevent="move(index, index + 1, true)"
          >
            <UIcon
              :name="SETTINGS_ICONS.expand"
              class="size-4 shrink-0 text-muted transition-transform motion-reduce:transition-none"
              :class="isOpen(index) ? 'rotate-90' : ''"
            />
            <span class="truncate">{{ summary(item, index) }}</span>
          </button>
          <UButton
            :icon="SETTINGS_ICONS.up"
            color="neutral"
            variant="ghost"
            size="xs"
            square
            :disabled="disabled || index === 0"
            :aria-label="t('settings.list.move_up', { n: index + 1 })"
            :title="t('settings.list.move_up', { n: index + 1 })"
            data-testid="list-up"
            @click="move(index, index - 1)"
          />
          <UButton
            :icon="SETTINGS_ICONS.down"
            color="neutral"
            variant="ghost"
            size="xs"
            square
            :disabled="disabled || index === items.length - 1"
            :aria-label="t('settings.list.move_down', { n: index + 1 })"
            :title="t('settings.list.move_down', { n: index + 1 })"
            data-testid="list-down"
            @click="move(index, index + 1)"
          />
        </div>
        <div
          v-if="isOpen(index)"
          :id="`${triggerId(keys[index] ?? 0)}-panel`"
          role="region"
          :aria-labelledby="triggerId(keys[index] ?? 0)"
          class="flex flex-col gap-3 border-t border-default p-3"
        >
          <slot name="item" :item="item" :index="index" :update="(value: JsonValue) => replace(index, value)" />
          <div
            v-if="confirming === keys[index]"
            class="flex flex-wrap items-center gap-2 text-sm"
            data-testid="list-confirm"
          >
            <span class="text-highlighted">{{ t('settings.list.confirm_remove') }}</span>
            <UButton
              :icon="SETTINGS_ICONS.remove"
              color="error"
              size="xs"
              :label="t('settings.list.delete')"
              data-testid="list-remove-confirm"
              @click="remove(index)"
            />
            <UButton
              :icon="SETTINGS_ICONS.close"
              color="neutral"
              variant="ghost"
              size="xs"
              square
              :aria-label="t('settings.list.cancel_remove', { n: index + 1 })"
              :title="t('settings.list.cancel_remove', { n: index + 1 })"
              data-testid="list-remove-cancel"
              @click="confirming = null"
            />
          </div>
          <div v-else class="flex flex-wrap items-center gap-2">
            <UButton
              :icon="SETTINGS_ICONS.duplicate"
              color="neutral"
              variant="soft"
              size="xs"
              :disabled="disabled"
              :label="t('settings.list.duplicate')"
              data-testid="list-duplicate"
              @click="duplicate(index)"
            />
            <UButton
              :icon="SETTINGS_ICONS.remove"
              color="error"
              variant="ghost"
              size="xs"
              :disabled="disabled"
              :label="t('settings.list.delete')"
              data-testid="list-remove"
              @click="confirming = keys[index] ?? null"
            />
          </div>
        </div>
      </li>
    </ul>
    <div>
      <UButton
        :icon="SETTINGS_ICONS.add"
        color="neutral"
        variant="soft"
        size="xs"
        :disabled="disabled"
        :label="t('settings.list.add')"
        data-testid="list-add"
        @click="add"
      />
    </div>
  </div>
</template>
