<script setup lang="ts">
import { computed, ref, useTemplateRef, watch } from 'vue';
import { VButton, VInputText, VMessage, VPopover, VSelect, useIsPhone, vTooltip } from '../../../shared/ui/kit';
import { Waypoints } from '../../../shared/ui/icons';
import { t } from '../../../shared/lib/i18n/i18n';
import { toggleAt, type MenuAction } from '../../../shared/lib/menuAction';
import { continueAgentIn, describeError, entitySessionRef, switchAgentModel, useSessionDock, type AgentKind, type ProviderError } from '../../../entities/session';

const props = defineProps<{ agentId: string; kind: AgentKind; model: string | null; resumable: boolean; error: ProviderError | null }>();

const KINDS: { label: string; value: AgentKind }[] = [
  { label: 'Claude', value: 'claude' },
  { label: 'Codex', value: 'codex' },
  { label: 'Grok', value: 'grok' },
];

const dock = useSessionDock();
const open = ref(false);
const model = ref('');
const target = ref<AgentKind>('codex');
const targetModel = ref('');
const busy = ref(false);
const failure = ref<string | null>(null);
const phone = useIsPhone();
const popover = useTemplateRef<InstanceType<typeof VPopover>>('popover');

const errorText = computed(() => (props.error ? `${t(`agents.route.errorKind.${props.error.kind}`)}: ${props.error.message}` : null));
const targetLabel = computed(() => KINDS.find((entry) => entry.value === target.value)?.label ?? target.value);
const title = computed(() => errorText.value ?? t('agents.route.open'));

function toggle(event: Event) {
  popover.value?.toggle(event);
}

function opened() {
  open.value = true;
  model.value = props.model ?? '';
  target.value = props.kind === 'codex' ? 'claude' : 'codex';
  targetModel.value = '';
  failure.value = null;
}

function optional(text: string): string | null {
  const trimmed = text.trim();
  return trimmed ? trimmed : null;
}

async function run(task: () => Promise<void>) {
  if (busy.value) return;
  busy.value = true;
  failure.value = null;
  try {
    await task();
    popover.value?.hide();
  } catch (error) {
    failure.value = describeError(error);
  } finally {
    busy.value = false;
  }
}

function switchModel() {
  void run(() => switchAgentModel(props.agentId, optional(model.value)));
}

function continueIn() {
  void run(async () => {
    const session = entitySessionRef(await continueAgentIn(props.agentId, target.value, optional(targetModel.value)));
    if (session) dock.openWhenListed(session);
  });
}

const item = computed<MenuAction>(() => ({ label: t('agents.route.openShort'), testid: 'session-menu-route', run: (target) => toggleAt(popover.value, target) }));
defineExpose({ item });

watch(
  () => props.agentId,
  () => popover.value?.hide(),
);
</script>

<template>
  <div class="route">
    <VButton
      v-tooltip="title"
      :class="{ alert: !!error }"
      text
      severity="secondary"
      size="small"
      :label="phone ? t('agents.route.openShort') : undefined"
      :aria-label="title"
      :aria-expanded="open"
      aria-haspopup="dialog"
      data-testid="route-button"
      @click="toggle"
    >
      <template #icon><Waypoints :size="13" /></template>
    </VButton>
    <VPopover ref="popover" @show="opened" @hide="open = false">
      <div class="form" data-testid="route-form">
        <VMessage v-if="errorText" severity="warn" size="small" data-testid="route-provider-error">{{ errorText }}</VMessage>
        <form class="block" @submit.prevent="switchModel">
          <span class="heading">{{ t('agents.route.switchTitle') }}</span>
          <span class="hint">{{ resumable ? t('agents.route.switchHint') : t('agents.route.noSession') }}</span>
          <div class="row">
            <VInputText v-model="model" class="grow" :aria-label="t('agents.route.model')" :placeholder="t('agents.route.byDefault')" data-testid="route-model" />
            <VButton type="submit" :label="t('agents.route.switchModel')" size="small" :disabled="busy || !resumable" data-testid="route-switch" />
          </div>
        </form>
        <form class="block" @submit.prevent="continueIn">
          <span class="heading">{{ t('agents.route.continueTitle') }}</span>
          <span class="hint">{{ t('agents.route.continueHint') }}</span>
          <div class="row">
            <VSelect v-model="target" :options="KINDS" option-label="label" option-value="value" append-to="self" data-testid="route-kind" />
            <VInputText v-model="targetModel" class="grow" :aria-label="t('agents.route.model')" :placeholder="t('agents.route.byDefault')" data-testid="route-target-model" />
          </div>
          <div class="actions">
            <VButton :label="t('agents.route.cancel')" text severity="secondary" size="small" @click="popover?.hide()" />
            <VButton type="submit" :label="t('agents.route.continueIn', { kind: targetLabel })" size="small" :disabled="busy" data-testid="route-continue" />
          </div>
        </form>
        <VMessage v-if="failure" severity="error" size="small" data-testid="route-failure">{{ failure }}</VMessage>
      </div>
    </VPopover>
  </div>
</template>

<style scoped>
.route {
  display: flex;
  flex: none;
}

.alert {
  color: var(--vr-status-limit);
}

.form {
  display: flex;
  flex-direction: column;
  gap: var(--vr-space-3);
  width: min(360px, 80vw);
}

.block {
  display: flex;
  flex-direction: column;
  gap: var(--vr-space-2);
}

.row {
  display: flex;
  gap: var(--vr-space-2);
  align-items: center;
}

.grow {
  flex: 1;
  min-width: 0;
}

.heading {
  font-size: var(--vr-font-size-sm);
  font-weight: 600;
}

.hint {
  color: var(--vr-color-text-subtle);
  font-size: var(--vr-font-size-xs);
}

.actions {
  display: flex;
  justify-content: flex-end;
  gap: var(--vr-space-2);
}
</style>
