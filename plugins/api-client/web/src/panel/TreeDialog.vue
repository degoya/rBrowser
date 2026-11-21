<script setup lang="ts">
// The question a collection action asks (RB-096-11), at the top of the side column where the
// action was chosen - no window over the panel: a new collection's name, a new name for a
// collection or a request's title, the collection a request moves to (a pick list), and the
// word before a delete, which names what goes (a collection's requests counted). Enter answers,
// Escape cancels.
import { computed, ref, watch } from 'vue';
import { usePanel } from '../../../../web-common/panel-ui/src/mount';
import { type Dialog, folderName, isCollectionName, joinFolder } from './collections';

const props = defineProps<{ dialog: Dialog }>();
const emit = defineEmits<{ answer: [value: string]; cancel: [] }>();
const t = usePanel().t;

/** The top of the collections as a select item: an item's value cannot be empty. */
const TOP = '/';
const text = ref('');
const choice = ref(TOP);
watch(
  () => props.dialog,
  (dialog) => {
    text.value = dialog.kind === 'rename' ? dialog.name : dialog.kind === 'renameFolder' ? folderName(dialog.folder) : '';
    choice.value = dialog.kind === 'move' ? (dialog.targets[0] || TOP) : TOP;
  },
  { immediate: true },
);

const asks = computed(() => ['newFolder', 'renameFolder', 'rename'].includes(props.dialog.kind));
const danger = computed(() => props.dialog.kind === 'delete' || props.dialog.kind === 'deleteFolder');
const title = computed(() => {
  const dialog = props.dialog;
  switch (dialog.kind) {
    case 'newFolder':
      return dialog.parent ? t('panel.dialog.newFolderIn', { folder: dialog.parent }) : t('panel.dialog.newFolder');
    case 'renameFolder':
      return t('panel.dialog.renameFolder', { folder: dialog.folder });
    case 'rename':
      return t('panel.dialog.rename', { id: dialog.id });
    case 'move':
      return t('panel.dialog.move', { id: dialog.id });
    case 'delete':
      return t('panel.confirmDelete', { id: dialog.id });
    case 'deleteFolder':
      return t('panel.dialog.deleteFolder', { folder: dialog.folder, count: dialog.count });
  }
  return '';
});
const targetItems = computed(() =>
  props.dialog.kind === 'move' ? props.dialog.targets.map((folder) => ({ label: folder || t('panel.top'), value: folder || TOP })) : [],
);
/** The folder a name makes: below the parent, or beside the renamed one. */
const folderOfText = computed(() => {
  const dialog = props.dialog;
  if (dialog.kind === 'newFolder') return joinFolder(dialog.parent, text.value);
  if (dialog.kind === 'renameFolder') return joinFolder(dialog.folder.slice(0, Math.max(dialog.folder.lastIndexOf('/'), 0)), text.value);
  return '';
});
const problem = computed(() => {
  if (props.dialog.kind === 'rename') return text.value.trim() === '' ? t('panel.dialog.noName') : null;
  if (!asks.value) return null;
  return isCollectionName(folderOfText.value) ? null : t('panel.dialog.badFolder');
});

function answer(): void {
  if (problem.value) return;
  const dialog = props.dialog;
  if (dialog.kind === 'move') emit('answer', choice.value === TOP ? '' : choice.value);
  else if (dialog.kind === 'rename') emit('answer', text.value.trim());
  else if (asks.value) emit('answer', folderOfText.value);
  else emit('answer', '');
}
</script>

<template>
  <form id="tree-dialog" role="alertdialog" :aria-label="title" class="flex flex-col gap-1.5 rounded-md border border-default p-2" @submit.prevent="answer" @keydown.escape.prevent="emit('cancel')">
    <p class="text-sm" :class="danger ? 'text-error' : ''">{{ title }}</p>
    <UInput v-if="asks" id="tree-dialog-text" v-model="text" size="sm" class="w-full font-mono" autofocus :aria-label="title" :aria-invalid="problem ? 'true' : undefined" />
    <USelect v-if="dialog.kind === 'move'" id="tree-dialog-target" v-model="choice" :items="targetItems" value-key="value" size="sm" icon="i-lucide-folder" class="w-full" :aria-label="title" />
    <p v-if="problem && text !== ''" id="tree-dialog-problem" class="text-xs text-error">{{ problem }}</p>
    <div class="flex justify-end gap-1.5">
      <UButton id="tree-dialog-cancel" size="xs" color="neutral" variant="outline" :label="danger ? t('panel.keep') : t('panel.dialog.cancel')" :autofocus="danger" @click="emit('cancel')" />
      <UButton id="tree-dialog-ok" type="submit" size="xs" :color="danger ? 'error' : 'primary'" :disabled="problem !== null" :label="danger ? t('panel.delete') : t('panel.dialog.ok')" />
    </div>
  </form>
</template>
