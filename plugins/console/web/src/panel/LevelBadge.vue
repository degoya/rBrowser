<script lang="ts">
// How a level looks (RB-080-84): its Lucide icon and Nuxt UI colour beside its word - never the
// colour alone - and the tint of its row. In a single-file component so the build bundles the
// icons it names.
export const LEVEL_ICON: Record<string, string> = {
  error: 'i-lucide-circle-x',
  warning: 'i-lucide-triangle-alert',
  info: 'i-lucide-info',
  log: 'i-lucide-message-square',
  debug: 'i-lucide-bug',
  network: 'i-lucide-network',
};

const LEVEL_COLOR = { error: 'error', warning: 'warning', info: 'info', log: 'neutral', debug: 'neutral', network: 'secondary' } as const;

export const levelColor = (level: string): (typeof LEVEL_COLOR)[keyof typeof LEVEL_COLOR] => LEVEL_COLOR[level as keyof typeof LEVEL_COLOR] ?? 'neutral';

export const LEVEL_TINT: Record<string, string> = { error: 'bg-error/10', warning: 'bg-warning/10' };
</script>

<script setup lang="ts">
import { usePanel } from '../../../../web-common/panel-ui/src/mount';

defineProps<{ level: string }>();
const { t } = usePanel();
</script>

<template>
  <UBadge data-level :color="levelColor(level)" variant="subtle" size="sm" :icon="LEVEL_ICON[level]" :label="t('levels.' + level)" />
</template>
