<script setup lang="ts">
// The Videos panel (RB-110-12), a core panel of the panels page (`panels/core.ts`): the open
// project's recordings (`.rbrowser/out/video/<date-time>/`, one file per view) grouped by day,
// newest first, the chosen one's views side by side and played together (`VideoPlayer.vue`).
// Nothing plays or loads before the person picks a recording (security review: no video reaches
// the page's renderer unasked; the core lists only what rBrowser recorded itself), and
// `capture.video.autoplay` applies only to that pick.
// `capture.video.panel` (Ctrl+Shift+Space C T) opens it; the core's `videosChanged` (a recording
// saved, the retention ran) refreshes it without a loading state. Deleting a recording or a view
// asks first, in place, and says how many videos go; the players of the recording leave the page
// before the request, so Windows lets the files go. The recording being written is registered
// only once saved, so it is not in the list. A recording shows its poster in the list (a JPEG
// of its first frame, from the core's video route like the videos); a video that cannot play
// is reported to the status bar once (`reportStatus`). A view's "Export as MP4" or "Convert to
// WebM" (RB-090-52) queues the job in the core (`convertVideo`); the list then carries it, and
// `videosChanged` brings its progress and its result.
import { computed, nextTick, onBeforeUnmount, onMounted, ref } from 'vue';
import { useI18n } from 'vue-i18n';
import UButton from '@nuxt/ui/components/Button.vue';
import type { IpcClient } from '../ipc/client';
import { IpcErrorCode, type VideoList, type VideoTarget } from '../ipc/protocol';
import { IpcError } from '../ipc/transport';
import { windowEvents, type Subscribe } from '../whichkey/whichkey';
import { videosChannel } from './host';
import { VIDEO_ICONS } from './icons';
import VideoPlayer from './VideoPlayer.vue';
import { deleteQuestion, groupByDay, recordingPoster, recordingTime, sizeText, viewsOf, type Text } from './videos';

defineOptions({ inheritAttrs: false });
const props = withDefaults(
  defineProps<{
    client: IpcClient;
    subscribe?: Subscribe;
    /** The clipboard "Copy path" writes to; the browser's when absent. */
    clipboard?: Pick<Clipboard, 'writeText'>;
  }>(),
  { subscribe: undefined, clipboard: undefined },
);
const { t } = useI18n();
const channel = videosChannel(props.client, props.subscribe ?? windowEvents);

const list = ref<VideoList | null>(null);
/** Why the list could not be had; with no list yet, the page shows it instead. */
const failure = ref<string | null>(null);
/** Why the last action failed. */
const actionError = ref<string | null>(null);
const chosen = ref<string | null>(null);
/** The deletion waiting for the person's word: a recording, or one of its files. */
const asking = ref<{ recording: string; file: string | null } | null>(null);
/** The recording whose players left the page for its deletion. */
const unloading = ref<string | null>(null);
const copied = ref(false);

const groups = computed(() => groupByDay(list.value?.recordings ?? []));
const current = computed(() => list.value?.recordings.find((recording) => recording.id === chosen.value) ?? null);
const question = computed(() => {
  if (asking.value === null || current.value === null) {
    return '';
  }
  const text = deleteQuestion(current.value, asking.value.file);
  return t(text.key, text.args);
});
const total = computed(() => {
  const bytes = current.value?.files.reduce((sum, file) => sum + file.bytes, 0) ?? 0;
  const text = sizeText(bytes);
  return t(text.key, text.args);
});

function errorText(error: unknown): string {
  if (error instanceof IpcError && error.code === IpcErrorCode.Refused) {
    return t('videos.error.refused');
  }
  if (error instanceof IpcError && error.code === IpcErrorCode.NotFound) {
    return t('videos.error.not_found');
  }
  return t('videos.error.failed');
}

function take(next: VideoList): void {
  list.value = next;
  failure.value = null;
  // Nothing is chosen for the person: a recording is loaded only once they pick it.
  if (!next.recordings.some((recording) => recording.id === chosen.value)) {
    chosen.value = null;
  }
}

