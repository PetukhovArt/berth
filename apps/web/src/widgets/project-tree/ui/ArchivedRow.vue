<script setup lang="ts">
import { shallowRef } from 'vue';
import { ArchiveRestore, Trash2 } from '../../../shared/ui/icons';
import { VButton, VMessage, vTooltip } from '../../../shared/ui/kit';
import { t } from '../../../shared/lib/i18n/i18n';
import { deleteAgent, unarchiveAgent, type SessionNode, useAction } from '../../../entities/session';
import SessionDeleteConfirm from './SessionDeleteConfirm.vue';
import { UiButton } from '../../../shared/ui/UiButton';
const props = defineProps<{ session: SessionNode; where?: string | undefined }>();

const confirming = shallowRef(false);
const action = useAction();

function restore() {
  void action.run(() => unarchiveAgent(props.session.id));
}

async function confirmDelete() {
  if (await action.run(() => deleteAgent(props.session.id))) confirming.value = false;
}
</script>

<template>
  <div class="archived" data-testid="archived-row" :data-session-id="session.id" :data-busy="action.busy.value">
    <div class="line">
      <span v-tooltip="where ? `${session.name} · ${where}` : session.name" class="name">{{ session.name }}<span v-if="where" class="where"> · {{ where }}</span></span>
      <template v-if="!confirming">
        <VButton v-tooltip="t('agents.session.restore')" class="icon vr-reveal" text severity="secondary" size="small" :aria-label="t('agents.session.restore')" :disabled="action.busy.value" data-testid="archived-restore" @click="restore">
          <template #icon><ArchiveRestore :size="14" /></template>
        </VButton>
        <VButton v-tooltip="t('agents.session.deleteForever')" class="icon danger vr-reveal" text severity="secondary" size="small" :aria-label="t('agents.session.deleteForever')" :disabled="action.busy.value" data-testid="archived-delete" @click="confirming = true">
          <template #icon><Trash2 :size="14" /></template>
        </VButton>
      </template>
    </div>
    <SessionDeleteConfirm v-if="confirming" :name="session.name" :busy="action.busy.value" @confirm="confirmDelete" @cancel="confirming = false" />
    <VMessage v-if="action.error.value" class="row-error" severity="error" size="small" data-testid="session-error">{{ action.error.value }}</VMessage>
  </div>
</template>

<style scoped>
.line {
  display: flex;
  align-items: center;
  gap: var(--vr-space-1);
  height: 28px;
  padding: 0 4px 0 var(--vr-space-6);
  font-size: var(--vr-font-size-label);
}

.name {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
  color: var(--vr-color-text-muted);
}

.where {
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-text-subtle);
}

.icon {
  flex: none;
  width: 22px;
  height: 22px;
  opacity: 0;
}

.archived:hover .icon,
.icon:focus-visible {
  opacity: 1;
}

.icon.danger:not(:disabled):hover {
  color: var(--vr-color-danger);
}

.icon:disabled {
  opacity: 0.4;
}

.archived[data-busy='true'] .name {
  opacity: 0.6;
}

.row-error {
  margin: 0 var(--vr-space-2) var(--vr-space-1) var(--vr-space-5);
}
</style>
