<script setup lang="ts">
// The message history (RB-080-15, `settings.html#/status-history`): a popup standing on the
// status bar, opened by its history button or `status.history`. Every message of the window's
// log, newest first, its text whole and selectable; filtered by level, copied as text. Escape
// hides it (`dismissHistory`), as does a click anywhere else.
import { computed, onBeforeUnmount, onMounted, ref } from 'vue';
import { useI18n } from 'vue-i18n';
import UButton from '@nuxt/ui/components/Button.vue';
import type { IpcClient } from '../ipc/client';
import type { NotificationLevel } from '../ipc/protocol';
import { createStatusFeed } from './feed';
import { copiedText, shown, type LevelFilter } from './history';
import { STATUS_ICONS } from './icons';
import { LEVELS, levelOf } from './level';
import StatusLine from './StatusLine.vue';

// The pages window hands every page its route key and trust state; the history needs neither.
defineOptions({ inheritAttrs: false });
const props = defineProps<{ client: IpcClient }>();
const { t } = useI18n();
const feed = createStatusFeed(props.client);
const filter = ref<LevelFilter>('all');
const note = ref<{ key: string; error: boolean } | null>(null);

const FILTERS: readonly LevelFilter[] = ['all', ...LEVELS];
const messages = computed(() => shown(feed.items.value, filter.value));
const word = (level: NotificationLevel): string => t(levelOf(level).wordKey);
const filterLabel = (value: LevelFilter): string => (value === 'all' ? t('status.history.all') : word(value));

async function copy(): Promise<void> {
  const text = copiedText(messages.value, word, (message) => t(message.messageKey, message.args ?? {}));
  try {
    await navigator.clipboard.writeText(text);
    note.value = { key: 'status.history.copied', error: false };
  } catch {
    note.value = { key: 'status.history.copy_failed', error: true };
  }
}

function close(): void {
  props.client.request({ type: 'dismissHistory' }).catch(() => undefined);
}

function onKey(event: KeyboardEvent): void {
  if (event.key === 'Escape') {
    event.preventDefault();
    close();
  }
}

function openSetting(key: string): void {
  props.client.request({ type: 'openSettings', key }).catch(() => undefined);
}

onMounted(() => {
  void feed.start();
  window.addEventListener('keydown', onKey);
});
onBeforeUnmount(() => {
  feed.stop();
  window.removeEventListener('keydown', onKey);
});
</script>

<template>
  <section
    :aria-label="t('status.history.title')"
    class="flex h-screen flex-col border border-default bg-default text-xs text-default"
    data-testid="status-history-page"
  >
    <header class="flex shrink-0 flex-wrap items-center gap-1 border-b border-default px-2 py-1">
      <h1 class="me-2 text-sm font-semibold">{{ t('status.history.title') }}</h1>
      <div role="group" :aria-label="t('status.history.filter')" class="flex flex-wrap items-center gap-0.5">
        <UButton
          v-for="value in FILTERS"
          :key="value"
          :color="filter === value ? 'primary' : 'neutral'"
          :variant="filter === value ? 'soft' : 'ghost'"
          size="xs"
          :label="filterLabel(value)"
          :aria-pressed="filter === value"
          :data-testid="`status-filter-${value}`"
          @click="filter = value"
        />
      </div>
      <span class="flex-1" />
      <UButton
        :icon="STATUS_ICONS.statusCopy"
        color="neutral"
        variant="ghost"
        size="xs"
        :label="t('status.history.copy')"
        :disabled="messages.length === 0"
        data-testid="status-copy"
        @click="copy"
      />
      <UButton
        :icon="STATUS_ICONS.statusClose"
        color="neutral"
        variant="ghost"
        size="xs"
        square
        :aria-label="t('status.history.close')"
        :title="t('status.history.close')"
        data-testid="status-history-close"
        @click="close"
      />
    </header>
    <p role="status" aria-live="polite" class="shrink-0 px-2 empty:hidden" :class="note?.error ? 'text-error' : 'text-muted'">{{ note === null ? '' : t(note.key) }}</p>
    <ol v-if="messages.length > 0" :aria-label="t('status.history.list')" class="min-h-0 flex-1 overflow-y-auto">
      <li
        v-for="message in messages"
        :key="message.id"
        class="flex items-start gap-2 border-b border-default px-2 py-1 last:border-b-0"
        data-testid="status-history-item"
      >
        <StatusLine :message="message" wrap class="flex-1" />
        <UButton
          v-if="message.settingsKey !== undefined"
          color="neutral"
          variant="link"
          size="xs"
          class="shrink-0 px-0"
          :label="t('shell.status.open_setting')"
          :aria-label="t('shell.status.open_setting_key', { key: message.settingsKey })"
          :title="message.settingsKey"
          @click="openSetting(message.settingsKey)"
        />
      </li>
    </ol>
    <p v-else class="px-2 py-3 text-muted" data-testid="status-history-empty">
      {{ t(feed.items.value.length === 0 ? 'status.history.empty' : 'status.history.none_filtered') }}
    </p>
  </section>
</template>
