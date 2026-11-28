<script setup lang="ts">
import { computed, shallowRef } from 'vue';
import { VInputText, VListbox, VMessage, vTooltip } from '../../../shared/ui/kit';
import { useI18n } from '../../../shared/lib/i18n/i18n';
import { HarnessIcon } from '../../../entities/session';
import type { ConversationChoice, ProjectConversations } from '../model/conversations';

const props = defineProps<{ conversations: ProjectConversations }>();
const picked = defineModel<string | null>({ required: true });
const { t, formatRelative } = useI18n();
const filter = shallowRef('');
const { choices, loading, error } = props.conversations;

const shown = computed(() => {
  const words = filter.value.trim().toLowerCase().split(/\s+/).filter(Boolean);
  if (!words.length) return choices.value;
  return choices.value.filter((choice) => {
    const text = `${choice.title} ${choice.checkout} ${choice.harness}`.toLowerCase();
    return words.every((word) => text.includes(word));
  });
});

const options = computed(() => shown.value.map((choice) => ({ value: choice.key, label: choice.title, disabled: !!choice.busy })));
const byKey = computed(() => new Map(shown.value.map((choice) => [choice.key, choice])));
const of = (key: string): ConversationChoice => byKey.value.get(key)!;

const pickedKey = computed({
  get: () => picked.value,
  set: (value) => {
    picked.value = typeof value === 'string' ? value : null;
  },
});
</script>

<template>
  <div class="continue" data-testid="new-session-continue-list">
    <VInputText v-model="filter" class="filter" :placeholder="t('agents.newSession.continueFilter')" :aria-label="t('agents.newSession.continueFilter')" data-testid="new-session-continue-filter" />
    <VMessage v-if="error" severity="error" size="small">{{ error }}</VMessage>
    <p v-else-if="loading && !choices.length" class="state">{{ t('agents.newSession.continueLoading') }}</p>
    <p v-else-if="!choices.length" class="state">{{ t('agents.newSession.continueEmpty') }}</p>
    <p v-else-if="!shown.length" class="state">{{ t('agents.newSession.continueNoMatch') }}</p>
    <VListbox v-else v-model="pickedKey" flat allow-empty :filter="false" :options="options" :label="t('agents.newSession.continueMode')">
      <template #option="{ option }">
        <span v-tooltip="of(option.value).busy ?? of(option.value).title" class="item" :data-busy="!!of(option.value).busy" data-testid="new-session-continue-item" :data-session-id="of(option.value).id">
          <HarnessIcon :harness="of(option.value).harness" :size="13" />
          <span class="title">{{ option.label }}</span>
          <span class="meta">
            <span v-if="of(option.value).busy" class="busy">{{ of(option.value).busy }}</span>
            <template v-else>
              <span v-if="of(option.value).checkout" class="checkout mono">{{ of(option.value).checkout }}</span>
              <span v-if="of(option.value).at" class="age">{{ formatRelative(of(option.value).at) }}</span>
            </template>
          </span>
        </span>
      </template>
    </VListbox>
  </div>
</template>

<style scoped>
.continue {
  display: flex;
  flex-direction: column;
  gap: var(--vr-space-2);
  padding: var(--vr-space-2);
  border: 1px solid var(--vr-color-border-strong);
  border-radius: 12px;
  background: var(--vr-color-surface-sunken);
}

.filter {
  width: 100%;
}

.item {
  display: flex;
  flex: 1;
  align-items: center;
  gap: var(--vr-space-2);
  min-width: 0;
  font-size: var(--vr-font-size-label);
}

.title {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
}

.meta {
  display: inline-flex;
  flex: none;
  gap: var(--vr-space-2);
  max-width: 50%;
  overflow: hidden;
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-text-muted);
  white-space: nowrap;
}

.checkout {
  overflow: hidden;
  text-overflow: ellipsis;
}

.mono {
  font-family: var(--vr-font-mono);
}

.busy {
  color: var(--vr-status-feedback);
}

.state {
  margin: 0;
  padding: var(--vr-space-2);
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-text-muted);
}
</style>
