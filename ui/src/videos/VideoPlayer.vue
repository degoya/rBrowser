<script setup lang="ts">
// One recording's views side by side (RB-110-12), played synchronously: one Play/Pause, one
// scrubber and one speed for all of them (`sync.ts`), a video that drifts pulled back to the
// longest. Each view says its breakpoint, duration and size and offers Show in folder, Copy path
// and Delete (the page asks before it deletes). The videos come from the core's route
// `rbrowser://video/<recording>/<file>` (range requests, so the scrubber seeks); a video leaving
// the page drops its source, so the core can delete the file. `capture.video.autoplay` starts
// the recording once its first video knows its length. Each video shows its poster (its first
// frame, written by the recorder) until it has a frame of its own, both in one box of the
// picture's shape (`mediaBox`). A video the player cannot stream from the route is loaded whole
// once and played from memory (`memory.ts`, at most 512 MiB per recording, revoked when the
// recording is left); one that still cannot play says why under it with the player's error - an
// MP4 is H.264, which rBrowser's Chromium cannot decode - and the page reports it once.
// RB-090-52: one view per breakpoint (`viewsOf`) - the WebM played, its MP4 export listed under
// it with its own Show, Copy and Delete; a WebM offers "Export as MP4" where the platform can
// (else one line says why), an old MP4 "Convert to WebM"; a waiting or running job shows how far
// it is instead of the button.
import { computed, onBeforeUnmount, reactive, ref } from 'vue';
import { useI18n } from 'vue-i18n';
import UButton from '@nuxt/ui/components/Button.vue';
import UIcon from '@nuxt/ui/components/Icon.vue';
import USelect from '@nuxt/ui/components/Select.vue';
import USlider from '@nuxt/ui/components/Slider.vue';
import type { VideoConversion, VideoFile, VideoRecording, VideoTarget } from '../ipc/protocol';
import FileActions from './FileActions.vue';
import { VIDEO_ICONS } from './icons';
import { MemoryCopies, Released, TooLarge, failureOf, playsFromMemory } from './memory';
import { SyncPlayer } from './sync';
import {
  SPEEDS,
  conversionOf,
  conversionText,
  durationText,
  mediaBox,
  memoryNotice,
  offerFor,
  playError,
  sizeText,
  tooLargeError,
  viewsOf,
  type RecordedView,
  type Text,
} from './videos';

const props = withDefaults(
  defineProps<{
    recording: VideoRecording;
    autoplay: boolean;
    /** The platform can export MP4 (`VideoList.mp4Export`). */
    mp4Export?: boolean;
    /** The exports and conversions waiting and running. */
    conversions?: VideoConversion[];
  }>(),
  { mp4Export: false, conversions: () => [] },
);
const emit = defineEmits<{
  reveal: [file: string];
  copy: [path: string];
  remove: [file: string];
  /** Make `file` into `to` beside it (RB-090-52). */
  convert: [file: string, to: VideoTarget];
  /** Autoplay started: the page plays the pick only once. */
  autoplayed: [];
  /** A video cannot play, or plays only from memory (`level` info), once per video: the page
   * reports it to the status bar (and so to the log). */
  failed: [why: Text & { settingsKey?: string; level?: 'info' }];
}>();
const { t } = useI18n();

const player = new SyncPlayer();
const durations = reactive<Record<string, number>>({});
/** Why a video does not play, by file name. */
const failures = reactive<Record<string, Text>>({});
/** The videos played from memory: their object URL, by file name. */
const fromMemory = reactive<Record<string, string>>({});
/** The videos whose copy in memory was tried. */
const triedMemory = new Set<string>();
const copies = new MemoryCopies();
/** The picture's size of each video: its poster's until the video knows its own. */
const sizes = reactive<Record<string, { width: number; height: number }>>({});
/** The videos showing a frame of their own: their poster goes. */
const framed = reactive<Record<string, boolean>>({});
const time = ref(0);
const total = ref(0);
const playing = ref(false);
const rate = ref(1);
let autoplayed = false;

const views = computed(() => viewsOf(props.recording));
/** A WebM without its MP4 where the platform cannot export: the line that says why. */
const exportMissing = computed(() => !props.mp4Export && views.value.some((view) => offerFor(view, true) === 'mp4'));

function conversion(file: string): VideoConversion | undefined {
  return conversionOf(props.conversions, props.recording.id, file);
}

/** What the view offers to make now: nothing while its own job waits or runs. */
function offer(view: RecordedView): VideoTarget | null {
  return conversion(view.play.name) === undefined ? offerFor(view, props.mp4Export) : null;
}

