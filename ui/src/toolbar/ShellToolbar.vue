<script setup lang="ts">
// The toolbar at the top of the window (its own CEF view, never over a browser view): navigation
// for all views, the address bar, sync with the desynced views (`DesyncIndicator`), layout
// preset, focus mode and zoom (`LayoutSwitch`), the environment (`EnvSwitch`), the recording
// indicator (`RecordIndicator`) and the other activities' compact forms while they are not in the
// activity bar (`ActivityIndicators`, RB-090-67), the split buttons DevTools, Screenshot, Video and Checks
// (`SplitButton.vue`, RB-090-63), settings and the always-visible main menu `≡`. Every control is a
// native, Tab-reachable element with a label; every action is a command of the registry
// (`actions.ts`). Messages are not shown here: they go to the status bar at the bottom of the
// window (RB-080-15).
//
// The row never runs out of the window (RB-080-52): every group stands in a `ToolbarGroup`, the
// fit (`useToolbarFit.ts`) shrinks the wide pulldowns to icons and sends what still does not fit
// into the main menu `≡` (`OverflowMenu.vue`, always in the row, RB-090-63) - or, with `ui.toolbar.overflow = wrap`, into a
// second row the core makes room for. Navigation and the address bar always stay.
import { computed, provide, ref } from 'vue';
import { useI18n } from 'vue-i18n';
import UButton from '@nuxt/ui/components/Button.vue';
import USwitch from '@nuxt/ui/components/Switch.vue';
import { ICONS } from '../icons';
import { useShell } from '../state/shell';
import EmulationControls from '../emulation/EmulationControls.vue';
import TrustBadge from '../trust/TrustBadge.vue';
import { devtoolsIndicator } from '../devtools/indicator';
import { BUTTONS } from '../menu/buttonMenus';
import RecordIndicator from '../recorder/RecordIndicator.vue';
import ActivityIndicators from '../activity/ActivityIndicators.vue';
import SitemapIndicator from '../sitemap/SitemapIndicator.vue';
import AddressBar from './AddressBar.vue';
import EnvSwitch from './EnvSwitch.vue';
import PanelsToggle from './PanelsToggle.vue';
import CanvasMinimap from '../layout/CanvasMinimap.vue';
import BookmarksMenu from './BookmarksMenu.vue';
import DesyncIndicator from './DesyncIndicator.vue';
import LayoutSwitch from './LayoutSwitch.vue';
import OverflowMenu from './OverflowMenu.vue';
import PortalButton from './PortalButton.vue';
import ShellLogo from './ShellLogo.vue';
import SplitButton from './SplitButton.vue';
import ToolbarGroup from './ToolbarGroup.vue';
import { LOGO, NAVIGATION, OVERFLOW_SETTING } from './groups';
import { createToolbarFit, overflowMode, TOOLBAR_FIT_KEY } from './useToolbarFit';
import BrowsersyncNotice from '../sync/BrowsersyncNotice.vue';
import FrameworkDevtoolsOffer from '../devtools/FrameworkDevtoolsOffer.vue';

const shell = useShell();
const { t } = useI18n();

const snapshot = computed(() => shell.state.value);
const url = computed(() => snapshot.value?.url ?? '');
const syncEnabled = computed(() => snapshot.value?.sync.enabled ?? false);

function setSync(enabled: boolean): void {
  void shell.run({ command: 'sync.toggle', enabled });
}

const header = ref<HTMLElement | null>(null);
const fit = createToolbarFit({
  header,
  mode: computed(() => overflowMode(shell.settings.value[OVERFLOW_SETTING])),
  requestRows: (rows) => shell.ask({ type: 'toolbarRows', rows }),
});
provide(TOOLBAR_FIT_KEY, fit);
const rows = computed(() => fit.layout.value.rows);
/**
 * The address bar: what is left of one row; in the first of two rows the width that ends the
 * row; alone in the first row where navigation has no room beside its 240 px (`stacked`).
 */
const addressStyle = computed(() => {
  const { address, stacked, tight } = fit.layout.value;
  if (stacked) {
    return { flex: '1 1 100%', order: '-1' };
  }
  return {
    ...(address === null ? {} : { flex: `1 1 ${address}px` }),
    // One row narrower than navigation, 240 px and the menu together: the floor gives way.
    ...(tight ? { minWidth: '0' } : {}),
  };
});
const hasPresets = computed(() => (snapshot.value?.layout.presets.length ?? 0) > 0);
/** DevTools open: the button is pressed and its tooltip names the view and where they are. */
const devtools = computed(() => devtoolsIndicator(snapshot.value));
const devtoolsTitle = computed(() =>
  devtools.value === null
    ? t('shell.menu.button.devtools_hint')
    : t('shell.devtools.title', { view: devtools.value.view, mode: t(devtools.value.modeKey) }),
);
</script>

