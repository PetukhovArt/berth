<script setup lang="ts">
import { ArchiveRestore } from '../../../shared/ui/icons';
import { VButton, VMessage, vTooltip } from '../../../shared/ui/kit';
import { t } from '../../../shared/lib/i18n/i18n';
import { primaryPressed } from '../../../shared/lib/platform';
import { unarchiveExternal, type ExternalSession, HarnessIcon, useAction } from '../../../entities/session';

import { UiButton } from '../../../shared/ui/UiButton';
const props = defineProps<{ session: ExternalSession }>();
const emit = defineEmits<{ open: [split: boolean] }>();

const action = useAction();

function restore() {
  void action.run(() => unarchiveExternal(props.session));
}
</script>

<template>
  <div class="archived" data-testid="archived-external-row" :data-session-id="session.id" :data-harness="session.harness" :data-busy="action.busy.value">
    <div class="line">
      <UiButton v-tooltip="session.cwd" class="open" data-testid="archived-external-open" @click="emit('open', primaryPressed($event))">
        <HarnessIcon :harness="session.harness" />
        <span class="name">{{ session.title }}</span>
      </UiButton>
      <VButton v-tooltip="t('agents.session.restore')" class="icon vr-reveal" text severity="secondary" size="small" :aria-label="t('agents.session.restore')" :disabled="action.busy.value" data-testid="archived-external-restore" @click="restore">
        <template #icon><ArchiveRestore :size="14" /></template>
      </VButton>
    </div>
    <VMessage v-if="action.error.value" class="row-error" severity="error" size="small" data-testid="archived-external-error">{{ action.error.value }}</VMessage>
  </div>
</template>

<style scoped>
.line {
  display: flex;
  align-items: center;
  gap: var(--vr-space-1);
  height: 28px;
  padding: 0 4px 0 var(--vr-space-5);
  font-size: var(--vr-font-size-label);
}

.open {
  display: flex;
  flex: 1;
  align-items: center;
  gap: var(--vr-space-2);
  min-width: 0;
  height: var(--vr-size-row);
  padding: 0 var(--vr-space-1);
  font: inherit;
  text-align: left;
  color: var(--vr-color-text-muted);
  background: none;
  border: 0;
  border-radius: var(--vr-radius-md);
  cursor: pointer;
}

.open:hover {
  color: var(--vr-color-text);
  background: var(--vr-color-nav-hover);
}

.name {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
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
