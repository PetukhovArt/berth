<script setup lang="ts">
import { computed } from 'vue';
import { UiButton } from '../../../shared/ui/UiButton';
import { MessageSquare } from '../../../shared/ui/icons';
import { t } from '../../../shared/lib/i18n/i18n';
import { canContinueAsAgent, continueTerminalInDock, useAction, type ExternalSession } from '../../../entities/session';

const props = defineProps<{ terminal: string; terminalKey: string; conversation: ExternalSession }>();

const action = useAction();
const label = computed(() => action.error.value ?? t('workbench.external.continueAsAgent'));

function run() {
  void action.run(() => continueTerminalInDock(props.terminal, props.terminalKey, props.conversation));
}
</script>

<template>
  <div class="continue">
    <UiButton type="button" class="chip" :class="{ failed: !!action.error.value }" :title="label" :disabled="action.busy.value || !canContinueAsAgent(conversation)" data-testid="session-header-continue-as-agent" @click="run">
      <MessageSquare :size="13" />
      {{ t('workbench.external.continueAsAgent') }}
    </UiButton>
    <span v-if="action.error.value" class="error" role="alert" data-testid="session-header-continue-error">{{ action.error.value }}</span>
  </div>
</template>

<style scoped>
.continue {
  position: relative;
  display: flex;
  flex: none;
}

.chip {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  height: 26px;
  padding: 0 10px;
  color: var(--vr-color-text);
  font: inherit;
  font-size: var(--vr-font-size-label);
  white-space: nowrap;
  background: var(--vr-color-surface-muted);
  border: 0;
  border-radius: var(--vr-radius-full);
  cursor: pointer;
}

.chip:disabled {
  opacity: 0.5;
  cursor: default;
}

.chip.failed {
  color: var(--vr-status-crashed);
}

.error {
  position: absolute;
  top: 30px;
  right: 0;
  z-index: 6;
  width: 320px;
  padding: var(--vr-space-2) var(--vr-space-3);
  color: var(--vr-status-crashed);
  font-size: var(--vr-font-size-xs);
  overflow-wrap: anywhere;
  background: var(--vr-color-surface-raised);
  border-radius: 9px;
  box-shadow: var(--vr-shadow-popover);
}
</style>
