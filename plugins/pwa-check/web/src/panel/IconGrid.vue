<script setup lang="ts">
// The manifest's icons (RB-080-08, on Nuxt UI since RB-080-85): a card per icon with its preview
// on a checkerboard, its load state as a badge, its declared and actual size and its purpose.
// The maskable safe zone - a dashed circle of 80 % of the icon, centred - is the panel's own
// element: Nuxt UI has none (RB-080-85, job file).
import { computed } from 'vue';
import { usePanel } from '../../../../web-common/panel-ui/src/mount';
import { iconCard, type IconPreview, type PageReport, text } from './model';

const props = defineProps<{ page: PageReport; previews: Map<string, IconPreview> }>();
const { strings } = usePanel();

const cards = computed(() =>
  props.page.icons.map((icon) => {
    const preview = icon.url ? props.previews.get(icon.url) : undefined;
    return { icon, preview, card: iconCard(strings.value, icon, preview) };
  }),
);

const COLOR = { ok: 'success', error: 'error', neutral: 'neutral' } as const;
</script>

<template>
  <div class="flex flex-col gap-2">
    <p v-if="page.icons.length === 0" id="no-icons" class="text-sm text-muted">{{ text(strings, 'panel.icons.none') }}</p>
    <template v-else>
      <p class="text-xs text-muted">{{ text(strings, 'panel.icons.safeZone') }}</p>
      <div class="grid grid-cols-[repeat(auto-fill,minmax(150px,1fr))] gap-2">
        <UCard
          v-for="{ icon, preview, card } in cards"
          :id="card.id"
          :key="icon.index"
          as="figure"
          data-icon-card
          :data-ok="card.ok ? '' : undefined"
          tabindex="-1"
          class="m-0 outline-none focus-visible:ring-2 focus-visible:ring-primary"
          :class="card.ok ? '' : 'ring-error'"
          :ui="{ body: 'flex flex-col gap-2 p-2 sm:p-2' }"
        >
          <div
            class="relative mx-auto flex size-24 items-center justify-center bg-[repeating-conic-gradient(var(--ui-bg-accented)_0_25%,transparent_0_50%)] bg-[length:12px_12px]"
          >
            <img
              v-if="preview?.dataUri"
              :src="preview.dataUri"
              :alt="icon.src"
              class="max-h-full max-w-full"
            />
            <span v-else class="text-center text-xs text-muted">{{ text(strings, 'panel.icons.noPreview') }}</span>
            <span
              v-if="card.maskable"
              data-safe-zone
              aria-hidden="true"
              class="pointer-events-none absolute inset-[10%] rounded-full border-[1.5px] border-dashed border-primary"
            />
          </div>
          <figcaption class="flex flex-col items-start gap-1 text-xs break-all">
            <code class="text-[0.6875rem]">{{ icon.src }}</code>
            <UBadge
              data-state
              :data-tone="card.state.tone"
              :color="COLOR[card.state.tone]"
              variant="subtle"
              size="sm"
              :label="card.state.text"
            />
            <span v-for="line in card.lines" :key="line">{{ line }}</span>
          </figcaption>
        </UCard>
      </div>
    </template>
  </div>
</template>
