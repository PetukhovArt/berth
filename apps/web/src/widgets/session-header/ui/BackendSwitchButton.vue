<script setup lang="ts">
import { computed } from 'vue';
import { VButton, useIsPhone, vTooltip } from '../../../shared/ui/kit';
import { MessageSquare, SquareTerminal } from '../../../shared/ui/icons';
import { t } from '../../../shared/lib/i18n/i18n';
import type { MenuAction } from '../../../shared/lib/menuAction';
import { useProviders } from '../../../shared/api/providers';
import { otherBackend, setAgentBackend, useAction, useBerth } from '../../../entities/session';

const props = defineProps<{ agentId: string }>();

const providers = useProviders();
const action = useAction();
const phone = useIsPhone();
const target = computed(() => {
  const agent = useBerth()?.entities.value.agents.get(props.agentId);
  return agent ? otherBackend(agent, providers.value) : null;
});
const label = computed(() => action.error.value ?? t(target.value === 'terminal' ? 'agents.session.openInTerminal' : 'agents.session.backToChat'));

function run() {
  const next = target.value;
  if (next) void action.run(() => setAgentBackend(props.agentId, next));
}

const item = computed<MenuAction | null>(() => (target.value ? { label: label.value, testid: 'session-menu-switch-backend', disabled: action.busy.value, run } : null));
defineExpose({ item });
</script>

<template>
  <VButton
    v-if="target"
    v-tooltip="label"
    :class="{ failed: !!action.error.value }"
    text
    severity="secondary"
    size="small"
    :label="phone ? label : undefined"
    :aria-label="label"
    :disabled="action.busy.value"
    data-testid="session-header-switch-backend"
    @click="run"
  >
    <template #icon>
      <SquareTerminal v-if="target === 'terminal'" :size="13" />
      <MessageSquare v-else :size="13" />
    </template>
  </VButton>
</template>

<style scoped>
.failed {
  color: var(--vr-status-crashed);
}
</style>
