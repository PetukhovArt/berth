<script setup lang="ts">
import { ArchiveRestore } from '../../../shared/ui/icons';
import { VButton, VMessage, VRowButton, vTooltip } from '../../../shared/ui/kit';
import { t } from '../../../shared/lib/i18n/i18n';
import { primaryPressed } from '../../../shared/lib/platform';
import { unarchiveExternal, type ExternalSession, HarnessIcon, useAction } from '../../../entities/session';
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
      <VRowButton v-tooltip="session.cwd" class="open" data-testid="archived-external-open" @click="emit('open', primaryPressed($event))">
        <template #leading><HarnessIcon :harness="session.harness" /></template>
        {{ session.title }}
      </VRowButton>
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
  flex: 1;
  color: var(--vr-color-text-muted);
}

.open:hover {
  color: var(--vr-color-text);
}

:root[data-vr-viewport='phone'] .open {
  min-height: var(--vr-size-row);
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

.archived[data-busy='true'] .open {
  opacity: 0.6;
}

.row-error {
  margin: 0 var(--vr-space-2) var(--vr-space-1) var(--vr-space-5);
}
</style>
