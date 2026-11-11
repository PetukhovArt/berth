<script setup lang="ts">
import 'dockview-vue/dist/styles/dockview.css';
import { computed, onBeforeUnmount, shallowRef, useSlots } from 'vue';
import { DockviewVue, themeLight, type DockviewApi, type DockviewPanelApi, type DockviewReadyEvent } from 'dockview-vue';
import { t } from '../../../shared/lib/i18n/i18n';
import { useDock } from '../../../shared/model/dock';
import { isDesktop } from '../../../bridge';
import { useSettings } from '../../../shared/config/settings/store';
import { startTabDrag, trackTabOrder } from '../lib/tabOrder';
import DockTab from './DockTab.vue';
import ProjectTabRows from './ProjectTabRows.vue';

defineProps<{ activeKey: string | null }>();
defineSlots<Record<`panel-${string}`, (props: { params: Record<string, unknown>; api: DockviewPanelApi }) => unknown>>();
const emit = defineEmits<{ ready: [event: DockviewReadyEvent] }>();

const dock = useDock();
const slots = useSlots();
const panelSlots = computed(() => Object.keys(slots).filter((name): name is `panel-${string}` => name.startsWith('panel-')));
const { values } = useSettings();
const subscriptions: { dispose(): void }[] = [];
let dockApi: DockviewApi | null = null;
const readyApi = shallowRef<DockviewApi | null>(null);

function onReady(event: DockviewReadyEvent) {
  const api = event.api;
  dockApi = api;
  readyApi.value = api;
  subscriptions.push(...trackTabOrder(api));
  emit('ready', event);
}

function onTabPointerDown(event: PointerEvent, panel: DockviewPanelApi) {
  if (isDesktop) startTabDrag(event, panel, dockApi);
}

onBeforeUnmount(() => {
  for (const subscription of subscriptions) subscription.dispose();
  dockApi = null;
  readyApi.value = null;
});
</script>

<template>
  <div class="dock" :data-tabs="values['workbench.tabs']">
    <ProjectTabRows v-if="values['workbench.tabs'] === 'projects'" :api="readyApi" />
    <DockviewVue class="dockview" :theme="themeLight" :disable-dnd="isDesktop" @ready="onReady">
      <template v-for="name in panelSlots" :key="name" #[name]="scope">
        <slot :name="name" v-bind="scope" />
      </template>
      <template #defaultTab="{ api }">
        <DockTab :panel="api" :active="api.id === activeKey" @dblclick="dock.toggleMaximize(api.id)" @pointerdown="onTabPointerDown($event, api)" />
      </template>
      <template #watermark>
        <p class="watermark">{{ t('workbench.pick') }}</p>
      </template>
    </DockviewVue>
  </div>
</template>

<style scoped>
.dock {
  flex: 1;
  min-width: 0;
  min-height: 0;
}

.dock :deep(.dockview) {
  height: 100%;
}

.dock[data-tabs='projects'] {
  display: flex;
  flex-direction: column;
}

.dock[data-tabs='projects'] :deep(.dockview) {
  flex: 1;
  height: auto;
  min-height: 0;
}

.dock[data-tabs='projects'] :deep(.dv-tabs-and-actions-container) {
  display: none;
}

:root[data-vr-viewport='phone'] .dock :deep(.dv-tabs-and-actions-container) {
  display: none;
}

.dock :deep(.dockview-theme-light) {
  color-scheme: inherit;
  --dv-group-view-background-color: var(--vr-color-surface);
  --dv-tabs-and-actions-container-background-color: var(--vr-color-surface-muted);
  --dv-tabs-and-actions-container-font-size: var(--vr-font-size-xs);
  --dv-activegroup-visiblepanel-tab-background-color: var(--vr-color-surface);
  --dv-activegroup-hiddenpanel-tab-background-color: var(--vr-color-surface-muted);
  --dv-inactivegroup-visiblepanel-tab-background-color: var(--vr-color-surface);
  --dv-inactivegroup-hiddenpanel-tab-background-color: var(--vr-color-surface-muted);
  --dv-activegroup-visiblepanel-tab-color: var(--vr-color-text);
  --dv-activegroup-hiddenpanel-tab-color: var(--vr-color-text-muted);
  --dv-inactivegroup-visiblepanel-tab-color: var(--vr-color-text-muted);
  --dv-inactivegroup-hiddenpanel-tab-color: var(--vr-color-text-muted);
  --dv-tab-divider-color: var(--vr-color-border);
  --dv-separator-border: var(--vr-color-border);
  --dv-paneview-header-border-color: var(--vr-color-border);
  --dv-icon-hover-background-color: var(--vr-color-nav-hover);
  --dv-drag-over-background-color: var(--vr-color-nav-active);
  --dv-scrollbar-background-color: var(--vr-color-nav-active);
  --dv-tabs-container-scrollbar-color: var(--vr-color-nav-active);
}

.dock[data-tabs='scroll'] :deep(.dv-scrollable .dv-scrollbar-horizontal) {
  height: 6px;
  background-color: var(--dv-scrollbar-background-color);
}

.dock[data-tabs='wrap'] :deep(.dv-tabs-and-actions-container) {
  height: auto;
  min-height: var(--dv-tabs-and-actions-container-height);
  align-items: flex-start;
}

.dock[data-tabs='wrap'] :deep(.dv-tabs-container) {
  flex-wrap: wrap;
  height: auto;
  overflow: visible;
  align-content: flex-start;
}

.dock[data-tabs='wrap'] :deep(.dv-tabs-container .dv-tab) {
  height: var(--dv-tabs-and-actions-container-height);
}

.dock[data-tabs='wrap'] :deep(.dv-scrollbar),
.dock[data-tabs='wrap'] :deep(.dv-tabs-overflow-dropdown-root) {
  display: none;
}

.watermark {
  display: flex;
  align-items: center;
  justify-content: center;
  height: 100%;
  margin: 0;
  color: var(--vr-color-text-muted);
}
</style>
