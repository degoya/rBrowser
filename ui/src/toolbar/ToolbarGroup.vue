<script setup lang="ts">
// One group of the toolbar's row (RB-080-52), a wrapper around the group's own component: it
// reports its element to the fit (`useToolbarFit.ts`) and draws what the fit decided. `compact`:
// the group's native select is 32 px wide with `icon` over it - its text is pushed out of the
// box, its title stays the tooltip, its list opens as before. `hidden`: the group left the row for
// the overflow menu - invisible and out of the flow, still mounted, so its controls still run. A
// group that renders nothing takes no room and no gap (`empty:hidden`).
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import UIcon from '@nuxt/ui/components/Icon.vue';
import { useToolbarGroup } from './useToolbarFit';

const props = withDefaults(defineProps<{ id: string; icon?: string }>(), { icon: undefined });
const fit = useToolbarGroup();

const root = ref<HTMLElement | null>(null);
const state = computed(() => fit.stateOf(props.id));
/** Where the icon stands: over the select, which is not always the group's first child. */
const iconLeft = ref(0);

watch(state, async (next) => {
  if (next === 'compact') {
    await nextTick();
    iconLeft.value = root.value?.querySelector('select')?.offsetLeft ?? 0;
  }
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
      '[&_select]:w-8 [&_select]:max-w-8 [&_select]:appearance-none [&_select]:pr-0 [&_select]:pl-8':
        state === 'compact',
    }"
  >
    <slot />
    <UIcon
      v-if="state === 'compact' && icon !== undefined"
      :name="icon"
      class="pointer-events-none absolute top-1/2 size-4 -translate-y-1/2"
      :style="{ left: `${iconLeft + 8}px` }"
      data-testid="toolbar-compact-icon"
    />
  </div>
</template>
