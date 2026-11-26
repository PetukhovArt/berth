<script lang="ts">
import type { WorktreeCommits } from '../../../shared/api/fs';

const listings = new Map<string, WorktreeCommits>();
</script>

<script setup lang="ts">
import { computed, onMounted, shallowRef, useTemplateRef } from 'vue';
import { ChevronDown } from '../../../shared/ui/icons';
import { VButton, VListbox, VPopover, vTooltip } from '../../../shared/ui/kit';
import { worktreeCommits, type ChangesScope } from '../../../shared/api/fs';
import { t } from '../../../shared/lib/i18n/i18n';
import { relativeTime } from '../../../entities/merge-request';

const props = defineProps<{ path: string; modelValue: ChangesScope }>();
const emit = defineEmits<{ 'update:modelValue': [scope: ChangesScope] }>();

const popover = useTemplateRef<InstanceType<typeof VPopover>>('popover');
const listing =shallowRef<WorktreeCommits>(listings.get(props.path) ?? { base: null, commits: [] });

const baseLabel = computed(() => t('agents.changes.scope.base', { base: listing.value.base ?? 'main' }));
const chosen = computed(() => {
  const scope = props.modelValue;
  if (scope === 'uncommitted' || scope === 'base') return null;
  const sha = scope.slice('commit:'.length);
  const commit = listing.value.commits.find((entry) => entry.sha.startsWith(sha));
  return { short: commit?.short ?? sha.slice(0, 7), subject: commit?.subject ?? '' };
});
const label = computed(() => (props.modelValue === 'uncommitted' ? t('agents.changes.scope.uncommitted') : baseLabel.value));

async function load() {
  try {
    const next = await worktreeCommits(props.path);
    listings.set(props.path, next);
    listing.value = next;
  } catch {
    return;
  }
}

function open(event: Event) {
  popover.value?.toggle(event);
  void load();
}

onMounted(load);

function pick(scope: ChangesScope) {
  popover.value?.hide();
  if (scope !== props.modelValue) emit('update:modelValue', scope);
}

const baseOptions = computed(() => [
  { value: 'uncommitted', label: t('agents.changes.scope.uncommitted') },
  { value: 'base', label: baseLabel.value },
]);
const commitOptions = computed(() => listing.value.commits.map((commit) => ({ value: `commit:${commit.sha}`, label: commit.subject })));
const commitByValue = computed(() => new Map(listing.value.commits.map((commit) => [`commit:${commit.sha}`, commit])));
const baseModel = computed(() => (props.modelValue === 'uncommitted' || props.modelValue === 'base' ? props.modelValue : null));
const commitModel = computed(() => (props.modelValue.startsWith('commit:') ? props.modelValue : null));
</script>

<template>
  <VButton v-tooltip="t('agents.changes.scope.title')" class="scope" size="small" severity="secondary" aria-haspopup="menu" data-testid="changes-scope" @click="open">
    <template v-if="chosen">
      <code class="scope-sha">{{ chosen.short }}</code>
      <span v-if="chosen.subject" class="scope-label">{{ chosen.subject }}</span>
    </template>
    <span v-else class="scope-label">{{ label }}</span>
    <ChevronDown :size="12" />
  </VButton>
  <VPopover ref="popover">
    <div class="menu" data-testid="changes-scope-menu">
      <VListbox flat :filter="false" :model-value="baseModel" :options="baseOptions" :label="t('agents.changes.scope.title')" test-id="changes-scope" @update:model-value="pick($event as ChangesScope)" />
      <template v-if="listing.commits.length">
        <p class="group">{{ t('agents.changes.scope.commits') }}</p>
        <VListbox flat :model-value="commitModel" :options="commitOptions" :label="t('agents.changes.scope.commits')" test-id="changes-scope-commit" @update:model-value="pick($event as ChangesScope)">
          <template #option="{ option }">
            <span v-tooltip="option.label" class="commit">
              <code>{{ commitByValue.get(option.value)?.short }}</code>
              <span class="subject">{{ option.label }}</span>
              <span class="when">{{ relativeTime(commitByValue.get(option.value)?.date ?? '') }}</span>
            </span>
          </template>
        </VListbox>
      </template>
    </div>
  </VPopover>
</template>

<style scoped>
.scope {
  min-width: 0;
  max-width: 18rem;
}

.scope-sha {
  flex: none;
  color: var(--vr-color-text-muted);
  font-family: var(--vr-font-mono);
}

.scope-label {
  min-width: 0;
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
}

.menu {
  display: flex;
  flex-direction: column;
  min-width: 240px;
  max-width: 360px;
  max-height: 320px;
  overflow: auto;
}

.group {
  margin: var(--vr-space-1) 0 0;
  padding: var(--vr-space-2) var(--vr-space-3) var(--vr-space-1);
  color: var(--vr-color-text-muted);
  font-size: var(--vr-font-size-xs);
  border-top: 1px solid var(--vr-color-border);
}

.commit {
  display: flex;
  align-items: center;
  gap: var(--vr-space-2);
  min-width: 0;
}

.commit code {
  flex: none;
  color: var(--vr-color-text-muted);
  font-family: var(--vr-font-mono);
  font-size: var(--vr-font-size-xs);
}

.subject {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
}

.when {
  flex: none;
  color: var(--vr-color-text-subtle);
  font-size: var(--vr-font-size-xs);
}
</style>
