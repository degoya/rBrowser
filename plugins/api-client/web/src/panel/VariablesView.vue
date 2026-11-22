<script setup lang="ts">
// The Variables tab (RB-096-11): .rbrowser/api/http-client.env.json as tables - `$shared` for
// every environment, then each environment -, a row a name and its value. A row marked private
// goes to the private values in config.local.json - edited here only, no setting of the
// settings window (RB-094-27) -, never into the checked-in file; one already kept shows no value, and typing one
// replaces it. A new environment has one way (RB-096-13): its name and Create - or one click on
// one of the project's rBrowser environments the file does not have yet; renaming one is its own
// action at the environment (Rename, the new name, OK). Nothing is written until Save
// (`saveVariables`), which the core refuses when the file changed since it was read. No form
// element: the frame's sandbox blocks a submission.
import { computed, ref, watch } from 'vue';
import { usePanel } from '../../../../web-common/panel-ui/src/mount';
import IconAction from '../../../../web-common/panel-ui/src/components/IconAction.vue';
import PanelForm from '../../../../web-common/panel-ui/src/components/PanelForm.vue';
import { type SaveOp, SHARED, type VarBlock, type VarRow, type VariablesAnswer, blocksOf, environmentNameProblem, saveOf } from './variables';

const props = defineProps<{ answer: VariablesAnswer | null; problem: string | null; known: string[] }>();
const emit = defineEmits<{ save: [op: SaveOp]; reload: [] }>();
const t = usePanel().t;

const blocks = ref<VarBlock[]>([]);
const dirty = ref(false);
const added = ref('');
/** The environment being renamed and the name typed for it. */
const renaming = ref<VarBlock | null>(null);
const renameText = ref('');
watch(
  () => props.answer,
  (answer) => {
    blocks.value = answer ? blocksOf(answer) : [];
    dirty.value = false;
    renaming.value = null;
  },
  { immediate: true },
);

const names = computed(() => blocks.value.map((block) => block.name));
/** The project's rBrowser environments the file does not have yet. */
const suggested = computed(() => props.known.filter((name) => !names.value.includes(name)));
const newProblem = computed(() => environmentNameProblem(added.value, names.value));
const renameProblem = computed(() =>
  renaming.value ? environmentNameProblem(renameText.value, names.value.filter((name) => name !== renaming.value?.name)) : null,
);

function changed(): void {
  dirty.value = true;
}

function updateRow(block: VarBlock, index: number, patch: Partial<VarRow>): void {
  block.rows = block.rows.map((row, at) => (at === index ? { ...row, ...patch } : row));
  changed();
}

function togglePrivate(block: VarBlock, index: number, on: boolean): void {
  const row = block.rows[index];
  if (!row) return;
  // A kept private value is not known here: made public, it starts empty.
  updateRow(block, index, on ? { secret: true, stored: false } : { secret: false, stored: false, value: row.stored ? '' : row.value });
}

function addRow(block: VarBlock): void {
  block.rows = [...block.rows, { name: '', value: '', secret: false, stored: false }];
  changed();
}

function removeRow(block: VarBlock, index: number): void {
  block.rows = block.rows.filter((_, at) => at !== index);
  changed();
}

/** A new, empty environment `name` (its rows follow with Add). */
function create(name: string): void {
  if (environmentNameProblem(name, names.value) !== null) return;
  blocks.value = [...blocks.value, { name: name.trim(), original: null, rows: [] }];
  changed();
}

function createTyped(): void {
  if (newProblem.value !== null) return;
  create(added.value);
  added.value = '';
}

function startRename(block: VarBlock): void {
  renaming.value = block;
  renameText.value = block.name;
}

function rename(): void {
  const block = renaming.value;
  if (!block || renameProblem.value !== null) return;
  block.name = renameText.value.trim();
  renaming.value = null;
  changed();
}

function removeBlock(block: VarBlock): void {
  blocks.value = blocks.value.filter((one) => one !== block);
  changed();
}

function save(): void {
  if (props.answer) emit('save', saveOf(blocks.value, props.answer));
}
</script>

