<script setup lang="ts">
import { computed, ref, useTemplateRef, watch } from 'vue';
import { FileText } from '../../../shared/ui/icons';
import { VButton, VPopover, useIsPhone, vTooltip } from '../../../shared/ui/kit';
import { t } from '../../../shared/lib/i18n/i18n';
import { toggleAt, type MenuAction } from '../../../shared/lib/menuAction';
import { readAcpAgentLog } from '../../../shared/api/providers';
import { agentLogFor } from '../../../shared/model/agentLog';
import { describeError } from '../../../entities/session';

const props = defineProps<{ agentId: string }>();

const open = ref(false);
const phone = useIsPhone();
const popover = useTemplateRef<InstanceType<typeof VPopover>>('popover');
const anchor = useTemplateRef<HTMLElement>('anchor');
const text = ref<string | null>(null);

function toggle(event: Event) {
  popover.value?.toggle(event);
}

async function opened() {
  open.value = true;
  agentLogFor.value = props.agentId;
  text.value = null;
  try {
    text.value = await readAcpAgentLog(props.agentId);
  } catch (failure) {
    text.value = describeError(failure);
  }
}

function closed() {
  open.value = false;
  if (agentLogFor.value === props.agentId) agentLogFor.value = null;
}

const item = computed<MenuAction>(() => ({ label: t('agents.sessionView.chatAgentLog'), testid: 'session-menu-agent-log', run: (target) => toggleAt(popover.value, target) }));
defineExpose({ item });

watch(agentLogFor, (value) => {
  if (value === props.agentId && !open.value && anchor.value) popover.value?.show({ currentTarget: anchor.value } as unknown as Event);
  else if (value !== props.agentId && open.value) popover.value?.hide();
});
</script>

<template>
  <div ref="anchor" class="agent-log">
    <VButton
      v-tooltip="t('agents.sessionView.chatAgentLog')"
      text
      severity="secondary"
      size="small"
      :label="phone ? t('agents.sessionView.chatAgentLog') : undefined"
      :aria-label="t('agents.sessionView.chatAgentLog')"
      :aria-expanded="open"
      aria-haspopup="dialog"
      data-testid="chat-agent-log-toggle"
      @click="toggle"
    >
      <template #icon><FileText :size="13" /></template>
    </VButton>
    <VPopover ref="popover" @show="opened" @hide="closed">
      <div class="panel" data-testid="chat-agent-log">
        <span class="heading">{{ t('agents.sessionView.chatAgentLog') }}</span>
        <pre class="text mono" data-testid="chat-agent-log-text">{{ text === null ? '' : text || t('agents.sessionView.chatAgentLogEmpty') }}</pre>
      </div>
    </VPopover>
  </div>
</template>

<style scoped>
.agent-log {
  display: flex;
  flex: none;
}

.panel {
  display: flex;
  flex-direction: column;
  gap: var(--vr-space-2);
  width: min(560px, 80vw);
}

.heading {
  font-size: var(--vr-font-size-sm);
  font-weight: 600;
}

.text {
  max-height: 320px;
  margin: 0;
  overflow: auto;
  font-size: var(--vr-font-size-2xs);
  white-space: pre-wrap;
  color: var(--vr-color-text-muted);
}
</style>
