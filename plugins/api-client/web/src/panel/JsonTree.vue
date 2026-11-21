<script setup lang="ts">
// The answer's JSON as a tree (RB-096-01): containers open and close by a click, a search shows
// the matching keys and values and the way to them, a JSONPath (`$.items[*].id`) shows what it
// selects instead. At most a few thousand rows; the source view shows the whole answer.
// An own list, not `UTree` (docs/design.md 9): `UTree` builds every node of the answer up front,
// this one only the rows shown, capped, so a large answer stays fluid.
import { computed, ref } from 'vue';
import { usePanel } from '../../../../web-common/panel-ui/src/mount';
import CodeBlock from '../../../../web-common/panel-ui/src/code/CodeBlock.vue';
import { select } from './jsonpath';
import { rows } from './tree';

const props = defineProps<{ value: unknown }>();
const t = usePanel().t;

const open = ref(new Set<string>());
const search = ref('');
const path = ref('');

const selected = computed(() => (path.value.trim() === '' ? null : select(props.value, path.value)));
const shown = computed(() => rows(props.value, open.value, search.value));

function toggle(at: string): void {
  const next = new Set(open.value);
  if (next.has(at)) next.delete(at);
  else next.add(at);
  open.value = next;
}
</script>

<template>
  <div id="tree" class="flex flex-col gap-2">
    <div class="flex flex-wrap gap-1.5">
      <UInput id="tree-search" v-model="search" type="search" icon="i-lucide-search" size="sm" class="min-w-0 flex-1" :placeholder="t('panel.tree.search')" :aria-label="t('panel.tree.search')" />
      <UInput id="tree-path" v-model="path" size="sm" icon="i-lucide-filter" class="min-w-0 flex-1 font-mono" spellcheck="false" :placeholder="t('panel.tree.path')" :aria-label="t('panel.tree.path')" />
    </div>
    <template v-if="selected !== null">
      <p id="path-count" class="text-xs text-muted">{{ t('panel.tree.selected', { count: selected.length }) }}</p>
      <CodeBlock :text="JSON.stringify(selected, null, 2)" lang="json" />
    </template>
    <p v-else-if="path.trim() !== ''" id="path-invalid" class="text-xs text-error">{{ t('panel.tree.invalidPath') }}</p>
    <ul v-else id="tree-rows" role="tree" class="font-mono text-xs">
      <li v-for="row in shown.rows" :key="row.path" role="treeitem" :aria-expanded="row.expandable ? row.open : undefined" :data-path="row.path" :style="{ paddingLeft: `${row.depth * 0.75}rem` }" class="flex items-center gap-1">
        <UButton
          v-if="row.expandable"
          size="xs"
          color="neutral"
          variant="ghost"
          :icon="row.open ? 'i-lucide-chevron-down' : 'i-lucide-chevron-right'"
          :aria-label="row.open ? t('panel.tree.close') : t('panel.tree.open')"
          @click="toggle(row.path)"
        />
        <span v-else class="inline-block w-6" />
        <span :class="row.match ? 'bg-warning/20 text-highlighted' : 'text-highlighted'">{{ row.label }}</span>
        <span v-if="row.expandable" class="text-muted">{{ row.kind === 'array' ? t('panel.tree.items', { count: row.size }) : t('panel.tree.keys', { count: row.size }) }}</span>
        <span v-else :data-kind="row.kind" :class="row.kind === 'string' ? 'text-success' : 'text-info'">{{ row.text }}</span>
      </li>
    </ul>
    <p v-if="shown.cut && selected === null" id="tree-cut" class="text-xs text-muted">{{ t('panel.tree.cut') }}</p>
  </div>
</template>
