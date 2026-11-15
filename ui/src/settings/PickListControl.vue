<script setup lang="ts">
// A list of values rBrowser knows (RB-080-72, `x-options` names the source: `plugins`,
// `plugin-panels`, `plugin-page-scripts`): the chosen values by name with the id beside it, each
// with a remove button, and a line that adds one of the values not chosen yet. A chosen value the
// core no longer offers (a removed plugin) stays, marked "Not found", so nothing is lost silently;
// an entry that is no text at all (a hand-edited file) stays too, shown as its JSON and marked
// "Not allowed", until it is removed. No entry is ever empty.
import { computed } from 'vue';
import { useI18n } from 'vue-i18n';
import UBadge from '@nuxt/ui/components/Badge.vue';
import UButton from '@nuxt/ui/components/Button.vue';
import type { JsonValue } from '../ipc/protocol';
import { SETTINGS_ICONS } from './icons';
import PickAdd from './PickAdd.vue';
import { isMissing, nameOf, picksOf } from './picks';
import type { DynamicData } from './schema';

const props = defineProps<{
  value: JsonValue[];
  /** The accessible name. */
  label: string;
  inputId: string;
  disabled?: boolean;
  dynamic?: DynamicData;
  /** The texts of the source (`settings.pick.<source>.*`) when it has its own. */
  texts: { add: string; empty: string };
}>();
const emit = defineEmits<{ update: [value: JsonValue[]] }>();
const { t } = useI18n();

const picks = computed(() => picksOf(props.dynamic));
const chosen = computed(() => props.value.filter((item): item is string => typeof item === 'string'));

/** What a row shows: a text entry by name, any other as its JSON. */
function shown(item: JsonValue): string {
  return typeof item === 'string' ? nameOf(picks.value, item) : JSON.stringify(item);
}

function add(value: string): void {
  emit('update', [...props.value, value]);
}

function remove(index: number): void {
  emit(
    'update',
    props.value.filter((_, i) => i !== index),
  );
}
</script>

<template>
  <div class="flex flex-col gap-2" role="group" :aria-label="label" data-testid="pick-list">
    <p v-if="value.length === 0" class="text-sm text-muted" data-testid="pick-empty">{{ texts.empty }}</p>
    <ul v-else class="flex flex-col gap-1">
      <li
        v-for="(item, index) in value"
        :key="index"
        class="flex items-center gap-2 rounded-md border border-default px-2 py-1"
        data-testid="pick-item"
      >
        <span class="min-w-0 flex-1 truncate text-sm" :title="typeof item === 'string' ? item : undefined">
          <span data-testid="pick-name">{{ shown(item) }}</span>
          <span v-if="typeof item === 'string' && shown(item) !== item" class="ml-2 font-mono text-xs text-muted" data-testid="pick-id">{{ item }}</span>
        </span>
        <UBadge
          v-if="typeof item !== 'string'"
          color="error"
          variant="subtle"
          size="sm"
          :label="t('settings.pick.not_allowed')"
          data-testid="pick-not-allowed"
        />
        <UBadge
          v-else-if="isMissing(picks, item, dynamic !== undefined && !dynamic.loading)"
          color="warning"
          variant="subtle"
          size="sm"
          :label="t('settings.pick.missing')"
          data-testid="pick-missing"
        />
        <UButton
          :icon="SETTINGS_ICONS.remove"
          color="neutral"
          variant="ghost"
          size="xs"
          square
          :disabled="disabled"
          :aria-label="t('settings.pick.remove', { name: shown(item) })"
          :title="t('settings.pick.remove', { name: shown(item) })"
          data-testid="pick-remove"
          @click="remove(index)"
        />
      </li>
    </ul>
    <PickAdd :picks="picks" :chosen="chosen" :input-id="inputId" :add-label="texts.add" :disabled="disabled" @add="add" />
  </div>
</template>