/** Lists in order: an answer older than the newest request is dropped. */
let seq = 0;
async function load(): Promise<void> {
  const mine = ++seq;
  try {
    const next = await channel.list();
    if (mine === seq) {
      take(next);
    }
  } catch (error) {
    if (mine === seq) {
      failure.value = errorText(error);
    }
  }
}

/** The recording the person just picked, for `capture.video.autoplay`: it plays once on that
 * pick, never again when its player is remounted (a view deleted). */
const autoplayFor = ref<string | null>(null);

function choose(id: string): void {
  chosen.value = id;
  autoplayFor.value = id;
  asking.value = null;
  actionError.value = null;
}

function ask(file: string | null): void {
  if (current.value !== null) {
    asking.value = { recording: current.value.id, file };
    actionError.value = null;
  }
}

async function confirmDelete(): Promise<void> {
  const target = asking.value;
  if (target === null) {
    return;
  }
  asking.value = null;
  // The player comes back without playing by itself: autoplay is for the pick only.
  autoplayFor.value = null;
  unloading.value = target.recording;
  await nextTick();
  try {
    seq += 1;
    take(await channel.remove(target.recording, target.file));
  } catch (error) {
    actionError.value = errorText(error);
  } finally {
    unloading.value = null;
  }
}

function reveal(file: string | null): void {
  if (current.value === null) {
    return;
  }
  actionError.value = null;
  channel.reveal(current.value.id, file).catch((error: unknown) => {
    actionError.value = errorText(error);
  });
}

/** Queues making `file` of the chosen recording into `to`; the core says why when it cannot. */
async function convert(file: string, to: VideoTarget): Promise<void> {
  if (current.value === null) {
    return;
  }
  actionError.value = null;
  try {
    seq += 1;
    take(await channel.convert(current.value.id, file, to));
  } catch (error) {
    const reason = error instanceof Error ? error.message : '';
    actionError.value = t('videos.convert.failed', { name: file, reason });
  }
}

/** A video of the chosen recording cannot play, or plays only from memory (`info`): the status
 * bar keeps why, and the core logs it with the player's error. */
function reportFailure(why: Text & { settingsKey?: string; level?: 'info' }): void {
  const args = Object.fromEntries(Object.entries(why.args).map(([name, value]) => [name, String(value)]));
  const settings = why.settingsKey === undefined ? {} : { settingsKey: why.settingsKey };
  void props.client
    .request({ type: 'reportStatus', level: why.level ?? 'warning', messageKey: why.key, args, ...settings })
    .catch(() => undefined);
}

let copiedTimer: ReturnType<typeof setTimeout> | undefined;
async function copy(path: string): Promise<void> {
  try {
    await (props.clipboard ?? navigator.clipboard).writeText(path);
    copied.value = true;
    clearTimeout(copiedTimer);
    copiedTimer = setTimeout(() => {
      copied.value = false;
    }, 2000);
  } catch {
    actionError.value = t('videos.error.copy');
  }
}

const stop = channel.onChanged(() => void load());
onMounted(() => void load());
onBeforeUnmount(() => {
  stop();
  clearTimeout(copiedTimer);
});
</script>

