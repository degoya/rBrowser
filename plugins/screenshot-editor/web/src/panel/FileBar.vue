<script setup lang="ts">
// The image to open (RB-080-01, on Nuxt UI since RB-080-86): the PNG files below
// `.rbrowser/out/` in a select grouped by folder (a screenshot run's breakpoints stand together;
// typing a name's start jumps to it), Open, and Refresh list; a note when there are none yet.
// USelect, not USelectMenu: its search would add 51 KB to a bundle near its budget.
import { computed } from 'vue';
import IconAction from '../../../../web-common/panel-ui/src/components/IconAction.vue';
import { usePanel } from '../../../../web-common/panel-ui/src/mount';
import { fileItems, shortName } from './files';
import type { Session } from './session';

const props = defineProps<{ session: Session }>();
const panel = usePanel();
const t = panel.t;
const state = props.session.state;
const items = computed(() => fileItems(state.files));
</script>

<template>
  <div class="flex flex-col gap-2">
    <div class="flex min-w-0 flex-wrap items-center gap-2" role="group" :aria-label="t('panel.group_file')">
      <label id="file-label" class="text-sm" for="file">{{ t('panel.file') }}</label>
      <USelect
        id="file"
        v-model="state.file"
        :items="items"
        value-key="value"
        size="sm"
        class="min-w-0 max-w-96 flex-1"
        aria-labelledby="file-label"
        :disabled="state.files.length === 0"
      >
        <span class="truncate" :title="state.file ? shortName(state.file) : undefined">{{ state.file ? shortName(state.file) : '' }}</span>
      </USelect>
      <UButton id="open" icon="i-lucide-folder-open" :label="t('panel.open')" color="neutral" variant="outline" size="sm" :disabled="state.listed && state.files.length === 0" @click="session.open(state.file ?? null)" />
      <IconAction id="refresh" icon="i-lucide-refresh-cw" :label="t('panel.refresh')" @click="session.refreshList()" />
    </div>
    <UAlert v-if="state.listed && state.files.length === 0" id="no-images" color="neutral" variant="subtle" icon="i-lucide-info" :title="t('panel.no_images')" :ui="{ root: 'px-3 py-1.5', title: 'text-sm font-normal' }" />
  </div>
</template>
