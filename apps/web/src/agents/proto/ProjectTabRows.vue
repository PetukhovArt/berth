<script setup lang="ts">
import { computed, onBeforeUnmount, shallowRef, watch } from 'vue';
import type { DockviewApi, IDockviewPanel } from 'dockview-vue';
import { t } from '../../i18n';
import { useDock } from '../../workbench/dock';
import { useBerth } from '../berth';
import { placeExternal, useExternalSessions } from '../external';
import DockTab from './DockTab.vue';

const props = defineProps<{ api: DockviewApi | null }>();

const berth = useBerth();
const external = useExternalSessions();
const dock = useDock();
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

function projectOf(panel: IDockviewPanel): string | null {
  const path = dock.paths.value.get(panel.id);
  if (path) return path.project.name;
  const entities = berth?.entities.value;
  const conversation = external?.anyByKey.value.get(panel.id);
  if (conversation && entities) {
    const placement = placeExternal(conversation.cwd, entities);
    if (placement.kind !== 'loose') return entities.projects.get(placement.projectId)?.name ?? null;
  }
  const project = panel.params?.project;
  return typeof project === 'string' ? (project.split('/').pop() ?? null) : null;
}

const rows = computed(() => {
  const byProject = new Map<string | null, IDockviewPanel[]>();
  for (const panel of panels.value) {
    const project = projectOf(panel);
    byProject.set(project, [...(byProject.get(project) ?? []), panel]);
  }
  return [...byProject.entries()]
    .sort(([a], [b]) => Number(a === null) - Number(b === null) || (a ?? '').localeCompare(b ?? ''))
    .map(([project, items]) => ({ key: project ?? '', label: project ?? t('workbench.notInProject'), items, current: items.some((panel) => panel.id === activeId.value) }));
});
</script>

<template>
  <div class="rows" data-testid="project-tab-rows">
    <div v-for="row in rows" :key="row.key" class="row" :data-current="row.current" data-testid="project-tab-row">
      <span class="project" :title="row.label">{{ row.label }}</span>
      <div class="items">
        <div v-for="panel in row.items" :key="panel.id" class="item" :data-active="panel.id === activeId" @click="panel.api.setActive()">
          <DockTab :panel="panel.api" :active="panel.id === activeId" />
        </div>
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

.item:hover {
  color: var(--vr-color-text);
  background: var(--vr-color-nav-hover);
}

.item[data-active='true'] {
  color: var(--vr-color-text);
  background: var(--vr-color-surface);
  box-shadow: inset 0 -2px 0 var(--vr-color-accent);
}
</style>
