<script setup lang="ts">
import { computed, onBeforeUnmount, shallowRef, watch } from 'vue';
import type { DockviewApi, IDockviewPanel } from 'dockview-vue';
import { Plus } from 'lucide-vue-next';
import { t } from '../../../shared/lib/i18n/i18n';
import { useSessionDock, useBerth, worktreesOfProject, placeExternal, useExternalSessions } from '../../../entities/session';
import { startCreate } from '../../../features/new-session';
import DockTab from './DockTab.vue';
import { FILE_PANEL_PREFIX, isFilePanel } from '../../../shared/lib/markdownFiles';

const props = defineProps<{ api: DockviewApi | null }>();

const berth = useBerth();
const external = useExternalSessions();
const dock = useSessionDock();
const panels = shallowRef<IDockviewPanel[]>([]);
const activeId = shallowRef<string | undefined>();
let subscriptions: { dispose(): void }[] = [];

function release() {
  for (const subscription of subscriptions) subscription.dispose();
  subscriptions = [];
}

watch(
  () => props.api,
  (api) => {
    release();
    if (!api) return;
    const sync = () => {
      panels.value = [...api.panels];
      activeId.value = api.activePanel?.id;
    };
    subscriptions = [api.onDidAddPanel(sync), api.onDidRemovePanel(sync), api.onDidActivePanelChange(sync), api.onDidLayoutChange(sync)];
    sync();
  },
  { immediate: true },
);

onBeforeUnmount(release);

function projectIdOf(panel: IDockviewPanel): string | undefined {
  const entities = berth?.entities.value;
  if (!entities) return undefined;
  const conversation = external?.anyByKey.value.get(panel.id);
  if (conversation) {
    const placement = placeExternal(conversation.cwd, entities);
    return placement.kind === 'loose' ? undefined : placement.projectId;
  }
  if (isFilePanel(panel.id)) return entities.worktrees.get(dock.worktreeOfFile(panel.id.slice(FILE_PANEL_PREFIX.length))?.id ?? '')?.project_id;
  return undefined;
}

function projectOf(panel: IDockviewPanel): { id?: string; name: string } | null {
  const known = dock.paths.value.get(panel.id)?.project ?? berth?.entities.value.projects.get(projectIdOf(panel) ?? '');
  if (known) return { id: known.id, name: known.name };
  const project = panel.params?.project;
  const name = typeof project === 'string' ? project.split('/').pop() : undefined;
  return name ? { name } : null;
}

const rows = computed(() => {
  const byProject = new Map<string | null, { id?: string; items: IDockviewPanel[] }>();
  for (const panel of panels.value) {
    const project = projectOf(panel);
    const name = project?.name ?? null;
    const row = byProject.get(name) ?? { items: [] };
    row.id ??= project?.id;
    row.items.push(panel);
    byProject.set(name, row);
  }
  const projects = berth?.entities.value.projects;
  const pin = (id?: string) => projects?.get(id ?? '')?.pin_order ?? Infinity;
  return [...byProject.entries()]
    .sort(([a, rowA], [b, rowB]) => Number(a === null) - Number(b === null) || Math.sign(pin(rowA.id) - pin(rowB.id)) || (a ?? '').localeCompare(b ?? ''))
    .map(([project, row]) => ({ key: project ?? '', label: project ?? t('workbench.notInProject'), projectId: row.id, items: row.items, current: row.items.some((panel) => panel.id === activeId.value) }));
});

function create(projectId: string) {
  const entities = berth?.entities.value;
  if (entities) startCreate(worktreesOfProject(entities, projectId)[0]?.id ?? null);
}
</script>

<template>
  <div class="rows" data-testid="project-tab-rows">
    <div v-for="row in rows" :key="row.key" class="row" :data-current="row.current" data-testid="project-tab-row">
      <span class="project" :title="row.label">{{ row.label }}</span>
      <div class="items">
        <div v-for="panel in row.items" :key="panel.id" class="item" :data-active="panel.id === activeId" @click="panel.api.setActive()">
          <DockTab :panel="panel.api" :active="panel.id === activeId" />
        </div>
        <button v-if="row.projectId" class="add" :title="t('agents.project.newSession')" :aria-label="t('agents.project.newSession')" data-testid="project-row-new-session" @click="create(row.projectId)">
          <Plus :size="12" />
        </button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.rows {
  flex: none;
  font-size: var(--vr-font-size-xs);
  background: var(--vr-color-surface-muted);
  border-bottom: 1px solid var(--vr-color-border);
}

.row {
  display: flex;
  align-items: stretch;
  height: 26px;
  min-width: 0;
}

.row + .row {
  border-top: 1px solid var(--vr-color-border);
}

.project {
  display: flex;
  flex: none;
  align-items: center;
  width: 96px;
  padding: 0 var(--vr-space-3);
  overflow: hidden;
  color: var(--vr-color-text-muted);
  white-space: nowrap;
  text-overflow: ellipsis;
}

.row[data-current='true'] .project {
  color: var(--vr-color-text);
  font-weight: 600;
}

.items {
  display: flex;
  flex: 1;
  min-width: 0;
  overflow-x: auto;
  scrollbar-width: none;
}

.item {
  flex: none;
  color: var(--vr-color-text-muted);
  cursor: pointer;
}

.item:hover,
.add:hover {
  color: var(--vr-color-text);
  background: var(--vr-color-nav-hover);
}

.add {
  display: flex;
  flex: none;
  align-items: center;
  padding: 0 var(--vr-space-3);
  color: var(--vr-color-text-muted);
  background: none;
  border: 0;
  cursor: pointer;
}

.item[data-active='true'] {
  color: var(--vr-color-text);
  background: var(--vr-color-surface);
  box-shadow: inset 0 -2px 0 var(--vr-color-accent);
}
</style>
