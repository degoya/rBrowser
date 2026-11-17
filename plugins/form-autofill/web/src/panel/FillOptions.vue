<script setup lang="ts">
// The options of a fill (RB-080-86), folded away under "Options": the form - a choice of the
// forms the last fill found, or a selector typed for another element -, the invalid values as a
// choice, and the seed as a number with a button that draws a new one; empty
// takes the settings' seed.
import { computed, ref } from 'vue';
import { usePanel } from '../../../../web-common/panel-ui/src/mount';
import { type AutofillForm, type FillForm, MAX_SEED, newSeed, STRATEGIES } from './model';

// The profile is chosen above the options, always in sight (`ProfileChoice.vue`, RB-090-71).
const props = defineProps<{ modelValue: FillForm; forms: AutofillForm[] }>();
const emit = defineEmits<{ 'update:modelValue': [FillForm] }>();
const { t } = usePanel();

// A choice cannot carry an empty value: this stands for "all forms".
const ALL = '\u0000all';

const typed = ref<string[]>([]);
const set = (patch: Partial<FillForm>): void => emit('update:modelValue', { ...props.modelValue, ...patch });

const formItems = computed(() => {
  const found = props.forms.map((form, at) => ({ label: form.name || t('panel.formNumber', { number: at + 1 }), value: form.selector, description: form.selector }));
  const known = new Set(found.map((item) => item.value));
  const own = typed.value.filter((selector) => !known.has(selector)).map((selector) => ({ label: selector, value: selector, description: undefined }));
  return [{ label: t('panel.allForms'), value: ALL, description: undefined }, ...found, ...own];
});
const selector = computed({
  get: () => props.modelValue.selector || ALL,
  set: (value: string) => set({ selector: value === ALL ? '' : value }),
});
function create(text: string): void {
  const selector = text.trim().slice(0, 1024);
  if (!selector) return;
  if (!typed.value.includes(selector)) typed.value = [...typed.value, selector];
  set({ selector });
}

const strategyItems = computed(() => STRATEGIES.map((id) => ({ label: t('panel.strategies.' + id), value: id })));
const strategy = computed({ get: () => props.modelValue.strategy, set: (value: string) => set({ strategy: value }) });


const seed = computed({
  get: () => props.modelValue.seed ?? null,
  set: (value: number | null | undefined) => set({ seed: value ?? null }),
});
</script>

<template>
  <div class="grid gap-3 rounded-md border border-default p-3 sm:grid-cols-2">
    <UFormField :label="t('panel.selector')" :help="t('panel.formHint')" class="min-w-0 sm:col-span-2">
      <USelectMenu
        id="selector"
        v-model="selector"
        :items="formItems"
        value-key="value"
        create-item
        size="sm"
        class="w-full"
        :ui="{ itemLabel: 'truncate', itemDescription: 'truncate font-mono' }"
        @create="create"
      />
    </UFormField>
    <UFormField :label="t('panel.strategy')" :help="t('panel.strategyHint')" class="min-w-0">
      <USelect id="strategy" v-model="strategy" :items="strategyItems" size="sm" class="w-full" />
    </UFormField>
    <UFormField :label="t('panel.seed')" class="min-w-0">
      <UFieldGroup size="sm" class="w-full">
        <UInputNumber
          id="seed"
          v-model="seed"
          :min="0"
          :max="MAX_SEED"
          :step="1"
          :increment="false"
          :decrement="false"
          :format-options="{ useGrouping: false, maximumFractionDigits: 0 }"
          :placeholder="t('panel.seedEmpty')"
          class="min-w-0 flex-1"
        />
        <UTooltip :text="t('panel.newSeed')">
          <UButton
            id="new-seed"
            color="neutral"
            variant="outline"
            icon="i-lucide-dices"
            :aria-label="t('panel.newSeed')"
            @click="seed = newSeed()"
          />
        </UTooltip>
      </UFieldGroup>
    </UFormField>
  </div>
</template>
