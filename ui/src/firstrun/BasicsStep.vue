<script setup lang="ts">
// The setup wizard's basics (RB-091-05), each at its default: rBrowser's theme (the user setting,
// through the registry's `ui.theme.set`), and for the project the wizard set up - else the open
// one - the layout it starts with (grid or canvas) and whether input is synced, written into its
// config. Without a project the two say why they are off. Nothing changes before "Apply".
import { computed, ref } from 'vue';
import { useI18n } from 'vue-i18n';
import UAlert from '@nuxt/ui/components/Alert.vue';
import UButton from '@nuxt/ui/components/Button.vue';
import UFormField from '@nuxt/ui/components/FormField.vue';
import URadioGroup from '@nuxt/ui/components/RadioGroup.vue';
import USwitch from '@nuxt/ui/components/Switch.vue';
import type { SetupLayout } from '../ipc/protocol';
import type { ThemeSetting } from '../theme/theme';
import { THEME_CHOICES } from '../toolbar/themeToggle';
import type { SetupApi } from './api';
import type { BasicsChoice } from './summary';

/** `target`: the project folder the layout and sync go into; `null` without one. */
const props = defineProps<{ api: SetupApi; target: string | null }>();
const emit = defineEmits<{ applied: [choice: BasicsChoice] }>();
const { t, te } = useI18n();

const theme = ref<ThemeSetting>('system');
const layout = ref<SetupLayout>('grid');
const sync = ref(true);
const busy = ref(false);
const failure = ref<string | null>(null);
const done = ref(false);

const themes = computed(() => THEME_CHOICES.map((value) => ({ value, label: t(`firstrun.basics.theme.${value}`) })));
const layouts = computed(() =>
  (['grid', 'canvas'] as const).map((value) => ({
    value,
    label: t(`firstrun.basics.layout.${value}`),
    description: t(`firstrun.basics.layout.${value}_hint`),
  })),
);
const hasProject = computed(() => props.target !== null);

async function apply(): Promise<void> {
  busy.value = true;
  failure.value = null;
  try {
    await props.api.setTheme(theme.value);
    if (hasProject.value) {
      const answer = await props.api.basics(layout.value, sync.value);
      if (answer.op === 'failed') {
        failure.value = te(answer.messageKey) ? t(answer.messageKey, answer.args ?? {}) : t('firstrun.basics.failed');
        return;
      }
    }
    done.value = true;
    emit('applied', {
      theme: theme.value,
      layout: hasProject.value ? layout.value : null,
      sync: hasProject.value ? sync.value : null,
    });
  } catch {
    failure.value = t('firstrun.basics.failed');
  } finally {
    busy.value = false;
  }
}
</script>

<template>
  <section class="flex flex-col gap-4" data-testid="setup-basics" :aria-busy="busy">
    <p class="text-sm text-muted">{{ t('firstrun.basics.intro') }}</p>
    <UFormField :label="t('firstrun.basics.theme.label')">
      <URadioGroup v-model="theme" :items="themes" orientation="horizontal" data-testid="basics-theme" />
    </UFormField>
    <UFormField
      :label="t('firstrun.basics.layout.label')"
      :description="hasProject ? t('firstrun.basics.project_target', { root: target ?? '' }) : t('firstrun.basics.no_project')"
    >
      <URadioGroup v-model="layout" :items="layouts" :disabled="!hasProject" data-testid="basics-layout" />
    </UFormField>
    <UFormField :label="t('firstrun.basics.sync.label')" :description="t('firstrun.basics.sync.hint')">
      <USwitch v-model="sync" :disabled="!hasProject" :label="t(sync ? 'firstrun.basics.sync.on' : 'firstrun.basics.sync.off')" data-testid="basics-sync" />
    </UFormField>
    <div class="flex flex-wrap items-center gap-2">
      <UButton :label="t('firstrun.basics.apply')" :loading="busy" data-testid="basics-apply" @click="apply" />
      <p v-if="done" class="text-sm text-muted" data-testid="basics-done">{{ t('firstrun.basics.applied') }}</p>
    </div>
    <UAlert v-if="failure !== null" color="error" variant="subtle" role="alert" :title="failure" data-testid="basics-failed" />
  </section>
</template>
