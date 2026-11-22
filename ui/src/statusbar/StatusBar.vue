<script setup lang="ts">
// The status bar at the bottom of the main window (RB-080-15, `status-bar.html`, its own CEF
// view below the grid, never over a browser view): on the left the message the feed shows
// (`feed.ts`) with its button when it carries a command (RB-090-71) and its copy button
// (RB-093-04, `StatusCopyButton.vue`), a link to its setting and a close button; on the right
// the history button with the number of unread messages, before it the canvas zoom while the
// canvas is shown (RB-080-94, `CanvasZoom.vue`). Errors are announced at once
// (`role="alert"`), the rest politely (`role="status"`); both regions are always there, so a
// screen reader hears each new message.
import { computed, onBeforeUnmount, onMounted } from 'vue';
import { useI18n } from 'vue-i18n';
import UButton from '@nuxt/ui/components/Button.vue';
import type { IpcClient } from '../ipc/client';
import type { StatusMessage } from '../ipc/protocol';
import { createStatusFeed, HISTORY_COMMAND } from './feed';
import { STATUS_ICONS } from './icons';
import { levelOf } from './level';
import StatusLine from './StatusLine.vue';
import CanvasZoom from './CanvasZoom.vue';
import StatusCopyButton from './StatusCopyButton.vue';
import { PANEL_PLACES } from '../notify/toastModel';

const props = defineProps<{ client: IpcClient }>();
const { t } = useI18n();
const feed = createStatusFeed(props.client);

const current = computed(() => feed.current.value);
const assertive = computed(() => current.value !== null && levelOf(current.value.level).live === 'assertive');
const setting = computed(() => current.value?.settingsKey ?? null);
// A key that opens a panel (RB-094-27) names the panel's place, not "Setting".
const settingPlace = computed(() => (setting.value === null ? undefined : PANEL_PLACES[setting.value]));
const action = computed(() => current.value?.action ?? null);
const unread = computed(() => feed.unread.value);
const historyLabel = computed(() =>
  unread.value > 0 ? t('status.bar.history_unread', { count: unread.value }) : t('status.bar.history'),
);

function send(message: Parameters<IpcClient['request']>[0]): void {
  props.client.request(message).catch((error: unknown) => {
    console.warn('the status bar could not reach the core', message.type, error);
  });
}

/** Runs the message's command through the registry, as the toolbar does, and closes it. */
function runAction(message: StatusMessage): void {
  const { action: button } = message;
  if (button === undefined) {
    return;
  }
  send({
    type: 'runCommand',
    id: button.command,
    ...(button.args === undefined ? {} : { args: button.args }),
  });
  feed.dismiss();
}

function openHistory(): void {
  feed.markRead();
  send({ type: 'runCommand', id: HISTORY_COMMAND });
}

onMounted(() => void feed.start());
onBeforeUnmount(() => feed.stop());
</script>

<template>
  <footer
    :aria-label="t('status.bar.label')"
    class="flex h-6 items-center gap-2 overflow-hidden border-t border-default bg-default px-2 text-xs text-default"
    data-testid="status-bar"
  >
    <div class="flex min-w-0 flex-1 items-center gap-2">
      <div role="alert" aria-live="assertive" aria-atomic="true" class="flex min-w-0 items-center" data-testid="status-alert">
        <StatusLine v-if="current !== null && assertive" :message="current" />
      </div>
      <div role="status" aria-live="polite" aria-atomic="true" class="flex min-w-0 items-center" data-testid="status-polite">
        <StatusLine v-if="current !== null && !assertive" :message="current" />
      </div>
      <!-- A message that offers a command runs it from its button (RB-090-71). -->
      <UButton
        v-if="current !== null && action !== null"
        color="primary"
        variant="soft"
        size="xs"
        class="shrink-0"
        :label="t(action.labelKey, action.labelArgs ?? {})"
        data-testid="status-action"
        @click="runAction(current)"
      />
      <!-- A saved bug report's message copies its Markdown (RB-093-04). -->
      <StatusCopyButton v-if="current !== null && current.copy !== undefined" :client="client" :copy="current.copy" />
      <!-- A notice about a setting links to it (RB-010-08, `rbrowser://settings/<key>`). -->
      <UButton
        v-if="setting !== null"
        color="neutral"
        variant="link"
        size="xs"
        class="shrink-0 px-0"
        :label="t(settingPlace ?? 'shell.status.open_setting')"
        :aria-label="settingPlace !== undefined ? t(settingPlace) : t('shell.status.open_setting_key', { key: setting })"
        :title="setting"
        data-testid="status-settings"
        @click="send({ type: 'openSettings', key: setting })"
      />
      <UButton
        v-if="current !== null"
        :icon="STATUS_ICONS.statusClose"
        color="neutral"
        variant="ghost"
        size="xs"
        square
        class="shrink-0"
        :aria-label="t('status.bar.close')"
        :title="t('status.bar.close')"
        data-testid="status-close"
        @click="feed.dismiss()"
      />
    </div>
    <CanvasZoom :client="client" />
    <UButton
      :icon="STATUS_ICONS.statusHistory"
      :color="unread > 0 ? 'primary' : 'neutral'"
      variant="ghost"
      size="xs"
      class="shrink-0 tabular-nums"
      :label="unread > 0 ? String(unread) : undefined"
      :aria-label="historyLabel"
      :title="historyLabel"
      data-testid="status-history"
      @click="openHistory"
    />
  </footer>
</template>
