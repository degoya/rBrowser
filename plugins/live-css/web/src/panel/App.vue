<script setup lang="ts">
// The Live CSS panel (RB-094-04) on Nuxt UI: the draft's blocks as cards (BlockCard), a switch
// that applies the draft in the views or takes it out, and Add block. An edit reaches the views
// a moment after the last keystroke (`set` with the whole draft, APPLY_DELAY_MS), the switch at
// once; a draft the component refuses (braces that do not balance, too long) stays in the panel,
// marked unsaved, with the reason in the status line. Copy as media query first hands the
// component what was typed, then copies the block as it answers it. The host's `changed` event
// (MCP, the command `toggle`) reloads the draft unless the panel holds edits of its own. Outside a
// trusted workspace the draft can be neither read nor kept: the panel says so and edits nothing.
import { computed, onBeforeUnmount, ref, watch } from 'vue';
import IconAction from '../../../../web-common/panel-ui/src/components/IconAction.vue';
import PanelFrame from '../../../../web-common/panel-ui/src/components/PanelFrame.vue';
import { onPanelEvent } from '../../../../web-common/panel-ui/src/events';
import { usePanel } from '../../../../web-common/panel-ui/src/mount';
import type { StatusLine, Tone } from '../../../../web-common/panel-ui/src/status';
import BlockCard from './BlockCard.vue';
import { APPLY_DELAY_MS, type DraftState, type EditBlock, readState, setArgs, toEdit } from './model';

const panel = usePanel();
const t = panel.t;

const state = ref<DraftState | null>(null);
const blocks = ref<EditBlock[]>([]);
const enabled = ref(true);
const loadError = ref<string | null>(null);
const note = ref<{ tone: Tone; text: string } | null>(null);
/** Edits the component has not stored: waiting for the delay, or refused. */
const unsaved = ref(false);
let nextKey = 1;
let timer: ReturnType<typeof setTimeout> | undefined;
let sending: Promise<boolean> | null = null;

const editable = computed(() => state.value?.available === 'trusted');

const line = computed<StatusLine | null>(() => {
  void panel.strings.value;
  if (note.value) return note.value;
  if (loadError.value) return { tone: 'error', text: loadError.value };
  if (state.value?.available === 'untrusted') return { tone: 'info', text: t('panel.untrusted') };
  if (state.value?.available === 'noProject') return { tone: 'info', text: t('panel.noProject') };
  return null;
});

function badAnswer(): Error {
  return new Error('plugins.error.bad_output');
}

async function refresh(): Promise<void> {
  try {
    const read = readState(await panel.call({ tool: 'set' }, {}));
    if (!read) throw badAnswer();
    loadError.value = null;
    state.value = read;
    if (!unsaved.value && !sending) {
      blocks.value = toEdit(read, () => nextKey++);
      enabled.value = read.enabled;
    }
  } catch (error) {
    loadError.value = panel.error(error);
  }
}

/** Hands the component the panel's draft; true once it stored it. */
async function send(): Promise<boolean> {
  clearTimeout(timer);
  timer = undefined;
  while (sending) await sending;
  if (!unsaved.value) return true;
  const args = setArgs(blocks.value, enabled.value);
  const sent = JSON.stringify(args);
  sending = (async () => {
    try {
      const read = readState(await panel.call({ tool: 'set' }, args));
      if (!read) throw badAnswer();
      state.value = read;
      note.value = null;
      // Typed on while the call ran: those edits wait for their own send.
      unsaved.value = JSON.stringify(setArgs(blocks.value, enabled.value)) !== sent;
      return true;
    } catch (error) {
      note.value = { tone: 'error', text: t('panel.notApplied', { reason: panel.error(error) }) };
      return false;
    } finally {
      sending = null;
    }
  })();
  return sending;
}

function edited(): void {
  unsaved.value = true;
  note.value = null;
  clearTimeout(timer);
  timer = setTimeout(() => void send(), APPLY_DELAY_MS);
}

