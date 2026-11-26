<script setup lang="ts">
import { computed, nextTick, shallowRef, useTemplateRef } from 'vue';
import { FolderPlus, LoaderCircle } from '../../../shared/ui/icons';
import { VButton, VMenu, VMessage, VTag, vTooltip } from '../../../shared/ui/kit';
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
const menu = useTemplateRef<InstanceType<typeof VMenu>>('menu');
const manual = useTemplateRef<{ openAdd: () => Promise<void> }>('manual');
const { error, busy, run, clear } = useAction();

const fallbackWorkspace = computed(() => berth?.tree.value[0]?.id);
const searching = computed(() => mode.value === 'searching' || busy.value);

async function add(clone: Clone) {
  const ok = await run(() => addProject(clone.workspaceId, { path: clone.path, name: null, createMissing: false }));
  mode.value = ok ? 'idle' : 'picking';
}

async function start() {
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
const menuItems = computed(() => [{ label: t('workbench.addToBerth'), testid: 'repo-menu-add', command: () => void start() }]);
</script>

<template>
  <div v-if="berth" class="repo-actions">
    <VButton v-tooltip="t('workbench.addToBerth')" size="small" text severity="secondary" :aria-label="t('workbench.addToBerth')" :disabled="searching" data-testid="repo-add" @click="start">
      <template #icon>
        <LoaderCircle v-if="searching" :size="14" class="spin" />
        <FolderPlus v-else :size="14" />
      </template>
    </VButton>
    <VButton label="⋯" size="small" text severity="secondary" :aria-label="t('workbench.repoActions')" aria-haspopup="menu" data-testid="repo-menu" @click="menu?.toggle($event)" />
    <VMenu ref="menu" :model="menuItems" :label="t('workbench.repoActions')" />
    <div v-if="mode === 'picking'" class="inline" role="group" :aria-label="t('workbench.pickClone')" data-testid="repo-clones">
      <span class="hint">{{ t('workbench.pickClone') }}</span>
      <UiButton v-for="clone in clones" :key="clone.path" v-tooltip="clone.path" class="pick" type="button" :disabled="busy" data-testid="repo-clone" @click="add(clone)">
        <b>{{ clone.name }}</b>
        <span class="muted">{{ folder(clone.path) }}</span>
        <VTag v-if="clone.main" :value="t('workbench.mainCheckout')" severity="secondary" rounded />
      </UiButton>
      <VButton :label="t('common.cancel')" size="small" text severity="secondary" @click="cancel" />
    </div>
    <VMessage v-if="error" class="error" severity="error" size="small" data-testid="repo-error">{{ error }}</VMessage>
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

.spin {
  animation: spin 1s linear infinite;
}

@keyframes spin {
  to {
    transform: rotate(360deg);
  }
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

.error {
  flex: 1 0 100%;
  margin: 0;
}
</style>
