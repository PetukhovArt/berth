<script setup lang="ts">
import { computed, shallowRef, useTemplateRef } from 'vue';
import { Check, ChevronDown } from '../../../shared/ui/icons';
import { VPopover } from '../../../shared/ui/kit';
import { UiButton } from '../../../shared/ui/UiButton';
import { compareText, t } from '../../../shared/lib/i18n/i18n';
import { describeError, openWorkspace, useBerth, useOpenWorkspace } from '../../../entities/session';

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

async function choose(id: string | null) {
  error.value = null;
  try {
    await openWorkspace(id);
    popover.value?.hide();
  } catch (failure) {
    error.value = describeError(failure);
  }
}
</script>

<template>
  <template v-if="berth && workspaces.length">
    <UiButton
      type="button"
      class="switcher"
      :title="t('titleBar.workspace.label', { name })"
      :aria-label="t('titleBar.workspace.label', { name })"
      aria-haspopup="menu"
      data-testid="titlebar-workspace"
      @click="popover?.toggle($event)"
    >
      <span class="name">{{ name }}</span>
      <ChevronDown :size="12" :stroke-width="1.8" />
    </UiButton>
    <VPopover ref="popover">
      <div class="menu" role="menu" :aria-label="t('titleBar.workspace.menu')" data-testid="workspace-switcher-menu">
        <UiButton v-if="workspaces.length > 1" class="item" type="button" role="menuitemradio" :aria-checked="open === null" data-testid="workspace-switch-all" @click="choose(null)">
          <span class="label">{{ t('titleBar.workspace.all') }}</span>
          <Check v-if="open === null" :size="13" />
        </UiButton>
        <UiButton
          v-for="workspace in workspaces"
          :key="workspace.id"
          class="item"
          type="button"
          role="menuitemradio"
          :aria-checked="open === workspace.id || (open === null && workspaces.length === 1)"
          :data-testid="`workspace-switch-${workspace.id}`"
          @click="choose(workspace.id)"
        >
          <span class="label">{{ workspace.name }}</span>
          <Check v-if="open === workspace.id || (open === null && workspaces.length === 1)" :size="13" />
        </UiButton>
        <p v-if="error" class="error" role="alert" data-testid="workspace-switch-error">{{ error }}</p>
      </div>
    </VPopover>
  </template>
</template>

<style scoped>
.switcher {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  min-width: 0;
  max-width: 180px;
  height: 26px;
  padding: 0 6px;
  font: inherit;
  font-size: var(--vr-font-size-label);
  color: var(--vr-color-text);
  background: none;
  border: 0;
  border-radius: var(--vr-radius-md);
  cursor: pointer;
}

.switcher:hover,
.switcher:focus-visible {
  background: var(--vr-color-nav-hover);
}

.name {
  min-width: 0;
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
}

.menu {
  display: flex;
  flex-direction: column;
  min-width: 180px;
  max-width: 280px;
}

.item {
  display: flex;
  align-items: center;
  gap: var(--vr-space-2);
  padding: var(--vr-space-2) var(--vr-space-3);
  font: inherit;
  text-align: left;
  color: var(--vr-color-text);
  background: none;
  border: 0;
  border-radius: var(--vr-radius-sm);
  cursor: pointer;
}

.item:hover,
.item:focus-visible {
  background: var(--vr-color-nav-hover);
}

.label {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
}

.error {
  margin: var(--vr-space-1) var(--vr-space-3);
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-danger);
}
</style>
