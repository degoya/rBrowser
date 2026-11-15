<script setup lang="ts">
// The image-overlay panel (RB-070-09, RB-080-74), on Nuxt UI since RB-080-86: show or hide the
// design, its appearance, offset and split view (OverlayControls), and the design per breakpoint
// with the folder's designs and their previews (DesignList) - all by keyboard. Every change is
// one call of the tool `set` (`imageoverlay_set`), whose answer - and the state the component
// posts after a change anywhere - it draws; the designs come from the tool `designs`, asked again
// only when their names change (each listing reads every design). *Add design...* runs the
// command `add_design`: the application opens the file dialog and copies the image into
// `.rbrowser/designs/` (a panel can do neither); its outcome comes back as `hostNotice`. A file
// dropped on the panel is not taken: the panel points at the button.
import { computed, onBeforeUnmount, onMounted, ref } from 'vue';
import { usePanel } from '../../../../web-common/panel-ui/src/mount';
import { onPanelEvent } from '../../../../web-common/panel-ui/src/events';
import type { StatusLine, Tone } from '../../../../web-common/panel-ui/src/status';
import IconAction from '../../../../web-common/panel-ui/src/components/IconAction.vue';
import PanelFrame from '../../../../web-common/panel-ui/src/components/PanelFrame.vue';
import DesignList from './DesignList.vue';
import OverlayControls from './OverlayControls.vue';
import { createPreviews } from './previews';
import type { Design, Library, OverlayState, SetAnswer, ViewRow } from './model';

const panel = usePanel();
const t = panel.t;

const state = ref<OverlayState | null>(null);
const views = ref<ViewRow[]>([]);
const files = ref<string[]>([]);
const library = ref<Design[]>([]);
const libraryMore = ref(false);
const trusted = ref<boolean | null>(null);
const adding = ref(false);
const message = ref<StatusLine | null>(null);
let listing = 0;

const previews = createPreviews((args) => panel.call({ tool: 'designs' }, args));

const say = (text: string, tone: Tone = 'info'): void => {
  message.value = { tone, text };
};
const failed = (error: unknown): void => say(t('panel.failed', { error: panel.error(error) }), 'error');

/** The status line: the host's waiting, else what the last action said. */
const line = computed<StatusLine | null>(() => {
  if (panel.waiting.value) return { tone: 'busy', text: panel.strings.value['plugins.panel.waiting'] ?? '' };
  return message.value;
});

/** Takes an answer of `set` (or the component's post): the state, the views, the designs' names. */
function adopt(answer: unknown): void {
  const now = answer as SetAnswer | null;
  if (!now || typeof now !== 'object' || !now.state) return;
  state.value = now.state;
  if (Array.isArray(now.views)) views.value = now.views;
  if (Array.isArray(now.designs)) {
    const names = now.designs.map(String);
    // Each listing reads every design (review C6): only when the names change, and once.
    const changed = listing === 0 || names.join('\n') !== files.value.join('\n');
    files.value = names;
    if (changed) list();
  }
}

/** Asks for the designs with their sizes; only the newest answer counts. */
function list(): void {
  const asked = ++listing;
  panel.call({ tool: 'designs' }, {}).then(
    (answer) => {
      const now = answer as Library | null;
      if (asked !== listing || !now || !Array.isArray(now.designs)) return;
      library.value = now.designs;
      libraryMore.value = now.more === true;
      trusted.value = typeof now.trusted === 'boolean' ? now.trusted : null;
      for (const design of now.designs) previews.request(design);
    },
    () => undefined,
  );
}

function change(args: Record<string, unknown>): void {
  panel
    .call({ tool: 'set' }, args)
    .then((answer) => {
      adopt(answer);
      const saved = (answer as SetAnswer | null)?.saved;
      if (typeof saved === 'boolean') say(t(saved ? 'panel.saved' : 'panel.not_saved'), saved ? 'ok' : 'info');
    })
    .catch(failed);
}

// Busy from the click until the application answers: a design, a refusal or a cancel.
function addDesign(): void {
  adding.value = true;
  say(t('panel.adding'), 'busy');
  panel.call({ command: 'add_design' }, {}).catch((error: unknown) => {
    adding.value = false;
    failed(error);
  });
}

// The plugin's posts; the host's own word (`waiting`) is panel.ts's.
const stopEvents = onPanelEvent((message) => {
  if (!message || typeof message !== 'object' || 'host' in message) return;
  const posted = message as Record<string, unknown>;
  const notice = posted.hostNotice as { text?: unknown; failed?: unknown } | null | undefined;
  if (notice && typeof notice === 'object') {
    adding.value = false;
    say(String(notice.text ?? ''), notice.failed === true ? 'error' : 'info');
    // A design added (or trust changed): the designs again.
    list();
  } else if (posted.reload) {
    panel.call({ tool: 'set' }, {}).then(adopt, () => undefined);
  } else adopt(posted);
});

// A file dropped on the frame would be opened by it; the panel cannot write into the project.
const over = (event: DragEvent): void => event.preventDefault();
function drop(event: DragEvent): void {
  event.preventDefault();
  say(t('panel.drop_unsupported'), 'error');
}
onMounted(() => {
  document.addEventListener('dragover', over);
  document.addEventListener('drop', drop);
});
onBeforeUnmount(() => {
  stopEvents();
  document.removeEventListener('dragover', over);
  document.removeEventListener('drop', drop);
});

// The state once the texts are there; again on every post of the component.
panel.onSettings((first) => {
  if (first) panel.call({ tool: 'set' }, {}).then(adopt, failed);
});
</script>

<template>
  <PanelFrame :title="t('panels.designs')" icon="i-lucide-image" :status="line">
    <template #actions>
      <IconAction
        v-if="files.length > 0"
        id="add-design"
        icon="i-lucide-image-plus"
        :label="t('panel.add_design')"
        primary
        :disabled="adding"
        :aria-busy="adding ? 'true' : undefined"
        @click="addDesign"
      />
    </template>

    <div v-if="!state" id="state" role="status">
      <UEmpty icon="i-lucide-loader-circle" variant="naked" size="sm" :ui="{ avatar: 'animate-spin' }" />
    </div>
    <div v-else class="flex max-w-2xl flex-col gap-6">
      <OverlayControls :state="state" @change="change" />
      <DesignList
        :state="state"
        :views="views"
        :files="files"
        :library="library"
        :more="libraryMore"
        :trusted="trusted"
        :adding="adding"
        :previews="previews"
        @change="change"
        @add="addDesign"
      />
      <section class="flex flex-col items-start gap-1 border-t border-default pt-4">
        <UButton id="reset" icon="i-lucide-rotate-ccw" color="neutral" variant="outline" size="sm" :label="t('panel.reset')" aria-describedby="reset-hint" @click="change({ reset: true })" />
        <p id="reset-hint" class="text-xs text-muted">{{ t('panel.reset_hint') }}</p>
      </section>
    </div>
  </PanelFrame>
</template>
