<script setup lang="ts">
import { computed, nextTick, shallowRef, useTemplateRef } from 'vue';
import { VButton, VInputText } from '../../../shared/ui/kit';
import { t } from '../../../shared/lib/i18n/i18n';
import { useBerth, useAction, addWorkspace, removeWorkspace, renameWorkspace } from '../../../entities/session';
import ActionMenu from '../../../shared/ui/ActionMenu.vue';
import ConfirmPopover from '../../../shared/ui/ConfirmPopover.vue';

const props = defineProps<{ workspaceId?: string }>();

type Mode = 'idle' | 'creating' | 'renaming';

const berth = useBerth();
const mode = shallowRef<Mode>('idle');
const draft = shallowRef('');
const { error, busy, run, clear } = useAction();
const input = useTemplateRef<{ $el: HTMLInputElement }>('input');
const root = useTemplateRef<HTMLElement>('root');
const confirm = useTemplateRef<InstanceType<typeof ConfirmPopover>>('confirm');

const current = computed(() => (props.workspaceId ? berth?.entities.value.workspaces.get(props.workspaceId) : undefined));
const name = computed(() => draft.value.trim());
const editing = computed(() => mode.value === 'creating' || mode.value === 'renaming');

async function open(next: Mode, value: string) {
  clear();
  draft.value = value;
  mode.value = next;
  await nextTick();
  input.value?.$el.focus();
}

function cancel() {
  clear();
  mode.value = 'idle';
}

async function submit() {
  const id = props.workspaceId;
  if (!name.value || busy.value) return;
  const ok = await run(() => (mode.value === 'renaming' && id ? renameWorkspace(id, name.value) : addWorkspace(name.value)));
  if (ok) mode.value = 'idle';
}

async function confirmRemove() {
  const id = props.workspaceId;
  if (!id) return;
  if (await run(() => removeWorkspace(id))) confirm.value?.hide();
}

function askRemove() {
  clear();
  setTimeout(() => confirm.value?.show(root.value), 0);
}
</script>

<template>
  <div v-if="berth" ref="root" class="actions">
    <ActionMenu
      v-if="workspaceId"
      testid="workspace-menu"
      :label="t('agents.workspace.actions')"
      :remove-label="t('common.delete')"
      @rename="open('renaming', current?.name ?? '')"
      @remove="askRemove"
    />
    <VButton v-else :label="t('agents.workspace.new')" size="small" text severity="secondary" data-testid="workspace-new" @click="open('creating', '')" />
    <form v-if="editing" class="inline" @submit.prevent="submit">
      <VInputText ref="input" v-model="draft" size="small" :placeholder="t('agents.workspace.name')" :aria-label="t('agents.workspace.nameAria')" data-testid="workspace-name-input" @keydown.esc="cancel" />
      <VButton type="submit" :label="t('common.save')" size="small" :disabled="!name" :loading="busy" data-testid="workspace-submit" />
      <VButton :label="t('common.cancel')" size="small" text severity="secondary" @click="cancel" />
    </form>
    <ConfirmPopover ref="confirm" :question="t('agents.workspace.removeQuestion', { name: current?.name ?? '' })" :busy="busy" testid="workspace-remove-confirm" @confirm="confirmRemove" />
    <p v-if="error" class="error" role="alert" data-testid="workspace-error">{{ error }}</p>
  </div>
</template>

<style scoped>
.actions {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  justify-content: flex-end;
  gap: var(--vr-space-1);
}

.actions:has(.inline) {
  flex: 1 0 100%;
}

.inline {
  display: flex;
  flex: 1 0 100%;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--vr-space-1);
}

.inline :deep(input) {
  flex: 1;
  min-width: 0;
}

.error {
  flex: 1 0 100%;
  margin: 0;
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-danger);
}
</style>
