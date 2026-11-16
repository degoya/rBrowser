<script setup lang="ts">
// The emulation controls of the toolbar (RB-010-05): colour scheme, reduced motion, the extras of
// RB-080-10 (geolocation, time zone, CPU, offline, print view) and reset for the active view;
// the sun and moon set the pages' colour scheme, never rBrowser's own theme, and their tooltips
// say so (that is `ThemeToggle.vue`, RB-080-95);
// Shift-click applies the same to every view (`emulation.set` with `target: "all"`). The pressed
// state is the active view's, read with `getEmulation`.
import { computed, inject, ref, watch } from 'vue';
import { useI18n } from 'vue-i18n';
import UButton from '@nuxt/ui/components/Button.vue';
import type { ColorScheme, Emulation } from '../ipc/protocol';
import { useShell } from '../state/shell';
import EmulationExtras from './EmulationExtras.vue';
import {
  EMULATION_QUERY_KEY,
  nextReducedMotion,
  resetMessage,
  setMessage,
  type EmulationFieldsPatch,
} from './emulation';

const shell = useShell();
const { t } = useI18n();
const query = inject(EMULATION_QUERY_KEY, null);

const activeView = computed(
  () => shell.state.value?.activeView ?? shell.state.value?.views[0]?.id ?? null,
);
const current = ref<Emulation | null>(null);
const disabled = computed(() => activeView.value === null);

async function refresh(): Promise<void> {
  const view = activeView.value;
  if (query === null || view === null) {
    current.value = null;
    return;
  }
  try {
    current.value = await query(view);
  } catch {
    current.value = null;
  }
}

watch(activeView, () => void refresh(), { immediate: true });

async function apply(fields: EmulationFieldsPatch, event: MouseEvent): Promise<void> {
  if (await shell.send(setMessage(fields, event.shiftKey))) {
    await refresh();
  }
}

function setScheme(scheme: ColorScheme, event: MouseEvent): void {
  void apply({ colorScheme: scheme }, event);
}

function toggleReducedMotion(event: MouseEvent): void {
  void apply({ reducedMotion: nextReducedMotion(current.value) }, event);
}

async function reset(event: MouseEvent): Promise<void> {
  if (await shell.send(resetMessage(event.shiftKey))) {
    await refresh();
  }
}

const hint = (key: string): string => `${t(key)} - ${t('emulation.toolbar.allHint')}`;
</script>

<template>
  <div
    role="group"
    :aria-label="t('emulation.toolbar.label')"
    class="flex shrink-0 items-center gap-0.5"
    data-testid="emulation"
  >
    <UButton
      icon="i-lucide-sun"
      color="neutral"
      :variant="current?.colorScheme === 'light' ? 'soft' : 'ghost'"
      size="sm"
      square
      :disabled="disabled"
      :aria-pressed="current?.colorScheme === 'light'"
      :aria-label="t('emulation.toolbar.light')"
      :title="t('emulation.toolbar.pageLightHint')"
      data-testid="emulation-light"
      @click="setScheme('light', $event)"
    />
    <UButton
      icon="i-lucide-moon"
      color="neutral"
      :variant="current?.colorScheme === 'dark' ? 'soft' : 'ghost'"
      size="sm"
      square
      :disabled="disabled"
      :aria-pressed="current?.colorScheme === 'dark'"
      :aria-label="t('emulation.toolbar.dark')"
      :title="t('emulation.toolbar.pageDarkHint')"
      data-testid="emulation-dark"
      @click="setScheme('dark', $event)"
    />
    <UButton
      icon="i-lucide-pause"
      color="neutral"
      :variant="current?.reducedMotion === true ? 'soft' : 'ghost'"
      size="sm"
      square
      :disabled="disabled"
      :aria-pressed="current?.reducedMotion === true"
      :aria-label="t('emulation.toolbar.reducedMotion')"
      :title="hint('emulation.toolbar.reducedMotion')"
      data-testid="emulation-reduced-motion"
      @click="toggleReducedMotion($event)"
    />
    <EmulationExtras :current="current" :disabled="disabled" @changed="refresh" />
    <UButton
      icon="i-lucide-undo-2"
      color="neutral"
      variant="ghost"
      size="sm"
      square
      :disabled="disabled"
      :aria-label="t('emulation.toolbar.reset')"
      :title="hint('emulation.toolbar.reset')"
      data-testid="emulation-reset"
      @click="reset($event)"
    />
  </div>
</template>
