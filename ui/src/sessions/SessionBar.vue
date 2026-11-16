<script setup lang="ts">
// The session picker and the "clear session data" confirmation of RB-020-07. `session.assign`
// and `session.clear` without arguments (their leader chords) reach the shell as `uiCommand`;
// this bar covers the toolbar strip (the shell view, never a page) and runs the command again
// with the chosen session. Clearing always asks first. Escape closes. The coordinator mounts it
// in `App.vue`. Closed by Escape, Cancel or done, the focus goes back to its opener
// (`returnFocus.ts`, RB-090-23). Nuxt UI since RB-080-88: both modes choose a session with
// `UButton`s (`aria-pressed`) - a select would open a popover the strip cuts off.
import { computed, nextTick, onScopeDispose, ref } from 'vue';
import { useI18n } from 'vue-i18n';
import UButton from '@nuxt/ui/components/Button.vue';
import { returnFocus } from '../focus/returnFocus';
import { useShell } from '../state/shell';
import { windowEvents, type Subscribe } from '../whichkey/whichkey';
import {
  ASSIGN_COMMAND,
  CLEAR_COMMAND,
  DEFAULT_SESSION,
  assignMessage,
  clearMessage,
  sessionsOf,
} from './sessions';

const props = withDefaults(defineProps<{ subscribe?: Subscribe }>(), { subscribe: undefined });
const shell = useShell();
const { t } = useI18n();

const mode = ref<'assign' | 'clear' | null>(null);
const chosen = ref(DEFAULT_SESSION);
const bar = ref<HTMLElement | null>(null);
const focus = returnFocus();
const sessions = computed(() => sessionsOf(shell.state.value));
const view = computed(() => shell.state.value?.activeView ?? shell.state.value?.views[0]?.id ?? null);
const current = computed(
  () => shell.state.value?.views.find((v) => v.id === view.value)?.session ?? DEFAULT_SESSION,
);

const stop = (props.subscribe ?? windowEvents)((message) => {
  if (message.type !== 'uiCommand') {
    return;
  }
  if (message.id === ASSIGN_COMMAND || message.id === CLEAR_COMMAND) {
    mode.value = message.id === ASSIGN_COMMAND ? 'assign' : 'clear';
    chosen.value = current.value;
    focus.remember();
    // Assigning starts at the first session; clearing at the chosen one, which the core clears.
    const start = mode.value === 'clear' ? '[aria-pressed="true"]' : 'button';
    void nextTick(() => bar.value?.querySelector<HTMLElement>(start)?.focus());
  }
});
onScopeDispose(stop);

function close(): void {
  mode.value = null;
  focus.restore();
}

function title(id: string): string {
  return id === DEFAULT_SESSION ? t('sessions.default') : id;
}

async function assign(session: string): Promise<void> {
  if (view.value === null) {
    return;
  }
  if (await shell.send(assignMessage(view.value, session))) {
    close();
  }
}

async function clear(): Promise<void> {
  if (await shell.send(clearMessage(chosen.value))) {
    close();
  }
}
</script>

<template>
  <div
    v-if="mode !== null"
    ref="bar"
    role="dialog"
    :aria-label="mode === 'assign' ? t('commands.session.assign') : t('commands.session.clear')"
    class="fixed inset-x-0 top-0 z-50 flex h-11 items-center gap-2 overflow-x-auto border-b border-default bg-elevated px-3 text-sm text-default"
    data-testid="session-bar"
    @keydown.escape.prevent="close"
  >
    <template v-if="mode === 'assign'">
      <span class="shrink-0 text-muted">{{ t('sessions.assign.label', { view: view ?? '' }) }}</span>
      <UButton
        v-for="session in sessions"
        :key="session.id"
        size="sm"
        :color="session.id === current ? 'primary' : 'neutral'"
        :variant="session.id === current ? 'soft' : 'outline'"
        class="shrink-0"
        :aria-pressed="session.id === current ? 'true' : 'false'"
        :data-testid="`session-option-${session.id}`"
        @click="assign(session.id)"
      >
        <span v-if="session.color" aria-hidden="true" :style="{ color: session.color }">&#9679;</span>
        {{ session.id === current ? t('sessions.assign.current', { session: title(session.id) }) : title(session.id) }}
      </UButton>
    </template>
    <template v-else>
      <span id="rb-session-clear" class="shrink-0 text-muted">{{ t('sessions.clear.label') }}</span>
      <div role="group" aria-labelledby="rb-session-clear" class="flex shrink-0 items-center gap-1" data-testid="session-clear-select">
        <UButton
          v-for="session in sessions"
          :key="session.id"
          size="sm"
          :color="session.id === chosen ? 'primary' : 'neutral'"
          :variant="session.id === chosen ? 'soft' : 'outline'"
          class="shrink-0"
          :aria-pressed="session.id === chosen ? 'true' : 'false'"
          :data-testid="`session-clear-option-${session.id}`"
          @click="chosen = session.id"
        >
          <span v-if="session.color" aria-hidden="true" :style="{ color: session.color }">&#9679;</span>
          {{ title(session.id) }}
        </UButton>
      </div>
      <span class="min-w-0 truncate" data-testid="session-clear-confirm">
        {{ t('sessions.clear.confirm', { session: chosen }) }}
      </span>
      <UButton size="sm" color="error" class="shrink-0" data-testid="session-clear-action" @click="clear">
        {{ t('sessions.clear.action') }}
      </UButton>
    </template>
    <UButton size="sm" color="neutral" variant="ghost" class="ml-auto shrink-0" data-testid="session-cancel" @click="close">
      {{ t('sessions.cancel') }}
    </UButton>
  </div>
</template>
