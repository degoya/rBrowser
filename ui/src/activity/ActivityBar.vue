<script setup lang="ts">
// The activity bar (RB-090-67): a strip of the shell, 28 px high - always, empty while nothing
// runs, with `ui.activityBar.mode` `always` (the default, RB-094-13: the views never jump), else
// only while something runs - one entry per running activity, side by side, each with its state and its controls, all
// commands of the registry (`entries.ts`). The toolbar's page draws it below the toolbar
// (`place="top"`), the status bar's page above the status bar (`place="bottom"`), and each only
// once the core made room in its view (`activityBar.place`), so it never draws over a view and
// never appears in a view's screenshot or video. In a narrow window an entry is its icon and
// counter, the full state its tooltip and accessible name. An entry's close button folds it into
// its toolbar indicator until its activity ends. Every control is a native, Tab-reachable button;
// a tooltip names the command's shortcut.
import { computed, onMounted, ref, watch } from 'vue';
import { useI18n } from 'vue-i18n';
import UButton from '@nuxt/ui/components/Button.vue';
import UIcon from '@nuxt/ui/components/Icon.vue';
import type { ActivityBarPosition } from '../ipc/activity';
import { ICONS } from '../icons';
import { useShell } from '../state/shell';
import { entryView, type EntryAction, type EntryView } from './entries';
import { useActivities } from './store';

const props = defineProps<{ place: ActivityBarPosition }>();
const shell = useShell();
const store = useActivities();
const { t } = useI18n();

onMounted(() => shell.loadCommands());

const views = computed(() =>
  store
    .drawn(props.place)
    .map((activity) => entryView(activity, store.data(), t))
    .filter((view): view is EntryView => view !== null),
);

// What a screen reader hears without acting (RB-090-08): the names of the activities drawn here,
// once when one starts or ends - never their ticking state, which the entries carry. Only the
// page the bar is granted to speaks (`null` elsewhere), and it says idle only after something ran.
const names = computed(() =>
  store.granted.value === props.place
    ? views.value.map((view) => t(`shell.activity.name.${view.activity}`)).join(', ')
    : null,
);
const announcement = ref('');
watch(names, (now, before) => {
  if (now === null) {
    announcement.value = '';
  } else if (now !== '') {
    announcement.value = t('shell.activity.announce.running', { names: now });
  } else {
    announcement.value = before ? t('shell.activity.announce.idle') : '';
  }
});

const TONE: Record<EntryView['color'], string> = {
  error: 'text-error',
  primary: 'text-primary',
  warning: 'text-warning',
  neutral: 'text-muted',
};

function title(action: EntryAction): string {
  const shortcut = shell.bindingOf(action.command);
  return shortcut === null ? action.label : t('shell.activity.shortcut', { label: action.label, shortcut });
}

function run(view: EntryView, action: EntryAction): void {
  if (action.copy !== undefined) {
    // The presentation's link (RB-096-05): the clipboard, never a message to the core.
    void navigator.clipboard.writeText(action.copy).catch(() => undefined);
    return;
  }
  void shell.send(action.message);
  if (action.closes === true) {
    store.collapse(view.activity, true);
  }
}
</script>

<template>
  <span class="sr-only" role="status" aria-live="polite" data-testid="activity-announce">{{ announcement }}</span>
  <div
    v-if="views.length > 0 || (store.settings.value.mode === 'always' && store.granted.value === place)"
    role="toolbar"
    :aria-label="t('shell.activity.label')"
    class="flex h-7 items-center gap-1.5 overflow-x-auto bg-elevated px-2 text-xs text-default"
    :class="place === 'top' ? 'border-b border-default' : 'border-t border-default'"
    :data-place="place"
    data-testid="activity-bar"
  >
    <div
      v-for="view in views"
      :key="view.activity"
      role="group"
      :aria-label="view.text"
      class="flex h-6 shrink-0 items-center gap-0.5 rounded-md border border-default bg-default pl-1.5"
      :data-testid="`activity-${view.activity}`"
    >
      <UIcon :name="view.icon" class="size-3.5 shrink-0" :class="TONE[view.color]" />
      <span class="hidden max-w-72 truncate md:inline" data-testid="activity-text">{{ view.text }}</span>
      <span class="tabular-nums md:hidden" :title="view.text" data-testid="activity-short">{{ view.short }}</span>
      <UButton
        v-for="action in view.actions"
        :key="action.key"
        :icon="action.icon"
        :color="action.primary === true ? (view.color === 'neutral' ? 'primary' : view.color) : 'neutral'"
        :variant="action.primary === true ? 'soft' : 'ghost'"
        size="xs"
        :square="action.primary !== true"
        :label="action.primary === true ? action.label : undefined"
        :ui="{ label: 'hidden md:inline' }"
        :aria-label="action.label"
        :title="title(action)"
        :data-testid="`activity-${view.activity}-${action.key}`"
        @click="run(view, action)"
      />
      <UButton
        :icon="ICONS.activityClose"
        color="neutral"
        variant="ghost"
        size="xs"
        square
        :aria-label="view.closeLabel"
        :title="view.closeLabel"
        :data-testid="`activity-${view.activity}-close`"
        @click="store.collapse(view.activity, true)"
      />
    </div>
  </div>
</template>
