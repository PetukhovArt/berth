<script setup lang="ts">
import { computed, ref, watch } from 'vue';
import { UiButton } from '../../../shared/ui/UiButton';
import { VInputText, VSelect } from '../../../shared/ui/kit';
import { Waypoints } from '../../../shared/ui/icons';
import { useSheetClamp } from '../../../shared/lib/useSheetClamp';
import { t } from '../../../shared/lib/i18n/i18n';
import { continueAgentIn, describeError, entitySessionRef, switchAgentModel, useSessionDock, type AgentKind, type ProviderError } from '../../../entities/session';

const props = defineProps<{ agentId: string; kind: AgentKind; model: string | null; resumable: boolean; error: ProviderError | null }>();

const KINDS: { label: string; value: AgentKind }[] = [
  { label: 'Claude', value: 'claude' },
  { label: 'Codex', value: 'codex' },
  { label: 'Grok', value: 'grok' },
  { label: 'OpenCode', value: 'opencode' },
  { label: 'Cursor', value: 'cursor' },
];

const dock = useSessionDock();
const open = ref(false);
const model = ref('');
const target = ref<AgentKind>('codex');
const targetModel = ref('');
const busy = ref(false);
const failure = ref<string | null>(null);
const sheet = ref<HTMLElement | null>(null);
useSheetClamp(sheet, open);

const errorText = computed(() => (props.error ? `${t(`agents.route.errorKind.${props.error.kind}`)}: ${props.error.message}` : null));
const targetLabel = computed(() => KINDS.find((entry) => entry.value === target.value)?.label ?? target.value);
const title = computed(() => errorText.value ?? t('agents.route.open'));

function toggle() {
  open.value = !open.value;
  if (!open.value) return;
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
    open.value = false;
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

watch(
  () => props.agentId,
  () => {
    open.value = false;
  },
);
</script>

<template>
  <div class="route">
    <UiButton type="button" class="chip" :class="{ alert: !!error }" :title="title" :aria-label="title" :aria-expanded="open" data-testid="route-button" @click="toggle">
      <Waypoints :size="13" />
    </UiButton>
    <div v-if="open" ref="sheet" class="form vr-sheet" data-testid="route-form">
      <span v-if="errorText" class="error" data-testid="route-provider-error">{{ errorText }}</span>
      <form class="block" @submit.prevent="switchModel">
        <span class="heading">{{ t('agents.route.switchTitle') }}</span>
        <span class="hint">{{ resumable ? t('agents.route.switchHint') : t('agents.route.noSession') }}</span>
        <div class="row">
          <VInputText v-model="model" size="small" class="grow" :aria-label="t('agents.route.model')" :placeholder="t('agents.route.byDefault')" data-testid="route-model" />
          <UiButton type="submit" class="primary" :disabled="busy || !resumable" data-testid="route-switch">{{ t('agents.route.switchModel') }}</UiButton>
        </div>
      </form>
      <form class="block" @submit.prevent="continueIn">
        <span class="heading">{{ t('agents.route.continueTitle') }}</span>
        <span class="hint">{{ t('agents.route.continueHint') }}</span>
        <div class="row">
          <VSelect v-model="target" :options="KINDS" option-label="label" option-value="value" size="small" data-testid="route-kind" />
          <VInputText v-model="targetModel" size="small" class="grow" :aria-label="t('agents.route.model')" :placeholder="t('agents.route.byDefault')" data-testid="route-target-model" />
        </div>
        <div class="actions">
          <UiButton type="button" class="secondary" @click="open = false">{{ t('agents.route.cancel') }}</UiButton>
          <UiButton type="submit" class="primary" :disabled="busy" data-testid="route-continue">{{ t('agents.route.continueIn', { kind: targetLabel }) }}</UiButton>
        </div>
      </form>
      <span v-if="failure" class="error" role="alert" data-testid="route-failure">{{ failure }}</span>
    </div>
  </div>
</template>

<style scoped>
.route {
  position: relative;
  display: flex;
  flex: none;
}

.chip {
  display: inline-flex;
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

.chip.alert {
  color: var(--vr-status-limit);
  background: var(--vr-color-surface-muted);
}

.form {
  position: absolute;
  top: 28px;
  right: 0;
  z-index: 6;
  display: flex;
  flex-direction: column;
  gap: var(--vr-space-3);
  width: 360px;
  padding: var(--vr-space-3);
  background: var(--vr-color-surface-raised);
  border-radius: 9px;
  box-shadow: var(--vr-shadow-popover);
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

.error {
  color: var(--vr-status-crashed);
  font-size: var(--vr-font-size-xs);
  overflow-wrap: anywhere;
}

.actions {
  display: flex;
  justify-content: flex-end;
  gap: var(--vr-space-2);
}

.primary,
.secondary {
  flex: none;
  height: 28px;
  padding: 0 11px;
  font: inherit;
  font-size: 12.5px;
  border: 0;
  border-radius: 7px;
  cursor: pointer;
}

.primary {
  color: var(--vr-color-accent-contrast);
  font-weight: 600;
  background: var(--vr-color-accent);
}

.primary:disabled {
  opacity: 0.5;
  cursor: default;
}

.secondary {
  color: var(--vr-color-text);
  background: var(--vr-color-surface-muted);
}
</style>
