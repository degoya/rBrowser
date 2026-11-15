<script setup lang="ts">
// The toolbar's DevTools indicator (RB-010-06): shown while DevTools are open, it names the
// inspected view and where DevTools are; a click runs `devtools.toggle`, which closes them. The
// visible label is its name, the title its description (label in name, RB-090-23).
import { computed } from 'vue';
import { useI18n } from 'vue-i18n';
import UButton from '@nuxt/ui/components/Button.vue';
import { useShell } from '../state/shell';
import { devtoolsIndicator } from './indicator';

const shell = useShell();
const { t } = useI18n();

const indicator = computed(() => devtoolsIndicator(shell.state.value));
const title = computed(() =>
  indicator.value === null
    ? ''
    : t('shell.devtools.title', { view: indicator.value.view, mode: t(indicator.value.modeKey) }),
);

function close(): void {
  void shell.send({ type: 'runCommand', id: 'devtools.toggle' });
}
</script>

<template>
  <UButton
    v-if="indicator !== null"
    color="neutral"
    variant="soft"
    size="sm"
    class="max-w-48 shrink truncate"
    :label="t('shell.devtools.label', { view: indicator.view })"
    :title="title"
    data-testid="devtools-indicator"
    @click="close"
  />
</template>