<template>
  <div class="flex h-full min-h-0 flex-col overflow-auto bg-default text-default" data-testid="videos-page">
    <p v-if="list === null && failure === null" class="p-4 text-sm text-muted" data-testid="videos-loading">
      {{ t('videos.loading') }}
    </p>
    <div
      v-else-if="list === null || list.error !== undefined"
      role="alert"
      class="m-3 flex flex-wrap items-center gap-2 rounded-md border border-error/40 p-3 text-sm text-error"
      data-testid="videos-failed"
    >
      <span>{{ list === null ? failure : t('videos.error.folder', { reason: list.error ?? '' }) }}</span>
      <UButton variant="link" size="xs" data-testid="videos-retry" @click="load">{{ t('videos.retry') }}</UButton>
    </div>
    <p
      v-else-if="!list.project"
      class="m-3 rounded-md border border-dashed border-default px-3 py-4 text-center text-sm text-muted"
      data-testid="videos-no-project"
    >
      {{ t('videos.no_project') }}
    </p>
    <p
      v-else-if="list.recordings.length === 0"
      class="m-3 rounded-md border border-dashed border-default px-3 py-4 text-center text-sm text-muted"
      data-testid="videos-empty"
    >
      {{ t('videos.empty') }}
    </p>
    <div v-else class="flex min-h-0 flex-1 flex-wrap">
      <nav :aria-label="t('videos.recordings')" class="max-h-full shrink-0 grow basis-48 overflow-auto border-e border-default sm:grow-0">
        <section v-for="group in groups" :key="group.day">
          <h3 class="px-3 pt-3 pb-1 text-xs font-semibold text-muted" data-testid="videos-day">{{ group.day }}</h3>
          <ul class="m-0 list-none p-0">
            <li v-for="recording in group.recordings" :key="recording.id">
              <UButton
                color="neutral"
                :variant="recording.id === chosen ? 'soft' : 'ghost'"
                size="sm"
                block
                class="flex-wrap justify-start gap-x-2 rounded-none px-3"
                :class="recording.id === chosen ? 'font-medium' : 'font-normal'"
                :aria-current="recording.id === chosen ? 'true' : undefined"
                :data-testid="`recording-${recording.id}`"
                @click="choose(recording.id)"
              >
                <img
                  v-if="recordingPoster(recording) !== undefined"
                  :src="recordingPoster(recording)"
                  alt=""
                  loading="lazy"
                  decoding="async"
                  class="aspect-video w-16 shrink-0 rounded-sm bg-elevated object-cover"
                />
                <span class="tabular-nums">{{ recordingTime(recording.id) }}</span>
                <span class="text-xs text-muted">{{ t('videos.views', { count: viewsOf(recording).length }) }}</span>
              </UButton>
            </li>
          </ul>
        </section>
      </nav>
      <section v-if="current !== null" class="min-w-64 flex-1 overflow-auto p-3" data-testid="videos-detail">
        <header class="mb-3 flex flex-wrap items-center gap-2">
          <h2 class="text-sm font-semibold">{{ recordingTime(current.id) }}</h2>
          <span class="text-xs text-muted">{{ total }}</span>
          <span class="ms-auto flex">
            <UButton
              :icon="VIDEO_ICONS.reveal"
              color="neutral"
              variant="ghost"
              size="xs"
              square
              :aria-label="t('videos.reveal_recording')"
              :title="t('videos.reveal_recording')"
              data-testid="reveal-recording"
              @click="reveal(null)"
            />
            <UButton
              :icon="VIDEO_ICONS.remove"
              color="error"
              variant="ghost"
              size="xs"
              square
              :aria-label="t('videos.delete_recording')"
              :title="t('videos.delete_recording')"
              data-testid="delete-recording"
              @click="ask(null)"
            />
          </span>
        </header>
        <div
          v-if="asking !== null"
          role="group"
          :aria-label="t('videos.delete_title')"
          class="mb-3 flex flex-wrap items-center gap-2 rounded-md border border-error/40 p-2 text-sm"
          data-testid="delete-confirm"
        >
          <span class="min-w-0 flex-1">{{ question }}</span>
          <UButton color="error" size="xs" data-testid="delete-accept" @click="confirmDelete">
            {{ t('videos.delete_accept') }}
          </UButton>
          <UButton color="neutral" variant="ghost" size="xs" data-testid="delete-cancel" @click="asking = null">
            {{ t('videos.delete_cancel') }}
          </UButton>
        </div>
        <p v-if="actionError !== null" role="alert" class="mb-3 text-sm text-error" data-testid="videos-action-error">
          {{ actionError }}
        </p>
        <VideoPlayer
          v-if="unloading !== current.id"
          :key="current.id"
          :recording="current"
          :autoplay="list.autoplay && autoplayFor === current.id"
          :mp4-export="list.mp4Export"
          :conversions="list.conversions ?? []"
          @reveal="reveal"
          @convert="convert"
          @copy="copy"
          @remove="ask"
          @autoplayed="autoplayFor = null"
          @failed="reportFailure"
        />
        <p class="sr-only" aria-live="polite" data-testid="videos-copied">{{ copied ? t('videos.copied') : '' }}</p>
      </section>
      <p v-else class="min-w-64 flex-1 p-4 text-sm text-muted" data-testid="videos-choose">
        {{ t('videos.choose') }}
      </p>
    </div>
  </div>
</template>
