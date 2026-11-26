<script setup lang="ts">
import { computed } from 'vue';
import { MessageSquare } from '../../../shared/ui/icons';
import { VButton, VMessage, vTooltip } from '../../../shared/ui/kit';
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
    <VButton
      v-tooltip="label"
      :label="t('workbench.external.continueAsAgent')"
      :severity="action.error.value ? 'danger' : 'secondary'"
      size="small"
      rounded
      :disabled="action.busy.value || !canContinueAsAgent(conversation)"
      data-testid="session-header-continue-as-agent"
      @click="run"
    >
      <template #icon><MessageSquare :size="13" /></template>
    </VButton>
    <VMessage v-if="action.error.value" class="error" severity="error" size="small" data-testid="session-header-continue-error">{{ action.error.value }}</VMessage>
  </div>
</template>

<style scoped>
.continue {
  position: relative;
  display: flex;
  flex: none;
}

.error {
  position: absolute;
  top: 30px;
  right: 0;
  z-index: 6;
  width: 320px;
}
</style>
