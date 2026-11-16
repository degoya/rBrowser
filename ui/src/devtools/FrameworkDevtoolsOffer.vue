<script setup lang="ts">
// The offer of the Vue and React DevTools for the active view (RB-080-24): one button per
// framework the core's probe found in the active view's page (`state.frameworks`, listed only
// while the framework's setting is not off). A click runs the plugin's command, which opens the
// panel; the title says when the view needs a reload (the hook was registered after the page
// loaded) or why a Vue production build shows nothing. The buttons are `UButton`s (RB-080-88): a
// framework ready to inspect in the primary colour, one that needs a reload first in neutral.
import { computed } from 'vue';
import { useI18n } from 'vue-i18n';
import UButton from '@nuxt/ui/components/Button.vue';
import type { Framework, FrameworkInfo } from '../ipc/frameworks';
import { useShell } from '../state/shell';

const shell = useShell();
const { t } = useI18n();

const NAMES: Record<Framework, string> = { vue: 'Vue', react: 'React' };
const COMMANDS: Record<Framework, string> = {
  vue: 'plugin.framework_devtools.vue',
  react: 'plugin.framework_devtools.react',
};

const offers = computed((): { framework: Framework; info: FrameworkInfo }[] => {
  const state = shell.state.value;
  if (!state || state.activeView === null) return [];
  const entry = state.frameworks?.find((item) => item.view === state.activeView);
  if (!entry) return [];
  const list: { framework: Framework; info: FrameworkInfo }[] = [];
  if (entry.vue) list.push({ framework: 'vue', info: entry.vue });
  if (entry.react) list.push({ framework: 'react', info: entry.react });
  return list;
});

function title(framework: Framework, info: FrameworkInfo): string {
  const name = NAMES[framework];
  if (!info.hook) return t('devtools.framework.reload', { framework: name });
  if (!info.devtools && framework === 'vue') return t('devtools.framework.vue_prod');
  if (!info.devtools) return t('devtools.framework.reload', { framework: name });
  return t('devtools.framework.offer_title', { framework: name });
}

function ready(info: FrameworkInfo): boolean {
  return info.hook && info.devtools;
}

function open(framework: Framework): void {
  void shell.send({ type: 'runCommand', id: COMMANDS[framework] });
}
</script>

<template>
  <div v-if="offers.length > 0" class="flex shrink-0 items-center gap-1" data-testid="framework-devtools-offer">
    <UButton
      v-for="offer in offers"
      :key="offer.framework"
      size="xs"
      variant="soft"
      :color="ready(offer.info) ? 'primary' : 'neutral'"
      class="shrink-0"
      :title="title(offer.framework, offer.info)"
      :data-ready="String(ready(offer.info))"
      :data-testid="`framework-devtools-${offer.framework}`"
      @click="open(offer.framework)"
    >
      {{ t('devtools.framework.offer', { framework: NAMES[offer.framework] }) }}
    </UButton>
  </div>
</template>
