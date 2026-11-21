<script setup lang="ts">
// The activities' compact form in the toolbar (RB-090-67): a running video, flow run or check
// run, a changed clock or animation rate (RB-094-07), a plugin call that reports its progress
// (RB-093-01) or forced pseudo-class states (RB-094-16) that is not in the activity bar - its
// entry folded, its switch `ui.activityBar.show.*` off, or `ui.activityBar.mode` = `indicator` -
// shows here as its icon and counter (the state in
// words as tooltip and name) beside its primary control, the same entry as in the bar
// (`entries.ts`). The recording, the views out of sync and the portal have their indicators of
// old (`RecordIndicator.vue`, `DesyncIndicator.vue`, `PortalButton.vue`). A folded entry also
// gets a button that shows it in the bar again, so the way back stays on screen.
import { computed, onMounted } from 'vue';
import { useI18n } from 'vue-i18n';
import UBadge from '@nuxt/ui/components/Badge.vue';
import UButton from '@nuxt/ui/components/Button.vue';
import type { Activity } from '../ipc/activity';
import { ICONS } from '../icons';
import { useShell } from '../state/shell';
import { ACTIVITIES } from './activities';
import { entryView, type EntryAction, type EntryView } from './entries';
import { useActivities } from './store';

/** The activities whose compact form is drawn here. */
const COMPACT: readonly Activity[] = ['video', 'flows', 'checks', 'time', 'plugins', 'forcedStates'];

const shell = useShell();
const store = useActivities();
const { t } = useI18n();

onMounted(() => shell.loadCommands());

const compact = computed(() =>
  COMPACT.filter((activity) => store.run.value.includes(activity) && !store.inBar(activity))
    .map((activity) => entryView(activity, store.data(), t))
    .filter((view): view is EntryView => view !== null)
    .map((view) => ({ view, action: view.actions.find((action) => action.primary === true) ?? null })),
);
const folded = computed(() => ACTIVITIES.filter((activity) => store.folded(activity)));

function run(action: EntryAction): void {
  void shell.send(action.message);
}

/** The control's name and its command's shortcut. */
function title(action: EntryAction): string {
  const shortcut = shell.bindingOf(action.command);
  return shortcut === null ? action.label : t('shell.activity.shortcut', { label: action.label, shortcut });
}

function unfoldLabel(activity: Activity): string {
  return t('shell.activity.unfold', { name: t(`shell.activity.name.${activity}`) });
}
</script>

<template>
  <div
    v-if="compact.length > 0 || folded.length > 0"
    class="flex shrink-0 items-center gap-1"
    data-testid="activity-indicators"
  >
    <div
      v-for="{ view, action } in compact"
      :key="view.activity"
      role="group"
      :aria-label="view.text"
      class="flex shrink-0 items-center gap-0.5"
      :data-testid="`activity-indicator-${view.activity}`"
    >
      <UBadge
        :icon="view.icon"
        :color="view.color"
        variant="soft"
        size="md"
        class="tabular-nums"
        :label="view.short"
        :title="view.text"
      />
      <UButton
        v-if="action !== null"
        :icon="action.icon"
        :color="view.color === 'neutral' ? 'primary' : view.color"
        variant="ghost"
        size="sm"
        square
        :aria-label="action.label"
        :title="title(action)"
        :data-testid="`activity-indicator-${view.activity}-${action.key}`"
        @click="run(action)"
      />
    </div>
    <UButton
      v-for="activity in folded"
      :key="`unfold-${activity}`"
      :icon="ICONS.activityUnfold"
      color="neutral"
      variant="ghost"
      size="sm"
      square
      :aria-label="unfoldLabel(activity)"
      :title="unfoldLabel(activity)"
      :data-testid="`activity-unfold-${activity}`"
      @click="store.collapse(activity, false)"
    />
  </div>
</template>
