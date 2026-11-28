<script setup lang="ts">
import { shallowRef } from 'vue';
import { Archive, Trash2 } from '../../../shared/ui/icons';
import { VButton, VMessage, VMiddleTruncate, VRowButton, vTooltip } from '../../../shared/ui/kit';
import { t } from '../../../shared/lib/i18n/i18n';
import { archiveAgent, deleteAgent, unarchiveAgent, type SessionNode, StatusIcon, useAction, useSessionDock } from '../../../entities/session';
import SessionDeleteConfirm from './SessionDeleteConfirm.vue';
const props = defineProps<{ session: SessionNode; where?: string | undefined }>();

const dock = useSessionDock();
const confirming = shallowRef(false);
const action = useAction();

async function open() {
  if (await action.run(() => unarchiveAgent(props.session.id))) dock.openWhenListed(props.session.ref);
}

function archive() {
  void action.run(() => archiveAgent(props.session.id));
}

async function confirmDelete() {
  if (await action.run(() => deleteAgent(props.session.id))) confirming.value = false;
}
</script>

<template>
  <div class="ended" data-testid="ended-row" :data-session-id="session.id" :data-busy="action.busy.value">
    <div class="line">
      <VRowButton v-tooltip="where ? `${session.name} · ${where}` : session.name" class="open" :disabled="action.busy.value" data-testid="ended-open" @click="open">
        <template #leading><StatusIcon tone="ended" /></template>
        <span class="title"><VMiddleTruncate :text="session.name" /><span v-if="where" class="where"> · {{ where }}</span></span>
      </VRowButton>
      <template v-if="!confirming">
        <VButton v-tooltip="t('agents.session.archive')" class="icon vr-reveal" text severity="secondary" size="small" :aria-label="t('agents.session.archive')" :disabled="action.busy.value" data-testid="ended-archive" @click="archive">
          <template #icon><Archive :size="14" /></template>
        </VButton>
        <VButton v-tooltip="t('agents.session.deleteForever')" class="icon danger vr-reveal" text severity="secondary" size="small" :aria-label="t('agents.session.deleteForever')" :disabled="action.busy.value" data-testid="ended-delete" @click="confirming = true">
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
  height: max(28px, var(--vr-row-min));
  padding: 0 4px 0 var(--vr-space-6);
  font-size: var(--vr-font-size-label);
}

.open {
  flex: 1;
  color: var(--vr-color-text-muted);
}

.open:hover {
  color: var(--vr-color-text);
}

.title {
  display: flex;
  min-width: 0;
}

.where {
  flex: 0 6 auto;
  min-width: 0;
  overflow: hidden;
  white-space: pre;
  text-overflow: ellipsis;
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-text-subtle);
}

.icon {
  flex: none;
  width: 22px;
  height: 22px;
  opacity: 0;
}

.ended:hover .icon,
.icon:focus-visible {
  opacity: 1;
}

.icon.danger:not(:disabled):hover {
  color: var(--vr-color-danger);
}

.icon:disabled {
  opacity: 0.4;
}

.row-error {
  margin: 0 var(--vr-space-2) var(--vr-space-1) var(--vr-space-5);
}
</style>
