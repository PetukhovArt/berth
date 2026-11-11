<script setup lang="ts">
import { ArchiveRestore } from 'lucide-vue-next';
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
      <button class="open" :title="session.cwd" data-testid="archived-external-open" @click="emit('open', primaryPressed($event))">
        <HarnessIcon :harness="session.harness" />
        <span class="name">{{ session.title }}</span>
      </button>
      <button class="icon vr-reveal" :title="t('agents.session.restore')" :aria-label="t('agents.session.restore')" :disabled="action.busy.value" data-testid="archived-external-restore" @click="restore"><ArchiveRestore :size="14" /></button>
    </div>
    <p v-if="action.error.value" class="row-error" role="alert" data-testid="archived-external-error">{{ action.error.value }}</p>
  </div>
</template>

<style scoped>
.line {
  display: flex;
  align-items: center;
  gap: var(--vr-space-1);
  min-height: var(--vr-size-row);
  padding: 0 var(--vr-space-1) 0 var(--vr-space-5);
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
  display: inline-flex;
  flex: none;
  align-items: center;
  justify-content: center;
  width: 22px;
  height: 22px;
  padding: 0;
  color: var(--vr-color-text-muted);
  background: none;
  border: none;
  border-radius: var(--vr-radius-sm);
  cursor: pointer;
  opacity: 0;
}

.archived:hover .icon,
.icon:focus-visible {
  opacity: 1;
}

.icon:hover {
  color: var(--vr-color-text);
  background: var(--vr-color-nav-hover);
}

.icon:disabled {
  cursor: default;
  opacity: 0.4;
}

.archived[data-busy='true'] .name {
  opacity: 0.6;
}

.row-error {
  margin: 0;
  padding: 0 var(--vr-space-2) var(--vr-space-1) var(--vr-space-5);
  font-size: var(--vr-font-size-2xs);
  color: var(--vr-color-danger);
}
</style>
