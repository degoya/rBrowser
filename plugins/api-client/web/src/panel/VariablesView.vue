<script setup lang="ts">
// The Variables tab (RB-096-11): .rbrowser/api/http-client.env.json as tables - `$shared` for
// every environment, then each environment -, a row a name and its value. A row marked private
// goes to the private values in config.local.json (Settings > Network > API client > Private
// values), never into the checked-in file; one already kept shows no value, and typing one
// replaces it. Environments are added (the project's rBrowser environments offered first),
// renamed and removed here; nothing is written until Save (`saveVariables`), which the core
// refuses when the file changed since it was read.
import { computed, ref, watch } from 'vue';
import { usePanel } from '../../../../web-common/panel-ui/src/mount';
import IconAction from '../../../../web-common/panel-ui/src/components/IconAction.vue';
import SettingsText from './SettingsText.vue';
import { type SaveOp, SHARED, type VarBlock, type VarRow, type VariablesAnswer, blocksOf, saveOf } from './variables';

const props = defineProps<{ answer: VariablesAnswer | null; problem: string | null; known: string[] }>();
const emit = defineEmits<{ save: [op: SaveOp]; reload: [] }>();
const t = usePanel().t;

const blocks = ref<VarBlock[]>([]);
const dirty = ref(false);
const added = ref('');
watch(
  () => props.answer,
  (answer) => {
    blocks.value = answer ? blocksOf(answer) : [];
    dirty.value = false;
  },
  { immediate: true },
);

/** "Another name" as a select item: an item's value cannot be empty. */
const OTHER = '/';
const offered = computed(() => [
  ...props.known.filter((name) => !blocks.value.some((block) => block.name === name)).map((name) => ({ label: name, value: name })),
  { label: t('panel.variables.otherName'), value: OTHER },
]);
const choice = ref(OTHER);
const newName = computed(() => (choice.value === OTHER ? added.value.trim() : choice.value));
const nameTaken = computed(() => blocks.value.some((block) => block.name === newName.value));

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

function addBlock(): void {
  if (!newName.value || nameTaken.value) return;
  blocks.value = [...blocks.value, { name: newName.value, original: null, rows: [] }];
  added.value = '';
  choice.value = OTHER;
  changed();
}

function renameBlock(block: VarBlock, name: string): void {
  block.name = name;
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
    <p class="text-xs text-muted">
      <SettingsText text="panel.variables.description" setting-key="api-client.secrets" />
    </p>
    <p v-if="problem" id="variables-problem" role="alert" class="text-xs text-error">{{ problem }}</p>
    <div class="flex flex-wrap gap-1.5">
      <UButton id="variables-save" size="sm" icon="i-lucide-save" :label="t('panel.save')" :disabled="!dirty || !answer" @click="save" />
      <UButton id="variables-reload" size="sm" color="neutral" variant="outline" icon="i-lucide-rotate-ccw" :label="t('panel.variables.reload')" @click="emit('reload')" />
    </div>
    <section v-for="(block, at) in blocks" :key="at" :data-block="block.name" class="flex flex-col gap-1.5" :aria-label="block.name">
      <div class="flex items-center gap-1.5">
        <UIcon :name="block.name === SHARED ? 'i-lucide-layers' : 'i-lucide-globe'" class="size-4 text-muted" />
        <p v-if="block.name === SHARED" class="flex-1 text-sm font-medium">{{ t('panel.variables.shared') }}</p>
        <UInput
          v-else
          :model-value="block.name"
          size="xs"
          class="min-w-0 flex-1 font-mono"
          :aria-label="t('panel.variables.environmentName')"
          @update:model-value="(name) => renameBlock(block, String(name))"
        />
        <IconAction v-if="block.name !== SHARED" icon="i-lucide-trash" :label="t('panel.variables.removeEnvironment', { name: block.name })" @click="removeBlock(block)" />
      </div>
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
    <form id="variables-add" class="flex flex-wrap items-center gap-1.5" @submit.prevent="addBlock">
      <USelect id="variables-add-choice" v-model="choice" :items="offered" value-key="value" size="xs" class="w-40" :aria-label="t('panel.variables.addEnvironment')" />
      <UInput v-if="choice === OTHER" id="variables-add-name" v-model="added" size="xs" class="w-32 font-mono" :aria-label="t('panel.variables.environmentName')" :placeholder="t('panel.variables.environmentName')" />
      <UButton id="variables-add-run" type="submit" size="xs" color="neutral" variant="outline" icon="i-lucide-plus" :label="t('panel.variables.addEnvironment')" :disabled="!newName || nameTaken" />
    </form>
  </section>
</template>
