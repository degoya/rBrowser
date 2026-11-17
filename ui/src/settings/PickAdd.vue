<script setup lang="ts">
// The "Add ..." control of a pick list (RB-080-72): Nuxt UI's searchable select (`USelectMenu`,
// docs/design.md 5.1 "Nuxt UI before anything of our own" and rule 7 "a long list searches as you
// type") over the values not chosen yet - each by name with its id below, searched by both.
// Choosing one adds it at once. An open source (`open`: the check rules, a rule of a plugin not
// loaded) also takes the typed text ("Create ..."); a closed one offers only what there is.
// While the core still enumerates the values (`loading`) the control says so.
import { computed, ref } from 'vue';
import { useI18n } from 'vue-i18n';
import USelectMenu from '@nuxt/ui/components/SelectMenu.vue';
import { type Picks, resolvePick } from './picks';

const props = defineProps<{
  picks: Picks;
  /** The values chosen already: not offered again. */
  chosen: readonly string[];
  inputId: string;
  /** What the control says it adds ("Add rule"). */
  addLabel: string;
  disabled?: boolean;
}>();
const emit = defineEmits<{ add: [value: string] }>();
const { t } = useI18n();

const picked = ref<string | undefined>(undefined);
const items = computed(() =>
  props.picks.options
    .filter((option) => !props.chosen.includes(option.value))
    .map((option) => ({ label: option.label, value: option.value, description: option.value })),
);

function add(value: string | undefined): void {
  picked.value = undefined;
  if (value !== undefined && value !== '' && !props.chosen.includes(value)) {
    emit('add', value);
  }
}

/** A typed value of an open list: an offered name typed in full is that value. */
function create(text: string): void {
  add(resolvePick(props.picks, text) ?? undefined);
}
</script>

<template>
  <div class="flex flex-col gap-1">
    <label :for="`${inputId}-add`" class="text-xs font-medium text-muted" data-testid="pick-add-label">{{ addLabel }}</label>
    <USelectMenu
      :id="`${inputId}-add`"
      v-model="picked"
      :items="items"
      value-key="value"
      :filter-fields="['label', 'value']"
      :create-item="picks.open ? 'always' : false"
      :search-input="{ placeholder: t('settings.pick.placeholder') }"
      :placeholder="picks.loading ? t('settings.pick.loading') : t('settings.pick.choose')"
      :loading="picks.loading"
      :disabled="disabled"
      :aria-label="addLabel"
      class="w-full max-w-sm"
      data-testid="pick-add"
      @update:model-value="add"
      @create="create"
    />
  </div>
</template>
