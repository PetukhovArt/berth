<script setup lang="ts">
import { computed } from 'vue';
import { UiButton } from '../../../shared/ui/UiButton';
import { MessageSquare, SquareTerminal } from '../../../shared/ui/icons';
import { t } from '../../../shared/lib/i18n/i18n';
import { useProviders } from '../../../shared/api/providers';
import { otherBackend, setAgentBackend, useAction, useBerth } from '../../../entities/session';

const props = defineProps<{ agentId: string }>();

const providers = useProviders();
const action = useAction();
const target = computed(() => {
  const agent = useBerth()?.entities.value.agents.get(props.agentId);
  return agent ? otherBackend(agent, providers.value) : null;
});
const label = computed(() => action.error.value ?? t(target.value === 'terminal' ? 'agents.session.openInTerminal' : 'agents.session.backToChat'));
</script>

<template>
  <UiButton v-if="target" type="button" class="chip" :class="{ failed: !!action.error.value }" :title="label" :aria-label="label" :disabled="action.busy.value" data-testid="session-header-switch-backend" @click="action.run(() => setAgentBackend(agentId, target!))">
    <SquareTerminal v-if="target === 'terminal'" :size="13" />
    <MessageSquare v-else :size="13" />
  </UiButton>
</template>

<style scoped>
.chip {
  display: inline-flex;
  flex: none;
  align-items: center;
  justify-content: center;
  width: 22px;
  height: 22px;
  color: var(--vr-color-text-subtle);
  background: none;
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
</style>