<template>
  <header
    ref="header"
    role="toolbar"
    :aria-label="t('shell.toolbar.label')"
    class="relative flex items-center gap-x-2 overflow-hidden border-b border-default bg-default px-2 text-default"
    :class="rows === 2 ? 'h-22 flex-wrap content-around' : 'h-11'"
    :data-rows="rows"
  >
    <ToolbarGroup :id="LOGO">
      <ShellLogo :theme="shell.resolvedTheme.value" />
    </ToolbarGroup>
    <ToolbarGroup id="minimap-start">
      <CanvasMinimap position="start" />
    </ToolbarGroup>

    <ToolbarGroup :id="NAVIGATION">
      <div class="flex shrink-0 items-center gap-0.5">
        <UButton
          :icon="ICONS.back"
          color="neutral"
          variant="ghost"
          size="sm"
          square
          :aria-label="t('shell.toolbar.back')"
          :title="t('shell.toolbar.back')"
          data-testid="back"
          @click="shell.run({ command: 'view.back' })"
        />
        <UButton
          :icon="ICONS.forward"
          color="neutral"
          variant="ghost"
          size="sm"
          square
          :aria-label="t('shell.toolbar.forward')"
          :title="t('shell.toolbar.forward')"
          data-testid="forward"
          @click="shell.run({ command: 'view.forward' })"
        />
        <UButton
          :icon="ICONS.reload"
          color="neutral"
          variant="ghost"
          size="sm"
          square
          :aria-label="t('shell.toolbar.reload')"
          :title="t('shell.toolbar.reload')"
          data-testid="reload"
          @click="shell.run({ command: 'reload.all', ignoreCache: $event.shiftKey })"
        />
      </div>
    </ToolbarGroup>

    <AddressBar
      :url="url"
      :focus-request="shell.focusAddressBarRequest.value"
      :style="addressStyle"
      @navigate="(next) => shell.run({ command: 'view.navigate', url: next })"
      @invalid="(key) => shell.report({ key, args: {}, level: 'warning' })"
    />

    <ToolbarGroup id="devtools">
      <SplitButton
        kind="devtools"
        :icon="ICONS.devtools"
        :label="t('shell.menu.button.devtools')"
        :menu-label="t('shell.menu.button.devtools_menu')"
        :command="BUTTONS.devtools.command"
        :menu="BUTTONS.devtools.menu"
        :state="devtools === null ? 'off' : 'on'"
        :title="devtoolsTitle"
      />
    </ToolbarGroup>
    <ToolbarGroup id="sync">
      <USwitch
        :model-value="syncEnabled"
        :label="t('shell.toolbar.sync')"
        size="sm"
        class="shrink-0"
        data-testid="sync"
        @update:model-value="setSync"
      />
      <DesyncIndicator />
    </ToolbarGroup>
    <ToolbarGroup id="capture">
      <div class="flex shrink-0 items-center gap-0.5">
        <SplitButton
          kind="capture"
          :icon="ICONS.screenshot"
          :label="t('shell.menu.button.screenshot')"
          :menu-label="t('shell.menu.button.screenshot_menu')"
          :command="BUTTONS.capture.command"
          :menu="BUTTONS.capture.menu"
        />
        <SplitButton
          kind="video"
          :icon="ICONS.video"
          :label="t('shell.menu.button.video')"
          :menu-label="t('shell.menu.button.video_menu')"
          :command="BUTTONS.video.command"
          :menu="BUTTONS.video.menu"
        />
      </div>
    </ToolbarGroup>

    <ToolbarGroup id="browsersync">
      <BrowsersyncNotice />
    </ToolbarGroup>
    <ToolbarGroup id="frameworks">
      <FrameworkDevtoolsOffer />
    </ToolbarGroup>
    <ToolbarGroup id="portal">
      <PortalButton />
    </ToolbarGroup>

    <ToolbarGroup id="layout" :icon="hasPresets ? ICONS.layoutPreset : undefined">
      <LayoutSwitch />
    </ToolbarGroup>
    <ToolbarGroup id="environment" :icon="ICONS.environment">
      <EnvSwitch />
    </ToolbarGroup>
    <ToolbarGroup id="bookmarks" :icon="ICONS.bookmarks">
      <BookmarksMenu />
    </ToolbarGroup>

    <ToolbarGroup id="emulation">
      <EmulationControls />
    </ToolbarGroup>

    <ToolbarGroup id="trust">
      <TrustBadge />
    </ToolbarGroup>

    <ToolbarGroup id="recorder">
      <RecordIndicator />
      <!-- The activities not in the activity bar, in their compact form (RB-090-67). -->
      <ActivityIndicators />
    </ToolbarGroup>
    <ToolbarGroup id="sitemap">
      <SitemapIndicator />
    </ToolbarGroup>
    <ToolbarGroup id="minimap-end">
      <CanvasMinimap position="end" />
    </ToolbarGroup>

    <ToolbarGroup id="checks">
      <SplitButton
        kind="checks"
        :icon="ICONS.checks"
        :label="t('shell.menu.button.checks')"
        :menu-label="t('shell.menu.button.checks_menu')"
        :command="BUTTONS.checks.command"
        :menu="BUTTONS.checks.menu"
      />
    </ToolbarGroup>
    <ToolbarGroup id="settings">
      <UButton
        :icon="ICONS.settings"
        color="neutral"
        variant="ghost"
        size="sm"
        square
        :aria-label="t('shell.toolbar.settings')"
        :title="t('shell.toolbar.settings')"
        data-testid="settings"
        @click="shell.run({ command: 'settings.open' })"
      />
    </ToolbarGroup>
    <!-- The plugin panels' switch, at the right end (RB-080-79). -->
    <ToolbarGroup id="panels">
      <PanelsToggle />
    </ToolbarGroup>
    <OverflowMenu />
  </header>
</template>
