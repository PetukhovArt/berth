<script setup lang="ts">
import { computed, nextTick, shallowRef, useTemplateRef } from 'vue';
import { FolderPlus, LoaderCircle } from '../../../shared/ui/icons';
import { VButton, VPopover } from '../../../shared/ui/kit';
import ProjectActions from './ProjectActions.vue';
import { addProject, useBerth, useAction } from '../../../entities/session';
import { t } from '../../../shared/lib/i18n/i18n';
import { findClones, type Clone, type RepoRef } from '../../../entities/repo';

import { UiButton } from '../../../shared/ui/UiButton';
const props = defineProps<{ repo: RepoRef }>();

type Mode = 'idle' | 'searching' | 'picking' | 'manual';

const berth = useBerth();
const mode = shallowRef<Mode>('idle');
const clones = shallowRef<Clone[]>([]);
const popover = useTemplateRef<InstanceType<typeof VPopover>>('popover');
const manual = useTemplateRef<{ openAdd: () => Promise<void> }>('manual');
const { error, busy, run, clear } = useAction();

const fallbackWorkspace = computed(() => berth?.tree.value[0]?.id);
const searching = computed(() => mode.value === 'searching' || busy.value);

async function add(clone: Clone) {
  const ok = await run(() => addProject(clone.workspaceId, { path: clone.path, name: null, createMissing: false }));
  mode.value = ok ? 'idle' : 'picking';
}

async function start() {
  popover.value?.hide();
  if (!berth || searching.value) return;
  clear();
  mode.value = 'searching';
  const projects = [...berth.entities.value.projects.values()].map((project) => ({ repoPath: project.repo_path, workspaceId: project.workspace_id }));
  const found = await findClones(props.repo, projects);
  clones.value = found;
  if (found.length === 1 && found[0]) {
    await add(found[0]);
    return;
  }
  if (found.length) {
    mode.value = 'picking';
    return;
  }
  mode.value = 'manual';
  await nextTick();
  await manual.value?.openAdd();
}

function cancel() {
  clear();
  mode.value = 'idle';
}

const folder = (path: string) => path.slice(0, path.lastIndexOf('/'));
</script>

<template>
  <div v-if="berth" class="repo-actions">
    <UiButton class="icon" type="button" :title="t('workbench.addToBerth')" :aria-label="t('workbench.addToBerth')" :disabled="searching" data-testid="repo-add" @click="start">
      <LoaderCircle v-if="searching" :size="14" class="spin" />
      <FolderPlus v-else :size="14" />
    </UiButton>
    <VButton label="⋯" size="small" text severity="secondary" :aria-label="t('workbench.repoActions')" aria-haspopup="menu" data-testid="repo-menu" @click="popover?.toggle($event)" />
    <VPopover ref="popover">
      <div class="menu" role="menu">
        <UiButton class="item" type="button" role="menuitem" data-testid="repo-menu-add" @click="start">{{ t('workbench.addToBerth') }}</UiButton>
      </div>
    </VPopover>
    <div v-if="mode === 'picking'" class="inline" role="group" :aria-label="t('workbench.pickClone')" data-testid="repo-clones">
      <span class="hint">{{ t('workbench.pickClone') }}</span>
      <UiButton v-for="clone in clones" :key="clone.path" class="pick" type="button" :disabled="busy" :title="clone.path" data-testid="repo-clone" @click="add(clone)">
        <b>{{ clone.name }}</b>
        <span class="muted">{{ folder(clone.path) }}</span>
        <span v-if="clone.main" class="chip">{{ t('workbench.mainCheckout') }}</span>
      </UiButton>
      <VButton :label="t('common.cancel')" size="small" text severity="secondary" @click="cancel" />
    </div>
    <p v-if="error" class="error" role="alert" data-testid="repo-error">{{ error }}</p>
    <ProjectActions ref="manual" bare :workspace-id="fallbackWorkspace" :hint="t('workbench.cloneNotFound')" />
  </div>
</template>

<style scoped>
.repo-actions {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  justify-content: flex-end;
  gap: var(--vr-space-1);
}

.repo-actions:has(.inline, .error) {
  flex: 1 0 100%;
}

.icon {
  display: inline-flex;
  flex: none;
  align-items: center;
  justify-content: center;
  width: 22px;
  height: 22px;
  padding: 0;
  color: var(--vr-color-text-muted);
  background: none;
  border: 0;
  border-radius: var(--vr-radius-sm);
}

.icon:hover:not(:disabled) {
  color: var(--vr-color-text);
  background: var(--vr-color-nav-hover);
}

.spin {
  animation: spin 1s linear infinite;
}

@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}

.menu {
  display: flex;
  flex-direction: column;
  min-width: 160px;
}

.item {
  padding: var(--vr-space-2) var(--vr-space-3);
  font: inherit;
  text-align: left;
  color: var(--vr-color-text);
  background: none;
  border: 0;
  border-radius: var(--vr-radius-sm);
}

.item:hover,
.item:focus-visible {
  background: var(--vr-color-nav-hover);
}

.inline {
  display: flex;
  flex: 1 0 100%;
  flex-direction: column;
  align-items: stretch;
  gap: var(--vr-space-1);
}

.hint {
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-text-muted);
}

.pick {
  display: flex;
  align-items: center;
  gap: var(--vr-space-1);
  min-width: 0;
  padding: var(--vr-space-1) var(--vr-space-2);
  font: inherit;
  font-size: var(--vr-font-size-xs);
  text-align: left;
  color: var(--vr-color-text);
  background: var(--vr-color-nav-hover);
  border: 0;
  border-radius: var(--vr-radius-sm);
}

.pick:hover:not(:disabled) {
  background: var(--vr-color-nav-active);
}

.muted {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  color: var(--vr-color-text-muted);
  white-space: nowrap;
  text-overflow: ellipsis;
}

.chip {
  flex: none;
  padding: 0 var(--vr-space-1);
  font-size: var(--vr-font-size-2xs);
  color: var(--vr-color-text-muted);
  background: var(--vr-color-chip-bg);
  border-radius: var(--vr-radius-full);
}

.error {
  flex: 1 0 100%;
  margin: 0;
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-danger);
}
</style>
