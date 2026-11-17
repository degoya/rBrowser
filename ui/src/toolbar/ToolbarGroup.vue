<script setup lang="ts">
// One group of the toolbar's row (RB-080-52), a wrapper around the group's own component: it
// reports its element to the fit (`useToolbarFit.ts`) and draws what the fit decided. `compact`:
// the group's pulldown (`PulldownButton.vue`, RB-090-45) is a 32 px button with `icon` alone - its
// name stays the tooltip, its menu opens as before. `hidden`: the group left the row for the
// overflow menu - invisible and out of the flow, still mounted, so its controls still run. A
// group that renders nothing takes no room and no gap (`empty:hidden`).
import { computed, onBeforeUnmount, onMounted, provide, ref } from 'vue';
import { GROUP_LOOK_KEY } from './pulldown';
import { useToolbarGroup } from './useToolbarFit';

const props = withDefaults(defineProps<{ id: string; icon?: string }>(), { icon: undefined });
const fit = useToolbarGroup();

const root = ref<HTMLElement | null>(null);
const state = computed(() => fit.stateOf(props.id));

provide(GROUP_LOOK_KEY, {
  compact: computed(() => state.value === 'compact'),
  icon: computed(() => props.icon),
});

onMounted(() => fit.register(props.id, root.value));
onBeforeUnmount(() => fit.register(props.id, null));
</script>

<template>
  <div
    ref="root"
    :data-toolbar-group="id"
    :data-state="state"
    :aria-hidden="state === 'hidden' ? 'true' : undefined"
    class="flex shrink-0 items-center empty:hidden"
    :class="{
      relative: state !== 'hidden',
      'pointer-events-none invisible absolute top-1.5 right-0': state === 'hidden',
    }"
  >
    <slot />
  </div>
</template>
