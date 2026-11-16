<script lang="ts">
import type { WorktreeCommits } from '../../../shared/api/fs';

const listings = new Map<string, WorktreeCommits>();
</script>

<script setup lang="ts">
import { computed, onMounted, shallowRef, useTemplateRef } from 'vue';
import { ChevronDown } from '../../../shared/ui/icons';
import { VPopover } from '../../../shared/ui/kit';
import { worktreeCommits, type ChangesScope } from '../../../shared/api/fs';
import { t } from '../../../shared/lib/i18n/i18n';
import { relativeTime } from '../../../entities/merge-request';

import { UiButton } from '../../../shared/ui/UiButton';
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
</script>

<template>
  <UiButton class="scope" type="button" aria-haspopup="menu" :title="t('agents.changes.scope.title')" data-testid="changes-scope" @click="open">
    <template v-if="chosen">
      <code class="scope-sha">{{ chosen.short }}</code>
      <span v-if="chosen.subject" class="scope-label">{{ chosen.subject }}</span>
    </template>
    <span v-else class="scope-label">{{ label }}</span>
    <ChevronDown :size="12" />
  </UiButton>
  <VPopover ref="popover">
    <div class="menu" role="menu" data-testid="changes-scope-menu">
      <UiButton class="item" type="button" role="menuitemradio" :aria-checked="modelValue === 'uncommitted'" data-testid="changes-scope-uncommitted" @click="pick('uncommitted')">{{ t('agents.changes.scope.uncommitted') }}</UiButton>
      <UiButton class="item" type="button" role="menuitemradio" :aria-checked="modelValue === 'base'" data-testid="changes-scope-base" @click="pick('base')">{{ baseLabel }}</UiButton>
      <template v-if="listing.commits.length">
        <p class="group">{{ t('agents.changes.scope.commits') }}</p>
        <UiButton v-for="commit in listing.commits" :key="commit.sha" class="item commit" type="button" role="menuitemradio" :aria-checked="modelValue === `commit:${commit.sha}`" :title="commit.subject" data-testid="changes-scope-commit" @click="pick(`commit:${commit.sha}`)">
          <code>{{ commit.short }}</code>
          <span class="subject">{{ commit.subject }}</span>
          <span class="when">{{ relativeTime(commit.date) }}</span>
        </UiButton>
      </template>
    </div>
  </VPopover>
</template>

<style scoped>
.scope {
  display: inline-flex;
  align-items: center;
  gap: var(--vr-space-1);
  min-width: 0;
  max-width: 18rem;
  padding: 2px var(--vr-space-2);
  color: var(--vr-color-text);
  font: inherit;
  font-size: var(--vr-font-size-xs);
  background: var(--vr-color-surface-muted);
  border: 1px solid var(--vr-color-border);
  border-radius: var(--vr-radius-sm);
  cursor: pointer;
}

.scope:hover {
  background: var(--vr-color-nav-hover);
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

.item {
  display: flex;
  align-items: center;
  gap: var(--vr-space-2);
  padding: var(--vr-space-2) var(--vr-space-3);
  color: var(--vr-color-text);
  font: inherit;
  text-align: left;
  background: none;
  border: 0;
  border-radius: var(--vr-radius-sm);
  cursor: pointer;
}

.item:hover,
.item:focus-visible {
  background: var(--vr-color-nav-hover);
}

.item[aria-checked='true'] {
  background: var(--vr-color-nav-active);
}

.group {
  margin: var(--vr-space-1) 0 0;
  padding: var(--vr-space-2) var(--vr-space-3) var(--vr-space-1);
  color: var(--vr-color-text-muted);
  font-size: var(--vr-font-size-xs);
  border-top: 1px solid var(--vr-color-border);
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
