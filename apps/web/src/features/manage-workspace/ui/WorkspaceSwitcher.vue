<script setup lang="ts">
import { computed, shallowRef, useTemplateRef } from 'vue';
import { ChevronDown } from '../../../shared/ui/icons';
import { VButton, VListbox, VMessage, VPopover, vTooltip } from '../../../shared/ui/kit';
import { compareText, t } from '../../../shared/lib/i18n/i18n';
import { describeError, openWorkspace, useBerth, useOpenWorkspace } from '../../../entities/session';
import WorkspaceActions from './WorkspaceActions.vue';

const ALL = 'all';

const berth = useBerth();
const open = useOpenWorkspace();
const popover = useTemplateRef<InstanceType<typeof VPopover>>('popover');
const error = shallowRef<string | null>(null);

const workspaces = computed(() => [...(berth?.entities.value.workspaces.values() ?? [])].sort((a, b) => compareText(a.name, b.name)));
const name = computed(() => {
  const id = open.value;
  if (id) return berth?.entities.value.workspaces.get(id)?.name ?? '';
  return workspaces.value.length === 1 ? (workspaces.value[0]?.name ?? '') : t('titleBar.workspace.all');
});

const options = computed(() => [
  ...(workspaces.value.length > 1 ? [{ value: ALL, label: t('titleBar.workspace.all') }] : []),
  ...workspaces.value.map((workspace) => ({ value: workspace.id, label: workspace.name })),
]);
const current = computed(() => open.value ?? (workspaces.value.length === 1 ? (workspaces.value[0]?.id ?? ALL) : ALL));

async function choose(value: string | string[] | null | undefined) {
  if (typeof value !== 'string') return;
  error.value = null;
  try {
    await openWorkspace(value === ALL ? null : value);
    popover.value?.hide();
  } catch (failure) {
    error.value = describeError(failure);
  }
}
</script>

<template>
  <template v-if="berth && workspaces.length">
    <VButton
      v-tooltip="t('titleBar.workspace.label', { name })"
      class="switcher"
      size="small"
      text
      severity="secondary"
      icon-pos="right"
      :label="name"
      :aria-label="t('titleBar.workspace.label', { name })"
      aria-haspopup="menu"
      data-testid="titlebar-workspace"
      @click="popover?.toggle($event)"
    >
      <template #icon><ChevronDown :size="12" :stroke-width="1.8" /></template>
    </VButton>
    <VPopover ref="popover">
      <div class="menu" data-testid="workspace-switcher-menu">
        <VListbox flat :model-value="current" :options="options" :label="t('titleBar.workspace.menu')" :filter="false" test-id="workspace-switch" @update:model-value="choose" />
        <VMessage v-if="error" severity="error" size="small" data-testid="workspace-switch-error">{{ error }}</VMessage>
        <div class="create"><WorkspaceActions /></div>
      </div>
    </VPopover>
  </template>
</template>

<style scoped>
.switcher {
  max-width: 180px;
}

.menu {
  display: flex;
  flex-direction: column;
  min-width: 180px;
  max-width: 280px;
}

.create {
  margin-top: var(--vr-space-1);
  padding-top: var(--vr-space-1);
  border-top: 1px solid var(--vr-color-border);
}

.create :deep(.actions) {
  justify-content: flex-start;
}
</style>
