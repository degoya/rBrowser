<script setup lang="ts">
// The console's filters (RB-060-02, on Nuxt UI since RB-080-84): a toggle button per level with
// its icon, word and count (pressed says it is shown, a check marks it too), the text filter,
// and a toggle button per view with its dot.
import { usePanel } from '../../../../web-common/panel-ui/src/mount';
import { type GroupedAnswer, LEVELS, viewColor } from './model';
import { LEVEL_ICON, levelColor } from './LevelBadge.vue';

defineProps<{ answer: GroupedAnswer | null; levels: Set<string>; views: Set<number> }>();
const emit = defineEmits<{ level: [level: string]; view: [view: number] }>();
const search = defineModel<string>('search', { default: '' });
const { t } = usePanel();

const SHAPE: Record<string, string> = { firefox: 'rounded-[2px]', webkit: 'rotate-45 rounded-[1px]' };
</script>

<template>
  <div class="flex flex-col gap-2">
    <div class="flex flex-wrap items-center gap-2">
      <div id="levels" role="group" :aria-label="t('panel.levels')" class="flex flex-wrap items-center gap-1">
        <UButton
          v-for="level in LEVELS"
          :key="level"
          :data-level="level"
          :aria-pressed="levels.has(level)"
          size="xs"
          :color="levels.has(level) ? levelColor(level) : 'neutral'"
          :variant="levels.has(level) ? 'subtle' : 'ghost'"
          :icon="levels.has(level) ? 'i-lucide-check' : LEVEL_ICON[level]"
          @click="emit('level', level)"
        >
          <span>{{ t('levels.' + level) }}</span>
          <UBadge data-count color="neutral" variant="outline" size="sm" :label="String(answer?.levels?.[level] ?? 0)" />
        </UButton>
      </div>
      <UInput
        id="search"
        v-model="search"
        icon="i-lucide-search"
        size="sm"
        maxlength="200"
        autocomplete="off"
        :placeholder="t('panel.search')"
        :aria-label="t('panel.search')"
        class="min-w-40 flex-1"
      />
    </div>
    <div v-if="(answer?.views ?? []).length > 0" id="views" role="group" :aria-label="t('panel.views')" class="flex flex-wrap items-center gap-1">
      <UTooltip v-for="(view, index) in answer?.views ?? []" :key="view.id" :text="view.breakpoint + ' - ' + view.engine">
        <UButton
          :data-view="view.id"
          :aria-pressed="views.has(view.id)"
          size="xs"
          :color="views.has(view.id) ? 'primary' : 'neutral'"
          :variant="views.has(view.id) ? 'subtle' : 'ghost'"
          @click="emit('view', view.id)"
        >
          <UIcon v-if="views.has(view.id)" name="i-lucide-check" class="size-3.5" />
          <span role="presentation" data-dot :data-engine="view.engine" class="inline-block size-2.5" :class="SHAPE[view.engine] ?? 'rounded-full'" :style="{ background: viewColor(index) }" />
          <span>{{ view.label }}</span>
        </UButton>
      </UTooltip>
    </div>
  </div>
</template>