function setCss(block: EditBlock, css: string): void {
  block.css = css;
  edited();
}

function setBreakpoints(block: EditBlock, breakpoints: string[]): void {
  block.breakpoints = breakpoints;
  edited();
}

function switchTo(on: boolean): void {
  enabled.value = on;
  unsaved.value = true;
  void send();
}

function add(): void {
  blocks.value.push({ key: nextKey++, css: '', breakpoints: [] });
  edited();
}

function remove(index: number): void {
  blocks.value.splice(index, 1);
  edited();
}

/** Writes `text` to the clipboard; the sandboxed frame may refuse the async clipboard. */
async function write(text: string): Promise<boolean> {
  try {
    await navigator.clipboard.writeText(text);
    return true;
  } catch {
    const field = document.createElement('textarea');
    field.value = text;
    field.setAttribute('readonly', '');
    field.style.position = 'fixed';
    field.style.opacity = '0';
    document.body.append(field);
    field.select();
    const copied = document.execCommand('copy');
    field.remove();
    return copied;
  }
}

async function copy(index: number): Promise<void> {
  if (!(await send())) return;
  const text = state.value?.blocks[index]?.copy;
  if (!text) {
    note.value = { tone: 'error', text: t('panel.copyNothing', { n: index + 1 }) };
    return;
  }
  note.value = (await write(text))
    ? { tone: 'ok', text: t('panel.copied', { n: index + 1 }) }
    : { tone: 'error', text: t('panel.copyFailed') };
}

watch(unsaved, (now) => panel.post({ type: 'dirty', dirty: now }));
watch(
  () => panel.strings.value,
  () => {
    document.title = t('panels.editor');
  },
);

const stopEvents = onPanelEvent((message) => {
  if (message && typeof message === 'object' && (message as { changed?: unknown }).changed === true) {
    if (!unsaved.value && !sending) void refresh();
  }
});
panel.onSettings(() => void refresh());
onBeforeUnmount(() => {
  clearTimeout(timer);
  stopEvents();
});
</script>

<template>
  <PanelFrame :title="t('panels.editor')" icon="i-lucide-code" :status="line">
    <template #actions>
      <USwitch
        id="enabled"
        :model-value="enabled"
        :label="t('panel.enabled')"
        size="sm"
        :disabled="!editable"
        @update:model-value="switchTo"
      />
      <IconAction id="add" icon="i-lucide-plus" :label="t('panel.add')" :disabled="!editable" @click="add" />
    </template>

    <p v-if="!state && !loadError" id="loading" class="text-sm text-muted">{{ t('panel.loading') }}</p>
    <div v-else-if="!state" id="failed" role="alert" class="flex flex-col items-start gap-2">
      <UButton id="retry" size="sm" color="neutral" variant="outline" icon="i-lucide-rotate-cw" :label="t('panel.retry')" @click="refresh" />
    </div>
    <div v-else class="flex flex-col gap-3">
      <UEmpty v-if="blocks.length === 0 && editable" id="empty" icon="i-lucide-code" :title="t('panel.empty')" variant="naked">
        <template #actions>
          <UButton id="add-first" size="sm" icon="i-lucide-plus" :label="t('panel.add')" @click="add" />
        </template>
      </UEmpty>
      <BlockCard
        v-for="(block, index) in blocks"
        :key="block.key"
        :block="block"
        :number="index + 1"
        :stored="state.blocks[index]"
        :breakpoints="state.breakpoints"
        :disabled="!editable"
        @css="(css: string) => setCss(block, css)"
        @breakpoints="(ids: string[]) => setBreakpoints(block, ids)"
        @copy="copy(index)"
        @remove="remove(index)"
      />
      <p v-if="editable" id="file" class="text-xs text-muted">{{ t('panel.file', { file: state.file }) }}</p>
    </div>
  </PanelFrame>
</template>
