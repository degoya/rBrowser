<script setup lang="ts">
// One message as the status bar and its history show it (RB-080-15): the level as an icon and a
// word (never the colour alone), the time and the text. In the bar the text stays on one line
// (the whole of it in the tooltip and in the history); in the history it wraps.
import { computed } from 'vue';
import { useI18n } from 'vue-i18n';
import UIcon from '@nuxt/ui/components/Icon.vue';
import type { StatusMessage } from '../ipc/protocol';
import { formatTime, levelOf } from './level';

const props = defineProps<{ message: StatusMessage; wrap?: boolean }>();
const { t } = useI18n();

const look = computed(() => levelOf(props.message.level));
const text = computed(() => t(props.message.messageKey, props.message.args ?? {}));
const time = computed(() => formatTime(props.message.at));
const iso = computed(() => new Date(props.message.at).toISOString());
</script>

<template>
  <span
    class="flex min-w-0 gap-1.5"
    :class="wrap ? 'items-start' : 'items-center'"
    :title="wrap ? undefined : text"
    data-testid="status-line"
  >
    <UIcon :name="look.icon" class="size-3.5 shrink-0" :class="[look.color, wrap ? 'mt-0.5' : '']" aria-hidden="true" />
    <span class="shrink-0 font-medium" :class="look.color" data-testid="status-level">{{ t(look.wordKey) }}</span>
    <time :datetime="iso" class="shrink-0 text-muted tabular-nums">{{ time }}</time>
    <span :class="wrap ? 'min-w-0 whitespace-pre-wrap break-words select-text' : 'min-w-0 truncate'" data-testid="status-text">{{ text }}</span>
  </span>
</template>
