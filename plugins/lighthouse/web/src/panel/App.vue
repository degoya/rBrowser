<script setup lang="ts">
// The Lighthouse panel (RB-060-07) on Nuxt UI (RB-080-83). It asks the tool `status` every
// POLL_MS while visible - the run going on and the last run's summary - and shows the scores as
// a card per breakpoint with a tile per category, coloured by its rating, which it also says in
// words, with its change since the previous run as an arrow; arrow keys move through the tiles.
// `Run` calls the command `run` (its answer may come after the panels window stopped waiting;
// the status shows the run either way), `Cancel` the tool `cancel`. The measured page opens in
// the browser through the host's `open_link`, on a click only.
import { computed, nextTick, onBeforeUnmount, ref, watch } from 'vue';
import { usePanel } from '../../../../web-common/panel-ui/src/mount';
import { format } from '../../../../web-common/panel-ui/src/format';
import type { StatusLine } from '../../../../web-common/panel-ui/src/status';
import IconAction from '../../../../web-common/panel-ui/src/components/IconAction.vue';
import PanelFrame from '../../../../web-common/panel-ui/src/components/PanelFrame.vue';
import { moveCell, pageLink, rows as scoreRows, signed, type LighthouseStatus, type ScoreCell } from './model';

const POLL_MS = 2000;
const panel = usePanel();
const t = panel.t;

const status = ref<LighthouseStatus | null>(null);
const runFailure = ref<string | null>(null);
const statusFailure = ref<string | null>(null);
const starting = ref(false);
const cell = ref<[number, number]>([0, 0]);
const cards = ref<HTMLElement | null>(null);
let inFlight = false;

const latest = computed(() => status.value?.latest ?? null);
const table = computed(() => scoreRows(latest.value));
const running = computed(() => status.value?.sidecar?.running ?? null);
const link = computed(() => pageLink(latest.value?.url ?? latest.value?.breakpoints?.[0]?.url));
const lastUrl = computed(() => latest.value?.url ?? latest.value?.breakpoints?.[0]?.url ?? '');

async function refresh(): Promise<void> {
  if (inFlight) return;
  inFlight = true;
  try {
    status.value = (await panel.call({ tool: 'status' })) as LighthouseStatus;
    statusFailure.value = null;
  } catch (error) {
    statusFailure.value = panel.error(error);
  } finally {
    inFlight = false;
    starting.value = false;
  }
}

/** The status line: a failure, the sidecar missing, the host's waiting, or the run going on. */
const line = computed<StatusLine | null>(() => {
  void panel.strings.value;
  const failure = runFailure.value ?? statusFailure.value;
  if (failure) return { tone: 'error', text: failure };
  const sidecar = status.value?.sidecar;
  if (sidecar && !sidecar.available) {
    const template = panel.strings.value[sidecar.error ?? ''];
    const reason = template === undefined ? [sidecar.error ?? '', sidecar.reason ?? ''].join(' ').trim() : format(template, { reason: sidecar.reason ?? '' });
    return { tone: 'error', text: t('panel.unavailable', { reason }) };
  }
  if (panel.waiting.value) return { tone: 'busy', text: panel.strings.value['plugins.panel.waiting'] ?? '' };
  if (running.value) {
    let text = t('panel.running', { breakpoint: running.value.label });
    if ((sidecar?.queued ?? 0) > 0) text += ' - ' + t('panel.queued', { count: sidecar?.queued });
    return { tone: 'busy', text };
  }
  if (starting.value) return { tone: 'busy', text: t('panel.starting') };
  return null;
});

const TONE = { good: 'success', average: 'warning', poor: 'error' } as const;
const TILE = {
  good: 'border-success/40 bg-success/10',
  average: 'border-warning/40 bg-warning/10',
  poor: 'border-error/40 bg-error/10',
} as const;
const VALUE = { good: 'text-success', average: 'text-warning', poor: 'text-error' } as const;
const DATA_TONE = { good: 'ok', average: 'warn', poor: 'error' } as const;

const ratingWord = (score: ScoreCell): string => (score.rating ? t('panel.rating.' + score.rating) : '');
function tileTitle(score: ScoreCell): string {
  const delta = score.delta === null ? '' : ', ' + t('panel.delta', { delta: signed(score.delta) });
  return t('panel.score', { score: score.score ?? '', rating: ratingWord(score) }) + delta;
}

