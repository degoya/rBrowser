<script setup lang="ts">
// The setup wizard's basics (RB-091-05), each at its default: rBrowser's theme (the user setting,
// through the registry's `ui.theme.set`), and for the project the wizard set up - else the open
// one - the layout it starts with (the canvas, rBrowser's built-in mode since RB-099-26, or the
// grid) and whether input is synced, written into its config. Without a project the two say why they are off. Nothing changes before the wizard's
// "Apply & continue" (RB-091-08: the footer button, `next`, carries it out).
import { computed, ref } from 'vue';
import { useI18n } from 'vue-i18n';
import UAlert from '@nuxt/ui/components/Alert.vue';
import UFormField from '@nuxt/ui/components/FormField.vue';
import URadioGroup from '@nuxt/ui/components/RadioGroup.vue';
import USwitch from '@nuxt/ui/components/Switch.vue';
import type { SetupLayout } from '../ipc/protocol';
import type { ThemeSetting } from '../theme/theme';
import { THEME_CHOICES } from '../toolbar/themeToggle';
import type { SetupApi } from './api';
import type { NextAction } from './steps';
import type { BasicsChoice } from './summary';

/** `target`: the project folder the layout and sync go into; `null` without one. */
const props = defineProps<{ api: SetupApi; target: string | null }>();
const emit = defineEmits<{ applied: [choice: BasicsChoice] }>();
const { t, te } = useI18n();

const theme = ref<ThemeSetting>('system');
const layout = ref<SetupLayout>('canvas');
const sync = ref(true);
const busy = ref(false);
const failure = ref<string | null>(null);

const themes = computed(() => THEME_CHOICES.map((value) => ({ value, label: t(`firstrun.basics.theme.${value}`) })));
const layouts = computed(() =>
  (['grid', 'canvas'] as const).map((value) => ({
    value,
    label: t(`firstrun.basics.layout.${value}`),
    description: t(`firstrun.basics.layout.${value}_hint`),
  })),
);
const hasProject = computed(() => props.target !== null);

/** Applies the choices; `true` when they were. */
async function apply(): Promise<boolean> {
  busy.value = true;
  failure.value = null;
  try {
    await props.api.setTheme(theme.value);
    if (hasProject.value) {
      const answer = await props.api.basics(layout.value, sync.value);
      if (answer.op === 'failed') {
        failure.value = te(answer.messageKey) ? t(answer.messageKey, answer.args ?? {}) : t('firstrun.basics.failed');
        return false;
      }
    }
    emit('applied', {
      theme: theme.value,
      layout: hasProject.value ? layout.value : null,
      sync: hasProject.value ? sync.value : null,
    });
    return true;
  } catch {
    failure.value = t('firstrun.basics.failed');
    return false;
  } finally {
    busy.value = false;
  }
}

const next = computed<NextAction>(() => ({ key: 'firstrun.basics.apply_next', busy: busy.value }));

defineExpose({ next, proceed: apply });
</script>

<template>
  <section class="flex flex-col gap-4" data-testid="setup-basics" :aria-busy="busy">
    <p class="text-sm text-muted">{{ t('firstrun.basics.intro') }}</p>
    <UFormField :label="t('firstrun.basics.theme.label')">
      <!-- A radio group is no labelable element: the field's label does not name it (RB-090-08). -->
      <URadioGroup v-model="theme" :items="themes" :aria-label="t('firstrun.basics.theme.label')" orientation="horizontal" data-testid="basics-theme" />
    </UFormField>
    <UFormField
      :label="t('firstrun.basics.layout.label')"
      :description="hasProject ? t('firstrun.basics.project_target', { root: target ?? '' }) : t('firstrun.basics.no_project')"
    >
      <URadioGroup v-model="layout" :items="layouts" :aria-label="t('firstrun.basics.layout.label')" :disabled="!hasProject" data-testid="basics-layout" />
    </UFormField>
    <UFormField :label="t('firstrun.basics.sync.label')" :description="t('firstrun.basics.sync.hint')">
      <USwitch v-model="sync" :disabled="!hasProject" :label="t(sync ? 'firstrun.basics.sync.on' : 'firstrun.basics.sync.off')" data-testid="basics-sync" />
    </UFormField>
    <UAlert v-if="failure !== null" color="error" variant="subtle" role="alert" :title="failure" data-testid="basics-failed" />
  </section>
</template>