<template>
  <section id="variables" class="flex flex-col gap-3" :aria-label="t('panel.variables.title')">
    <p class="text-xs text-muted">{{ t('panel.variables.description') }}</p>
    <p v-if="problem" id="variables-problem" role="alert" class="text-xs text-error">{{ problem }}</p>
    <div class="flex flex-wrap gap-1.5">
      <UButton id="variables-save" size="sm" icon="i-lucide-save" :label="t('panel.save')" :disabled="!dirty || !answer" @click="save" />
      <UButton id="variables-reload" size="sm" color="neutral" variant="outline" icon="i-lucide-rotate-ccw" :label="t('panel.variables.reload')" @click="emit('reload')" />
    </div>
    <section v-for="(block, at) in blocks" :key="at" :data-block="block.name" class="flex flex-col gap-1.5" :aria-label="block.name">
      <div class="flex items-center gap-1.5">
        <UIcon :name="block.name === SHARED ? 'i-lucide-layers' : 'i-lucide-globe'" class="size-4 text-muted" />
        <p v-if="block.name === SHARED" class="flex-1 text-sm font-medium">{{ t('panel.variables.shared') }}</p>
        <PanelForm
          v-else-if="renaming === block"
          class="flex min-w-0 flex-1 items-center gap-1.5"
          :aria-label="t('panel.variables.renameTo', { name: block.original ?? block.name })"
          :disabled="renameProblem !== null"
          @submit="rename"
          @keydown.escape.prevent="renaming = null"
        >
          <UInput
            id="variables-rename-name"
            v-model="renameText"
            size="xs"
            class="min-w-0 flex-1 font-mono"
            autofocus
            :aria-label="t('panel.variables.renameTo', { name: block.original ?? block.name })"
            :aria-invalid="renameProblem && renameProblem !== 'empty' ? 'true' : undefined"
          />
          <UButton id="variables-rename-ok" size="xs" :label="t('panel.dialog.ok')" :disabled="renameProblem !== null" @click="rename" />
          <UButton id="variables-rename-cancel" size="xs" color="neutral" variant="ghost" :label="t('panel.dialog.cancel')" @click="renaming = null" />
        </PanelForm>
        <template v-else>
          <p class="min-w-0 flex-1 truncate font-mono text-sm">{{ block.name }}</p>
          <IconAction data-rename icon="i-lucide-pencil" :label="t('panel.variables.rename', { name: block.name })" @click="startRename(block)" />
        </template>
        <IconAction v-if="block.name !== SHARED" icon="i-lucide-trash" :label="t('panel.variables.removeEnvironment', { name: block.name })" @click="removeBlock(block)" />
      </div>
      <p v-if="renaming === block && renameProblem && renameProblem !== 'empty'" class="text-xs text-error">{{ t(`panel.variables.nameProblem.${renameProblem}`, { name: renameText.trim() }) }}</p>
      <div v-for="(row, index) in block.rows" :key="index" data-variable class="flex items-center gap-1.5">
        <UInput
          :model-value="row.name"
          size="xs"
          class="min-w-0 flex-1 font-mono"
          spellcheck="false"
          autocomplete="off"
          :disabled="row.stored"
          :placeholder="t('panel.table.name')"
          :aria-label="t('panel.table.name')"
          @update:model-value="(name) => updateRow(block, index, { name: String(name) })"
        />
        <UInput
          :model-value="row.value"
          size="xs"
          class="min-w-0 flex-[2] font-mono"
          spellcheck="false"
          autocomplete="off"
          :type="row.secret ? 'password' : 'text'"
          :placeholder="row.stored ? t('panel.variables.kept') : t('panel.table.value')"
          :aria-label="t('panel.table.value')"
          @update:model-value="(value) => updateRow(block, index, { value: String(value) })"
        />
        <UCheckbox
          :model-value="row.secret"
          :label="t('panel.variables.private')"
          size="xs"
          @update:model-value="(on) => togglePrivate(block, index, on === true)"
        />
        <IconAction icon="i-lucide-x" :label="t('panel.table.remove')" @click="removeRow(block, index)" />
      </div>
      <div>
        <UButton data-add size="xs" color="neutral" variant="ghost" icon="i-lucide-plus" :label="t('panel.table.add')" @click="addRow(block)" />
      </div>
    </section>
    <section id="variables-new" class="flex flex-col gap-1.5" :aria-label="t('panel.variables.newEnvironment')">
      <p class="text-sm font-medium">{{ t('panel.variables.newEnvironment') }}</p>
      <PanelForm class="flex flex-wrap items-center gap-1.5" :aria-label="t('panel.variables.newEnvironment')" :disabled="newProblem !== null" @submit="createTyped">
        <UInput
          id="variables-new-name"
          v-model="added"
          size="xs"
          class="min-w-0 flex-1 font-mono"
          spellcheck="false"
          autocomplete="off"
          :placeholder="t('panel.variables.newName')"
          :aria-label="t('panel.variables.newName')"
          :aria-invalid="newProblem && newProblem !== 'empty' ? 'true' : undefined"
        />
        <UButton id="variables-new-create" size="xs" icon="i-lucide-plus" :label="t('panel.variables.create')" :disabled="newProblem !== null" @click="createTyped" />
      </PanelForm>
      <p v-if="newProblem && newProblem !== 'empty'" id="variables-new-problem" class="text-xs text-error">{{ t(`panel.variables.nameProblem.${newProblem}`, { name: added.trim() }) }}</p>
      <div v-if="suggested.length > 0" id="variables-suggested" class="flex flex-wrap items-center gap-1.5">
        <span class="text-xs text-muted">{{ t('panel.variables.fromProject') }}</span>
        <UButton
          v-for="name in suggested"
          :key="name"
          :data-suggested="name"
          size="xs"
          color="neutral"
          variant="soft"
          icon="i-lucide-plus"
          class="font-mono"
          :label="name"
          :aria-label="t('panel.variables.createNamed', { name })"
          @click="create(name)"
        />
      </div>
    </section>
  </section>
</template>