/** A focusable tile is a group named by what it shows (RB-090-23). */
function tileName(score: ScoreCell): string {
  const shown = score.score === null ? t('panel.noScore') : tileTitle(score);
  return `${t('panel.categories.' + score.category)}: ${shown}`;
}

function run(): void {
  runFailure.value = null;
  starting.value = true;
  panel.call({ command: 'run' }).then(
    () => refresh(),
    (error: unknown) => {
      const code = error instanceof Error ? error.message : String(error);
      // The panels window stops waiting after its timeout; the run goes on.
      if (!code.startsWith('plugins.error.host_failed')) runFailure.value = panel.error(error);
      void refresh();
    },
  );
  void refresh();
}

function cancel(): void {
  panel.call({ tool: 'cancel' }).then(
    () => refresh(),
    () => refresh(),
  );
}

function openPage(): void {
  if (link.value) panel.post({ target: 'host', action: 'open_link', url: link.value });
}

const tileAt = (at: [number, number]): HTMLElement | undefined =>
  (cards.value?.querySelectorAll<HTMLElement>('[data-card]')[at[0]]?.querySelectorAll<HTMLElement>('[data-tile]')[at[1]]) ?? undefined;

function onKey(event: KeyboardEvent): void {
  const moved = moveCell(cell.value[0], cell.value[1], event.key, table.value.rows.length, table.value.categories.length);
  if (moved === null) return;
  event.preventDefault();
  if (!tileAt(moved)) return;
  cell.value = moved;
  void nextTick(() => tileAt(moved)?.focus());
}

// A new summary keeps the focus on the same tile; the same summary rebuilds nothing.
watch(table, (now) => {
  const focused = cards.value?.contains(document.activeElement) ?? false;
  cell.value = [Math.min(cell.value[0], Math.max(now.rows.length - 1, 0)), Math.min(cell.value[1], Math.max(now.categories.length - 1, 0))];
  if (focused) void nextTick(() => tileAt(cell.value)?.focus());
});

panel.onSettings(() => {
  void refresh();
});
const timer = setInterval(() => {
  if (!document.hidden && panel.ready.value) void refresh();
}, POLL_MS);
onBeforeUnmount(() => clearInterval(timer));
</script>

