<script setup lang="ts">
// The side bar of the plugin panels page (RB-080-61), on Nuxt UI (RB-080-83, step 1): a
// `UNavigationMenu` with the core's panels first (Findings, RB-080-69, `core.ts`) and every
// plugin panel with its icon - the manifest's `icon` when the shell bundles it, else the fallback
// (`icons.ts`) - and its title, or the icons alone (`collapsed`) with the title as tooltip and
// name (`panels.sidebar = icons`, or a page too narrow for titles). The current entry is marked
// by a line beside it as well as its ground and colour (`docs/design.md`, 12; the menu's own
// `highlight` marks nested entries only). The button at the bottom switches the two modes.
//
// Folded (`rail`, RB-080-93) it is the whole page: the icon rail - a 44 px column beside the
// views, a 36 px row below them - with the expand button first. A panel a command opened while
// folded carries a chip until the person opens the panels. The tooltips are the entries' `title`,
// which the window draws outside the page: an HTML tooltip would be cut off at the rail's edge.
import { computed } from 'vue';
import { useI18n } from 'vue-i18n';
import UButton from '@nuxt/ui/components/Button.vue';
import UChip from '@nuxt/ui/components/Chip.vue';
import UIcon from '@nuxt/ui/components/Icon.vue';
import UNavigationMenu, { type NavigationMenuItem } from '@nuxt/ui/components/NavigationMenu.vue';
import type { SideEntry } from './core';
import type { DockSide, SidebarMode } from './dock';
import { DOCK_ICONS } from './icons';

const props = withDefaults(
  defineProps<{
    entries: SideEntry[];
    /** The selected entry's key. */
    selected?: string | null;
    iconsOnly?: boolean;
    /** The page is too narrow for titles: icons only, without the mode button. */
    narrow?: boolean;
    /** Folded to the icon rail at this side, with an expand button. */
    strip?: DockSide | null;
    /** Folded, the entry a command opened: marked until the person unfolds. */
    attention?: string | null;
  }>(),
  { selected: null, iconsOnly: false, narrow: false, strip: null, attention: null },
);
const emit = defineEmits<{
  open: [key: string];
  sidebar: [mode: SidebarMode];
  expand: [];
}>();
const { t } = useI18n();

const compact = computed(() => props.iconsOnly || props.narrow || props.strip !== null);
/** Below the views the rail is a row. */
const row = computed(() => props.strip === 'bottom');
/** The current entry's line: at the start of a column entry, under a row entry. */
const linkUi = computed(() =>
  [
    'cursor-pointer data-active:before:bg-elevated',
    'data-active:after:absolute data-active:after:rounded-full data-active:after:bg-primary',
    row.value
      ? 'data-active:after:inset-x-1 data-active:after:bottom-0 data-active:after:h-0.5'
      : 'data-active:after:inset-y-1 data-active:after:start-0 data-active:after:w-0.5',
  ].join(' '),
);
const modeLabel = computed(() => t(props.iconsOnly ? 'panels.sidebar.expand' : 'panels.sidebar.collapse'));

interface SideItem extends NavigationMenuItem {
  key: string;
}

const items = computed<SideItem[]>(() =>
  props.entries.map((entry) => {
    const current = entry.key === props.selected;
    const named =
      entry.key === props.attention ? t('panels.sidebar.attention', { title: entry.title }) : undefined;
    return {
      key: entry.key,
      // Icons only: the title is the tooltip and the name.
      label: compact.value ? undefined : entry.title,
      icon: entry.icon,
      active: current,
      title: compact.value ? entry.title : undefined,
      'aria-label': named ?? (compact.value ? entry.title : undefined),
      'aria-current': current ? 'page' : undefined,
      'data-testid': `panel-tab-${entry.key}`,
      onSelect: () => emit('open', entry.key),
    };
  }),
);
</script>

<template>
  <nav
    class="flex shrink-0 gap-1 border-default"
    :class="[
      row ? 'h-9 flex-row items-center px-1.5' : 'flex-col',
      !row && compact ? 'w-11 items-center px-1 py-1.5' : '',
      !row && !compact ? 'w-48 p-2' : '',
      strip === null ? 'border-r' : '',
    ]"
    :aria-label="t('panels.list')"
    data-testid="panel-sidebar"
  >
    <UButton
      v-if="strip !== null"
      color="neutral"
      variant="ghost"
      size="sm"
      square
      class="shrink-0"
      :class="row ? 'me-1' : 'mb-1'"
      :title="t('panels.dockbar.expand')"
      :aria-label="t('panels.dockbar.expand')"
      aria-expanded="false"
      data-testid="panel-strip-expand"
      @click="emit('expand')"
    >
      <UIcon
        :name="DOCK_ICONS.expand[strip]"
        :data-icon="DOCK_ICONS.expand[strip]"
        data-testid="panel-icon"
        class="size-4"
        aria-hidden="true"
      />
    </UButton>
    <UNavigationMenu
      :items="items"
      :orientation="row ? 'horizontal' : 'vertical'"
      :collapsed="!row && compact"
      color="primary"
      class="min-h-0 min-w-0 flex-1"
      :class="row ? 'overflow-x-auto' : 'overflow-y-auto'"
      :ui="{ list: row ? 'flex gap-1' : 'gap-0.5', link: linkUi }"
      data-testid="panel-sidebar-menu"
    >
      <template #item-leading="{ item }">
        <UChip v-if="(item as SideItem).key === attention" color="primary" size="lg" inset>
          <UIcon
            :name="item.icon ?? ''"
            :data-icon="item.icon"
            data-testid="panel-icon"
            class="size-5 shrink-0"
            aria-hidden="true"
          />
        </UChip>
        <UIcon
          v-else
          :name="item.icon ?? ''"
          :data-icon="item.icon"
          data-testid="panel-icon"
          class="size-5 shrink-0"
          aria-hidden="true"
        />
      </template>
    </UNavigationMenu>
    <UButton
      v-if="strip === null && !narrow"
      :icon="iconsOnly ? DOCK_ICONS.sidebarExpand : DOCK_ICONS.sidebarCollapse"
      color="neutral"
      variant="ghost"
      size="sm"
      square
      class="shrink-0"
      :class="compact ? '' : 'self-end'"
      :title="modeLabel"
      :aria-label="modeLabel"
      data-testid="panel-sidebar-toggle"
      @click="emit('sidebar', iconsOnly ? 'expanded' : 'icons')"
    />
  </nav>
</template>