function offerLabel(view: RecordedView): string {
  const key = offer(view) === 'mp4' ? 'videos.convert.export_mp4' : 'videos.convert.to_webm';
  return t(key, { name: view.play.name });
}

function convert(view: RecordedView): void {
  const to = offer(view);
  if (to !== null) {
    emit('convert', view.play.name, to);
  }
}

function progressText(file: string): string {
  const job = conversion(file);
  if (job === undefined) {
    return '';
  }
  const text = conversionText(job);
  return t(text.key, text.args);
}

const speeds = computed(() =>
  SPEEDS.map((speed) => ({ label: t('videos.speed_value', { speed: String(speed) }), value: speed })),
);

/** The `<video>` of each file, attached to the player while it is in the page. */
const elements = new Map<string, HTMLVideoElement>();

/** Lets go of a video: no source, so the browser cancels its request and the file is free. */
function release(element: HTMLVideoElement): void {
  player.detach(element);
  element.pause();
  element.removeAttribute('src');
  if (typeof element.load === 'function') {
    element.load();
  }
}

function videoRef(name: string) {
  return (element: unknown) => {
    const before = elements.get(name);
    if (element instanceof HTMLVideoElement) {
      if (before !== element) {
        if (before !== undefined) {
          release(before);
        }
        elements.set(name, element);
        player.attach(element);
      }
    } else if (element === null && before !== undefined) {
      release(before);
      elements.delete(name);
    }
  };
}

function refresh(): void {
  time.value = player.time;
  total.value = player.duration;
  playing.value = player.playing;
}

async function toggle(play = !player.playing): Promise<void> {
  if (play) {
    await player.play();
  } else {
    player.pause();
  }
  refresh();
}

/** Where video `name` plays from: the route, or its copy in memory once the route failed. */
function sourceOf(file: { name: string; url: string }): string {
  return fromMemory[file.name] ?? file.url;
}

function onPoster(name: string, event: Event): void {
  const image = event.target;
  if (image instanceof HTMLImageElement && sizes[name] === undefined && image.naturalWidth > 0) {
    sizes[name] = { width: image.naturalWidth, height: image.naturalHeight };
  }
}

function onMetadata(name: string, event: Event): void {
  const element = event.target;
  if (element instanceof HTMLVideoElement) {
    durations[name] = element.duration;
    if (element.videoWidth > 0 && element.videoHeight > 0) {
      sizes[name] = { width: element.videoWidth, height: element.videoHeight };
    }
  }
  refresh();
  if (props.autoplay && !autoplayed) {
    autoplayed = true;
    emit('autoplayed');
    void toggle(true);
  }
}

function fail(name: string, why: Text & { settingsKey?: string }): void {
  if (failures[name] === undefined) {
    failures[name] = why;
    emit('failed', why);
  }
}

/** A video failed: the first time from the route, it is loaded whole and played from memory
 * (an MP4 is not: memory decodes no H.264); else, or beyond the limit, it says why. */
async function onError(file: VideoFile, event: Event): Promise<void> {
  const element = event.target;
  const name = file.name;
  // A video without a source (one let go of) is no failure.
  if (!(element instanceof HTMLVideoElement) || failures[name] !== undefined || !element.getAttribute('src')) {
    return;
  }
  const failure = failureOf(element.error);
  if (triedMemory.has(name) || !playsFromMemory(name)) {
    fail(name, playError(name, failure));
    return;
  }
  triedMemory.add(name);
  try {
    fromMemory[name] = await copies.load(file);
    emit('failed', memoryNotice(name, failure));
  } catch (error) {
    if (error instanceof Released) {
      return;
    }
    fail(name, error instanceof TooLarge ? tooLargeError(name, file.bytes, error.limit, failure) : playError(name, failure));
  }
}

onBeforeUnmount(() => {
  copies.release();
});

function failureText(name: string): string {
  const why = failures[name];
  return why === undefined ? '' : t(why.key, why.args);
}

function onTime(): void {
  player.resync();
  refresh();
}

function seek(value: number | undefined): void {
  if (typeof value === 'number') {
    player.seek(value);
    time.value = value;
  }
}

function setRate(value: unknown): void {
  const speed = Number(value);
  if (SPEEDS.includes(speed)) {
    rate.value = speed;
    player.setRate(speed);
  }
}

function size(bytes: number): string {
  const text = sizeText(bytes);
  return t(text.key, text.args);
}
</script>

