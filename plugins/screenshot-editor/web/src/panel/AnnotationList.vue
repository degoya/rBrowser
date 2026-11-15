<script setup lang="ts">
// The annotations of the image (RB-080-01, on Nuxt UI since RB-080-86): each with its tool's name
// (a text with its words) and a Remove button named after it.
import IconAction from '../../../../web-common/panel-ui/src/components/IconAction.vue';
import { usePanel } from '../../../../web-common/panel-ui/src/mount';
import type { Session } from './session';
import type { ShotAnnotation } from './types';

const props = defineProps<{ session: Session }>();
const panel = usePanel();
const t = panel.t;
const state = props.session.state;

const named = (item: ShotAnnotation): string => {
  const tool = props.session.toolName(item.type);
  return item.type === 'text' ? `${tool}: ${item.text ?? ''}` : tool;
};
</script>

<template>
  <section aria-labelledby="annotations-title" class="flex flex-col gap-1.5">
    <h2 id="annotations-title" class="text-sm font-semibold text-highlighted">{{ t('panel.annotations') }}</h2>
    <p v-if="state.items.length === 0" id="annotations-none" class="text-sm text-muted">{{ t('panel.annotations_none') }}</p>
    <ol v-else id="annotations" class="flex flex-col gap-1">
      <li v-for="(item, index) in state.items" :key="index" class="flex min-w-0 items-center gap-2 rounded-md border border-default px-2 py-0.5 text-sm">
        <span class="w-6 shrink-0 text-right tabular-nums text-muted">{{ index + 1 }}</span>
        <span class="min-w-0 flex-1 truncate" :title="named(item)" data-name>{{ named(item) }}</span>
        <IconAction icon="i-lucide-x" :label="t('panel.remove_named', { name: named(item), index: index + 1 })" data-remove @click="session.remove(index)" />
      </li>
    </ol>
  </section>
</template>
