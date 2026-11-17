<script setup lang="ts">
// One video file's buttons in the Videos panel (RB-110-12, RB-090-52): Show in folder, Copy path
// and Delete (the page asks before it deletes) - for a recorded view and for its MP4 export alike.
import { useI18n } from 'vue-i18n';
import UButton from '@nuxt/ui/components/Button.vue';
import type { VideoFile } from '../ipc/protocol';
import { VIDEO_ICONS } from './icons';

defineProps<{ file: VideoFile }>();
const emit = defineEmits<{
  reveal: [file: string];
  copy: [path: string];
  remove: [file: string];
}>();
const { t } = useI18n();
</script>

<template>
  <UButton
    :icon="VIDEO_ICONS.reveal"
    color="neutral"
    variant="ghost"
    size="xs"
    square
    :aria-label="t('videos.reveal_file', { name: file.name })"
    :title="t('videos.reveal_file', { name: file.name })"
    :data-testid="`reveal-${file.name}`"
    @click="emit('reveal', file.name)"
  />
  <UButton
    :icon="VIDEO_ICONS.copyPath"
    color="neutral"
    variant="ghost"
    size="xs"
    square
    :aria-label="t('videos.copy_path', { name: file.name })"
    :title="t('videos.copy_path', { name: file.name })"
    :data-testid="`copy-${file.name}`"
    @click="emit('copy', file.path)"
  />
  <UButton
    :icon="VIDEO_ICONS.remove"
    color="error"
    variant="ghost"
    size="xs"
    square
    :aria-label="t('videos.delete_file', { name: file.name })"
    :title="t('videos.delete_file', { name: file.name })"
    :data-testid="`delete-${file.name}`"
    @click="emit('remove', file.name)"
  />
</template>
