<script setup lang="ts">
// Run chosen checks again (RB-094-17): a checkbox per check of the last run and "Run selected
// again", in the progress details of a finished run (`FindingsProgress.vue`). The panel sends the
// run over the last run's target and page, and the core merges each check's result into it.
import { computed, ref, watch } from 'vue';
import { useI18n } from 'vue-i18n';
import UButton from '@nuxt/ui/components/Button.vue';
import UCheckbox from '@nuxt/ui/components/Checkbox.vue';

const props = defineProps<{ plugins: string[]; disabled?: boolean }>();
const emit = defineEmits<{ rerun: [plugins: string[]] }>();
const { t } = useI18n();

const chosen = ref<string[]>([]);
// A check the run no longer has is no choice.
watch(
  () => props.plugins,
  (plugins) => (chosen.value = chosen.value.filter((id) => plugins.includes(id))),
);

function toggle(plugin: string, on: boolean | 'indeterminate'): void {
  const rest = chosen.value.filter((id) => id !== plugin);
  chosen.value = on === true ? props.plugins.filter((id) => id === plugin || rest.includes(id)) : rest;
}

const none = computed(() => chosen.value.length === 0);
</script>

<template>
  <div class="flex flex-wrap items-center gap-3 pb-1 text-sm" data-testid="rerun">
    <fieldset class="flex flex-wrap items-center gap-3">
      <legend class="float-left me-1 font-medium">{{ t('findings.rerun.legend') }}</legend>
      <UCheckbox
        v-for="plugin in plugins"
        :key="plugin"
        :model-value="chosen.includes(plugin)"
        :label="plugin"
        :disabled="disabled"
        size="sm"
        :data-testid="`rerun-choice-${plugin}`"
        @update:model-value="(on) => toggle(plugin, on)"
      />
    </fieldset>
    <UButton
      size="sm"
      variant="outline"
      :disabled="disabled || none"
      data-testid="rerun-selected"
      @click="emit('rerun', [...chosen])"
    >
      {{ t('findings.rerun.selected') }}
    </UButton>
  </div>
</template>