<template>
  <PanelFrame :title="t('panels.lighthouse')" icon="i-lucide-gauge" :status="line">
    <template #actions>
      <IconAction id="cancel" icon="i-lucide-circle-stop" :label="t('panel.cancel')" :disabled="!running" @click="cancel" />
      <IconAction id="run" icon="i-lucide-play" :label="t('panel.run')" primary @click="run" />
    </template>

    <div class="flex flex-col gap-3">
      <p v-if="latest" id="last" class="flex flex-wrap items-center gap-1.5 text-sm text-muted">
        <UIcon name="i-lucide-clock" class="size-4 shrink-0" />
        <span class="break-all">{{ t('panel.last', { run: latest.runId, url: lastUrl }) }}</span>
        <UButton
          v-if="link"
          id="open-page"
          size="xs"
          variant="link"
          color="primary"
          icon="i-lucide-external-link"
          :label="t('panel.openPage')"
          @click="openPage"
        />
      </p>

      <UEmpty v-if="!latest" id="empty" icon="i-lucide-gauge" :title="t('panel.idle')" variant="naked" />

      <p v-if="table.rows.length > 0" id="caption" class="text-xs text-muted">{{ t('panel.caption') }}</p>

      <div id="rows" ref="cards" class="flex flex-col gap-3" @keydown="onKey">
        <UCard
          v-for="(row, r) in table.rows"
          :key="row.breakpoint"
          as="section"
          data-card
          :data-breakpoint="row.breakpoint"
          :aria-labelledby="'breakpoint-' + r"
          :ui="{ header: 'flex items-center gap-2 px-3 py-2 sm:px-3', body: 'p-3 sm:p-3' }"
        >
          <template #header>
            <h2 :id="'breakpoint-' + r" class="flex-1 text-sm font-semibold text-highlighted">{{ row.breakpoint }}</h2>
            <UBadge
              data-findings
              :color="row.findings > 0 ? 'warning' : 'neutral'"
              variant="subtle"
              :icon="row.findings > 0 ? 'i-lucide-triangle-alert' : undefined"
              :label="t('panel.findings', { count: row.findings })"
            />
          </template>
          <div class="grid grid-cols-[repeat(auto-fit,minmax(140px,1fr))] gap-2">
            <div
              v-for="(score, c) in row.cells"
              :key="score.category"
              data-tile
              :data-category="score.category"
              :data-tone="score.rating ? DATA_TONE[score.rating] : undefined"
              :tabindex="r === cell[0] && c === cell[1] ? 0 : -1"
              role="group"
              :aria-label="tileName(score)"
              :title="score.score === null ? undefined : tileTitle(score)"
              class="rounded-md border p-2 outline-none focus-visible:ring-2 focus-visible:ring-primary"
              :class="score.rating ? TILE[score.rating] : 'border-default bg-elevated/50'"
              @focus="cell = [r, c]"
            >
              <div data-label class="text-xs text-muted">{{ t('panel.categories.' + score.category) }}</div>
              <div v-if="score.score === null" data-none class="py-1 text-sm text-muted">{{ t('panel.noScore') }}</div>
              <template v-else>
                <div class="flex flex-wrap items-baseline gap-1.5">
                  <span data-value class="text-2xl font-semibold tabular-nums" :class="score.rating ? VALUE[score.rating] : ''">{{ score.score }}</span>
                  <span
                    v-if="score.delta !== null"
                    data-delta
                    :data-tone="score.delta > 0 ? 'ok' : 'error'"
                    class="inline-flex items-center gap-0.5 text-xs"
                    :class="score.delta > 0 ? 'text-success' : 'text-error'"
                  >
                    <UIcon :name="score.delta > 0 ? 'i-lucide-arrow-up' : 'i-lucide-arrow-down'" :data-icon="score.delta > 0 ? 'arrow-up' : 'arrow-down'" class="size-3.5" />
                    <span aria-hidden="true">{{ signed(score.delta) }}</span>
                    <span class="sr-only">{{ t('panel.delta', { delta: signed(score.delta) }) }}</span>
                  </span>
                </div>
                <UBadge v-if="score.rating" data-rating :color="TONE[score.rating]" variant="soft" size="sm" :label="ratingWord(score)" />
              </template>
            </div>
          </div>
        </UCard>
      </div>

      <div id="notes" class="flex flex-col gap-1.5">
        <UAlert
          v-for="skip in latest?.skipped ?? []"
          :key="'skip-' + skip.breakpoint"
          data-note="info"
          color="info"
          variant="subtle"
          icon="i-lucide-info"
          :title="t('panel.skipped', { breakpoint: skip.breakpoint, reason: t('panel.reasons.' + skip.reason, { engine: skip.engine ?? '' }) })"
          :ui="{ root: 'px-3 py-1.5', title: 'text-sm font-normal' }"
        />
        <UAlert
          v-for="failed in latest?.failures ?? []"
          :key="'failed-' + failed.breakpoint"
          data-note="error"
          color="error"
          variant="subtle"
          icon="i-lucide-circle-x"
          :title="t('panel.failed', { breakpoint: failed.breakpoint, reason: failed.reason === 'too_large' ? t('panel.reasons.too_large') : panel.error(failed.reason) })"
          :ui="{ root: 'px-3 py-1.5', title: 'text-sm font-normal' }"
        />
        <UAlert
          v-if="latest?.cancelled"
          data-note="warn"
          color="warning"
          variant="subtle"
          icon="i-lucide-triangle-alert"
          :title="t('panel.cancelled')"
          :ui="{ root: 'px-3 py-1.5', title: 'text-sm font-normal' }"
        />
      </div>

      <section v-if="table.rows.length > 0" class="flex flex-col gap-1">
        <h2 id="reports-title" class="flex items-center gap-1.5 text-xs font-semibold uppercase text-toned">
          <UIcon name="i-lucide-file-text" class="size-4" />
          <span>{{ t('panel.reports') }}</span>
        </h2>
        <ul id="reports" class="divide-y divide-default">
          <li v-for="row in table.rows" :key="row.breakpoint" class="flex flex-wrap items-center gap-2 py-1.5">
            <span class="flex min-w-0 flex-[1_1_200px] items-center gap-1.5 text-sm" :title="row.report">
              <UIcon name="i-lucide-file-text" data-icon="file-text" class="size-4 shrink-0 text-muted" />
              <span class="break-all">{{ t('panel.report', { breakpoint: row.breakpoint, path: row.report }) }}</span>
            </span>
            <UBadge :color="row.findings > 0 ? 'warning' : 'neutral'" variant="subtle" :label="t('panel.findings', { count: row.findings })" />
          </li>
        </ul>
      </section>
    </div>
  </PanelFrame>
</template>