<template>
  <div class="flex flex-col gap-3" data-testid="video-player">
    <div role="group" :aria-label="t('videos.controls')" class="flex flex-wrap items-center gap-2">
      <UButton
        :icon="playing ? VIDEO_ICONS.pause : VIDEO_ICONS.play"
        :label="playing ? t('videos.pause') : t('videos.play')"
        size="xs"
        data-testid="video-play"
        @click="toggle()"
      />
      <span class="text-xs tabular-nums text-muted" data-testid="video-time">
        {{ durationText(time) || '0:00' }} / {{ durationText(total) || '0:00' }}
      </span>
      <USlider
        :model-value="time"
        :min="0"
        :max="Math.max(total, 0.01)"
        :step="0.01"
        size="xs"
        class="min-w-32 flex-1"
        :aria-label="t('videos.seek')"
        data-testid="video-seek"
        @update:model-value="seek"
      />
      <USelect
        :model-value="rate"
        :items="speeds"
        size="xs"
        class="w-24"
        :aria-label="t('videos.speed')"
        data-testid="video-speed"
        @update:model-value="setRate"
      />
    </div>
    <p v-if="exportMissing" class="m-0 text-xs text-muted" data-testid="mp4-unavailable">
      {{ t('videos.convert.mp4_unavailable') }}
    </p>
    <div class="grid gap-3 [grid-template-columns:repeat(auto-fit,minmax(12rem,1fr))]">
      <figure
        v-for="view in views"
        :key="view.play.name"
        class="m-0 flex min-w-0 flex-col gap-1"
        :data-testid="`video-${view.play.name}`"
      >
        <div
          class="relative mx-auto w-full overflow-hidden rounded-sm bg-elevated"
          :style="mediaBox(sizes[view.play.name])"
          :data-testid="`video-box-${view.play.name}`"
        >
          <video
            :ref="videoRef(view.play.name)"
            :src="sourceOf(view.play)"
            preload="metadata"
            muted
            playsinline
            class="absolute inset-0 size-full object-contain"
            :aria-label="t('videos.view', { breakpoint: view.breakpoint })"
            @loadedmetadata="onMetadata(view.play.name, $event)"
            @loadeddata="framed[view.play.name] = true"
            @timeupdate="onTime"
            @play="refresh"
            @pause="refresh"
            @ended="refresh"
            @error="onError(view.play, $event)"
          />
          <img
            v-if="view.play.poster !== undefined && !framed[view.play.name]"
            :src="view.play.poster"
            alt=""
            decoding="async"
            class="pointer-events-none absolute inset-0 size-full object-contain"
            :data-testid="`poster-${view.play.name}`"
            @load="onPoster(view.play.name, $event)"
          />
        </div>
        <p
          v-if="failures[view.play.name] !== undefined"
          role="alert"
          class="m-0 text-xs text-error"
          :data-testid="`video-error-${view.play.name}`"
        >
          {{ failureText(view.play.name) }}
        </p>
        <figcaption class="flex min-w-0 flex-col gap-1 text-xs">
          <span class="flex min-w-0 flex-wrap items-center gap-x-2">
            <span class="truncate font-medium">{{ view.breakpoint }}</span>
            <span v-if="durationText(durations[view.play.name] ?? Number.NaN)" class="tabular-nums text-muted">
              {{ durationText(durations[view.play.name] ?? Number.NaN) }}
            </span>
            <span class="text-muted">{{ size(view.play.bytes) }}</span>
            <span class="ms-auto flex">
              <UButton
                v-if="offer(view) !== null"
                :icon="offer(view) === 'mp4' ? VIDEO_ICONS.exportMp4 : VIDEO_ICONS.convertWebm"
                color="neutral"
                variant="ghost"
                size="xs"
                square
                :aria-label="offerLabel(view)"
                :title="offerLabel(view)"
                :data-testid="`convert-${view.play.name}`"
                @click="convert(view)"
              />
              <FileActions :file="view.play" @reveal="emit('reveal', $event)" @copy="emit('copy', $event)" @remove="emit('remove', $event)" />
            </span>
          </span>
          <span
            v-if="conversion(view.play.name) !== undefined"
            role="status"
            class="flex items-center gap-1 text-muted"
            :data-testid="`converting-${view.play.name}`"
          >
            <UIcon :name="VIDEO_ICONS.converting" class="size-3.5 shrink-0 motion-safe:animate-spin" />
            {{ progressText(view.play.name) }}
          </span>
          <span v-if="view.mp4 !== undefined" class="flex min-w-0 flex-wrap items-center gap-x-2" :data-testid="`export-${view.mp4.name}`">
            <span class="truncate">{{ view.mp4.name }}</span>
            <span class="text-muted">{{ size(view.mp4.bytes) }}</span>
            <span class="ms-auto flex">
              <FileActions :file="view.mp4" @reveal="emit('reveal', $event)" @copy="emit('copy', $event)" @remove="emit('remove', $event)" />
            </span>
          </span>
        </figcaption>
      </figure>
    </div>
  </div>
</template>
